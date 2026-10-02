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
