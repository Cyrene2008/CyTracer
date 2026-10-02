use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::ffmpeg;

/// 媒体探测结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub video_codec: String,
    pub audio_codec: String,
    pub container: String,
    pub rotation: i32,
    pub has_audio: bool,
    /// direct | remux | transcode
    pub playback: String,
}

impl MediaInfo {
    pub fn resolution(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    pub fn playable_directly(&self) -> bool {
        self.playback == "direct"
    }
}

/// 预览策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackAction {
    Direct,
    Remux,
    Transcode,
}

impl PlaybackAction {
    pub fn as_str(self) -> &'static str {
        match self {
            PlaybackAction::Direct => "direct",
            PlaybackAction::Remux => "remux",
            PlaybackAction::Transcode => "transcode",
        }
    }
}

pub fn probe(path: &Path) -> Result<MediaInfo> {
    let ffprobe = ffmpeg::locate("ffprobe")?;
    let path_str = path.to_string_lossy().to_string();
    let json = ffmpeg::run_ok(
        &ffprobe,
        &[
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            "-show_entries",
            "stream_side_data=rotation",
            &path_str,
        ],
    )?;
    parse_probe_json(path, &json)
}

pub fn parse_probe_json(path: &Path, json: &str) -> Result<MediaInfo> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|err| CoreError::Probe(err.to_string()))?;

    let streams = value
        .get("streams")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CoreError::Probe("no streams in probe output".into()))?;

    let video = streams
        .iter()
        .find(|s| s.get("codec_type").and_then(|v| v.as_str()) == Some("video"))
        .ok_or_else(|| CoreError::Probe("no video stream".into()))?;

    let audio = streams
        .iter()
        .find(|s| s.get("codec_type").and_then(|v| v.as_str()) == Some("audio"));

    let width = json_u32(video, "width").unwrap_or(0);
    let height = json_u32(video, "height").unwrap_or(0);
    let fps = video
        .get("avg_frame_rate")
        .and_then(|v| v.as_str())
        .and_then(parse_fraction)
        .or_else(|| {
            video
                .get("r_frame_rate")
                .and_then(|v| v.as_str())
                .and_then(parse_fraction)
        })
        .unwrap_or(0.0);

    let rotation = video
        .get("side_data_list")
        .and_then(|v| v.as_array())
        .and_then(|list| {
            list.iter().find_map(|item| {
                item.get("rotation")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32)
            })
        })
        .or_else(|| {
            video
                .get("tags")
                .and_then(|tags| tags.get("rotate"))
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<i32>().ok())
        })
        .unwrap_or(0);

    let format = value.get("format").cloned().unwrap_or_default();
    let duration = format
        .get("duration")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    let size_bytes = format
        .get("size")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse::<u64>().ok())
        .or_else(|| std::fs::metadata(path).ok().map(|m| m.len()))
        .unwrap_or(0);
    let container = format
        .get("format_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut info = MediaInfo {
        path: path.to_string_lossy().to_string(),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        size_bytes,
        duration,
        width,
        height,
        fps,
        video_codec: video
            .get("codec_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        audio_codec: audio
            .and_then(|a| a.get("codec_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        container,
        rotation,
        has_audio: audio.is_some(),
        playback: String::new(),
    };
    info.playback = playback_action(&info).as_str().to_string();
    Ok(info)
}

fn json_u32(value: &serde_json::Value, key: &str) -> Option<u32> {
    value
        .get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
}

fn parse_fraction(text: &str) -> Option<f32> {
    let (num, den) = text.split_once('/')?;
    let num: f32 = num.parse().ok()?;
    let den: f32 = den.parse().ok()?;
    if den == 0.0 {
        return None;
    }
    Some(num / den)
}

/// 依据扩展名 / 编码判断 WebView2 直接播放能力，给出预览策略。
pub fn playback_action(info: &MediaInfo) -> PlaybackAction {
    let container = info.container.to_lowercase();
    let video = info.video_codec.to_lowercase();
    let audio = info.audio_codec.to_lowercase();
    let ext = Path::new(&info.path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    // webm / mkv 的 ffprobe format_name 都是 matroska,webm，只能靠扩展名区分
    let is_webm = ext == "webm";
    let is_mp4ish = matches!(ext.as_str(), "mp4" | "m4v" | "mov")
        || (ext.is_empty() && (container.contains("mp4") || container.contains("mov")));

    let audio_ok = audio.is_empty() || matches!(audio.as_str(), "aac" | "mp3" | "opus" | "vorbis");
    if audio_ok {
        if is_mp4ish && video == "h264" {
            return PlaybackAction::Direct;
        }
        if is_webm && matches!(video.as_str(), "vp8" | "vp9" | "av1") {
            return PlaybackAction::Direct;
        }
    }

    // h264 + aac/mp3：浏览器可解，只是容器不合适 → 无损封装为 mp4
    let remuxable_video = video == "h264";
    let remuxable_audio = audio.is_empty() || matches!(audio.as_str(), "aac" | "mp3");
    if remuxable_video && remuxable_audio {
        return PlaybackAction::Remux;
    }

    PlaybackAction::Transcode
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(path: &str, container: &str, video: &str, audio: &str) -> MediaInfo {
        MediaInfo {
            path: path.into(),
            container: container.into(),
            video_codec: video.into(),
            audio_codec: audio.into(),
            ..Default::default()
        }
    }

    #[test]
    fn mp4_h264_is_direct() {
        assert_eq!(
            playback_action(&info("a.mp4", "mov,mp4,m4a", "h264", "aac")),
            PlaybackAction::Direct
        );
    }

    #[test]
    fn mkv_h264_aac_is_remux() {
        assert_eq!(
            playback_action(&info("a.mkv", "matroska,webm", "h264", "aac")),
            PlaybackAction::Remux
        );
    }

    #[test]
    fn webm_vp9_is_direct() {
        assert_eq!(
            playback_action(&info("a.webm", "matroska,webm", "vp9", "opus")),
            PlaybackAction::Direct
        );
    }

    #[test]
    fn hevc_transcodes() {
        assert_eq!(
            playback_action(&info("a.mp4", "mov,mp4", "hevc", "aac")),
            PlaybackAction::Transcode
        );
    }

    #[test]
    fn mpeg4_avi_transcodes() {
        assert_eq!(
            playback_action(&info("a.avi", "avi", "mpeg4", "mp3")),
            PlaybackAction::Transcode
        );
    }
}
