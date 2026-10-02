pub mod cache;
pub mod error;
pub mod ffmpeg;
pub mod metrics;
pub mod probe;
pub mod types;

pub use error::{CoreError, Result};
pub use metrics::{analyze_video, plan_frame_size, FrameSize};
pub use probe::{probe, MediaInfo, PlaybackAction};
pub use types::{
    AnalysisOutcome, AnalysisParams, FrameMetric, MarkerStore, MotionEvent, KIND_CUT, KIND_MANUAL,
    KIND_MOTION,
};
