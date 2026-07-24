//! Voice Activity Detection 分段模块（ASR-LONG-AUDIO-001）
//!
//! 仅 accuracy 分支使用：native 模型 max_total_len=512 限制 ~28s 音频，
//! 长音频需 VAD 切分后逐段转录再拼接。
//! performance 分支不使用本模块（CTC 无此限制）。
//!
//! VAD 懒加载：仅 accuracy 模式首次遇到长音频时初始化并缓存。

use std::path::{Path, PathBuf};

use sherpa_onnx::{VadModelConfig, VoiceActivityDetector};

/// 安全阈值：音频时长 > 此值触发分段。
/// 实测 native 临界：27.88s(context_len=487) 正常 / 29.88s(520) 截断。
/// context_len = prompt(~18) + LFR_tokens + after(~5)，LFR 每 ~60ms 一个 token。
/// 24s ≈ 400 LFR tokens + 23 prompt = 423 < 512，留 ~89 token(~5.3s) 裕量。
pub const SEGMENT_TRIGGER_SECS: f64 = 24.0;

/// 单段上限：保证 context_len < 512。
/// 20s ≈ 333 LFR tokens + 23 prompt = 356 < 512，留 ~156 token 裕量。
pub const SEGMENT_MAX_SECS: f64 = 20.0;

/// 段前后 padding：保护边界音节（送气清声母 ~60-100ms）。200ms = 3200 samples @ 16kHz。
pub const SEGMENT_PADDING_SAMPLES: usize = 3200;

/// silero VAD 窗口大小（512 samples = 32ms @ 16kHz，silero_vad.onnx 要求）
const VAD_WINDOW_SIZE: i32 = 512;
const VAD_THRESHOLD: f32 = 0.5;
const VAD_MIN_SILENCE_DURATION: f32 = 0.3;
const VAD_MIN_SPEECH_DURATION: f32 = 0.1;
const VAD_MAX_SPEECH_DURATION: f32 = SEGMENT_MAX_SECS as f32;

/// VAD 分段器（懒加载，仅 accuracy 长音频使用）
pub struct VadSegmenter {
    detector: VoiceActivityDetector,
}

impl VadSegmenter {
    /// 尝试创建 VAD 分段器；模型缺失/失败返回 None（调用方降级单次转录）
    pub fn try_new(model_dir: &Path) -> Option<Self> {
        let vad_model = find_silero_vad_model(model_dir)?;
        let config = VadModelConfig {
            silero_vad: sherpa_onnx::SileroVadModelConfig {
                model: vad_model.to_str().map(|s| s.to_string()),
                threshold: VAD_THRESHOLD,
                min_silence_duration: VAD_MIN_SILENCE_DURATION,
                min_speech_duration: VAD_MIN_SPEECH_DURATION,
                window_size: VAD_WINDOW_SIZE,
                max_speech_duration: VAD_MAX_SPEECH_DURATION,
            },
            ten_vad: sherpa_onnx::TenVadModelConfig::default(),
            sample_rate: 16000,
            num_threads: 1,
            provider: Some("cpu".to_string()),
            debug: false,
        };
        let detector = VoiceActivityDetector::create(&config, 300.0)?;
        log::info!(
            "VAD segmenter initialized (silero, model={})",
            vad_model.display()
        );
        Some(Self { detector })
    }

