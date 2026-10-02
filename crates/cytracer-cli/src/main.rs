use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use clap::{Parser, Subcommand};
use cytracer_analyze::{detect, ensure_proxy};
use cytracer_core::cache::{cache_key, CacheReader, CacheWriter};
use cytracer_core::metrics::DEFAULT_LONG_SIDE;
use cytracer_core::probe::probe;
use cytracer_core::types::{AnalysisParams, FrameMetric, KIND_CUT};
use cytracer_core::analyze_video;

#[derive(Parser)]
#[command(name = "cytracer-cli", version, about = "CyTracer engine CLI (dev / benchmark)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 输出媒体信息与预览策略
    Probe {
        file: PathBuf,
    },
    /// 分析视频并输出运动事件
    Analyze {
        file: PathBuf,
        #[arg(long, default_value_t = 8.0)]
        fps: f32,
        #[arg(long, default_value_t = 50.0)]
        sensitivity: f32,
        #[arg(long, default_value_t = 0.4)]
        min_duration: f32,
        #[arg(long, default_value_t = 0.3)]
        merge_gap: f32,
        #[arg(long)]
        no_compensate: bool,
        #[arg(long)]
        no_cuts: bool,
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        #[arg(long)]
        no_cache: bool,
        /// 以 JSON 输出完整结果
        #[arg(long)]
        json: bool,
        /// 将事件导出为 <out>/<视频名>.events.{json,csv,srt}
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// 计时基准：重复分析 N 次（不写缓存）
    Bench {
        file: PathBuf,
        #[arg(long, default_value_t = 1)]
        runs: u32,
        #[arg(long, default_value_t = 8.0)]
        fps: f32,
    },
    /// 生成 / 复用预览代理
    Proxy {
        file: PathBuf,
        #[arg(long)]
        cache_dir: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();
    let code = match run(cli) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    };
    std::process::exit(code);
}

fn default_cache_dir() -> PathBuf {
    std::env::temp_dir().join("cytracer-cli")
}

fn run(cli: Cli) -> cytracer_core::Result<()> {
    match cli.command {
        Commands::Probe { file } => {
            let info = probe(&file)?;
            println!("{}", serde_json::to_string_pretty(&info).unwrap_or_default());
        }
        Commands::Analyze {
            file,
            fps,
            sensitivity,
            min_duration,
            merge_gap,
            no_compensate,
            no_cuts,
            cache_dir,
            no_cache,
            json,
            out_dir,
        } => {
            let params = AnalysisParams {
                fps,
                sensitivity,
                min_duration,
                merge_gap,
                compensate_camera: !no_compensate,
                mark_scene_cuts: !no_cuts,
            };
            let info = probe(&file)?;
            let cache_dir = cache_dir.unwrap_or_else(default_cache_dir);
            let cancel = AtomicBool::new(false);
            let started = Instant::now();
            let metrics = load_or_analyze(&file, &info, &params, &cache_dir, !no_cache, &cancel, !json)?;
            let analyze_time = started.elapsed();
            let outcome = detect(&metrics, &params);
            let total_time = started.elapsed();

            if let Some(dir) = &out_dir {
                std::fs::create_dir_all(dir)?;
                let stem = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "events".into());
                let video = cytracer_project::export::ExportVideo {
                    name: file
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    path: file.to_string_lossy().to_string(),
                    events: outcome.events.clone(),
                };
                for format in ["json", "csv", "srt"] {
                    let content = cytracer_project::export::render(
                        format,
                        std::slice::from_ref(&video),
                        true,
                    );
                    let path = dir.join(format!("{stem}.events.{format}"));
                    std::fs::write(&path, content)?;
                    eprintln!("导出: {}", path.display());
                }
            }

            if json {
                let payload = serde_json::json!({
                    "info": info,
                    "params": params,
                    "frameCount": metrics.len(),
                    "analyzeSeconds": analyze_time.as_secs_f64(),
                    "totalSeconds": total_time.as_secs_f64(),
                    "outcome": outcome,
                });
                println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_default());
            } else {
                let analyzed_fps = metrics.len() as f64 / analyze_time.as_secs_f64().max(0.001);
                eprintln!(
                    "\n分析完成：{} 帧 / {:.2}s（{:.0} fps，{:.2}x 实时）",
                    metrics.len(),
                    analyze_time.as_secs_f64(),
                    analyzed_fps,
                    if info.duration > 0.0 { info.duration / analyze_time.as_secs_f64() } else { 0.0 }
                );
                eprintln!("噪声底（进入阈值）: {:.4}\n", outcome.noise_floor);
                println!(
                    "{:<10} {:>13} {:>13} {:>9} {:>7} {}",
                    "type", "start", "end", "duration", "score", "level"
                );
                for event in &outcome.events {
                    println!(
                        "{:<10} {:>13} {:>13} {:>8.2}s {:>7.3} {}",
                        if event.kind == KIND_CUT { "cut" } else { "motion" },
                        fmt_time(event.start),
                        fmt_time(event.end),
                        event.duration(),
                        event.score,
                        event.level
                    );
                }
            }
        }
        Commands::Bench { file, runs, fps } => {
            let params = AnalysisParams {
                fps,
                ..Default::default()
            };
            let info = probe(&file)?;
            let cancel = AtomicBool::new(false);
            for run in 0..runs {
                let started = Instant::now();
                let (metrics, size) = analyze_video(
                    &file,
                    &info,
                    &params,
                    DEFAULT_LONG_SIDE,
                    &mut |_, _| {},
                    &cancel,
                )?;
                let elapsed = started.elapsed().as_secs_f64();
                eprintln!(
                    "run {}: {} 帧, {}x{}, {:.2}s, {:.0} fps, {:.2}x 实时",
                    run + 1,
                    metrics.len(),
                    size.width,
                    size.height,
                    elapsed,
                    metrics.len() as f64 / elapsed.max(0.001),
                    if info.duration > 0.0 { info.duration / elapsed } else { 0.0 }
                );
            }
        }
        Commands::Proxy { file, cache_dir } => {
            let info = probe(&file)?;
            let cancel = AtomicBool::new(false);
            let cache_dir = cache_dir.unwrap_or_else(default_cache_dir);
            let mut last = 0;
            let path = ensure_proxy(&file, &info, &cache_dir, &mut |fraction| {
                let pct = (fraction * 100.0) as i32;
                if pct != last {
                    last = pct;
                    eprint!("\r代理生成 {pct}%");
                }
            }, &cancel)?;
            eprintln!("\n代理就绪: {}", path.display());
        }
    }
    Ok(())
}

