use std::path::{Path, PathBuf};

use cytracer_core::cache::cache_key;
use cytracer_core::error::{CoreError, Result};
use cytracer_core::types::{MarkerStore, MotionEvent};

/// 标记存储路径：以源文件指纹命名，源文件变化即失效。
pub fn markers_path(cache_dir: &Path, source: &Path) -> PathBuf {
    let (key, _, _) = cache_key(source);
    cache_dir.join("markers").join(format!("{key}.json"))
}

pub fn load_markers(cache_dir: &Path, source: &Path) -> MarkerStore {
    let path = markers_path(cache_dir, source);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| empty_store(source)),
        Err(_) => empty_store(source),
    }
}

pub fn save_markers(cache_dir: &Path, store: &MarkerStore) -> Result<()> {
    let source = PathBuf::from(&store.path);
    let path = markers_path(cache_dir, &source);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(store).map_err(|e| CoreError::Cache(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

fn empty_store(source: &Path) -> MarkerStore {
    MarkerStore {
        path: source.to_string_lossy().to_string(),
        events: Vec::new(),
        updated_at: 0,
    }
}

/// 重新检测后合并：保留人工标记与已编辑标记，替换其余自动标记。
pub fn merge_detected(existing: &MarkerStore, detected: Vec<MotionEvent>) -> MarkerStore {
    let mut events: Vec<MotionEvent> = existing
        .events
        .iter()
        .filter(|e| e.kind == cytracer_core::KIND_MANUAL || e.edited)
        .cloned()
        .collect();
    events.extend(detected);
    events.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    MarkerStore {
        path: existing.path.clone(),
        events,
        updated_at: now_seconds(),
    }
}

pub fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cytracer_core::types::KIND_MOTION;

    fn event(id: &str, start: f64, auto: bool, edited: bool, kind: &str) -> MotionEvent {
        MotionEvent {
            id: id.into(),
            kind: kind.into(),
            start,
            end: start + 1.0,
            peak: start,
            score: 0.1,
            level: 1,
            label: String::new(),
            note: String::new(),
            color: String::new(),
            auto,
            edited,
        }
    }

    #[test]
    fn merge_keeps_edited_and_manual() {
        let existing = MarkerStore {
            path: "v.mp4".into(),
            events: vec![
                event("auto-0", 1.0, true, false, KIND_MOTION),
                event("auto-1", 5.0, true, true, KIND_MOTION),
                event("manual-0", 9.0, false, false, cytracer_core::KIND_MANUAL),
            ],
            updated_at: 0,
        };
        let detected = vec![event("auto-0", 2.0, true, false, KIND_MOTION)];
        let merged = merge_detected(&existing, detected);
        let starts: Vec<f64> = merged.events.iter().map(|e| e.start).collect();
        assert_eq!(starts, vec![2.0, 5.0, 9.0]);
    }
}
