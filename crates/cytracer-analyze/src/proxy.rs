use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

use cytracer_core::cache::cache_key;
use cytracer_core::error::{CoreError, Result};
use cytracer_core::ffmpeg;
use cytracer_core::probe::{self, MediaInfo, PlaybackAction};

/// 强制代理时的策略：能无损封装就 remux，否则转码。
fn force_action(info: &MediaInfo) -> PlaybackAction {
    let video = info.video_codec.to_lowercase();
    let audio = info.audio_codec.to_lowercase();
    let remuxable_audio = audio.is_empty() || matches!(audio.as_str(), "aac" | "mp3");
    if video == "h264" && remuxable_audio {
        PlaybackAction::Remux
    } else {
        PlaybackAction::Transcode
    }
}

/// 预览代理缓存路径（源文件指纹决定）。
pub fn proxy_path(cache_dir: &Path, source: &Path) -> PathBuf {
    let (key, _, _) = cache_key(source);
    cache_dir.join("proxies").join(format!("{key}.mp4"))
}

/// 确保可预览：直接播放返回源文件；否则生成 / 复用 mp4 预览代理。
/// `force = true` 时忽略 Direct 判定，强制生成代理（用于直放失败的运行时兜底）。
pub fn ensure_proxy(
    source: &Path,
    info: &MediaInfo,
    cache_dir: &Path,
    force: bool,
    progress: &mut dyn FnMut(f64),
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    let mut action = probe::playback_action(info);
    if force && action == PlaybackAction::Direct {
        action = force_action(info);
    }
    if action == PlaybackAction::Direct {
        progress(1.0);
        return Ok(source.to_path_buf());
    }

    let out = proxy_path(cache_dir, source);
    if out.is_file() {
        let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
        if size > 1024 {
            progress(1.0);
            return Ok(out);
        }
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let ffmpeg = ffmpeg::locate("ffmpeg")?;
    let tmp = out.with_extension("tmp.mp4");
    let _ = std::fs::remove_file(&tmp);

    let mut cmd = ffmpeg::command(&ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-nostdin")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?");
    match action {
        PlaybackAction::Remux => {
            cmd.args(["-c", "copy"]);
        }
        PlaybackAction::Transcode => {
            cmd.args([
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "23",
                "-pix_fmt",
                "yuv420p",
                "-vf",
                "scale=720:720:force_original_aspect_ratio=decrease:force_divisible_by=2",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
            ]);
        }
        PlaybackAction::Direct => unreachable!(),
    }
    cmd.args(["-movflags", "+faststart", "-progress", "pipe:1", "-nostats"])
        .arg(&tmp)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CoreError::FfmpegFailed("cannot capture proxy stdout".into()))?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| CoreError::FfmpegFailed("cannot capture proxy stderr".into()))?;
    let stderr_thread = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr_pipe.read_to_string(&mut text);
        text
    });

    let duration = info.duration.max(0.01);
    for line in BufReader::new(stdout).lines() {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&tmp);
            return Err(CoreError::Canceled);
        }
        let line = line?;
        if let Some(value) = line.strip_prefix("out_time_ms=") {
            if let Ok(ms) = value.trim().parse::<i64>() {
                let fraction = (ms as f64 / 1_000_000.0 / duration).clamp(0.0, 0.99);
                progress(fraction);
            }
        } else if line.starts_with("progress=end") {
            progress(0.99);
        }
    }

    let status = child.wait()?;
    let stderr_text = stderr_thread.join().unwrap_or_default();
    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        let tail: String = stderr_text
            .chars()
            .rev()
            .take(500)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        return Err(CoreError::FfmpegFailed(tail.trim().to_string()));
    }

    if out.exists() {
        let _ = std::fs::remove_file(&out);
    }
    std::fs::rename(&tmp, &out)?;
    progress(1.0);
    Ok(out)
}
