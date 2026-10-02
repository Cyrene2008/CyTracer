//! 端到端管线测试：用内置 ffmpeg 合成视频，验证指标提取与事件检测。
//! 若本机缺少 ffmpeg/ffprobe，则跳过（打印跳过信息返回）。

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use cytracer_analyze::detect;
use cytracer_core::analyze_video;
use cytracer_core::cache::{CacheHeader, CacheReader, CacheWriter};
use cytracer_core::ffmpeg;
use cytracer_core::metrics::{DEFAULT_LONG_SIDE, BLOCK};
use cytracer_core::probe::probe;
use cytracer_core::types::{AnalysisParams, FrameMetric, KIND_CUT, KIND_MOTION};

fn setup_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "cytracer-it-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn ffmpeg_or_skip() -> Option<PathBuf> {
    match ffmpeg::locate("ffmpeg") {
        Ok(path) => Some(path),
        Err(_) => {
            eprintln!("[skip] ffmpeg not found; pipeline integration tests skipped");
            None
        }
    }
}

fn generate(ffmpeg: &Path, args: &[&str], out: &Path) {
    let status = ffmpeg::command(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .arg(out)
        .status()
        .expect("spawn ffmpeg");
    assert!(status.success(), "ffmpeg generation failed for {}", out.display());
}

fn analyze(path: &Path) -> Vec<cytracer_core::MotionEvent> {
    let info = probe(path).expect("probe");
    let params = AnalysisParams {
        fps: 8.0,
        ..Default::default()
    };
    let cancel = AtomicBool::new(false);
    let (metrics, _size) = analyze_video(
        path,
        &info,
        &params,
        DEFAULT_LONG_SIDE,
        &mut |_, _| {},
        &cancel,
    )
    .expect("analyze");
    assert!(!metrics.is_empty(), "metrics should not be empty");
    detect(&metrics, &params).events
}

#[test]
fn static_video_has_no_motion_events() {
    let Some(ffmpeg) = ffmpeg_or_skip() else { return };
    let dir = setup_dir();
    let file = dir.join("static.mp4");
    generate(
        &ffmpeg,
        &[
            "-f", "lavfi", "-i", "color=c=0x303030:s=320x180:d=3:r=15", "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
        ],
        &file,
    );
    let events = analyze(&file);
    assert!(
        events.iter().all(|e| e.kind != KIND_MOTION),
        "static video should not report motion: {events:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn moving_box_is_detected() {
    let Some(ffmpeg) = ffmpeg_or_skip() else { return };
    let dir = setup_dir();
    let file = dir.join("move.mp4");
    generate(
        &ffmpeg,
        &[
            "-f", "lavfi", "-i", "color=c=0x303030:s=320x180:d=4:r=15",
            "-f", "lavfi", "-i", "color=c=white:s=60x60:d=4:r=15",
            "-filter_complex", "[0][1]overlay=x='mod(t*140,260)':y=60",
            "-c:v", "libx264", "-pix_fmt", "yuv420p",
        ],
        &file,
    );
    let events = analyze(&file);
    let motion: Vec<_> = events.iter().filter(|e| e.kind == KIND_MOTION).collect();
    assert!(!motion.is_empty(), "moving box should produce motion events");
    let total: f64 = motion.iter().map(|e| e.duration()).sum();
    assert!(total > 1.0, "motion coverage should be > 1s, got {total}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hard_cut_is_detected_as_scene_cut() {
    let Some(ffmpeg) = ffmpeg_or_skip() else { return };
    let dir = setup_dir();
    let file = dir.join("cut.mp4");
    generate(
        &ffmpeg,
        &[
            "-f", "lavfi", "-i", "color=c=black:s=320x180:d=1:r=15",
            "-f", "lavfi", "-i", "color=c=white:s=320x180:d=1:r=15",
            "-filter_complex", "[0:v][1:v]concat=n=2:v=1:a=0",
            "-c:v", "libx264", "-pix_fmt", "yuv420p",
        ],
        &file,
    );
    let events = analyze(&file);
    let cuts: Vec<_> = events.iter().filter(|e| e.kind == KIND_CUT).collect();
    assert_eq!(cuts.len(), 1, "expected exactly one scene cut: {events:?}");
    assert!((cuts[0].start - 1.0).abs() < 0.2, "cut near 1s, got {}", cuts[0].start);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cache_roundtrip_preserves_metrics() {
    let dir = setup_dir();
    let file = dir.join("cache.cymc");
    let header = CacheHeader {
        fps: 8.0,
        width: 160,
        height: 90,
        block: BLOCK as u16,
        compensation: true,
        frame_count: 0,
        duration: 10.0,
        source_path: "D:/videos/sample.mp4".into(),
        source_size: 12345,
        source_mtime: 67890,
    };
    let metrics: Vec<FrameMetric> = (0..16)
        .map(|i| FrameMetric {
            mean_diff: i as f32,
            median_diff: i as f32 / 2.0,
            max_diff: i as f32 * 2.0,
            scene_cut: i == 7,
            bbox: [1, 2, 3, 4],
            hist: [i as u16; 16],
        })
        .collect();
    {
        let mut writer = CacheWriter::create(&file, header.clone()).unwrap();
        for metric in &metrics {
            writer.push(metric).unwrap();
        }
        let written = writer.finish().unwrap();
        assert_eq!(written.frame_count, 16);
    }
    let mut reader = CacheReader::open(&file).unwrap();
    assert_eq!(reader.header.source_size, 12345);
    let restored = reader.read_all().unwrap();
    assert_eq!(restored.len(), metrics.len());
    assert_eq!(restored[7].scene_cut, true);
    assert_eq!(restored[3].hist, [3u16; 16]);
    let _ = std::fs::remove_dir_all(&dir);
}
