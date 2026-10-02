use cytracer_core::types::{
    AnalysisOutcome, AnalysisParams, FrameMetric, MotionEvent, KIND_CUT, KIND_MOTION,
};

/// 曲线最大桶数（IPC / UI 渲染上限）
pub const MAX_CURVE_BUCKETS: usize = 4096;
/// 强度分级阈值（峰值活跃块比例）
pub const LEVEL_HIGH: f32 = 0.06;
pub const LEVEL_MID: f32 = 0.015;
/// 进入阈值的下限 / 上限
const ENTER_MIN: f32 = 0.003;
const ENTER_MAX: f32 = 0.12;
/// 退出阈值相对进入阈值的比例
const EXIT_RATIO: f32 = 0.5;
/// 退出判定所需的低分帧数（秒）
const RELEASE_SECONDS: f32 = 0.2;

#[derive(Debug, Clone, Copy)]
struct RawEvent {
    start: usize,
    end: usize,
    peak: usize,
    peak_score: f32,
}

/// 从逐帧指标提取运动事件与镜头切换。
pub fn detect(metrics: &[FrameMetric], params: &AnalysisParams) -> AnalysisOutcome {
    let fps = params.fps_clamped().max(0.1);
    let delta = params.block_delta();

    // 运动分数：镜头切换帧不计入运动状态
    let scores: Vec<f32> = metrics
        .iter()
        .map(|m| if m.scene_cut { 0.0 } else { m.score(delta) })
        .collect();

    // 自适应噪声底：低分位 + MAD 稳健估计
    let floor = percentile(&scores, 0.10);
    let median = percentile(&scores, 0.50);
    let deviations: Vec<f32> = scores.iter().map(|s| (s - median).abs()).collect();
    let mad = percentile(&deviations, 0.50);
    let enter = (floor + 4.0 * mad).clamp(ENTER_MIN, ENTER_MAX);
    let exit = (enter * EXIT_RATIO).max(ENTER_MIN * 0.5);

    let min_frames = ((params.min_duration * fps).ceil() as usize).max(1);
    let release_frames = ((RELEASE_SECONDS * fps).ceil() as usize).max(1);
    let merge_frames = ((params.merge_gap * fps).ceil() as usize).max(1);

    let mut raw: Vec<RawEvent> = Vec::new();
    let mut cuts: Vec<usize> = Vec::new();
    let mut in_event = false;
    let mut start = 0usize;
    let mut last_above = 0usize;
    let mut peak = 0usize;
    let mut peak_score = 0.0f32;
    let mut low = 0usize;

    for (i, metric) in metrics.iter().enumerate() {
        if metric.scene_cut {
            if in_event {
                raw.push(RawEvent { start, end: last_above, peak, peak_score });
                in_event = false;
            }
            if cuts.last().map(|last| i - last > 2).unwrap_or(true) {
                cuts.push(i);
            }
            continue;
        }
        let score = scores[i];
        if !in_event {
            if score >= enter {
                in_event = true;
                start = i;
                last_above = i;
                peak = i;
                peak_score = score;
                low = 0;
            }
        } else if score >= exit {
            last_above = i;
            low = 0;
            if score > peak_score {
                peak_score = score;
                peak = i;
            }
        } else {
            low += 1;
            if low >= release_frames {
                raw.push(RawEvent { start, end: last_above, peak, peak_score });
                in_event = false;
            }
        }
    }
    if in_event {
        raw.push(RawEvent { start, end: last_above, peak, peak_score });
    }

    // 丢弃过短事件
    raw.retain(|ev| ev.end + 1 - ev.start >= min_frames);
    // 合并相邻事件
    let mut merged: Vec<RawEvent> = Vec::with_capacity(raw.len());
    for ev in raw {
        if let Some(last) = merged.last_mut() {
            if ev.start.saturating_sub(last.end) <= merge_frames {
                last.end = ev.end;
                if ev.peak_score > last.peak_score {
                    last.peak = ev.peak;
                    last.peak_score = ev.peak_score;
                }
                continue;
            }
        }
        merged.push(ev);
    }

    let mut events: Vec<MotionEvent> = Vec::with_capacity(merged.len() + cuts.len());
    for (idx, ev) in merged.iter().enumerate() {
        events.push(MotionEvent {
            id: format!("auto-{idx}"),
            kind: KIND_MOTION.to_string(),
            start: ev.start as f64 / fps as f64,
            end: (ev.end as f64 + 1.0) / fps as f64,
            peak: ev.peak as f64 / fps as f64,
            score: ev.peak_score,
            level: level_of(ev.peak_score),
            label: String::new(),
            note: String::new(),
            color: String::new(),
            auto: true,
            edited: false,
        });
    }
    if params.mark_scene_cuts {
        for (idx, &frame) in cuts.iter().enumerate() {
            events.push(MotionEvent {
                id: format!("cut-{idx}"),
                kind: KIND_CUT.to_string(),
                start: frame as f64 / fps as f64,
                end: (frame as f64 + 1.0) / fps as f64,
                peak: frame as f64 / fps as f64,
                score: 1.0,
                level: 2,
                label: String::new(),
                note: String::new(),
                color: String::new(),
                auto: true,
                edited: false,
            });
        }
    }
    events.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));

    AnalysisOutcome {
        events,
        curve: downsample_curve(&scores),
        curve_buckets: scores.len().min(MAX_CURVE_BUCKETS).max(1),
        noise_floor: enter,
    }
}

