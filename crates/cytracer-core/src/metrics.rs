use std::io::Read;
use std::path::Path;
use std::process::{Child, ChildStdout, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::error::{CoreError, Result};
use crate::ffmpeg;
use crate::probe::MediaInfo;
use crate::types::{AnalysisParams, FrameMetric};

/// 分析画幅长边像素（越小越快）
pub const DEFAULT_LONG_SIDE: u32 = 160;
/// 分块边长（像素）
pub const BLOCK: usize = 10;
/// 直方图桶宽（灰度级）
const HIST_BLOCK: f32 = 4.0;
const HIST_BINS: usize = 16;
/// 场景切换：全图灰度直方图 L1 距离阈值
const CUT_L1_THRESHOLD: f32 = 0.5;

#[derive(Debug, Clone, Copy)]
pub struct FrameSize {
    pub width: usize,
    pub height: usize,
    pub cols: usize,
    pub rows: usize,
}

impl FrameSize {
    pub fn frame_bytes(&self) -> usize {
        self.width * self.height
    }

    pub fn block_count(&self) -> usize {
        self.cols * self.rows
    }
}

/// 计算适配的分析画幅：长边不超过 long_side，宽高取块大小整数倍。
pub fn plan_frame_size(width: u32, height: u32, long_side: u32) -> FrameSize {
    let long_side = long_side.max((BLOCK as u32) * 2);
    let (w, h) = if width == 0 || height == 0 {
        (long_side, long_side * 9 / 16)
    } else {
        let source_long = width.max(height) as f64;
        let factor = (long_side as f64 / source_long).min(1.0);
        let w = (width as f64 * factor).round().max(BLOCK as f64 * 2.0) as u32;
        let h = (height as f64 * factor).round().max(BLOCK as f64 * 2.0) as u32;
        (w, h)
    };
    let w = (w as usize / BLOCK).max(2) * BLOCK;
    let h = (h as usize / BLOCK).max(2) * BLOCK;
    FrameSize {
        width: w,
        height: h,
        cols: w / BLOCK,
        rows: h / BLOCK,
    }
}

/// ffmpeg rawvideo 抽帧流（灰度定长帧）
pub struct FrameStream {
    child: Child,
    stdout: ChildStdout,
    pub size: FrameSize,
    frame_bytes: usize,
}

impl FrameStream {
    pub fn open(source: &Path, params: &AnalysisParams, size: FrameSize) -> Result<Self> {
        let ffmpeg = ffmpeg::locate("ffmpeg")?;
        let fps = params.fps_clamped();
        let filter = format!(
            "fps={fps},scale={}:{}:flags=fast_bilinear,format=gray",
            size.width, size.height
        );
        let mut cmd = ffmpeg::command(&ffmpeg);
        cmd.arg("-hide_banner")
            .arg("-nostdin")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(source)
            .arg("-map")
            .arg("0:v:0")
            .arg("-an")
            .arg("-sn")
            .arg("-vf")
            .arg(&filter)
            .arg("-f")
            .arg("rawvideo")
            .arg("-pix_fmt")
            .arg("gray")
            .arg("-threads")
            .arg("0")
            .arg("-")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CoreError::FfmpegFailed("cannot capture stdout".into()))?;
        Ok(Self {
            child,
            stdout,
            size,
            frame_bytes: size.frame_bytes(),
        })
    }

    /// 读取一帧；EOF 返回 false。
    pub fn read_frame(&mut self, buf: &mut [u8]) -> Result<bool> {
        debug_assert_eq!(buf.len(), self.frame_bytes);
        let mut filled = 0;
        while filled < buf.len() {
            match self.stdout.read(&mut buf[filled..]) {
                Ok(0) => {
                    if filled == 0 {
                        return Ok(false);
                    }
                    return Err(CoreError::FfmpegFailed("truncated raw frame".into()));
                }
                Ok(n) => filled += n,
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(err.into()),
            }
        }
        Ok(true)
    }

    /// 结束进程并返回 stderr 文本（错误时用于诊断）。
    pub fn finish(mut self) -> Result<String> {
        let status = self.child.wait()?;
        let mut stderr = String::new();
        if let Some(mut pipe) = self.child.stderr.take() {
            let _ = pipe.read_to_string(&mut stderr);
        }
        if !status.success() {
            let tail: String = stderr
                .chars()
                .rev()
                .take(500)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            return Err(CoreError::FfmpegFailed(tail.trim().to_string()));
        }
        Ok(stderr)
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for FrameStream {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            self.kill();
        }
    }
}

