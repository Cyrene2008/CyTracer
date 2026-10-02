use std::path::{Path, PathBuf};
use std::process::Stdio;

use cytracer_core::error::{CoreError, Result};
use cytracer_core::ffmpeg;
use cytracer_core::types::{MotionEvent, KIND_CUT};

/// 单个视频的剪辑导出输入
#[derive(Debug, Clone)]
pub struct ClipSpec {
    pub name: String,
    pub path: String,
    pub events: Vec<MotionEvent>,
}

#[derive(Debug, Clone)]
pub struct ClipExportOptions {
    pub out_dir: PathBuf,
    pub include_cuts: bool,
    /// 事件前后额外保留的秒数
    pub pad: f64,
    /// 是否生成合并合集
    pub merge: bool,
}

impl Default for ClipExportOptions {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::new(),
            include_cuts: true,
            pad: 0.3,
            merge: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct ClipExportResult {
    pub clips: Vec<PathBuf>,
    pub reels: Vec<PathBuf>,
    pub exported: usize,
    pub skipped: usize,
    pub reel_errors: Vec<String>,
}

/// 导出标记区间为独立剪辑；`merge` 时尝试拼接为合集（copy 失败自动重编码）。
pub fn export_clips(specs: &[ClipSpec], opts: &ClipExportOptions) -> Result<ClipExportResult> {
    if !opts.out_dir.is_dir() {
        return Err(CoreError::Cache("导出目录不存在".into()));
    }
    // concat 列表中的路径相对列表文件目录解析；统一转为绝对路径避免嵌套解析失败
    let out_dir = std::path::absolute(&opts.out_dir).unwrap_or_else(|_| opts.out_dir.clone());
    let ffmpeg = ffmpeg::locate("ffmpeg")?;
    let pad = opts.pad.clamp(0.0, 10.0);
    let mut result = ClipExportResult::default();

    for spec in specs {
        let source = PathBuf::from(&spec.path);
        if !source.is_file() {
            result.skipped += 1;
            continue;
        }
        let stem = sanitize_stem(&spec.name, &source);
        let events: Vec<&MotionEvent> = spec
            .events
            .iter()
            .filter(|e| opts.include_cuts || e.kind != KIND_CUT)
            .collect();
        let mut video_clips: Vec<PathBuf> = Vec::new();

        for (index, event) in events.iter().enumerate() {
            let start = (event.start - pad).max(0.0);
            let end = event.end + pad;
            if end - start < 0.05 {
                result.skipped += 1;
                continue;
            }
            let out = out_dir.join(format!("{stem}.clip-{:03}.mp4", index + 1));
            if cut_clip(&ffmpeg, &source, start, end, &out).is_err() {
                result.skipped += 1;
                continue;
            }
            result.clips.push(out.clone());
            video_clips.push(out);
            result.exported += 1;
        }

        if opts.merge && !video_clips.is_empty() {
            match build_reel(&ffmpeg, &out_dir, &stem, &video_clips) {
                Ok(reel) => result.reels.push(reel),
                Err(err) => result.reel_errors.push(format!("{stem}: {err}")),
            }
        }
    }

    Ok(result)
}

/// 单个片段：提前 2 秒开始解码（避开 HEVC 长 GOP / 开放 GOP 首帧异常），
/// 再用输出侧精确定位到目标时刻，最后按精确时长输出。
fn cut_clip(ffmpeg: &Path, source: &Path, start: f64, end: f64, out: &Path) -> Result<()> {
    let duration = (end - start).max(0.05);
    let preroll = 2.0_f64.min(start);
    let seek = start - preroll;
    let output = ffmpeg::command(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
        .arg(format!("{seek:.3}"))
        .arg("-i")
        .arg(source)
        .args(["-ss"])
        .arg(format!("{preroll:.3}"))
        .args(["-t"])
        .arg(format!("{duration:.3}"))
        .args([
            "-map", "0:v:0", "-map", "0:a:0?",
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "20",
            "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k",
            "-movflags", "+faststart",
        ])
        .arg(out)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| CoreError::FfmpegFailed(err.to_string()))?;
    if output.status.success() && out.is_file() {
        Ok(())
    } else {
        let _ = std::fs::remove_file(out);
        Err(CoreError::FfmpegFailed(format!(
            "cut failed: {} ({})",
            out.display(),
            stderr_tail(&output.stderr)
        )))
    }
}

fn stderr_tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let tail: String = text
        .chars()
        .rev()
        .take(400)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    tail.trim().replace('\r', " ").replace('\n', " ")
}

fn build_reel(
    ffmpeg: &Path,
    out_dir: &Path,
    stem: &str,
    clips: &[PathBuf],
) -> Result<PathBuf> {
    let list_path = out_dir.join(format!(".{stem}.concat.txt"));
    let mut list = String::new();
    for clip in clips {
        let absolute = std::path::absolute(clip).unwrap_or_else(|_| clip.clone());
        let path = absolute
            .to_string_lossy()
            .replace('\\', "/")
            .replace('\'', "'\\''");
        list.push_str(&format!("file '{path}'\n"));
    }
    std::fs::write(&list_path, list)?;

    let reel = out_dir.join(format!("{stem}.highlights.mp4"));
    let tmp = out_dir.join(format!(".{stem}.highlights.tmp.mp4"));
    let _ = std::fs::remove_file(&tmp);

    // 先尝试流复制；失败则重编码合并
    let attempts: [&[&str]; 2] = [
        &["-c", "copy"],
        &[
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "160k",
        ],
    ];
    let mut last_error = String::new();
    for (index, codec_args) in attempts.iter().enumerate() {
        let output = ffmpeg::command(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i"])
            .arg(&list_path)
            .args(*codec_args)
            .args(["-movflags", "+faststart"])
            .arg(&tmp)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .map_err(|err| CoreError::FfmpegFailed(err.to_string()))?;
        if output.status.success() && tmp.is_file() {
            if reel.exists() {
                let _ = std::fs::remove_file(&reel);
            }
            std::fs::rename(&tmp, &reel)?;
            let _ = std::fs::remove_file(&list_path);
            return Ok(reel);
        }
        last_error = format!(
            "{} ({})",
            if index == 0 {
                "copy merge failed"
            } else {
                "re-encode merge failed"
            },
            stderr_tail(&output.stderr)
        );
        let _ = std::fs::remove_file(&tmp);
    }
    let _ = std::fs::remove_file(&list_path);
    Err(CoreError::FfmpegFailed(last_error))
}

fn sanitize_stem(name: &str, source: &Path) -> String {
    let raw = if name.is_empty() {
        source
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "video".into())
    } else {
        Path::new(name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| name.to_string())
    };
    raw.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}
