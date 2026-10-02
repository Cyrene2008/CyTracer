use serde::{Deserialize, Serialize};

pub const KIND_MOTION: &str = "motion";
pub const KIND_CUT: &str = "cut";
pub const KIND_MANUAL: &str = "manual";

/// 分析参数（影响事件提取；sensitivity / 时长类参数不改变原始指标缓存）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisParams {
    /// 抽帧分析帧率（默认 8fps）
    pub fps: f32,
    /// 灵敏度 0..100，越大越容易触发
    pub sensitivity: f32,
    /// 事件最短持续（秒）
    pub min_duration: f32,
    /// 相邻事件间隙小于该值时合并（秒）
    pub merge_gap: f32,
    /// 是否补偿相机整体运动（手持抖动 / 摇镜）
    pub compensate_camera: bool,
    /// 是否输出镜头切换事件
    pub mark_scene_cuts: bool,
}

impl Default for AnalysisParams {
    fn default() -> Self {
        Self {
            fps: 8.0,
            sensitivity: 50.0,
            min_duration: 0.4,
            merge_gap: 0.3,
            compensate_camera: true,
            mark_scene_cuts: true,
        }
    }
}

impl AnalysisParams {
    /// 灵敏度映射为块差阈值（灰度差单位）
    pub fn block_delta(&self) -> f32 {
        let s = self.sensitivity.clamp(0.0, 100.0) / 100.0;
        24.0 + (6.0 - 24.0) * s
    }

    pub fn fps_clamped(&self) -> f32 {
        self.fps.clamp(1.0, 30.0)
    }
}

/// 单帧运动指标（缓存内容；不随灵敏度变化）
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameMetric {
    /// 补偿后块差的均值
    pub mean_diff: f32,
    /// 补偿后块差的中位数
    pub median_diff: f32,
    /// 补偿后块差的最大值
    pub max_diff: f32,
    /// 镜头切换
    pub scene_cut: bool,
    /// 活跃块包围盒（块坐标 [x0,y0,x1,y1]；无活跃块时为 [0,0,0,0]）
    pub bbox: [u16; 4],
    /// 块差直方图（bin 宽 4 灰度级，共 16 桶，上限 64）
    pub hist: [u16; 16],
}

impl Default for FrameMetric {
    fn default() -> Self {
        Self {
            mean_diff: 0.0,
            median_diff: 0.0,
            max_diff: 0.0,
            scene_cut: false,
            bbox: [0, 0, 0, 0],
            hist: [0; 16],
        }
    }
}

impl FrameMetric {
    /// 指定块差阈值下的活跃块比例（0..1）
    pub fn score(&self, delta: f32) -> f32 {
        let mut total: u32 = 0;
        let mut active: u32 = 0;
        for (i, count) in self.hist.iter().enumerate() {
            let n = *count as u32;
            total += n;
            let center = i as f32 * 4.0 + 2.0;
            if center > delta {
                active += n;
            }
        }
        if total == 0 {
            0.0
        } else {
            active as f32 / total as f32
        }
    }
}

/// 时间轴标记
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionEvent {
    pub id: String,
    /// motion | cut | manual
    pub kind: String,
    pub start: f64,
    pub end: f64,
    pub peak: f64,
    pub score: f32,
    /// 0 轻微 / 1 中等 / 2 剧烈
    pub level: u8,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub color: String,
    /// 是否自动检测产生
    #[serde(default)]
    pub auto: bool,
    /// 是否被用户编辑（编辑后不再被重新检测覆盖）
    #[serde(default)]
    pub edited: bool,
}

impl MotionEvent {
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// 检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisOutcome {
    pub events: Vec<MotionEvent>,
    /// 降采样后的运动强度曲线（每桶取最大值）
    pub curve: Vec<f32>,
    pub curve_buckets: usize,
    /// 自适应噪声底（进入阈值）
    pub noise_floor: f32,
}

/// 标记存储（自动 + 人工编辑，独立于指标缓存）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerStore {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub events: Vec<MotionEvent>,
    #[serde(default)]
    pub updated_at: u64,
}