pub struct AnalyzeProgress<'a> {
    pub expected_frames: u64,
    pub callback: &'a mut dyn FnMut(u64, u64),
    last_emit: Instant,
}

impl<'a> AnalyzeProgress<'a> {
    pub fn new(expected_frames: u64, callback: &'a mut dyn FnMut(u64, u64)) -> Self {
        Self {
            expected_frames: expected_frames.max(1),
            callback,
            last_emit: Instant::now() - std::time::Duration::from_secs(1),
        }
    }

    fn maybe_emit(&mut self, processed: u64, force: bool) {
        if force || self.last_emit.elapsed().as_millis() >= 120 {
            (self.callback)(processed, self.expected_frames);
            self.last_emit = Instant::now();
        }
    }
}

struct PrevFrame {
    blocks: Vec<f32>,
    hist32: [f32; HIST_BINS * 2],
}

/// 逐帧运动指标提取主循环。
pub fn analyze_video(
    source: &Path,
    info: &MediaInfo,
    params: &AnalysisParams,
    long_side: u32,
    callback: &mut dyn FnMut(u64, u64),
    cancel: &AtomicBool,
) -> Result<(Vec<FrameMetric>, FrameSize)> {
    let (src_w, src_h) = if info.rotation.abs() == 90 || info.rotation.abs() == 270 {
        (info.height.max(1), info.width.max(1))
    } else {
        (info.width.max(1), info.height.max(1))
    };
    let size = plan_frame_size(src_w, src_h, long_side);
    let mut stream = FrameStream::open(source, params, size)?;
    let fps = params.fps_clamped();
    let expected = (info.duration.max(0.0) * fps as f64).ceil().max(1.0) as u64;
    let mut progress = AnalyzeProgress::new(expected, callback);

    let mut gray = vec![0u8; size.frame_bytes()];
    let mut prev: Option<PrevFrame> = None;
    let mut metrics: Vec<FrameMetric> = Vec::with_capacity(expected as usize);
    let delta = params.block_delta();
    let compensate = params.compensate_camera;

    loop {
        if cancel.load(Ordering::Relaxed) {
            stream.kill();
            return Err(CoreError::Canceled);
        }
        if !stream.read_frame(&mut gray)? {
            break;
        }
        let hist32 = pixel_histogram(&gray);
        let blocks = block_means(&gray, size);
        let metric = match &prev {
            None => FrameMetric::default(),
            Some(prev) => build_metric(&blocks, &hist32, prev, size, delta, compensate),
        };
        metrics.push(metric);
        prev = Some(PrevFrame { blocks, hist32 });
        let processed = metrics.len() as u64;
        progress.maybe_emit(processed, false);
    }
    progress.maybe_emit(metrics.len() as u64, true);
    stream.finish()?;
    Ok((metrics, size))
}

fn build_metric(
    blocks: &[f32],
    hist32: &[f32; HIST_BINS * 2],
    prev: &PrevFrame,
    size: FrameSize,
    delta: f32,
    compensate: bool,
) -> FrameMetric {
    // 与前一帧逐块比较（同位置）。相机整体运动导致的全局一致变化由中位差兜底消除。
    let mut valid: Vec<(usize, f32)> = Vec::with_capacity(size.block_count());
    for r in 0..size.rows {
        for c in 0..size.cols {
            let index = r * size.cols + c;
            let cur = blocks[index];
            let old = prev.blocks[index];
            valid.push((index, (cur - old).abs()));
        }
    }

    let mut raw_sorted: Vec<f32> = valid.iter().map(|(_, d)| *d).collect();
    raw_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = raw_sorted[raw_sorted.len() / 2];

    // 相机整体运动（摇镜 / 抖动）在残差中体现为全局中位数；减掉它只保留局部运动。
    let mut hist = [0u16; HIST_BINS];
    let mut active = 0u32;
    let (mut x0, mut y0, mut x1, mut y1) = (u16::MAX, u16::MAX, 0u16, 0u16);
    let mut sum = 0.0f32;
    let mut max_diff = 0.0f32;
    let mut adjusted_sorted: Vec<f32> = Vec::with_capacity(valid.len());
    for &(flat, raw) in &valid {
        let value = if compensate { (raw - median).max(0.0) } else { raw };
        sum += value;
        if value > max_diff {
            max_diff = value;
        }
        adjusted_sorted.push(value);
        let bin = ((value / HIST_BLOCK) as usize).min(HIST_BINS - 1);
        hist[bin] = hist[bin].saturating_add(1);
        if value > delta {
            active += 1;
            let r = flat / size.cols;
            let c = flat % size.cols;
            x0 = x0.min(c as u16);
            y0 = y0.min(r as u16);
            x1 = x1.max(c as u16);
            y1 = y1.max(r as u16);
        }
    }
    let mean_diff = sum / valid.len() as f32;
    adjusted_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let adj_median = adjusted_sorted[adjusted_sorted.len() / 2];
    let bbox = if active == 0 {
        [0, 0, 0, 0]
    } else {
        [x0, y0, x1, y1]
    };

    let mut l1 = 0.0f32;
    for i in 0..prev.hist32.len() {
        l1 += (hist32[i] - prev.hist32[i]).abs();
    }
    let pixels = size.frame_bytes().max(1) as f32;
    let scene_cut = l1 / (2.0 * pixels) > CUT_L1_THRESHOLD;

    FrameMetric {
        mean_diff,
        median_diff: adj_median,
        max_diff,
        scene_cut,
        bbox,
        hist,
    }
}