    /// 对音频做 VAD 分段，返回切分后的段样本列表（已含 padding）。
    ///
    /// FIX-VAD-STATE-RESET-001: detector 在 Transcriber 生命周期内复用。
    /// `clear()` 仅清空段队列，不重置内部全局样本游标。第二次长音频
    /// 调用时 seg.start() 返回累计的绝对坐标（接着上次音频末尾），
    /// 导致 build_padded_segments slice 越界 panic（crash.json 实测
    /// range start 812992 out of range for slice of length 770400）。
    /// 修复：segment() 末尾 `clear()` 后调 `reset()`，将游标归零。
    pub fn segment(&self, samples: &[f32]) -> Vec<Vec<f32>> {
        // 喂样本：silero VAD 按 window_size=512 块处理
        let win = VAD_WINDOW_SIZE as usize;
        let mut offset = 0usize;
        while offset < samples.len() {
            let end = (offset + win).min(samples.len());
            self.detector.accept_waveform(&samples[offset..end]);
            offset = end;
        }
        self.detector.flush();

        // 收集 VAD 原始段 (start, samples_len)
        let raw: Vec<(usize, usize)> = std::iter::from_fn(|| {
            self.detector.front().map(|seg| {
                let start = seg.start() as usize;
                let n = seg.n() as usize;
                self.detector.pop();
                (start, n)
            })
        })
        .collect();
        // FIX-VAD-STATE-RESET-001: clear 清段队列 + reset 归零全局样本游标，
        // 确保下次 segment() 调用的 seg.start() 从 0 开始（相对本次音频）。
        self.detector.clear();
        self.detector.reset();

        if raw.is_empty() {
            return Vec::new();
        }

        // 合并 + padding + 从原音频提取（纵深防御：内部对越界段做过滤/clamp）
        build_padded_segments(&raw, samples.len(), samples)
    }
}

fn find_silero_vad_model(model_dir: &Path) -> Option<PathBuf> {
    let candidate = model_dir.join("silero-vad").join("silero_vad.onnx");
    if candidate.exists() {
        return Some(candidate);
    }
    let fallback = model_dir.join("silero_vad.onnx");
    if fallback.exists() {
        return Some(fallback);
    }
    None
}

/// 纯函数：合并相邻短段 + padding + 从原音频提取段样本。
///
/// 输入 raw: VAD 检测的原始段 (start_sample, len_samples)
/// 输出: 每段的实际样本数据（已加 padding），可直接送 recognizer 转录。
///
/// 规则：
/// 1. 相邻段合并后总长 ≤ SEGMENT_MAX_SECS 则合并
/// 2. 单段自身 > SEGMENT_MAX_SECS（VAD max_speech_duration 已硬切，兜底）→ 硬切
/// 3. 每段前后加 SEGMENT_PADDING_SAMPLES，padding 区填 0（静音保护边界音节）
/// 4. padding 不超出原音频边界，相邻段 padding 不重叠
///
/// FIX-VAD-STATE-RESET-001 纵深防御：对 raw 段做边界过滤——
/// - start >= total_samples 的段丢弃并 log warn（detector 游标未重置导致越界）
/// - end 超界 clamp 到 total_samples
/// - 任何情况下不允许 slice 越界
pub fn build_padded_segments(
    raw: &[(usize, usize)],
    total_samples: usize,
    full_audio: &[f32],
) -> Vec<Vec<f32>> {
    if raw.is_empty() {
        return Vec::new();
    }
    let max_seg_samples = (SEGMENT_MAX_SECS * 16000.0) as usize;
    let pad = SEGMENT_PADDING_SAMPLES;

    // 第一步：边界过滤 + 合并相邻短段 → (start, end) 列表
    // FIX-VAD-STATE-RESET-001: 丢弃 start 越界的段（detector 游标未重置
    // 致 seg.start() 返回跨音频累计坐标），clamp end 越界段
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for &(start, len) in raw {
        if start >= total_samples {
            log::warn!(
                "VAD segment start {} >= total_samples {}, dropping (detector cursor not reset?)",
                start,
                total_samples
            );
            continue;
        }
        let end = (start + len).min(total_samples);
        let clamped_len = end - start;
        if clamped_len == 0 {
            continue;
        }
        if clamped_len >= max_seg_samples {
            // 单段超上限：硬切
            let mut pos = start;
            while pos < end {
                let sub_end = (pos + max_seg_samples).min(end);
                merged.push((pos, sub_end));
                pos = sub_end;
            }
            continue;
        }
        if let Some(last) = merged.last_mut() {
            let combined = end - last.0;
            if combined <= max_seg_samples {
                last.1 = end;
                continue;
            }
        }
        merged.push((start, end));
    }

    if merged.is_empty() {
        return Vec::new();
    }

    // 第二步：加 padding 并提取样本
    let mut result: Vec<Vec<f32>> = Vec::with_capacity(merged.len());
    for (i, &(start, end)) in merged.iter().enumerate() {
        // padding 起点：首段 saturating_sub；后续段与前段间隙取中点
        let pad_start = if i == 0 {
            start.saturating_sub(pad)
        } else {
            let prev_end = merged[i - 1].1;
            let gap_mid = prev_end + (start.saturating_sub(prev_end)) / 2;
            // 不与前段重叠
            if gap_mid > prev_end {
                gap_mid.saturating_sub(pad).max(prev_end)
            } else {
                prev_end
            }
        };
        // padding 终点：末段 +pad(min total)；后续段与下段间隙取中点
        let pad_end = if i == merged.len() - 1 {
            (end + pad).min(total_samples)
        } else {
            let next_start = merged[i + 1].0;
            let gap_mid = end + (next_start.saturating_sub(end)) / 2;
            // 不与下段重叠
            if gap_mid < next_start {
                (gap_mid + pad).min(next_start)
            } else {
                next_start
            }
        };

        let mut seg = Vec::with_capacity(pad_end - pad_start);
        // padding 区（pad_start 到 start）填 0（静音，避免引入邻段语音边界伪影）
        if pad_start < start {
            seg.extend(std::iter::repeat(0.0f32).take(start - pad_start));
        }
        // 主段（start 到 end）：从原音频取（end 已 clamp 到 total_samples，安全）
        seg.extend_from_slice(&full_audio[start..end.min(total_samples)]);
        // padding 区（end 到 pad_end）填 0
        if end < pad_end {
            seg.extend(std::iter::repeat(0.0f32).take(pad_end - end));
        }
        result.push(seg);
    }
    result
}

