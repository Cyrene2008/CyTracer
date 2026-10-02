use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use cytracer_analyze::{detect, ensure_proxy};
use cytracer_core::analyze_video;
use cytracer_core::cache::{cache_key, CacheHeader, CacheReader, CacheWriter};
use cytracer_core::error::CoreError;
use cytracer_core::ffmpeg;
use cytracer_core::metrics::{FrameSize, BLOCK, DEFAULT_LONG_SIDE};
use cytracer_core::probe::{probe, MediaInfo};
use cytracer_core::types::{AnalysisParams, FrameMetric, MarkerStore, MotionEvent};
use cytracer_project::export::{render, ExportVideo};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobState {
    pub running: bool,
    pub finished: bool,
    pub canceled: bool,
    pub stage: String,
    pub path: String,
    pub name: String,
    pub processed: u64,
    pub total: u64,
    pub progress: f64,
    pub speed: f64,
    pub elapsed_ms: u64,
    pub error: String,
}

impl Default for JobState {
    fn default() -> Self {
        Self {
            running: false,
            finished: false,
            canceled: false,
            stage: "idle".into(),
            path: String::new(),
            name: String::new(),
            processed: 0,
            total: 0,
            progress: 0.0,
            speed: 0.0,
            elapsed_ms: 0,
            error: String::new(),
        }
    }
}

pub struct AppState {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub settings: RwLock<Value>,
    pub job: Mutex<JobState>,
    pub cancel: AtomicBool,
}

pub fn default_settings() -> Value {
    json!({
        "lang": "zh",
        "theme": "light",
        "palette": "peach",
        "exportDir": "",
        "analyze": {
            "preset": "custom",
            "speed": "standard",
            "fps": 8,
            "sensitivity": 50,
            "minDuration": 0.4,
            "mergeGap": 0.3,
            "compensateCamera": true,
            "markSceneCuts": true
        }
    })
}

impl AppState {
    pub fn new(config_dir: PathBuf, cache_dir: PathBuf) -> Self {
        let settings_path = config_dir.join("settings.json");
        let settings = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .unwrap_or_else(default_settings);
        Self {
            config_dir,
            cache_dir,
            settings: RwLock::new(settings),
            job: Mutex::new(JobState::default()),
            cancel: AtomicBool::new(false),
        }
    }

    pub fn save_settings(&self) {
        let value = self
            .settings
            .read()
            .map(|v| v.clone())
            .unwrap_or_else(|_| default_settings());
        let path = self.config_dir.join("settings.json");
        let _ = std::fs::write(
            path,
            serde_json::to_string_pretty(&value).unwrap_or_default(),
        );
    }

    pub fn analysis_file(&self, source: &Path, params: &AnalysisParams) -> PathBuf {
        let (key, _, _) = cache_key(source);
        self.cache_dir.join("analyses").join(format!(
            "{}-f{:.2}-c{}.cymc",
            key,
            params.fps_clamped(),
            params.compensate_camera as u8
        ))
    }

    pub fn meta_file(&self, source: &Path, params: &AnalysisParams) -> PathBuf {
        let (key, _, _) = cache_key(source);
        self.cache_dir.join("analyses").join(format!(
            "{}-f{:.2}-c{}.meta.json",
            key,
            params.fps_clamped(),
            params.compensate_camera as u8
        ))
    }
}

// ---------------------------------------------------------------------------
// 基础命令
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

static FFMPEG_VERSION: OnceLock<Option<String>> = OnceLock::new();

#[tauri::command]
pub fn app_status() -> Value {
    let ffmpeg_found = ffmpeg::locate("ffmpeg").is_ok();
    let version = FFMPEG_VERSION
        .get_or_init(ffmpeg::ffmpeg_version)
        .clone();
    json!({
        "ready": ffmpeg_found,
        "ffmpeg": ffmpeg_found,
        "ffmpegVersion": version.unwrap_or_default(),
        "version": env!("CARGO_PKG_VERSION"),
    })
}