pub fn level_of(score: f32) -> u8 {
    if score >= LEVEL_HIGH {
        2
    } else if score >= LEVEL_MID {
        1
    } else {
        0
    }
}

fn percentile(values: &[f32], p: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((p.clamp(0.0, 1.0) * (sorted.len() - 1) as f32).round() as usize).min(sorted.len() - 1);
    sorted[idx]
}

fn downsample_curve(scores: &[f32]) -> Vec<f32> {
    if scores.is_empty() {
        return Vec::new();
    }
    let buckets = scores.len().min(MAX_CURVE_BUCKETS);
    let mut curve = vec![0.0f32; buckets];
    for (i, score) in scores.iter().enumerate() {
        let bucket = i * buckets / scores.len();
        if *score > curve[bucket] {
            curve[bucket] = *score;
        }
    }
    curve
}

#[cfg(test)]
mod tests {
    use super::*;
    use cytracer_core::types::FrameMetric;

    fn metric(active_bins: &[(usize, u16)]) -> FrameMetric {
        let mut hist = [0u16; 16];
        for (bin, count) in active_bins {
            hist[*bin] = *count;
        }
        FrameMetric {
            hist,
            ..Default::default()
        }
    }

    #[test]
    fn quiet_metrics_produce_no_events() {
        let metrics = vec![metric(&[(0, 144)]); 100];
        let outcome = detect(&metrics, &AnalysisParams::default());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn sustained_motion_produces_event() {
        // 8fps 下约 1 秒运动（活跃块占 10/144 ≈ 0.069）
        let mut metrics = vec![metric(&[(0, 144)]); 20];
        metrics.extend(vec![metric(&[(0, 134), (5, 10)]); 8]);
        metrics.extend(vec![metric(&[(0, 144)]); 20]);
        let params = AnalysisParams::default();
        let outcome = detect(&metrics, &params);
        let motion: Vec<_> = outcome
            .events
            .iter()
            .filter(|e| e.kind == KIND_MOTION)
            .collect();
        assert_eq!(motion.len(), 1);
        assert!((motion[0].start - 2.5).abs() < 0.05);
        assert!(motion[0].end > 3.4);
    }

    #[test]
    fn short_motion_is_filtered() {
        let mut metrics = vec![metric(&[(0, 144)]); 20];
        metrics.extend(vec![metric(&[(0, 134), (5, 10)]); 1]);
        metrics.extend(vec![metric(&[(0, 144)]); 20]);
        let outcome = detect(&metrics, &AnalysisParams::default());
        assert!(outcome.events.iter().all(|e| e.kind != KIND_MOTION));
    }

    #[test]
    fn scene_cut_detected_separately() {
        let mut metrics = vec![metric(&[(0, 144)]); 20];
        metrics[10].scene_cut = true;
        let outcome = detect(&metrics, &AnalysisParams::default());
        assert!(outcome.events.iter().any(|e| e.kind == KIND_CUT));
        assert!(outcome.events.iter().all(|e| e.kind != KIND_MOTION));
    }

    #[test]
    fn curve_is_downsampled() {
        let metrics = vec![metric(&[(0, 144)]); 10000];
        let outcome = detect(&metrics, &AnalysisParams::default());
        assert!(outcome.curve.len() <= MAX_CURVE_BUCKETS);
    }
}
