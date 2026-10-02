use serde::{Deserialize, Serialize};
use serde_json::json;

use cytracer_core::types::{MotionEvent, KIND_CUT, KIND_MOTION};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportVideo {
    pub name: String,
    pub path: String,
    pub events: Vec<MotionEvent>,
}

const CSV_HEADER: &str = "video,path,kind,start,end,peak,duration,score,level,label,note,timecode_start,timecode_end";

/// 按格式渲染导出文本：json | csv | srt
pub fn render(format: &str, videos: &[ExportVideo], include_cuts: bool) -> String {
    match format {
        "csv" => render_csv(videos, include_cuts),
        "srt" => render_srt(videos, include_cuts),
        _ => render_json(videos, include_cuts),
    }
}

fn included(event: &MotionEvent, include_cuts: bool) -> bool {
    include_cuts || event.kind != KIND_CUT
}

fn render_json(videos: &[ExportVideo], include_cuts: bool) -> String {
    let mut items = Vec::new();
    for video in videos {
        for event in &video.events {
            if !included(event, include_cuts) {
                continue;
            }
            items.push(json!({
                "video": video.name,
                "path": video.path,
                "id": event.id,
                "kind": event.kind,
                "start": event.start,
                "end": event.end,
                "peak": event.peak,
                "duration": event.duration(),
                "score": event.score,
                "level": event.level,
                "label": event.label,
                "note": event.note,
                "timecodeStart": timecode(event.start),
                "timecodeEnd": timecode(event.end),
            }));
        }
    }
    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string())
}

fn render_csv(videos: &[ExportVideo], include_cuts: bool) -> String {
    let mut out = String::from("\u{FEFF}");
    out.push_str(CSV_HEADER);
    out.push_str("\r\n");
    for video in videos {
        for event in &video.events {
            if !included(event, include_cuts) {
                continue;
            }
            let row = [
                video.name.clone(),
                video.path.clone(),
                event.kind.clone(),
                format!("{:.3}", event.start),
                format!("{:.3}", event.end),
                format!("{:.3}", event.peak),
                format!("{:.3}", event.duration()),
                format!("{:.4}", event.score),
                event.level.to_string(),
                event.label.clone(),
                event.note.clone(),
                timecode(event.start),
                timecode(event.end),
            ];
            out.push_str(&row.map(|field| csv_escape(&field)).join(","));
            out.push_str("\r\n");
        }
    }
    out
}

fn render_srt(videos: &[ExportVideo], include_cuts: bool) -> String {
    let mut out = String::new();
    let mut index = 1;
    for video in videos {
        for event in &video.events {
            if !included(event, include_cuts) {
                continue;
            }
            let text = if !event.label.is_empty() {
                event.label.clone()
            } else if event.kind == KIND_CUT {
                "Scene cut".to_string()
            } else {
                "Motion detected".to_string()
            };
            out.push_str(&format!("{index}\n"));
            out.push_str(&format!(
                "{} --> {}\n",
                srt_time(event.start),
                srt_time(event.end)
            ));
            out.push_str(&text);
            if !event.note.is_empty() {
                out.push('\n');
                out.push_str(&event.note);
            }
            out.push_str("\n\n");
            index += 1;
        }
    }
    out
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub fn timecode(seconds: f64) -> String {
    let seconds = seconds.max(0.0);
    let total_ms = (seconds * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

fn srt_time(seconds: f64) -> String {
    timecode(seconds).replace('.', ",")
}

pub fn default_kind_label(kind: &str) -> &'static str {
    if kind == KIND_MOTION {
        "Motion"
    } else if kind == KIND_CUT {
        "Scene cut"
    } else {
        "Marker"
    }
}