/// 纯函数：是否应触发分段（音频时长 > SEGMENT_TRIGGER_SECS）
pub fn should_segment(samples: &[f32]) -> bool {
    let secs = samples.len() as f64 / 16000.0;
    secs > SEGMENT_TRIGGER_SECS
}

/// ASR-SINGLE-MODEL-001（DEC-027）：朴素等分切段。
///
/// VAD segmenter 不可用时的兜底分段策略：按 SEGMENT_MAX_SECS 硬切，
/// 保证 accuracy 长音频在 VAD 模型缺失时仍可用（禁止 >28s 整段喂 native，
/// max_total_len=512 是未定义行为区）。
///
/// 与 VAD 分段的差异：
/// - 无语音活动检测，静音段也被转录（native 模型对静音输出空，join 后自动过滤）
/// - 无 padding（朴素切分不需保护边界音节，段间边界可能在句中）
/// - 最后一段可能 < SEGMENT_MAX_SECS
///
/// 边界覆盖保证：start..end 步进 max_seg_samples，最后一段包含余量，
/// 全部样本被覆盖，无遗漏。
pub fn naive_chunk(samples: &[f32]) -> Vec<Vec<f32>> {
    if samples.is_empty() {
        return Vec::new();
    }
    let max_seg_samples = (SEGMENT_MAX_SECS * 16000.0) as usize;
    let mut result = Vec::new();
    let mut offset = 0usize;
    while offset < samples.len() {
        let end = (offset + max_seg_samples).min(samples.len());
        result.push(samples[offset..end].to_vec());
        offset = end;
    }
    result
}