fn load_or_analyze(
    file: &PathBuf,
    info: &cytracer_core::MediaInfo,
    params: &AnalysisParams,
    cache_dir: &PathBuf,
    use_cache: bool,
    cancel: &AtomicBool,
    quiet: bool,
) -> cytracer_core::Result<Vec<FrameMetric>> {
    let (key, size, mtime) = cache_key(file);
    let cache_file = cache_dir.join("analyses").join(format!(
        "{}-f{:.2}-c{}.cymc",
        key,
        params.fps_clamped(),
        params.compensate_camera as u8
    ));

    if use_cache && cache_file.is_file() {
        if let Ok(mut reader) = CacheReader::open(&cache_file) {
            let header = &reader.header;
            if header.source_size == size && header.source_mtime == mtime {
                if let Ok(metrics) = reader.read_all() {
                    if !metrics.is_empty() {
                        if !quiet {
                            eprintln!("使用缓存: {}", cache_file.display());
                        }
                        return Ok(metrics);
                    }
                }
            }
        }
    }

    let (metrics, frame_size) = analyze_video(
        file,
        info,
        params,
        DEFAULT_LONG_SIDE,
        &mut |processed, expected| {
            if quiet {
                return;
            }
            let pct = (processed * 100 / expected.max(1)).min(100);
            eprint!("\r分析中 {pct}% ({processed}/{expected})");
        },
        cancel,
    )?;
    if !quiet {
        eprintln!();
    }

    if use_cache {
        let header = cytracer_core::cache::CacheHeader {
            fps: params.fps_clamped(),
            width: frame_size.width as u16,
            height: frame_size.height as u16,
            block: cytracer_core::metrics::BLOCK as u16,
            compensation: params.compensate_camera,
            frame_count: 0,
            duration: info.duration as f32,
            source_path: file.to_string_lossy().to_string(),
            source_size: size,
            source_mtime: mtime,
        };
        if let Ok(mut writer) = CacheWriter::create(&cache_file, header) {
            let mut ok = true;
            for metric in &metrics {
                if writer.push(metric).is_err() {
                    ok = false;
                    break;
                }
            }
            if ok {
                let _ = writer.finish();
            } else {
                writer.abandon();
            }
        }
    }
    Ok(metrics)
}

fn fmt_time(seconds: f64) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}
