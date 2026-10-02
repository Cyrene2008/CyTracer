use std::fmt;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug)]
pub enum CoreError {
    Io(std::io::Error),
    FfmpegNotFound(String),
    FfmpegFailed(String),
    Probe(String),
    Cache(String),
    Canceled,
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Io(err) => write!(f, "IO error: {err}"),
            CoreError::FfmpegNotFound(name) => {
                write!(f, "ffmpeg/ffprobe not found: {name} (set CYTRACER_FFMPEG_DIR or place it next to the executable)")
            }
            CoreError::FfmpegFailed(msg) => write!(f, "ffmpeg failed: {msg}"),
            CoreError::Probe(msg) => write!(f, "probe failed: {msg}"),
            CoreError::Cache(msg) => write!(f, "cache error: {msg}"),
            CoreError::Canceled => write!(f, "canceled"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        CoreError::Io(err)
    }
}