#[tauri::command]
pub fn settings_get(state: State<AppState>) -> Value {
    state
        .settings
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| default_settings())
}

#[tauri::command]
pub fn settings_put(state: State<AppState>, payload: Value) -> Value {
    if let Ok(mut guard) = state.settings.write() {
        *guard = payload;
    }
    state.save_settings();
    json!({ "ok": true })
}

#[tauri::command]
pub async fn probe_video(path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        probe(Path::new(&path))
            .map(|info| serde_json::to_value(info).unwrap_or(Value::Null))
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

// ---------------------------------------------------------------------------
// 分析任务
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRequest {
    pub path: String,
    pub params: AnalysisParams,
}

#[tauri::command]
pub fn analyze_start(
    app: AppHandle,
    state: State<AppState>,
    payload: AnalyzeRequest,
    on_progress: Channel<Value>,
) -> Result<Value, String> {
    {
        let job = state.job.lock().map_err(|_| "任务状态锁定失败")?;
        if job.running {
            return Err("已有分析任务正在运行".into());
        }
    }
    if !Path::new(&payload.path).is_file() {
        return Err("文件不存在".into());
    }
    state.cancel.store(false, Ordering::SeqCst);
    let name = Path::new(&payload.path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    {
        let mut job = state.job.lock().map_err(|_| "任务状态锁定失败")?;
        *job = JobState {
            running: true,
            stage: "analyzing".into(),
            path: payload.path.clone(),
            name,
            ..Default::default()
        };
    }
    let app_handle = app.clone();
    let path = payload.path.clone();
    let params = payload.params.clone();
    std::thread::Builder::new()
        .name("cytracer-analyze".into())
        .spawn(move || run_analysis_job(app_handle, path, params, on_progress))
        .map_err(|err| err.to_string())?;
    Ok(json!({ "started": true }))
}

fn run_analysis_job(
    app: AppHandle,
    path: String,
    params: AnalysisParams,
    channel: Channel<Value>,
) {
    let state = app.state::<AppState>();
    let started = std::time::Instant::now();
    let result = analyze_job_inner(&state, &path, &params, &channel, started);
    let mut job = match state.job.lock() {
        Ok(job) => job,
        Err(_) => return,
    };
    job.running = false;
    job.finished = true;
    match result {
        Ok(value) => {
            job.stage = "done".into();
            job.progress = 1.0;
            let _ = channel.send(json!({
                "stage": "done",
                "elapsedMs": started.elapsed().as_millis() as u64,
                "frameCount": value.get("frameCount").cloned().unwrap_or(Value::Null),
            }));
        }
        Err(err) => {
            if matches!(err, CoreError::Canceled) {
                job.canceled = true;
                job.stage = "canceled".into();
                let _ = channel.send(json!({ "stage": "canceled" }));
            } else {
                job.stage = "error".into();
                job.error = err.to_string();
                let _ = channel.send(json!({ "stage": "error", "message": err.to_string() }));
            }
        }
    }
}

fn analyze_job_inner(
    state: &AppState,
    path: &str,
    params: &AnalysisParams,
    channel: &Channel<Value>,
    started: std::time::Instant,
) -> cytracer_core::Result<Value> {
    let source = PathBuf::from(path);
    let info = probe(&source)?;
    let cache_file = state.analysis_file(&source, params);
    let meta_file = state.meta_file(&source, params);

    let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let (metrics, size) = analyze_video(
        &source,
        &info,
        params,
        DEFAULT_LONG_SIDE,
        &mut |processed, total| {
            let elapsed_ms = started.elapsed().as_millis() as u64;
            let speed = processed as f64 / started.elapsed().as_secs_f64().max(0.001);
            let progress = processed as f64 / total.max(1) as f64;
            if let Ok(mut job) = state.job.lock() {
                job.processed = processed;
                job.total = total;
                job.progress = progress;
                job.speed = speed;
                job.elapsed_ms = elapsed_ms;
            }
            if last_emit.elapsed().as_millis() >= 120 {
                last_emit = std::time::Instant::now();
                let _ = channel.send(json!({
                    "stage": "analyzing",
                    "processed": processed,
                    "total": total,
                    "progress": progress,
                    "speed": speed,
                    "elapsedMs": elapsed_ms,
                }));
            }
        },
        &state.cancel,
    )?;

    write_cache(&cache_file, &info, params, &metrics, size)?;
    let meta = json!({
        "info": info,
        "params": params,
        "frameCount": metrics.len(),
        "width": size.width,
        "height": size.height,
        "analyzedAt": now_millis(),
    });
    if let Ok(text) = serde_json::to_string(&meta) {
        let _ = std::fs::write(&meta_file, text);
    }

    let outcome = detect(&metrics, params);
    let store = cytracer_project::load_markers(&state.cache_dir, &source);
    let merged = cytracer_project::merge_detected(&store, outcome.events);
    let events = dedupe_ids(merged.events);
    Ok(json!({
        "info": info,
        "params": params,
        "frameCount": metrics.len(),
        "curve": outcome.curve,
        "curveBuckets": outcome.curve_buckets,
        "noiseFloor": outcome.noise_floor,
        "events": events,
        "width": size.width,
        "height": size.height,
    }))
}

fn write_cache(
    path: &Path,
    info: &MediaInfo,
    params: &AnalysisParams,
    metrics: &[FrameMetric],
    size: FrameSize,
) -> cytracer_core::Result<()> {
    let (_, source_size, source_mtime) = cache_key(Path::new(&info.path));
    let header = CacheHeader {
        fps: params.fps_clamped(),
        width: size.width as u16,
        height: size.height as u16,
        block: BLOCK as u16,
        compensation: params.compensate_camera,
        frame_count: 0,
        duration: info.duration as f32,
        source_path: info.path.clone(),
        source_size,
        source_mtime,
    };
    let mut writer = CacheWriter::create(path, header)?;
    for metric in metrics {
        writer.push(metric)?;
    }
    writer.finish()?;
    Ok(())
}

#[tauri::command]
pub fn analyze_status(state: State<AppState>) -> Value {
    state
        .job
        .lock()
        .map(|job| serde_json::to_value(job.clone()).unwrap_or(Value::Null))
        .unwrap_or(Value::Null)
}

#[tauri::command]
pub fn analyze_cancel(state: State<AppState>) -> Value {
    state.cancel.store(true, Ordering::SeqCst);
    json!({ "ok": true })
}

// ---------------------------------------------------------------------------
// 结果读取 / 参数重算
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn analysis_result(
    app: AppHandle,
    path: String,
    params: AnalysisParams,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        analysis_result_inner(&state, &path, &params).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn events_retreshold(
    app: AppHandle,
    path: String,
    params: AnalysisParams,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        analysis_result_inner(&state, &path, &params).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

fn analysis_result_inner(
    state: &AppState,
    path: &str,
    params: &AnalysisParams,
) -> cytracer_core::Result<Value> {
    let source = Path::new(path);
    if !source.is_file() {
        return Err(CoreError::Cache("文件不存在".into()));
    }
    let cache_file = state.analysis_file(source, params);
    if !cache_file.is_file() {
        return Err(CoreError::Cache(
            "该视频尚未按当前帧率 / 补偿设置分析，请先运行分析".into(),
        ));
    }
    let meta_file = state.meta_file(source, params);
    let meta: Value = std::fs::read_to_string(&meta_file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let info = meta
        .get("info")
        .cloned()
        .unwrap_or(Value::Null);

    let mut reader = CacheReader::open(&cache_file)?;
    let metrics = reader.read_all()?;
    if metrics.is_empty() {
        return Err(CoreError::Cache("分析缓存为空".into()));
    }
    let outcome = detect(&metrics, params);
    let store = cytracer_project::load_markers(&state.cache_dir, source);
    let merged = cytracer_project::merge_detected(&store, outcome.events);
    let events = dedupe_ids(merged.events);
    Ok(json!({
        "info": info,
        "params": params,
        "frameCount": metrics.len(),
        "curve": outcome.curve,
        "curveBuckets": outcome.curve_buckets,
        "noiseFloor": outcome.noise_floor,
        "events": events,
        "analyzedAt": meta.get("analyzedAt").cloned().unwrap_or(Value::Null),
    }))
}

// ---------------------------------------------------------------------------
// 预览代理
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn proxy_ensure(
    app: AppHandle,
    path: String,
    on_progress: Channel<Value>,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let info = probe(Path::new(&path)).map_err(|err| err.to_string())?;
        let mut last = -1i32;
        let out = ensure_proxy(
            Path::new(&path),
            &info,
            &state.cache_dir,
            &mut |fraction| {
                let pct = (fraction * 100.0) as i32;
                if pct != last {
                    last = pct;
                    let _ = on_progress.send(json!({
                        "stage": "proxy",
                        "progress": fraction,
                        "action": info.playback,
                    }));
                }
            },
            &state.cancel,
        )
        .map_err(|err| err.to_string())?;
        Ok(json!({
            "path": out.to_string_lossy(),
            "action": info.playback,
        }))
    })
    .await
    .map_err(|err| err.to_string())?
}

// ---------------------------------------------------------------------------
// 示例视频生成（无素材时一键测试）
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn generate_samples(app: AppHandle) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        generate_samples_inner(&state).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

fn generate_samples_inner(state: &AppState) -> cytracer_core::Result<Value> {
    use std::process::Stdio;

    let dir = state.cache_dir.join("samples");
    std::fs::create_dir_all(&dir)?;
    let ffmpeg = ffmpeg::locate("ffmpeg")?;

    let specs: [(&str, Vec<&str>); 4] = [
        (
            "sample-motion.mp4",
            vec![
                "-f", "lavfi", "-i", "color=c=0x2a2a33:s=640x360:d=8:r=15",
                "-f", "lavfi", "-i", "color=c=white:s=90x90:d=8:r=15",
                "-filter_complex", "[0][1]overlay=x='mod(t*160,560)':y=120",
                "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
            ],
        ),
        (
            "sample-pan.mp4",
            vec![
                "-f", "lavfi", "-i", "testsrc2=s=960x540:d=8:r=15",
                "-vf", "crop=640:360:x='(in_w-out_w)*t/8':y='(in_h-out_h)/2'",
                "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
            ],
        ),
        (
            "sample-cuts.mp4",
            vec![
                "-f", "lavfi", "-i", "color=c=0x203040:s=640x360:d=2:r=15",
                "-f", "lavfi", "-i", "color=c=0xd0c0b0:s=640x360:d=2:r=15",
                "-f", "lavfi", "-i", "testsrc2=s=640x360:d=2:r=15",
                "-filter_complex", "[0:v][1:v][2:v]concat=n=3:v=1:a=0",
                "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
            ],
        ),
        (
            "sample-static.mp4",
            vec![
                "-f", "lavfi", "-i", "color=c=0x303030:s=640x360:d=4:r=15",
                "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
            ],
        ),
    ];

    let mut paths = Vec::new();
    for (name, args) in specs {
        let out = dir.join(name);
        if !out.is_file() {
            let status = ffmpeg::command(&ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-y"])
                .args(&args)
                .arg(&out)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|err| CoreError::FfmpegFailed(err.to_string()))?;
            if !status.success() {
                return Err(CoreError::FfmpegFailed(format!("生成示例失败: {name}")));
            }
        }
        paths.push(out.to_string_lossy().to_string());
    }
    Ok(json!({ "paths": paths }))
}

// ---------------------------------------------------------------------------
// 标记缩略图拼图导出
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactSheetRequest {
    pub out_dir: String,
    pub include_cuts: bool,
    #[serde(default)]
    pub columns: Option<u32>,
    #[serde(default)]
    pub thumb_width: Option<u32>,
    pub videos: Vec<ExportVideo>,
}

const SHEET_MAX_PER_IMAGE: usize = 40;

#[tauri::command]
pub async fn export_contact_sheet(app: AppHandle, payload: ContactSheetRequest) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        export_contact_sheet_inner(&state, &payload).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

fn export_contact_sheet_inner(
    state: &AppState,
    req: &ContactSheetRequest,
) -> cytracer_core::Result<Value> {
    use std::process::Stdio;

    let out_dir = PathBuf::from(&req.out_dir);
    if !out_dir.is_dir() {
        return Err(CoreError::Cache("导出目录不存在".into()));
    }
    let ffmpeg = ffmpeg::locate("ffmpeg")?;
    let columns = req.columns.unwrap_or(4).clamp(2, 8) as usize;
    let thumb_width = req.thumb_width.unwrap_or(480).clamp(240, 1280);
    let font = "C:/Windows/Fonts/msyh.ttc";

    let tmp_root = state.cache_dir.join("tmp").join(format!("sheet-{}", now_millis()));
    std::fs::create_dir_all(&tmp_root)?;

    let mut sheets: Vec<String> = Vec::new();
    let mut exported = 0usize;
    let mut skipped = 0usize;

    for video in &req.videos {
        let source = PathBuf::from(&video.path);
        if !source.is_file() {
            skipped += 1;
            continue;
        }
        let events: Vec<&MotionEvent> = video
            .events
            .iter()
            .filter(|e| req.include_cuts || e.kind != "cut")
            .collect();
        if events.is_empty() {
            continue;
        }
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "video".into());
        let video_tmp = tmp_root.join(&stem);
        std::fs::create_dir_all(&video_tmp)?;

        let mut thumbs: Vec<PathBuf> = Vec::new();
        for (index, event) in events.iter().enumerate() {
            let peak = if event.peak > 0.0 { event.peak } else { event.start };
            let thumb = video_tmp.join(format!("raw_{:03}.jpg", index + 1));
            let timecode = cytracer_project::export::timecode(event.peak.max(event.start));
            if capture_frame(
                &ffmpeg,
                &source,
                peak,
                thumb_width,
                Some((index + 1, &timecode)),
                font,
                &thumb,
            )
            .is_err()
            {
                // drawtext 不可用时退回无文字缩略图
                if capture_frame(&ffmpeg, &source, peak, thumb_width, None, font, &thumb).is_err() {
                    skipped += 1;
                    continue;
                }
            }
            thumbs.push(thumb);
        }
        if thumbs.is_empty() {
            continue;
        }

        // 分块平铺，避免单图过大
        let parts = thumbs.chunks(SHEET_MAX_PER_IMAGE).count();
        for (part, chunk) in thumbs.chunks(SHEET_MAX_PER_IMAGE).enumerate() {
            let chunk_dir = video_tmp.join(format!("part-{part}"));
            std::fs::create_dir_all(&chunk_dir)?;
            for (i, src) in chunk.iter().enumerate() {
                std::fs::copy(src, chunk_dir.join(format!("thumb_{:03}.jpg", i + 1)))?;
            }
            let rows = chunk.len().div_ceil(columns);
            let filter = format!(
                "tile={}x{}:padding=6:color=0x111111",
                columns, rows
            );
            let name = if parts > 1 {
                format!("{stem}.sheet-{}.jpg", part + 1)
            } else {
                format!("{stem}.sheet.jpg")
            };
            let out = out_dir.join(name);
            let status = ffmpeg::command(&ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-y"])
                .arg("-framerate")
                .arg("1")
                .arg("-i")
                .arg(chunk_dir.join("thumb_%03d.jpg"))
                .args(["-vf", &filter])
                .args(["-frames:v", "1"])
                .arg(&out)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|err| CoreError::FfmpegFailed(err.to_string()))?;
            if !status.success() {
                return Err(CoreError::FfmpegFailed(format!(
                    "拼图失败: {}",
                    out.display()
                )));
            }
            sheets.push(out.to_string_lossy().to_string());
            exported += chunk.len();
        }
    }

    let _ = std::fs::remove_dir_all(&tmp_root);
    Ok(json!({
        "sheets": sheets,
        "events": exported,
        "skipped": skipped,
    }))
}

fn capture_frame(
    ffmpeg: &Path,
    source: &Path,
    time: f64,
    thumb_width: u32,
    label: Option<(usize, &str)>,
    font: &str,
    out: &Path,
) -> cytracer_core::Result<()> {
    use std::process::Stdio;

    let mut filter = format!("scale={thumb_width}:-2");
    if let Some((index, timecode)) = label {
        let safe = timecode.replace(':', "\\:");
        filter.push_str(&format!(
            ",drawtext=fontfile='{}':text='#{}  {}':x=8:y=8:fontsize=20:fontcolor=white:box=1:boxcolor=black@0.6",
            font, index, safe
        ));
    }
    let status = ffmpeg::command(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
        .arg(format!("{time:.3}"))
        .arg("-i")
        .arg(source)
        .args(["-frames:v", "1", "-vf", &filter, "-q:v", "3"])
        .arg(out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| CoreError::FfmpegFailed(err.to_string()))?;
    if status.success() && out.is_file() {
        Ok(())
    } else {
        Err(CoreError::FfmpegFailed("capture frame failed".into()))
    }
}

// ---------------------------------------------------------------------------
// 标记剪辑导出（独立片段 / 合集）
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipExportRequest {
    pub out_dir: String,
    pub include_cuts: bool,
    #[serde(default)]
    pub merge: bool,
    #[serde(default)]
    pub pad: Option<f64>,
    pub videos: Vec<ExportVideo>,
}

#[tauri::command]
pub async fn export_clips(payload: ClipExportRequest) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        export_clips_inner(&payload).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

fn export_clips_inner(req: &ClipExportRequest) -> cytracer_core::Result<Value> {
    use std::process::Stdio;

    let out_dir = PathBuf::from(&req.out_dir);
    if !out_dir.is_dir() {
        return Err(CoreError::Cache("导出目录不存在".into()));
    }
    let ffmpeg = ffmpeg::locate("ffmpeg")?;
    let pad = req.pad.unwrap_or(0.3).clamp(0.0, 5.0);

    let mut clips: Vec<String> = Vec::new();
    let mut reels: Vec<String> = Vec::new();
    let mut exported = 0usize;
    let mut skipped = 0usize;

    for video in &req.videos {
        let source = PathBuf::from(&video.path);
        if !source.is_file() {
            skipped += 1;
            continue;
        }
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "video".into());
        let events: Vec<&MotionEvent> = video
            .events
            .iter()
            .filter(|e| req.include_cuts || e.kind != "cut")
            .collect();
        let mut video_clips: Vec<PathBuf> = Vec::new();

        for (index, event) in events.iter().enumerate() {
            let start = (event.start - pad).max(0.0);
            let end = event.end + pad;
            if end - start < 0.05 {
                skipped += 1;
                continue;
            }
            let out = out_dir.join(format!("{stem}.clip-{:03}.mp4", index + 1));
            let status = ffmpeg::command(&ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
                .arg(format!("{start:.3}"))
                .arg("-to")
                .arg(format!("{end:.3}"))
                .arg("-i")
                .arg(&source)
                .args([
                    "-map", "0:v:0", "-map", "0:a:0?",
                    "-c:v", "libx264", "-preset", "veryfast", "-crf", "20",
                    "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k",
                    "-movflags", "+faststart",
                ])
                .arg(&out)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|err| CoreError::FfmpegFailed(err.to_string()))?;
            if status.success() && out.is_file() {
                clips.push(out.to_string_lossy().to_string());
                video_clips.push(out);
                exported += 1;
            } else {
                skipped += 1;
            }
        }

        if req.merge && !video_clips.is_empty() {
            let list_path = out_dir.join(format!("{stem}.concat.txt"));
            let mut list = String::new();
            for clip in &video_clips {
                let path = clip.to_string_lossy().replace('\\', "/").replace('\'', "'\\''");
                list.push_str(&format!("file '{path}'\n"));
            }
            std::fs::write(&list_path, list)?;
            let reel = out_dir.join(format!("{stem}.highlights.mp4"));
            let status = ffmpeg::command(&ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i"])
                .arg(&list_path)
                .args(["-c", "copy"])
                .arg(&reel)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|err| CoreError::FfmpegFailed(err.to_string()))?;
            let _ = std::fs::remove_file(&list_path);
            if status.success() && reel.is_file() {
                reels.push(reel.to_string_lossy().to_string());
            }
        }
    }

    Ok(json!({
        "clips": clips,
        "reels": reels,
        "events": exported,
        "skipped": skipped,
    }))
}

// ---------------------------------------------------------------------------
// 标记持久化 / 导出
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn markers_load(app: AppHandle, path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let store = cytracer_project::load_markers(&state.cache_dir, Path::new(&path));
        serde_json::to_value(store).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn markers_save(state: State<AppState>, payload: MarkerStore) -> Result<Value, String> {
    let mut store = payload;
    store.updated_at = cytracer_project::store::now_seconds();
    cytracer_project::save_markers(&state.cache_dir, &store).map_err(|err| err.to_string())?;
    Ok(json!({ "ok": true }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub out_path: String,
    pub format: String,
    pub include_cuts: bool,
    pub videos: Vec<ExportVideo>,
}

#[tauri::command]
pub fn export_markers(payload: ExportRequest) -> Result<Value, String> {
    if payload.videos.is_empty() {
        return Err("没有可导出的视频".into());
    }
    let content = render(&payload.format, &payload.videos, payload.include_cuts);
    std::fs::write(&payload.out_path, content).map_err(|err| err.to_string())?;
    Ok(json!({ "path": payload.out_path }))
}

// ---------------------------------------------------------------------------
// 工具命令
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn reveal_path(path: String) -> Result<Value, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("explorer")
            .arg(format!("/select,{path}"))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    Ok(json!({ "ok": true }))
}

fn dir_stats(dir: &Path) -> (u64, u64) {
    let mut bytes = 0u64;
    let mut files = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    bytes += meta.len();
                    files += 1;
                }
            }
        }
    }
    (bytes, files)
}

#[tauri::command]
pub fn cache_stats(state: State<AppState>) -> Value {
    let (mut bytes, mut files) = (0u64, 0u64);
    for dir in [
        state.cache_dir.join("analyses"),
        state.cache_dir.join("proxies"),
    ] {
        let (b, f) = dir_stats(&dir);
        bytes += b;
        files += f;
    }
    json!({ "bytes": bytes, "files": files })
}

#[tauri::command]
pub fn clear_cache(state: State<AppState>) -> Value {
    let mut freed = 0u64;
    for name in ["analyses", "proxies"] {
        let dir = state.cache_dir.join(name);
        let (bytes, _) = dir_stats(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        freed += bytes;
    }
    json!({ "freed": freed })
}

// ---------------------------------------------------------------------------

fn dedupe_ids(events: Vec<MotionEvent>) -> Vec<MotionEvent> {
    let mut seen: HashSet<String> = HashSet::new();
    events
        .into_iter()
        .map(|mut event| {
            let base = event.id.clone();
            let mut candidate = base.clone();
            let mut n = 1;
            while !seen.insert(candidate.clone()) {
                candidate = format!("{base}-{n}");
                n += 1;
            }
            event.id = candidate;
            event
        })
        .collect()
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