fn block_means(gray: &[u8], size: FrameSize) -> Vec<f32> {
    let mut blocks = vec![0.0f32; size.block_count()];
    for r in 0..size.rows {
        for c in 0..size.cols {
            let mut sum = 0u32;
            let y0 = r * BLOCK;
            let x0 = c * BLOCK;
            for y in y0..y0 + BLOCK {
                let row = y * size.width;
                for x in x0..x0 + BLOCK {
                    sum += gray[row + x] as u32;
                }
            }
            blocks[r * size.cols + c] = sum as f32 / (BLOCK * BLOCK) as f32;
        }
    }
    blocks
}

fn pixel_histogram(gray: &[u8]) -> [f32; HIST_BINS * 2] {
    let mut hist = [0.0f32; HIST_BINS * 2];
    for &pixel in gray {
        let bin = (pixel as usize) / 8;
        hist[bin.min(HIST_BINS * 2 - 1)] += 1.0;
    }
    hist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_size_is_block_aligned() {
        let size = plan_frame_size(1920, 1080, 160);
        assert_eq!(size.width % BLOCK, 0);
        assert_eq!(size.height % BLOCK, 0);
        assert!(size.width <= 160);
        assert_eq!(size.cols, size.width / BLOCK);
        assert_eq!(size.rows, size.height / BLOCK);
    }

    #[test]
    fn vertical_video_keeps_long_side() {
        let size = plan_frame_size(1080, 1920, 160);
        assert_eq!(size.height, 160);
        assert!(size.width < size.height);
    }

    #[test]
    fn static_frames_have_zero_score() {
        let size = plan_frame_size(160, 90, 160);
        let blocks = vec![100.0f32; size.block_count()];
        let hist = [0.0f32; 32];
        let prev = PrevFrame {
            blocks: blocks.clone(),
            hist32: hist,
        };
        let metric = build_metric(&blocks, &hist, &prev, size, 12.0, true);
        assert_eq!(metric.score(12.0), 0.0);
        assert!(!metric.scene_cut);
    }

    #[test]
    fn moving_blocks_trigger_score() {
        let size = plan_frame_size(160, 90, 160);
        let prev_blocks = vec![100.0f32; size.block_count()];
        let mut cur = prev_blocks.clone();
        for value in cur.iter_mut().take(6) {
            *value = 180.0;
        }
        let hist = [0.0f32; 32];
        let prev = PrevFrame {
            blocks: prev_blocks,
            hist32: hist,
        };
        let metric = build_metric(&cur, &hist, &prev, size, 12.0, true);
        assert!(metric.score(12.0) > 0.0);
    }

    #[test]
    fn global_change_is_suppressed_by_median() {
        // 全画面同样变化（仿佛相机整体平移）应被中位差抵消，不产生局部运动
        let size = plan_frame_size(160, 90, 160);
        let prev_blocks = vec![100.0f32; size.block_count()];
        let cur: Vec<f32> = prev_blocks.iter().map(|v| v + 30.0).collect();
        let hist = [0.0f32; 32];
        let prev = PrevFrame {
            blocks: prev_blocks,
            hist32: hist,
        };
        let metric = build_metric(&cur, &hist, &prev, size, 12.0, true);
        assert_eq!(metric.score(12.0), 0.0);
    }
}