/// 拼接分段文本（中文直接连接；段尾/段首均为拉丁字母间补空格）
pub fn join_segment_texts(segments: &[String]) -> String {
    let mut result = String::new();
    for (i, seg) in segments.iter().enumerate() {
        if i == 0 {
            result.push_str(seg);
            continue;
        }
        let prev = &segments[i - 1];
        let prev_last = prev.chars().last();
        let this_first = seg.chars().next();
        match (prev_last, this_first) {
            (Some(pl), Some(tf)) if pl.is_ascii_alphabetic() && tf.is_ascii_alphabetic() => {
                result.push(' ');
                result.push_str(seg);
            }
            _ => result.push_str(seg),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_segment_below_threshold() {
        // 24s = 384000 samples，恰好不触发（> 而非 >=）
        assert!(!should_segment(&vec![0.0f32; 384000]));
    }

    #[test]
    fn should_segment_above_threshold() {
        assert!(should_segment(&vec![0.0f32; 400000])); // 25s
    }

    #[test]
    fn should_segment_empty_not_triggered() {
        assert!(!should_segment(&[]));
    }

    #[test]
    fn join_chinese_direct() {
        let segs = vec!["周末要不要去露营".to_string(), "最近天气超舒服".to_string()];
        assert_eq!(join_segment_texts(&segs), "周末要不要去露营最近天气超舒服");
    }

    #[test]
    fn join_english_adds_space() {
        let segs = vec!["hello world".to_string(), "this is a test".to_string()];
        assert_eq!(join_segment_texts(&segs), "hello world this is a test");
    }

    #[test]
    fn join_mixed_no_space_chinese_to_english() {
        let segs = vec!["今天天气很好".to_string(), "very nice".to_string()];
        assert_eq!(join_segment_texts(&segs), "今天天气很好very nice");
    }

    #[test]
    fn join_single_segment() {
        let segs = vec!["only one".to_string()];
        assert_eq!(join_segment_texts(&segs), "only one");
    }

    #[test]
    fn join_empty() {
        assert_eq!(join_segment_texts(&[]), "");
    }

    #[test]
    fn build_padded_single_segment() {
        // 单段：start=3200(0.2s), len=12800(0.8s)，total=32000
        let audio: Vec<f32> = (0..32000).map(|i| i as f32 / 1000.0).collect();
        let raw = vec![(3200, 12800)];
        let result = build_padded_segments(&raw, 32000, &audio);
        assert_eq!(result.len(), 1);
        // pad_start=0, start=3200 → 3200 zeros + 12800 audio + pad_end=16000+3200=19200 → 3200 zeros
        assert_eq!(result[0].len(), 19200);
        // padding 区为 0
        assert_eq!(result[0][0], 0.0);
        assert_eq!(result[0][3199], 0.0);
        // 主段起始 = audio[3200]
        assert_eq!(result[0][3200], audio[3200]);
    }

    #[test]
    fn build_padded_adjacent_short_merged() {
        // seg1=(0, 8000=0.5s), seg2=(9000, 8000=0.5s)，间隔 1000 samples
        // 合并后 0..17000=1.0625s < 20s → 单段
        let audio: Vec<f32> = (0..32000).map(|i| i as f32 / 1000.0).collect();
        let raw = vec![(0, 8000), (9000, 8000)];
        let result = build_padded_segments(&raw, 32000, &audio);
        assert_eq!(result.len(), 1, "adjacent short should merge");
        // start=0 pad_start=0, end=17000 pad_end=17000+3200=20200
        assert!(result[0].len() >= 17000);
    }

    #[test]
    fn build_padded_long_hard_cut() {
        // 25s 单段 → 硬切为 20s + 5s
        let audio: Vec<f32> = (0..400000).map(|i| i as f32 / 1000.0).collect();
        let raw = vec![(0, 400000)];
        let result = build_padded_segments(&raw, 400000, &audio);
        assert_eq!(result.len(), 2, "25s should hard-cut into 2");
        // 第一段 0..320000，第二段 320000..400000
    }

    #[test]
    fn build_padded_empty() {
        let result = build_padded_segments(&[], 1000, &[0.0; 1000]);
        assert!(result.is_empty());
    }

    #[test]
    fn build_padded_adjacent_no_overlap() {
        // 两段充分接近，padding 不应重叠
        // seg1=(0, 16000=1s), seg2=(16500, 16000=1s)，间隔 500 samples
        let audio: Vec<f32> = (0..40000).map(|i| i as f32).collect();
        let raw = vec![(0, 16000), (16500, 16000)];
        let result = build_padded_segments(&raw, 40000, &audio);
        // 合并后 0..32500=2.03s < 20s → 仍 1 段（间隙小会合并）
        // 间隔 500 < max_seg 所以合并
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn find_silero_missing_returns_none() {
        let tmp = std::env::temp_dir().join("vad-nonexist-xyz-test");
        assert!(find_silero_vad_model(&tmp).is_none());
    }

    // ============================================================
    // FIX-VAD-STATE-RESET-001: 越界防御 + 连续调用测试
    // ============================================================

    #[test]
    fn build_padded_drops_start_out_of_range() {
        // 模拟 crash.json 场景：raw 段 start 超出 total_samples
        // 第二次音频只有 770400 samples，但 detector 游标未 reset 致 start=812992
        let audio: Vec<f32> = (0..770400).map(|i| i as f32).collect();
        let raw = vec![(812992, 16000)]; // start 越界
        let result = build_padded_segments(&raw, 770400, &audio);
        assert!(
            result.is_empty(),
            "out-of-range start segment must be dropped, not panic"
        );
    }

    #[test]
    fn build_padded_clamps_end_out_of_range() {
        // end 越界（start 在界内，start+len 超出 total）→ clamp 不 panic
        let audio: Vec<f32> = (0..32000).map(|i| i as f32).collect();
        let raw = vec![(16000, 32000)]; // start=16000 在界内，end=48000 越界
        let result = build_padded_segments(&raw, 32000, &audio);
        assert_eq!(result.len(), 1, "end-clamped segment must be kept");
        // 段内容 = audio[16000..32000]（clamped end）+ 末尾 padding
        // pad_start = 16000 - 3200 = 12800, pad_end = min(32000+3200, 32000) = 32000
        // seg = [12800..16000 zeros] + [16000..32000 audio] = 19200 samples
        assert_eq!(result[0].len(), 19200);
        // padding 区为 0
        assert_eq!(result[0][0], 0.0);
        assert_eq!(result[0][3199], 0.0);
        // 主段起始 = audio[16000]
        assert_eq!(result[0][3200], audio[16000]);
    }

    #[test]
    fn build_padded_mixed_in_range_and_out_of_range() {
        // 混合：第一个段在界内，第二个段 start 越界（detector 游标累计）
        let audio: Vec<f32> = (0..50000).map(|i| i as f32).collect();
        let raw = vec![(0, 16000), (60000, 16000)]; // 第二段 start 越界
        let result = build_padded_segments(&raw, 50000, &audio);
        assert_eq!(
            result.len(),
            1,
            "only in-range segment kept, out-of-range dropped"
        );
    }

    #[test]
    fn build_padded_all_out_of_range_returns_empty() {
        // 全部段 start 越界 → 返回空，不 panic
        let audio: Vec<f32> = vec![0.0; 1000];
        let raw = vec![(1000, 100), (2000, 100), (3000, 100)];
        let result = build_padded_segments(&raw, 1000, &audio);
        assert!(result.is_empty());
    }

    #[test]
    fn build_padded_zero_len_after_clamp_skipped() {
        // start 恰好等于 total_samples → 边界丢弃；start 在界内但 len=0 → clamp 后 0 长度跳过
        let audio: Vec<f32> = vec![0.0; 1000];
        let raw = vec![(500, 0), (1000, 100)]; // 0 长 + 边界
        let result = build_padded_segments(&raw, 1000, &audio);
        assert!(
            result.is_empty(),
            "zero-length and boundary segments skipped"
        );
    }

    #[test]
    #[ignore = "requires working ORT runtime (vendor ORT 1.17.1 may not support API v24)"]
    fn vad_segmenter_consecutive_calls_no_panic() {
        // FIX-VAD-STATE-RESET-001 核心测试：连续两次 segment() 调用，
        // 第二次段起点必须在第二次音频范围内（验证 reset 归零游标）。
        // 使用真实 silero VAD 模型（若存在）；否则跳过。
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = project_root.join("models");
        let segmenter = match VadSegmenter::try_new(&model_dir) {
            Some(s) => s,
            None => {
                eprintln!(
                    "skip: silero_vad.onnx not found at {}, VAD model required",
                    model_dir.display()
                );
                return;
            }
        };

        // 合成第一段长音频：30s 含两个语音段（用 sine 模拟语音能量）
        let audio1: Vec<f32> = (0..480000)
            .map(|i| {
                let t = i as f32 / 16000.0;
                // 0-10s 有语音 + 10-20s 静音 + 20-30s 有语音
                if (5.0..=10.0).contains(&t) || (25.0..=30.0).contains(&t) {
                    (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3
                } else {
                    0.0
                }
            })
            .collect();

        // 合成第二段长音频：40s（比第一段长，验证游标归零后起点在界内）
        let audio2: Vec<f32> = (0..640000)
            .map(|i| {
                let t = i as f32 / 16000.0;
                if (5.0..=15.0).contains(&t) || (25.0..=35.0).contains(&t) {
                    (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3
                } else {
                    0.0
                }
            })
            .collect();

        // 第一次调用：不应 panic
        let segs1 = segmenter.segment(&audio1);
        assert!(!segs1.is_empty(), "first call should produce segments");

        // 第二次调用：核心断言——不应 panic，段起点必须在 audio2 范围内
        // 旧 bug：seg.start() 返回 480000+（接着 audio1 末尾），slice 越界 panic
        let segs2 = segmenter.segment(&audio2);
        assert!(!segs2.is_empty(), "second call should produce segments");
        // 每段长度 ≤ SEGMENT_MAX_SECS + padding 裕量
        for seg in &segs2 {
            let seg_secs = seg.len() as f64 / 16000.0;
            assert!(
                seg_secs <= SEGMENT_MAX_SECS + 0.5,
                "segment {:.1}s exceeds {}s limit",
                seg_secs,
                SEGMENT_MAX_SECS
            );
        }
    }

    // ============================================================
    // ASR-SINGLE-MODEL-001: naive_chunk 朴素等分切段测试
    // ============================================================

    #[test]
    fn naive_chunk_empty() {
        let result = naive_chunk(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn naive_chunk_exact_one_segment() {
        // 恰好 20s = 320000 samples → 1 段
        let samples: Vec<f32> = (0..320000).map(|i| i as f32).collect();
        let result = naive_chunk(&samples);
        assert_eq!(result.len(), 1, "exactly 20s must be 1 segment");
        assert_eq!(result[0].len(), 320000);
        // 覆盖完整性：第一段从 0 开始
        assert_eq!(result[0][0], 0.0);
        assert_eq!(result[0][319999], 319999.0);
    }

    #[test]
    fn naive_chunk_just_over_one_segment() {
        // 20.1s = 321600 samples → 2 段（320000 + 1600）
        let samples: Vec<f32> = (0..321600).map(|i| i as f32).collect();
        let result = naive_chunk(&samples);
        assert_eq!(result.len(), 2, "20.1s must be 2 segments");
        assert_eq!(result[0].len(), 320000, "first segment = 20s");
        assert_eq!(result[1].len(), 1600, "second segment = 0.1s remainder");
        // 覆盖完整性：第二段从 320000 开始
        assert_eq!(result[1][0], 320000.0);
        assert_eq!(result[1][1599], 321599.0);
    }

    #[test]
    fn naive_chunk_60s_three_segments() {
        // 60s = 960000 samples → 3 段（各 20s）
        let samples: Vec<f32> = (0..960000).map(|i| i as f32).collect();
        let result = naive_chunk(&samples);
        assert_eq!(result.len(), 3, "60s must be 3 segments");
        for seg in &result {
            assert_eq!(seg.len(), 320000, "each segment = 20s");
        }
        // 覆盖完整性：段连续无遗漏
        assert_eq!(result[0][0], 0.0);
        assert_eq!(result[1][0], 320000.0);
        assert_eq!(result[2][0], 640000.0);
        assert_eq!(result[2][319999], 959999.0);
    }

    #[test]
    fn naive_chunk_coverage_completeness() {
        // 验证所有样本被覆盖，无遗漏、无重复
        let samples: Vec<f32> = (0..500000).map(|i| i as f32).collect();
        let result = naive_chunk(&samples);
        let mut covered: Vec<f32> = Vec::new();
        for seg in &result {
            covered.extend_from_slice(seg);
        }
        assert_eq!(covered.len(), 500000, "all samples must be covered");
        assert_eq!(covered, samples, "coverage must be exact, no gaps/overlaps");
    }

    #[test]
    fn naive_chunk_uneven_remainder() {
        // 50s = 800000 samples → 2 段 20s + 1 段 10s
        let samples: Vec<f32> = (0..800000).map(|i| i as f32).collect();
        let result = naive_chunk(&samples);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].len(), 320000);
        assert_eq!(result[1].len(), 320000);
        assert_eq!(result[2].len(), 160000, "last segment = 10s remainder");
    }

    /// 集成验证：真实 VAD 模型切分长音频（30/60/90s）
    /// 手动运行：cargo test vad_integration_long_audio -- --ignored --nocapture
    #[test]
    #[ignore = "requires silero_vad.onnx in models/ + long wav files"]
    fn vad_integration_long_audio() {
        // 用 CARGO_MANIFEST_DIR 定位项目根 models/，不依赖 exe 位置
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = project_root.join("models");
        let segmenter = VadSegmenter::try_new(&model_dir)
            .unwrap_or_else(|| panic!("VAD model should be present at {}", model_dir.display()));

        // 读取 long_30s/60s/90s.wav（需提前用 target/long_audio_test 脚本生成）
        let test_dir = project_root.join("target").join("long_audio_test");

        for secs in [30, 60, 90] {
            let wav_path = test_dir.join(format!("long_{}s.wav", secs));
            if !wav_path.exists() {
                eprintln!("skip {}: {} not found", secs, wav_path.display());
                continue;
            }
            let samples = read_wav_mono(&wav_path);
            let segs = segmenter.segment(&samples);
            let seg_secs: Vec<f64> = segs.iter().map(|s| s.len() as f64 / 16000.0).collect();
            let max_seg = seg_secs.iter().cloned().fold(0.0f64, f64::max);
            println!(
                "{}s audio -> {} segments, max segment {:.1}s, durations: {:?}",
                secs,
                segs.len(),
                max_seg,
                seg_secs
            );
            // 关键断言：每段 ≤ SEGMENT_MAX_SECS + 2*padding(0.4s) 裕量
            assert!(
                max_seg <= SEGMENT_MAX_SECS + 0.5,
                "segment {}s exceeds {}s limit",
                max_seg,
                SEGMENT_MAX_SECS
            );
            assert!(!segs.is_empty(), "should produce at least 1 segment");
        }
    }

    fn read_wav_mono(path: &std::path::Path) -> Vec<f32> {
        // 简单 16-bit mono PCM 读取（避免引入额外依赖）
        use std::io::Read;
        let mut file = std::fs::File::open(path).expect("open wav");
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).expect("read wav");
        // 跳过 44 字节 WAV header，读 16-bit samples
        let data = &buf[44..];
        let mut samples = Vec::with_capacity(data.len() / 2);
        for chunk in data.chunks_exact(2) {
            let v = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
            samples.push(v);
        }
        samples
    }
}
