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

/// FIX-SLICE-CUT-AT-GAP-381：滑窗（accuracy 派发）路径的**字缝切点起始搜索位置** = 10s。
///
/// 🔴 Gavin 2026-09-23 统一口径（原话见本单任务书）：**窗口上限与切片起搜点统一为 10s**
/// ——「单窗口主窗最大音频长度 12 秒」与「最长连续语音硬切点 13 秒」这两个时间点同时作废。
/// 单片剩余长度超过 10s ⇒ 从「本片起点 + 10s」往后找第一个**字缝**切（见 [`plan_gap_cuts`]），
/// 不再按固定秒数硬切 —— 硬切点可能正好落在字上，把字切碎导致识别出错。
///
/// 🔴 必须与 `WINDOW_MAX_SECS`（`transcription/mod.rs`，组窗上限）**相等** ——
/// 由单测 `sliding_cut_search_start_equals_window_max` 锁定，防二者再次漂移。
pub const SLIDING_CUT_SEARCH_START_SECS: f64 = 10.0;

/// FIX-SLICE-CUT-AT-GAP-381：字缝搜索的**最远兜底偏移** = 2.0s（即最晚切到 12s）。
///
/// 从 [`SLIDING_CUT_SEARCH_START_SECS`](10s) 起最多再往后搜这么久；仍找不到达标字缝 ⇒
/// 在 [10s, 12s] 内取 RMS **最低**的帧切。任何情况下都不会无限变长（有硬上限）。
pub const GAP_SEARCH_MAX_SECS: f64 = 2.0;

/// FIX-SLICE-CUT-AT-GAP-381：字缝能量判据 —— 帧 RMS ≤ 该比值 × 本片前 10s 帧 RMS 的**中位数**。
///
/// 用**相对阈值**（对本片前 10s 的中位数取比）⇒ 不受麦克风增益 / 整体音量影响。
/// 初值 0.3（主控方案），由本单实测校准（`poc_slice_cut_381`）。
pub const GAP_RMS_RATIO: f32 = 0.3;

/// FIX-SLICE-CUT-AT-GAP-381：**尾巴保护** —— 剩余长度 < 起搜点 + 此值 ⇒ 不切。
///
/// 即剩余 < 11s 时整段作为一片（最长约 11s），避免切出零点几秒的碎片单独解码；
/// 超过窗口上限的单片由 `group_window_start_secs` 单独成窗，KV 远非瓶颈。
pub const MIN_TAIL_SECS: f64 = 1.0;

/// FIX-SLICE-CUT-AT-GAP-381：字缝能量帧长 = 20ms @16kHz = 320 样本。
pub const GAP_FRAME_SAMPLES: usize = 320;

/// silero VAD 窗口大小（512 samples = 32ms @ 16kHz，silero_vad.onnx 要求）
const VAD_WINDOW_SIZE: i32 = 512;

/// ASR-038-B: 导出 VAD 窗口大小供流式入口门控使用
pub fn vad_window_size() -> usize {
    VAD_WINDOW_SIZE as usize
}

/// FIX-VAD-FEED-BY-WINDOW-391：按 `VAD_WINDOW_SIZE`(512) **逐块**喂入 `audio`（末尾不足一块照常喂入）。
/// 返回喂入次数（= `ceil(len / 512)`）。纯逻辑，可单测。
///
/// 🔴 **为什么不能整段一次性 `accept_waveform`**（sherpa `voice-activity-detector.cc` `AcceptWaveform`）：
/// - 注释原文：`// note n is usually window_size` 与 `// NOTE(fangjun): Please don't use a very large n.`；
/// - 一次调用内对所有窗 `is_speech = is_speech || this_window_is_speech`（**整次调用只得一个结论**）；
/// - 语音开始时 `start_ = max(buffer_.Tail() - 2*WindowSize - MinSpeechDurationSamples, Head)`
///   ⇒ 起点被定在**本次输入末尾往前约 0.164s**。
/// ⇒ 一次性喂整段只得到末尾一小段（388 现场 `in=6.31s out=0.37s`）；**必须逐 512 块喂**。
/// 🔴 388 记录里「384 任务书补充第 2 条：整块一次 `accept_waveform`、不要切 512」**该指示错误**，
/// 本单纠正 —— 原 `segment()` 按 512 切块喂入才是正确用法。
fn feed_in_vad_windows<F: FnMut(&[f32])>(audio: &[f32], mut accept: F) -> usize {
    let win = VAD_WINDOW_SIZE as usize;
    let mut offset = 0usize;
    let mut calls = 0usize;
    while offset < audio.len() {
        let end = (offset + win).min(audio.len());
        accept(&audio[offset..end]);
        offset = end;
        calls += 1;
    }
    calls
}

const VAD_THRESHOLD: f32 = 0.5;
/// ASR-038-B: 入口门控 VAD 阈值（低于分段用 0.5）
/// 设计文档 §2.2：0.3 对应「只要有微弱语音特征就建连」，牺牲纯静音不建连效果换取首字安全
/// Gavin 硬指令：必须用真 VAD 防「键盘声/风扇声/音乐」无效上传，RMS 被否决
const VAD_STREAMING_THRESHOLD: f32 = 0.3;
const VAD_MIN_SILENCE_DURATION: f32 = 0.3;
const VAD_MIN_SPEECH_DURATION: f32 = 0.1;
const VAD_MAX_SPEECH_DURATION: f32 = SEGMENT_MAX_SECS as f32;

// ===========================================================================
// LOCALRT-VAD-SILENCE-384：**本地 realtime 静默判定**专用 silero VAD 配置
//   （Gavin：环境有背景声时音量阈值判静默失效，改用 VAD 判「有没有人声」；
//    VAD 缺失/失败退回原音量阈值兜底。只服务本地 realtime 管线，不动其它 VAD 用途。）
// ===========================================================================

/// LOCALRT-VAD-SILENCE-384：本地 realtime 静默判定专用 VAD 阈值。
///
/// 取 **0.5**（silero 标准值，同离线分段 `VAD_THRESHOLD`）：比在线门控的 `VAD_STREAMING_THRESHOLD`
/// (0.3) 更不易被环境噪声触发 —— 本用途是「判是否有人声以计时静默」，误报「有人声」会让
/// 1200ms 静默永不到达（正是本单要修的噪声环境病）⇒ 宁可保守。
pub const LOCALRT_VAD_THRESHOLD: f32 = 0.5;

/// LOCALRT-VAD-SILENCE-384：本地 realtime 专用 `min_silence_duration`（秒）。
///
/// 与既有 VAD 同值 0.3，但**独立命名**以便后续单独调；1200ms 计时补偿
/// （`local_stream::localrt_vad_seed_ms`）引用本常量。
pub const LOCALRT_VAD_MIN_SILENCE_SECS: f32 = 0.3;

/// LOCALRT-VAD-SILENCE-384：本地 realtime 专用 VAD 环形缓冲秒数（`buffer_size_in_seconds`）。
///
/// 取值依据：本用途只查 `detected()`，且每个 chunk 把**已完成**段 `pop` 掉 ⇒ 缓冲只需容纳
/// 「当前进行中的一段人声」。正常口述「连续 >60s 无 0.3s 停顿」已极罕见 ⇒ 取 **60s**
/// （≈3.8MB @16k f32），而非既有的 300s（≈19.2MB）。
pub const LOCALRT_VAD_BUFFER_SECS: f32 = 60.0;

/// VAD-393（B）：本地 realtime 两个构造（`try_new_for_local_silence` / `try_new_for_local_trim`）的
/// **单段最长语音**，取 60s = [`LOCALRT_VAD_BUFFER_SECS`]。
///
/// 依据（sherpa `voice-activity-detector.cc:51-53, 227-229`）：`max_speech_duration` 超限后
/// **阈值升 0.90、min_silence 降 0.1s 强制切段**；本地 realtime 需要「整句尽量不切」，
/// 故把上限定到缓冲允许的 60s（而非离线/在线的 20s），避免长句被强制切。
/// 🔴 离线分段 `try_new` / 在线门控 `try_new_for_streaming` 继续用
/// [`VAD_MAX_SPEECH_DURATION`](20s)，**不动**。
pub const LOCALRT_VAD_MAX_SPEECH_SECS: f32 = 60.0;

/// FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388：本地 realtime **窗口剪静音**时，语音段两侧各留的 padding 秒数。
///
/// 200ms 与既有多处边界 padding 同口径（保护送气清声母 ~60-100ms）。效果：
/// 首尾静音剩 ≤200ms；段间停顿 ≤400ms 原样保留、>400ms 压到 400ms（见 `trim_to_speech`）。
pub const LOCALRT_TRIM_PAD_SECS: f32 = 0.2;

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

    // ===========================================================================
    // ASR-038-B: 流式入口门控滚动 VAD（Gavin 硬指令：真 VAD 防「键盘声/风扇声/音乐」无效上传）
    // ===========================================================================

    /// 流式入口门控专用工厂：阈值 0.3（低于分段用 0.5），其他配置同 try_new。
    /// 模型缺失/失败返回 None → 调用方降级为「总是建连」（宁可多花钱不可吞字）。
    pub fn try_new_for_streaming(model_dir: &Path) -> Option<Self> {
        let vad_model = find_silero_vad_model(model_dir)?;
        let config = VadModelConfig {
            silero_vad: sherpa_onnx::SileroVadModelConfig {
                model: vad_model.to_str().map(|s| s.to_string()),
                threshold: VAD_STREAMING_THRESHOLD,
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
            "VAD streaming gate initialized (silero, threshold={}, model={})",
            VAD_STREAMING_THRESHOLD,
            vad_model.display()
        );
        Some(Self { detector })
    }

    /// 滚动 VAD 判定：喂一个窗口的样本（512 samples = 32ms），返回是否检测到语音。
    ///
    /// **与 segment() 的关键差异**（主控取证纠正）：
    /// - 不调 `flush()` —— flush 是批处理收尾，强制输出未完成的段；滚动判定不需要
    /// - 用 `detected()` 查询当前是否有语音（sherpa-onnx C API `SherpaOnnxVoiceActivityDetectorDetected`）
    /// - 不收段（不调 front/pop），不做分段，只回答「有没有语音」这一个布尔
    ///
    /// **调用方式**：ASR 线程每从音频 channel 收到一个 chunk，喂给本方法。
    /// 返回 true → 立即建连 WebSocket。
    /// 2s 内未命中 → 无条件建连（保底，防 VAD 漏检吞字）。
    ///
    /// **不调 reset/clear**：滚动判定依赖 detector 内部状态连续性，
    /// reset 会清空内部缓冲破坏滚动语义。整个录音会话用同一个 VadSegmenter 实例。
    /// 会话结束后由实例 drop 自动清理。
    ///
    /// **并发安全**：本方法用 `&self`，sherpa-onnx C++ 层 VAD 推理本身不可重入，
    /// 调用方须保证同一实例同一时刻只有一个线程访问（ASR 线程独占）。
    pub fn accept_and_check(&self, samples: &[f32]) -> bool {
        self.detector.accept_waveform(samples);
        self.detector.detected()
    }

    /// 重置 VAD 状态（会话结束时调，归零内部游标与段队列）。
    /// 滚动判定期间不调；只在会话结束、实例将复用时调。
    pub fn reset_for_new_session(&self) {
        self.detector.clear();
        self.detector.reset();
    }

    // ===========================================================================
    // LOCALRT-VAD-SILENCE-384：本地 realtime 静默判定（只新增，不改既有方法/常量）
    // ===========================================================================

    // ===========================================================================
    // FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388：本地 realtime **窗口剪静音**（只新增，不改既有方法/常量）
    // ===========================================================================

    /// FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388：本地 realtime **窗口剪静音**专用工厂。
    ///
    /// 参数同 [`try_new_for_local_silence`]（阈值 0.5 / min_silence 0.3s / window 512），
    /// buffer 取 [`LOCALRT_VAD_BUFFER_SECS`](60s)。**只新增**：旧构造一律不动。
    pub fn try_new_for_local_trim(model_dir: &Path) -> Option<Self> {
        let vad_model = find_silero_vad_model(model_dir)?;
        let config = VadModelConfig {
            silero_vad: sherpa_onnx::SileroVadModelConfig {
                model: vad_model.to_str().map(|s| s.to_string()),
                threshold: LOCALRT_VAD_THRESHOLD,
                min_silence_duration: LOCALRT_VAD_MIN_SILENCE_SECS,
                min_speech_duration: VAD_MIN_SPEECH_DURATION,
                window_size: VAD_WINDOW_SIZE,
                max_speech_duration: LOCALRT_VAD_MAX_SPEECH_SECS,
            },
            ten_vad: sherpa_onnx::TenVadModelConfig::default(),
            sample_rate: 16000,
            num_threads: 1,
            provider: Some("cpu".to_string()),
            debug: false,
        };
        let detector = VoiceActivityDetector::create(&config, LOCALRT_VAD_BUFFER_SECS)?;
        log::info!(
            "VAD local-trim initialized (silero, threshold={}, min_silence={}s, buffer={}s, model={})",
            LOCALRT_VAD_THRESHOLD,
            LOCALRT_VAD_MIN_SILENCE_SECS,
            LOCALRT_VAD_BUFFER_SECS,
            vad_model.display()
        );
        Some(Self { detector })
    }

    /// FIX-WINDOW-TRIM-AND-OUTPUT-FLOOR-388：返回 `audio` 中的**语音区间** `[(start, end)]`
    ///（样本下标，未加 pad；`end` 为**开区间**端点）。
    ///
    /// 做法（`segment()` 的精简版，只取区间、不做 padding/合并）：
    /// `reset()` → **逐 512 块** `accept_waveform`（[`feed_in_vad_windows`]）→ `flush()`
    /// → 逐个 `front()/pop()` 收集 `(start, start+n)` → `clear() + reset()`（游标归零，供下次复用）。
    ///
    /// 🔴 FIX-VAD-FEED-BY-WINDOW-391：388 原实现「整段一次性 `accept_waveform`」**错误**
    ///（依据见 [`feed_in_vad_windows`]：一次调用只得一个结论、起点定在输入末尾前 ~0.164s）
    /// ⇒ 5s 语音只留 ~0.37s。现改**逐 512 块**喂入。
    pub fn speech_ranges(&self, audio: &[f32]) -> Vec<(usize, usize)> {
        if audio.is_empty() {
            return Vec::new();
        }
        self.detector.reset();
        // FIX-391：逐 512 块喂入（整段一次会命中 sherpa 的「大 n」语义陷阱）。
        let _ = feed_in_vad_windows(audio, |block| self.detector.accept_waveform(block));
        self.detector.flush();
        let ranges: Vec<(usize, usize)> = std::iter::from_fn(|| {
            self.detector.front().map(|seg| {
                let start = seg.start() as usize;
                let n = seg.n() as usize;
                self.detector.pop();
                (start, start + n)
            })
        })
        .collect();
        // 与 `segment()` 同款：clear 清段队列 + reset 归零全局样本游标，防下次 start 变绝对坐标。
        self.detector.clear();
        self.detector.reset();
        ranges
    }

    /// LOCALRT-VAD-SILENCE-384：本地 realtime 静默判定专用工厂（silero，阈值 0.5）。
    ///
    /// 模型缺失/失败返回 `None` → 调用方退回**音量阈值**兜底。**只新增**：
    /// 不改 `try_new`（离线分段）/ `try_new_for_streaming`（在线门控）的行为。
    pub fn try_new_for_local_silence(model_dir: &Path) -> Option<Self> {
        let vad_model = find_silero_vad_model(model_dir)?;
        let config = VadModelConfig {
            silero_vad: sherpa_onnx::SileroVadModelConfig {
                model: vad_model.to_str().map(|s| s.to_string()),
                threshold: LOCALRT_VAD_THRESHOLD,
                min_silence_duration: LOCALRT_VAD_MIN_SILENCE_SECS,
                min_speech_duration: VAD_MIN_SPEECH_DURATION,
                window_size: VAD_WINDOW_SIZE,
                max_speech_duration: LOCALRT_VAD_MAX_SPEECH_SECS,
            },
            ten_vad: sherpa_onnx::TenVadModelConfig::default(),
            sample_rate: 16000,
            num_threads: 1,
            provider: Some("cpu".to_string()),
            debug: false,
        };
        let detector = VoiceActivityDetector::create(&config, LOCALRT_VAD_BUFFER_SECS)?;
        log::info!(
            "VAD local-silence initialized (silero, threshold={}, min_silence={}s, buffer={}s, model={})",
            LOCALRT_VAD_THRESHOLD,
            LOCALRT_VAD_MIN_SILENCE_SECS,
            LOCALRT_VAD_BUFFER_SECS,
            vad_model.display()
        );
        Some(Self { detector })
    }

    /// LOCALRT-VAD-SILENCE-384 / FIX-VAD-FEED-BY-WINDOW-391：本地 realtime「本 chunk 是否有人声」。
    ///
    /// 🔴 **逐 512 块喂入**（[`feed_in_vad_windows`]），**每块后不查询**；整块喂完后只调一次
    /// `detected()` 返回。现网录音块约 10ms（<512）故与旧行为一致，本单**消除对块大小的依赖**
    ///（>512 的大块若一次性喂入会命中 [`feed_in_vad_windows`] 描述的 sherpa 语义陷阱）。
    ///
    /// - 每 chunk 把**已完成**的语音段 `pop` 掉（只清段队列、**不 reset** 检测状态），
    ///   使长录音（≤300s）内部段队列不增长（内存有界）。
    /// 供本地 realtime 静音计时使用；与在线门控的 `accept_and_check` 平行、互不影响。
    ///
    /// 🔴 VAD-393（A1）：生产本地实时改走 [`Self::feed_speech`]（逐块喂 + 收集时间线）；本方法保留为
    /// **薄包装**（391 契约的「逐块喂」入口，供单测与跨端；生产已无调用点 ⇒ 允许 dead_code）。
    #[allow(dead_code)]
    pub fn feed_is_speech(&self, samples: &[f32]) -> bool {
        // VAD-393（A1）：改为薄包装（仍逐 512 块喂入、行为不变），段队列照常清空。
        let mut ignored: Vec<(usize, usize)> = Vec::new();
        self.feed_speech(samples, &mut ignored)
    }

    /// VAD-393（A1）：逐 512 块喂入本 chunk，**同时把已完成的语音段收集到 `done`**，返回 `detected()`。
    ///
    /// 🔴 `(start, n)` 是 sherpa **自本实例 reset 起**的绝对样本坐标；`local_stream` 用**同一实例**、
    /// 按**同一顺序喂入同样的 chunk**（见 `feed_in_vad_windows`），且实例在每次录音开始 `reset_for_new_session`
    /// ⇒ `start` 即该录音 `pcm` 的样本下标（二者同序同量）。`(start, start+n)` 为**开区间**端点。
    pub fn feed_speech(&self, samples: &[f32], done: &mut Vec<(usize, usize)>) -> bool {
        // FIX-391：逐 512 块喂入（不再一次性喂整块）。
        let _ = feed_in_vad_windows(samples, |block| self.detector.accept_waveform(block));
        let detected = self.detector.detected();
        // 收集已完成段（只清段队列、不 reset 检测状态 ⇒ 内存有界）。
        while let Some(seg) = self.detector.front() {
            let start = seg.start() as usize;
            let n = seg.n() as usize;
            self.detector.pop();
            done.push((start, start + n));
        }
        detected
    }

    /// VAD-393（A1）：录音收尾 —— `flush()` 后把剩余段收集到 `done`（尾句可能未达 min_silence）。
    /// 调用方：`local_stream` 在**尾片派发之前**调用（否则尾句语音不在时间线里）。
    pub fn flush_speech(&self, done: &mut Vec<(usize, usize)>) {
        self.detector.flush();
        while let Some(seg) = self.detector.front() {
            let start = seg.start() as usize;
            let n = seg.n() as usize;
            self.detector.pop();
            done.push((start, start + n));
        }
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
    build_padded_segments_capped(raw, total_samples, full_audio, SEGMENT_MAX_SECS)
}

/// FIX-REMOVE-HARDSPLIT-370：`max_seg_secs` **显式声明**单段长度上限（DEC-066，不靠输入长度反推）。
///
/// 其它路径（VAD 分段 / 本地离线 accuracy）传 `SEGMENT_MAX_SECS`(20s) —— native
/// `max_total_len=512` 时代遗留，逐位不变。
/// 🔴 滑窗派发路径**不再**走本函数（FIX-SLICE-CUT-AT-GAP-381 起改走 [`build_sliding_segments`]）。
///
/// 无论上限多少，都保留 FIX-VAD-STATE-RESET-001 的边界过滤/clamp 与 200ms 边界 padding。
pub fn build_padded_segments_capped(
    raw: &[(usize, usize)],
    total_samples: usize,
    full_audio: &[f32],
    max_seg_secs: f64,
) -> Vec<Vec<f32>> {
    let merged = plan_hard_cuts(raw, total_samples, max_seg_secs);
    pad_and_extract(&merged, total_samples, full_audio)
}

/// FIX-SLICE-CUT-AT-GAP-381：滑窗（accuracy 派发）路径专用片构建 —— 超限片按**字缝**切。
///
/// 与 [`build_padded_segments_capped`] 的差异**仅在「超限片怎么切」**：
/// - 前者：固定秒数**硬切**（切点可能落在字中间，把字切碎 ⇒ 识别出错）；
/// - 本函数：超限片交给 [`plan_gap_cuts`] 从 10s 起找字缝切（找不到取 RMS 最低帧兜底）。
///
/// 200ms 边界 padding 与 FIX-VAD-STATE-RESET-001 边界过滤/clamp **完全沿用**
/// （`plan_sliding_cuts` 与 `plan_hard_cuts` 共用同一段过滤代码）。
///
/// 🔴 VAD-393（A1）：带 spans 的姐妹函数 [`build_sliding_segments_with_spans`] 是新滑窗入口；本函数
/// 保留供 20s 路径 / PoC / 单测（生产非测试构建已无调用点 ⇒ 允许 dead_code）。
#[allow(dead_code)]
pub fn build_sliding_segments(
    raw: &[(usize, usize)],
    total_samples: usize,
    full_audio: &[f32],
) -> Vec<Vec<f32>> {
    let merged = plan_sliding_cuts(raw, total_samples, full_audio);
    pad_and_extract(&merged, total_samples, full_audio)
}

/// VAD-393（A1）：同 [`build_sliding_segments`]，**额外**返回每片在 `pcm` 中的区间与前置零 padding 数。
///
/// `spans[k] = (pcm_start_k, pcm_end_k, pad_before_k)`：第 k 片的**语音区间** `[pcm_start_k, pcm_end_k)`
///（`pcm` 坐标）与其片内**前置零样本数**（200ms padding 的一部分）。语音区间在片内起点 = `pad_before_k`。
/// 只供滑窗路径（把实时 VAD 时间线映射为片内坐标）；`build_sliding_segments` / 20s 路径**逐位不变**。
pub fn build_sliding_segments_with_spans(
    raw: &[(usize, usize)],
    total_samples: usize,
    full_audio: &[f32],
) -> (Vec<Vec<f32>>, Vec<(usize, usize, usize)>) {
    let merged = plan_sliding_cuts(raw, total_samples, full_audio);
    pad_and_extract_with_spans(&merged, total_samples, full_audio)
}

/// FIX-VAD-STATE-RESET-001 边界过滤 + 固定秒数硬切/合并相邻短段（原 `build_padded_segments_capped` 第一步）。
fn plan_hard_cuts(
    raw: &[(usize, usize)],
    total_samples: usize,
    max_seg_secs: f64,
) -> Vec<(usize, usize)> {
    if raw.is_empty() {
        return Vec::new();
    }
    let max_seg_samples = (max_seg_secs * 16000.0) as usize;
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
    merged
}

/// FIX-SLICE-CUT-AT-GAP-381：滑窗路径的切片规划 —— 与 [`plan_hard_cuts`] 同构，
/// 唯一差异是「超限片」改走 [`plan_gap_cuts`]（字缝切）；边界过滤/clamp/相邻短段合并**逐位不变**。
///
/// 合并上限取 [`SLIDING_CUT_SEARCH_START_SECS`](10s)：合并后 ≤10s 的相邻段直接并成一片（不切）。
fn plan_sliding_cuts(
    raw: &[(usize, usize)],
    total_samples: usize,
    full_audio: &[f32],
) -> Vec<(usize, usize)> {
    if raw.is_empty() {
        return Vec::new();
    }
    let max_seg_samples = (SLIDING_CUT_SEARCH_START_SECS * 16000.0) as usize;
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
            // 超限片：按字缝切（切点严格相接、不重不漏）
            merged.extend(plan_gap_cuts(full_audio, start, end));
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
    merged
}

/// FIX-SLICE-CUT-AT-GAP-381：**纯函数** —— 把 `[start, end)` 按「字缝」切成严格相接的若干片。
///
/// 规则（Gavin 2026-09-23 口径，取值依据见各常量注释）：
/// 1. 剩余长度 < 起搜点(10s) + [`MIN_TAIL_SECS`](1s) ⇒ **不切**，整段作一片（尾巴保护，最长约 11s）；
/// 2. 否则从「本片起点 + 10s」往后搜，最远 `GAP_SEARCH_MAX_SECS`(2s)（即到 12s）：
///    以 [`GAP_FRAME_SAMPLES`](20ms) 为一帧算 RMS，**字缝** = 该帧 RMS ≤
///    [`GAP_RMS_RATIO`] × 本片前 10s 帧 RMS 中位数，且为局部极小（≤ 左右相邻帧）；
///    取**最早**达标帧的**中心样本**为切点；
/// 3. 搜不到达标帧 ⇒ 在 [10s, 12s] 内取 RMS **最低**帧的中心切（硬兜底，绝不无限变长）；
/// 4. 切点严格相接（第 i 片 end == 第 i+1 片 start），不重不漏。
///
/// 用**相对阈值** ⇒ 整体增益变化不改变切点位置（单测 `gap_cut_scale_invariant`）。
pub fn plan_gap_cuts(audio: &[f32], start: usize, end: usize) -> Vec<(usize, usize)> {
    const RATE: usize = 16000;
    let frame = GAP_FRAME_SAMPLES;
    let search_start = (SLIDING_CUT_SEARCH_START_SECS * RATE as f64) as usize;
    let search_max = (GAP_SEARCH_MAX_SECS * RATE as f64) as usize;
    let min_tail = (MIN_TAIL_SECS * RATE as f64) as usize;

    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut pos = start;
    while pos < end {
        let remaining = end - pos;
        // 尾巴保护：剩余不足 10s + 1s ⇒ 不切，整段作一片
        if remaining < search_start + min_tail {
            out.push((pos, end));
            break;
        }
        let lower = pos + search_start;
        // 兜底上限：最晚 12s，且至少给尾巴留 min_tail
        let upper = (pos + search_start + search_max).min(end - min_tail);
        let cut = find_gap_cut(audio, pos, lower, upper, frame);
        out.push((pos, cut));
        pos = cut;
    }
    out
}

/// FIX-SLICE-CUT-AT-GAP-381：在 `[lower, upper]`（限定帧**中心**落点）内找一个字缝切点。
///
/// - 基线中位数取自 `[piece_start, piece_start + 10s)` 的完整帧（相对阈值）；
/// - 优先返回**最早**的达标帧（RMS ≤ ratio×中位数 且 ≤ 左右相邻帧）中心；
/// - 无达标帧 ⇒ 返回 RMS **最低**帧的中心（并列取最早）。
///
/// LOCALRT-TAIL-WINDOW-407：提为 `pub(crate)` —— 尾部组窗要在前一片「末尾回溯 ≥2s」区间里找字缝
/// （`local_stream::find_tail_cut` 复用本函数，**同一套字缝判据只保留这一处**，不另写一份）。
pub(crate) fn find_gap_cut(
    audio: &[f32],
    piece_start: usize,
    lower: usize,
    upper: usize,
    frame: usize,
) -> usize {
    match find_gap_cut_impl(audio, piece_start, lower, upper, frame) {
        // 取最早达标字缝帧
        (Some(gap), _) => gap,
        // 无达标帧 ⇒ 取区间内最低 RMS 帧（381 原语义）
        (None, Some(lowest)) => lowest,
        // [lower,upper] 不足一个完整帧 ⇒ 保守用 lower，保证严格推进
        (None, None) => lower,
    }
}

/// LOCALRT-TAIL-WINDOW-407：与 [`find_gap_cut`] **同一套字缝判据**，但**只返回真字缝**
/// （`None` = 区间内无达标帧）。供尾部组窗用「找不到字缝 ⇒ 回落指定下界（回溯 2s）」的语义
/// （与 `find_gap_cut` 的「无字缝取最低 RMS 帧」不同）。判据只此一处，不另写一份。
pub(crate) fn find_gap_cut_gap_only(
    audio: &[f32],
    piece_start: usize,
    lower: usize,
    upper: usize,
    frame: usize,
) -> Option<usize> {
    find_gap_cut_impl(audio, piece_start, lower, upper, frame).0
}

/// 381/407 共用的字缝搜索内核：返回 `(最早达标字缝帧中心, 区间内最低 RMS 帧中心)`。
fn find_gap_cut_impl(
    audio: &[f32],
    piece_start: usize,
    lower: usize,
    upper: usize,
    frame: usize,
) -> (Option<usize>, Option<usize>) {
    let total = audio.len();
    let search_start = (SLIDING_CUT_SEARCH_START_SECS * 16000.0) as usize;

    // 基线：本片前 10s 所有完整帧的 RMS 中位数
    let median_src_end = (piece_start + search_start).min(total);
    let mut rms_vals: Vec<f32> = Vec::new();
    let mut fs = piece_start;
    while fs + frame <= median_src_end {
        rms_vals.push(frame_rms(audio, fs, frame, total));
        fs += frame;
    }
    rms_vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if rms_vals.is_empty() {
        0.0
    } else {
        rms_vals[rms_vals.len() / 2]
    };
    let threshold = GAP_RMS_RATIO * median;

    // 候选帧：中心 center = piece_start + j*frame + half ∈ [lower, upper]
    let half = frame / 2;
    let first_j = {
        let need = lower.saturating_sub(piece_start + half);
        (need + frame - 1) / frame // ceil
    };
    let last_j = upper.saturating_sub(piece_start + half) / frame;

    let mut gap_center: Option<usize> = None;
    let mut lowest_center: Option<usize> = None;
    let mut lowest_rms = f32::INFINITY;
    let mut j = first_j;
    while j <= last_j {
        let fstart = piece_start + j * frame;
        if fstart + frame > total {
            break;
        }
        let rms = frame_rms(audio, fstart, frame, total);
        // 局部极小（边界帧只比存在的一侧；邻居帧超界时 frame_rms 返回 0 ⇒ 不误判）
        let prev = if j == 0 {
            f32::INFINITY
        } else {
            frame_rms(audio, piece_start + (j - 1) * frame, frame, total)
        };
        let next = frame_rms(audio, piece_start + (j + 1) * frame, frame, total);
        let local_min = rms <= prev && rms <= next;
        if rms <= threshold && local_min {
            gap_center = Some(piece_start + j * frame + half);
            break; // 取最早达标帧
        }
        if rms < lowest_rms {
            lowest_rms = rms;
            lowest_center = Some(piece_start + j * frame + half);
        }
        j += 1;
    }

    (gap_center, lowest_center)
}

/// 单帧 RMS（越界按可用长度算；空帧返回 0）。
fn frame_rms(audio: &[f32], frame_start: usize, frame: usize, total: usize) -> f32 {
    let begin = frame_start.min(total);
    let end = (frame_start + frame).min(total);
    if begin >= end {
        return 0.0;
    }
    let s = &audio[begin..end];
    (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt()
}

/// 原 `build_padded_segments_capped` 第二步，逐位不变：按 (start,end) 列表加 padding 并提取样本。
fn pad_and_extract(
    merged: &[(usize, usize)],
    total_samples: usize,
    full_audio: &[f32],
) -> Vec<Vec<f32>> {
    // VAD-393（A1）：单一实现 —— `pad_and_extract` 只是丢弃 spans（输出与旧实现逐位相同）。
    pad_and_extract_with_spans(merged, total_samples, full_audio).0
}

/// VAD-393（A1）：[`pad_and_extract`] 的**全量**版 —— 额外返回每片 `(pcm_start, pcm_end, pad_before)`。
/// 见 [`build_sliding_segments_with_spans`]。旧 `pad_and_extract` 委托本函数 ⇒ 20s 路径逐位不变。
fn pad_and_extract_with_spans(
    merged: &[(usize, usize)],
    total_samples: usize,
    full_audio: &[f32],
) -> (Vec<Vec<f32>>, Vec<(usize, usize, usize)>) {
    if merged.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let pad = SEGMENT_PADDING_SAMPLES;

    let mut result: Vec<Vec<f32>> = Vec::with_capacity(merged.len());
    let mut spans: Vec<(usize, usize, usize)> = Vec::with_capacity(merged.len());
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
        // VAD-393（A1）：语音区间（clamp 到 total）+ 片内前置零样本数。
        // 393-R4：用 `saturating_sub`（`pad_start ≥ start` 时原式在 debug 下 panic；该函数现被
        // 20s 路径共用）。片内语音起点 = 前置零个数，二者一致。
        spans.push((
            start,
            end.min(total_samples),
            start.saturating_sub(pad_start),
        ));
    }
    (result, spans)
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

    /// 393：读真人声 `full.wav`（供夹具用；缺失 ⇒ `None` ⇒ 用例跳过）。
    fn real_speech_samples() -> Option<Vec<f32>> {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        sherpa_onnx::Wave::read(wav.to_str()?).map(|w| w.samples().to_vec())
    }

    /// 393：总长 `total`，`windows`（样本区间）内用 `speech[src_off..]` 依序填「语音」，其余填 0。
    fn speech_with_silence(
        total: usize,
        windows: &[(usize, usize)],
        speech: &[f32],
        src_off: usize,
    ) -> Vec<f32> {
        let mut v = vec![0.0f32; total];
        let mut off = src_off;
        for &(s, e) in windows {
            let len = e - s;
            let end = (off + len).min(speech.len());
            let avail = end.saturating_sub(off);
            v[s..s + avail].copy_from_slice(&speech[off..end]);
            off += len;
        }
        v
    }

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

        // 🔴 393：v6.2 不再把纯正弦判为语音 ⇒ 夹具由「440Hz 正弦模拟语音」改用 full.wav
        // **真人声**片段（断言语义与所有 VAD 参数不动）。
        let Some(speech) = real_speech_samples() else {
            eprintln!("skip: full.wav not found");
            return;
        };
        // 第一段 30s：语音 [5,10]s 与 [25,30]s（素材自 full.wav 1s 起）
        let audio1 = speech_with_silence(
            30 * 16000,
            &[(5 * 16000, 10 * 16000), (25 * 16000, 30 * 16000)],
            &speech,
            16000,
        );
        // 第二段 40s（比第一段长，验证游标归零后起点在界内）：语音 [5,15]s 与 [25,35]s
        let audio2 = speech_with_silence(
            40 * 16000,
            &[(5 * 16000, 15 * 16000), (25 * 16000, 35 * 16000)],
            &speech,
            10 * 16000,
        );

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

    // =========================================================================
    // ASR-038-B: 流式入口门控滚动 VAD 测试
    // =========================================================================

    /// try_new_for_streaming：模型缺失返回 None（降级为总是建连）
    #[test]
    fn vad_streaming_try_new_missing_model_returns_none() {
        let model_dir = std::path::Path::new("nonexistent-vad-model-dir");
        assert!(
            VadSegmenter::try_new_for_streaming(model_dir).is_none(),
            "missing VAD model should return None for streaming gate"
        );
    }

    /// try_new_for_streaming：模型存在时返回 Some（与 try_new 一致行为）
    /// 需要 silero_vad.onnx + 工作的 ORT runtime（vendor ORT 1.17.1 可能不支持 API v24）
    #[test]
    #[ignore = "requires working ORT runtime (vendor ORT 1.17.1 may not support API v24)"]
    fn vad_streaming_try_new_with_real_model() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = project_root.join("models");
        let segmenter = match VadSegmenter::try_new_for_streaming(&model_dir) {
            Some(s) => s,
            None => {
                eprintln!(
                    "skip: silero_vad.onnx not found at {}, VAD model required",
                    model_dir.display()
                );
                return;
            }
        };
        // 能创建即通过（配置内部用 0.3 阈值，外部不可见）
        let _ = segmenter;
    }

    /// accept_and_check：静音样本不检测到语音
    /// 喂 512 样本全零（静音），detected() 应为 false
    #[test]
    #[ignore = "requires working ORT runtime + silero_vad.onnx"]
    fn vad_streaming_accept_and_check_silence_no_speech() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = project_root.join("models");
        let segmenter = match VadSegmenter::try_new_for_streaming(&model_dir) {
            Some(s) => s,
            None => {
                eprintln!("skip: silero_vad.onnx not found");
                return;
            }
        };
        // 喂多个静音窗口（VAD 需要积累一定上下文才稳定）
        let silence = vec![0.0f32; VAD_WINDOW_SIZE as usize];
        let mut detected = false;
        for _ in 0..10 {
            if segmenter.accept_and_check(&silence) {
                detected = true;
            }
        }
        assert!(
            !detected,
            "pure silence should not trigger speech detection"
        );
    }

    /// accept_and_check：语音样本（sine 模拟）应检测到语音
    #[test]
    #[ignore = "requires working ORT runtime + silero_vad.onnx"]
    fn vad_streaming_accept_and_check_speech_detected() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = project_root.join("models");
        let segmenter = match VadSegmenter::try_new_for_streaming(&model_dir) {
            Some(s) => s,
            None => {
                eprintln!("skip: silero_vad.onnx not found");
                return;
            }
        };
        // 🔴 393：v6.2 不再把纯正弦判为语音 ⇒ 夹具改用 full.wav 真人声（1s，自 2s 起）。
        let Some(speech) = real_speech_samples() else {
            eprintln!("skip: full.wav not found");
            return;
        };
        let win = VAD_WINDOW_SIZE as usize;
        let src_off = 2 * 16000usize;
        let mut detected = false;
        for i in 0..31 {
            let start = i * win;
            let chunk: Vec<f32> = (0..win)
                .map(|j| speech.get(src_off + start + j).copied().unwrap_or(0.0))
                .collect();
            if segmenter.accept_and_check(&chunk) {
                detected = true;
            }
        }
        assert!(
            detected,
            "sustained real-speech energy should trigger speech detection"
        );
    }

    /// 393：固化 v6.2 特性 —— **纯 440Hz 正弦不判为语音**（v4 会）⇒ 夹具须用真人声。
    #[test]
    #[ignore = "requires working ORT runtime + silero_vad.onnx"]
    fn v6_pure_sine_not_detected_as_speech() {
        let model_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
        let segmenter = match VadSegmenter::try_new_for_streaming(&model_dir) {
            Some(s) => s,
            None => {
                eprintln!("skip: silero_vad.onnx not found");
                return;
            }
        };
        let win = VAD_WINDOW_SIZE as usize;
        let mut detected = false;
        for i in 0..31 {
            let start = i * win;
            let chunk: Vec<f32> = (0..win)
                .map(|j| {
                    let t = (start + j) as f32 / 16000.0;
                    (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3
                })
                .collect();
            if segmenter.accept_and_check(&chunk) {
                detected = true;
            }
        }
        assert!(
            !detected,
            "393：v6.2 纯 440Hz 正弦不得被判为语音（夹具须用真人声）"
        );
    }

    /// reset_for_new_session：调后不 panic，可继续用
    #[test]
    #[ignore = "requires working ORT runtime + silero_vad.onnx"]
    fn vad_streaming_reset_for_new_session_no_panic() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = project_root.join("models");
        let segmenter = match VadSegmenter::try_new_for_streaming(&model_dir) {
            Some(s) => s,
            None => {
                eprintln!("skip: silero_vad.onnx not found");
                return;
            }
        };
        // 喂一些样本后 reset
        let silence = vec![0.0f32; VAD_WINDOW_SIZE as usize];
        let _ = segmenter.accept_and_check(&silence);
        segmenter.reset_for_new_session();
        // reset 后再喂不应 panic
        let _ = segmenter.accept_and_check(&silence);
    }

    /// 入口门控阈值 0.3 低于分段阈值 0.5（验证常量隔离）
    #[test]
    fn vad_streaming_threshold_lower_than_segment() {
        assert!(
            VAD_STREAMING_THRESHOLD < VAD_THRESHOLD,
            "streaming gate threshold ({}) must be lower than segment threshold ({}) for first-syllable safety",
            VAD_STREAMING_THRESHOLD,
            VAD_THRESHOLD
        );
        assert_eq!(VAD_STREAMING_THRESHOLD, 0.3);
        assert_eq!(VAD_THRESHOLD, 0.5);
    }

    /// C-1: vad_window_size() 必须导出 512（32ms @16kHz），供 qwen_inference 逐窗口切片
    #[test]
    fn vad_window_size_is_512() {
        assert_eq!(vad_window_size(), 512);
        assert_eq!(VAD_WINDOW_SIZE, 512);
    }

    /// C-1 覆盖点1：try_new_for_streaming 模型缺失返回 None（降级总是建连）
    /// 与 try_new 行为保持一致 —— 两条工厂都必须走 find_silero_vad_model 兜底
    #[test]
    fn vad_streaming_and_batch_factories_agree_on_missing_model() {
        let dir = std::path::Path::new("nonexistent-vad-model-dir-2");
        assert_eq!(
            VadSegmenter::try_new_for_streaming(dir).is_some(),
            VadSegmenter::try_new(dir).is_some(),
            "both factories must degrade identically when model missing"
        );
        assert!(VadSegmenter::try_new_for_streaming(dir).is_none());
        assert!(VadSegmenter::try_new(dir).is_none());
    }

    // ========================================================================
    // FIX-REMOVE-HARDSPLIT-370：`build_padded_segments_capped` 上限参数化
    //   验证：① 上限可显式传入并生效 ② 20s 路径（`build_padded_segments` / capped）逐位不变
    //        ③ FIX-VAD-STATE-RESET-001 边界护栏保留
    // ========================================================================
    // FIX-SLICE-CUT-AT-GAP-381：滑窗路径改「字缝切」，见下方 gap_cut_* / sliding_* 用例
    //   （`SLIDING_SLICE_MAX_SECS` 已删除，滑窗不再按固定秒数硬切）。
    // ========================================================================

    /// 上限参数**显式生效**：5s 单片配 cap=2s ⇒ 按 2s 切成 3 段（2/2/1s）。
    #[test]
    fn capped_honors_explicit_cap() {
        let sec = 16_000usize;
        let total = 5 * sec;
        let audio = vec![0.1f32; total];
        let segs = build_padded_segments_capped(&[(0, total)], total, &audio, 2.0);
        assert_eq!(segs.len(), 3, "5s / cap 2s ⇒ 3 段");
        assert_eq!(
            segs.iter().map(|s| s.len()).collect::<Vec<_>>(),
            vec![2 * sec, 2 * sec, 1 * sec],
            "每段 ≤ 2s（无 padding 余量：段间/末尾都被夹在边界内）"
        );
    }

    /// 🔴 其它路径**逐位不变**证明：`build_padded_segments` ≡ `capped(.., SEGMENT_MAX_SECS)`。
    ///
    /// 覆盖：空输入 / 单短段 / 超限单段 / 多段可合并 / 多段不可合并 / 越界段 / 越界 end。
    /// 🔴 FIX-SLICE-CUT-AT-GAP-381 单测 #5「20s 路径不变」由本用例 + 既有 `build_padded_long_hard_cut`
    /// 共同承担（既有断言未改）。
    #[test]
    fn legacy_wrapper_is_bit_identical_to_capped_segment_max() {
        let sec = 16_000usize;
        let total = 60 * sec;
        let audio: Vec<f32> = (0..total).map(|i| (i % 97) as f32 / 97.0).collect();
        let fixtures: Vec<Vec<(usize, usize)>> = vec![
            vec![],
            vec![(sec, 2 * sec)],
            vec![(0, 25 * sec)],                          // 超 20s ⇒ 旧路径硬切
            vec![(0, 5 * sec), (5 * sec + 100, 5 * sec)], // 相邻可合并
            vec![(0, 15 * sec), (16 * sec, 15 * sec)],    // 合并后 31s > 20s ⇒ 不合并
            vec![(total, 1000)],                          // start 越界 ⇒ 丢弃
            vec![(total - 1000, 5000)],                   // end 越界 ⇒ clamp
        ];
        for raw in fixtures {
            let a = build_padded_segments(&raw, total, &audio);
            let b = build_padded_segments_capped(&raw, total, &audio, SEGMENT_MAX_SECS);
            assert_eq!(
                a, b,
                "旧路径必须与 capped(SEGMENT_MAX_SECS) 逐位相同：raw={raw:?}"
            );
        }
    }

    // ========================================================================
    // FIX-SLICE-CUT-AT-GAP-381：滑窗路径「字缝切」纯函数 `plan_gap_cuts`
    //   口径：窗口上限与切片起搜点统一 10s；超 10s 从 10s 起找字缝（最晚 12s）
    // ========================================================================

    /// 合成音频：`period_ms` 周期内 `speech_ms` 正弦 + 其余近静音（模拟「字缝」）。
    fn synth_speech_with_gaps(total_secs: usize, speech_amp: f32) -> Vec<f32> {
        let rate = 16_000usize;
        let total = total_secs * rate;
        let period = 3840usize; // 240ms = 200ms 语音 + 40ms 近静音
        let speech = 3200usize; // 200ms 语音（@16kHz）
        let mut v = Vec::with_capacity(total);
        for i in 0..total {
            let ph = i % period;
            if ph < speech {
                let t = i as f32 / rate as f32;
                v.push((2.0 * std::f32::consts::PI * 220.0 * t).sin() * speech_amp);
            } else {
                v.push(0.001);
            }
        }
        v
    }

    fn max_abs(audio: &[f32], lo: usize, hi: usize) -> f32 {
        audio[lo.min(audio.len())..hi.min(audio.len())]
            .iter()
            .fold(0.0f32, |m, x| m.max(x.abs()))
    }

    /// 单测 #1：**切点落在低谷** —— 每个切点落在近静音段内，且相邻切点相距 ≥10s。
    #[test]
    fn gap_cut_falls_in_low_energy_gap() {
        let audio = synth_speech_with_gaps(20, 0.3);
        let total = audio.len();
        let cuts = plan_gap_cuts(&audio, 0, total);
        assert!(cuts.len() >= 2, "20s 有字缝应切成 ≥2 片：{cuts:?}");
        // 连续相接
        assert_eq!(cuts[0].0, 0);
        assert_eq!(cuts.last().unwrap().1, total);
        for w in cuts.windows(2) {
            assert_eq!(w[0].1, w[1].0, "切点必须严格相接：{cuts:?}");
        }
        // 每个内部切点在近静音内（±10ms 最大幅度远低于语音）
        for c in cuts.iter().skip(1) {
            let cut = c.0;
            let m = max_abs(&audio, cut.saturating_sub(160), cut + 160);
            assert!(
                m < 0.05,
                "切点 {cut} 落在有声区（±10ms max_abs={m}）：{cuts:?}"
            );
        }
        // 相邻切点相距 ≥10s（10s 起搜，不会更早）
        for w in cuts.windows(2) {
            let gap = w[1].0 - w[0].0;
            assert!(gap >= 10 * 16_000, "相邻切点相距 {} samples < 10s", gap);
        }
    }

    /// 单测 #2：**兜底** —— 20s 不间断正弦（无低谷）⇒ 切点在 [10s, 12s] 内，片数正确。
    #[test]
    fn gap_cut_fallback_when_no_gap() {
        let rate = 16_000usize;
        let total = 20 * rate;
        let audio: Vec<f32> = (0..total)
            .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / rate as f32).sin() * 0.3)
            .collect();
        let cuts = plan_gap_cuts(&audio, 0, total);
        assert_eq!(cuts.len(), 2, "20s 无低谷 ⇒ 恰切一次（2 片）：{cuts:?}");
        let cut = cuts[0].1;
        assert!(
            cut >= 10 * rate && cut <= 12 * rate,
            "兜底切点必在 [10s,12s]：{}s",
            cut as f64 / rate as f64
        );
        assert_eq!(cuts[1], (cut, total));
    }

    /// 单测 #3：**尾巴保护** —— 10.5s ⇒ 1 片；11.5s ⇒ 2 片且尾片 ≥1s。
    #[test]
    fn gap_cut_tail_protection() {
        let rate = 16_000usize;
        let sine = |n: usize| -> Vec<f32> {
            (0..n)
                .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / rate as f32).sin() * 0.3)
                .collect()
        };
        let a = sine(10 * rate + rate / 2); // 10.5s
        let cuts = plan_gap_cuts(&a, 0, a.len());
        assert_eq!(cuts.len(), 1, "10.5s 尾片 <1s ⇒ 不切（1 片）：{cuts:?}");

        let b = sine(11 * rate + rate / 2); // 11.5s
        let cuts = plan_gap_cuts(&b, 0, b.len());
        assert_eq!(cuts.len(), 2, "11.5s ⇒ 切 2 片：{cuts:?}");
        let tail = cuts[1].1 - cuts[1].0;
        assert!(tail >= rate, "尾片 {}s < 1s", tail as f64 / rate as f64);
    }

    /// 单测 #4：**不丢不重** —— 各片按序拼接（原音频全区间 ⇒ 无外层 padding）与原音频逐样本相等。
    #[test]
    fn gap_cut_no_loss_no_overlap() {
        let audio = synth_speech_with_gaps(25, 0.25);
        let total = audio.len();
        let cuts = plan_gap_cuts(&audio, 0, total);
        let mut pos = 0usize;
        let mut rebuilt: Vec<f32> = Vec::with_capacity(total);
        for &(s, e) in &cuts {
            assert_eq!(s, pos, "必须严格相接、不留缝：{cuts:?}");
            assert!(e > s, "空片：{cuts:?}");
            rebuilt.extend_from_slice(&audio[s..e]);
            pos = e;
        }
        assert_eq!(pos, total, "末尾必须覆盖到 total");
        assert_eq!(rebuilt, audio, "拼接必须与原音频逐样本相等");
    }

    /// 单测 #6：**相对阈值** —— 整体乘 0.1 增益，切点位置不变。
    #[test]
    fn gap_cut_scale_invariant() {
        let audio = synth_speech_with_gaps(20, 0.3);
        let total = audio.len();
        let a = plan_gap_cuts(&audio, 0, total);
        let scaled: Vec<f32> = audio.iter().map(|x| x * 0.1).collect();
        let b = plan_gap_cuts(&scaled, 0, total);
        assert_eq!(a, b, "相对阈值下整体增益不应改变切点");
    }

    /// 🔴 FIX-VAD-STATE-RESET-001 护栏在**滑窗路径**下同样保留：
    /// start 越界 ⇒ 丢弃（不 panic、返回空）；end 越界 ⇒ clamp（不 panic）。
    #[test]
    fn sliding_segments_keeps_bounds_guards() {
        let sec = 16_000usize;
        let total = 30 * sec;
        let audio = vec![0.2f32; total];
        assert!(
            build_sliding_segments(&[(total, 1000)], total, &audio).is_empty(),
            "start >= total_samples ⇒ 丢弃"
        );
        let segs = build_sliding_segments(&[(total - 1000, 5000)], total, &audio);
        assert_eq!(segs.len(), 1);
        assert_eq!(
            segs[0].len(),
            1000 + SEGMENT_PADDING_SAMPLES,
            "end 超界 clamp 到 total，再补前向 200ms padding"
        );
    }

    // ========================================================================
    // TEST-SYNC-381（阶段三 · 非作者视角）：按**设计契约**给 `FIX-SLICE-CUT-AT-GAP-381`
    //   补独立护栏。作者用例覆盖「有字缝 / 无字缝兜底 / 尾巴保护 / 不丢不重 / 相对阈值 /
    //   边界 clamp」；本组补作者心智模型照不到的：性质测试、退化输入、最早字缝优先、
    //   非帧对齐 start、空/倒置区间、滑窗 10s 合并阈值。
    //   🔴 任务书第 4 条「20s 路径逐位不变快照」经主控 2026-09-23 **恢复并强化**：本批只动
    //   本地 realtime 管线，离线 accuracy 的 20s 路径与 `naive_chunk` **不得变** ⇒ 对二者用
    //   固定输入做**逐位快照**（对 `1af7212^` 即 381 之前），见
    //   `ts381_padded_20s_snapshot_bit_identical_to_pre_381` /
    //   `ts381_naive_chunk_snapshot_bit_identical_to_pre_381`。
    // ========================================================================

    /// 契约常量（避免测试里写死 10/12 数字与实现漂移）。
    const TS381_RATE: usize = 16_000;
    const TS381_FRAME: usize = GAP_FRAME_SAMPLES; // 320（20ms）

    /// 确定性伪随机（xorshift64），让性质测试**可复现**、不引外部 crate。
    struct Ts381Rng(u64);
    impl Ts381Rng {
        fn new(seed: u64) -> Self {
            Self(seed | 1)
        }
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        /// [0, 1)
        fn unit(&mut self) -> f32 {
            (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
        }
        /// [lo, hi] 闭区间
        fn range(&mut self, lo: usize, hi: usize) -> usize {
            lo + (self.next_u64() as usize) % (hi - lo + 1)
        }
    }

    /// 混合「正弦 / 噪声 / 静音」块的伪随机音频，幅度 0.001~1.0。
    fn ts381_mixed_audio(rng: &mut Ts381Rng, total: usize) -> Vec<f32> {
        let mut v: Vec<f32> = Vec::with_capacity(total);
        let mut phase: f32 = 0.0;
        while v.len() < total {
            let block = rng.range(160, 3200); // 10ms ~ 200ms
            let kind = rng.next_u64() % 3; // 0 正弦 / 1 噪声 / 2 静音
            let amp = 0.001 + rng.unit() * 0.999;
            let freq = 100.0 + rng.unit() * 400.0;
            let inc = 2.0 * std::f32::consts::PI * freq / TS381_RATE as f32;
            for _ in 0..block {
                if v.len() >= total {
                    break;
                }
                let s = match kind {
                    0 => {
                        phase += inc;
                        phase.sin() * amp
                    }
                    1 => (rng.unit() * 2.0 - 1.0) * amp,
                    _ => 0.0,
                };
                v.push(s);
            }
        }
        v
    }

    /// 契约常量纯正弦（幅度 amp）。
    fn ts381_sine(n: usize, amp: f32) -> Vec<f32> {
        (0..n)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 220.0 * i as f32 / TS381_RATE as f32).sin() * amp
            })
            .collect()
    }

    /// 契约断言：严格相接 + 并集 == [start,end) + 非末片 ∈[10s,12s] + 末片 <11s（或整段 <11s）。
    fn ts381_assert_contract(cuts: &[(usize, usize)], start: usize, end: usize, ctx: &str) {
        assert!(!cuts.is_empty(), "{ctx}: 非空区间必须至少一片");
        assert_eq!(cuts[0].0, start, "{ctx}: 首片必须从 start 起");
        for w in cuts.windows(2) {
            assert_eq!(w[0].1, w[1].0, "{ctx}: 切点必须严格相接：{cuts:?}");
        }
        assert_eq!(cuts.last().unwrap().1, end, "{ctx}: 末片必须覆盖到 end");
        for &(s, e) in cuts.iter() {
            assert!(e > s, "{ctx}: 不允许空片 {s}..{e}");
        }
        let span = end - start;
        for (i, &(s, e)) in cuts.iter().enumerate() {
            if i + 1 == cuts.len() {
                continue;
            }
            let len = e - s;
            assert!(
                (10 * TS381_RATE..=12 * TS381_RATE).contains(&len),
                "{ctx}: 非末片 #{i} 长度 {:.3}s ∉ [10,12]s：{cuts:?}",
                len as f64 / TS381_RATE as f64
            );
        }
        let last_len = cuts.last().unwrap().1 - cuts.last().unwrap().0;
        assert!(
            last_len < 11 * TS381_RATE || span < 11 * TS381_RATE,
            "{ctx}: 末片 {:.3}s 应 <11s（整段 {:.3}s）",
            last_len as f64 / TS381_RATE as f64,
            span as f64 / TS381_RATE as f64
        );
    }

    /// #1 性质测试：100 段伪随机音频（0.5~60s，混合正弦/噪声/静音，幅度 0.001~1.0），
    /// 断言契约四性质：①严格相接且并集==[start,end) ②非末片∈[10,12]s ③末片<11s（或整段<11s）
    /// ④不 panic（能跑完即证）。
    #[test]
    fn ts381_property_plan_gap_cuts_invariants() {
        let mut rng = Ts381Rng::new(0x5EED_0381_C0DE_1234);
        for case in 0..100 {
            // 偏小分布（u^4）覆盖 0.5~60s，同时把 100 段的样本总量压在可控范围。
            let u = rng.unit();
            let secs = 0.5 + u * u * u * u * 59.5;
            let total = ((secs * TS381_RATE as f32) as usize).max(1);
            let audio = ts381_mixed_audio(&mut rng, total);
            assert_eq!(audio.len(), total, "case {case}: 生成样本数不符");
            let cuts = plan_gap_cuts(&audio, 0, total);
            ts381_assert_contract(&cuts, 0, total, &format!("case {case} total={total}"));
        }
    }

    /// #2a 全零音频：前 10s 中位数为 0 ⇒ 阈值 0 ⇒ **所有**帧并列达标，取**最早**候选帧。
    /// 最早候选帧中心 = 10s + 半帧（不是 12s 兜底、不是任意帧）。
    #[test]
    fn ts381_degenerate_all_zero_picks_earliest_at_threshold_zero() {
        let total = 20 * TS381_RATE;
        let audio = vec![0.0f32; total];
        let cuts = plan_gap_cuts(&audio, 0, total);
        ts381_assert_contract(&cuts, 0, total, "all-zero");
        assert_eq!(
            cuts[0],
            (0, 10 * TS381_RATE + TS381_FRAME / 2),
            "阈值 0 下必须取最早达标帧中心（10s + 半帧）"
        );
    }

    /// #2b NaN / ±inf：不得 panic、不得死循环；结构仍自洽（能返回即证无死循环）。
    #[test]
    fn ts381_degenerate_nan_inf_no_panic() {
        let total = 20 * TS381_RATE;
        let mut audio = ts381_sine(total, 0.3);
        for &pos in &[0usize, 12345, 160_000, 161_000, total - 1] {
            audio[pos] = f32::NAN;
        }
        audio[160_500] = f32::INFINITY;
        audio[160_600] = f32::NEG_INFINITY;
        let cuts = plan_gap_cuts(&audio, 0, total);
        ts381_assert_contract(&cuts, 0, total, "nan/inf-mixed");

        // 全 NaN
        let all_nan = vec![f32::NAN; total];
        let cn = plan_gap_cuts(&all_nan, 0, total);
        ts381_assert_contract(&cn, 0, total, "all-nan");

        // 全 +inf
        let all_inf = vec![f32::INFINITY; total];
        let ci = plan_gap_cuts(&all_inf, 0, total);
        ts381_assert_contract(&ci, 0, total, "all-inf");
    }

    /// #2c `end` 超出音频长度：`plan_gap_cuts` **不**负责 clamp（clamp 由调用方
    /// `plan_sliding_cuts` 在进入前完成）。契约要求是**不 panic / 不死循环 / 结构自洽**，
    /// 且末片 end 严格用调用方给的 `end`（不擅自改）。
    #[test]
    fn ts381_degenerate_end_beyond_audio_is_safe_no_clamp() {
        let audio_len = 12 * TS381_RATE; // 只有 12s 音频
        let audio = vec![0.2f32; audio_len];
        let end = 20 * TS381_RATE; // 声称到 20s（越界）
        let cuts = plan_gap_cuts(&audio, 0, end);
        assert!(!cuts.is_empty());
        assert_eq!(cuts[0].0, 0);
        for w in cuts.windows(2) {
            assert_eq!(w[0].1, w[1].0, "越界 end 下仍须严格相接：{cuts:?}");
        }
        assert_eq!(
            cuts.last().unwrap().1,
            end,
            "末片 end 用调用方给的 end（clamp 是调用方的职责）"
        );
    }

    /// #2d 长度恰为 10s / 11s / 11s+1 样本 / 12s 的边界。
    /// 11s 与 11s+1 时 `[lower,upper]` 内无任何帧**中心**落点 ⇒ 兜底切 `lower`(=10s)，
    /// 与音频内容无关 ⇒ 可精确断言。
    #[test]
    fn ts381_boundary_lengths_10_11_12s() {
        assert_eq!(
            plan_gap_cuts(&ts381_sine(10 * TS381_RATE, 0.3), 0, 10 * TS381_RATE),
            vec![(0, 10 * TS381_RATE)],
            "10s：剩余 <11s ⇒ 不切"
        );
        assert_eq!(
            plan_gap_cuts(&ts381_sine(11 * TS381_RATE, 0.3), 0, 11 * TS381_RATE),
            vec![(0, 10 * TS381_RATE), (10 * TS381_RATE, 11 * TS381_RATE)],
            "11s：恰达下限 ⇒ 切 10s + 1s 尾"
        );
        assert_eq!(
            plan_gap_cuts(
                &ts381_sine(11 * TS381_RATE + 1, 0.3),
                0,
                11 * TS381_RATE + 1
            ),
            vec![(0, 10 * TS381_RATE), (10 * TS381_RATE, 11 * TS381_RATE + 1)],
            "11s+1 样本：同上（尾巴 1s + 1 样本）"
        );
        let d = ts381_sine(12 * TS381_RATE, 0.3);
        let cuts = plan_gap_cuts(&d, 0, 12 * TS381_RATE);
        ts381_assert_contract(&cuts, 0, 12 * TS381_RATE, "12s");
        assert_eq!(cuts.len(), 2, "12s ⇒ 2 片：{cuts:?}");
    }

    /// #3 字缝优先（**最早达标**而非最深）：前 10s 语音，10.5s 一个 40ms 近静音、
    /// 11.5s 一个更深的静音 ⇒ 必须切在 10.5s（最早达标帧中心），不得跳到更深的 11.5s。
    #[test]
    fn ts381_gap_priority_earliest_not_deepest() {
        let total = 20 * TS381_RATE;
        let mut audio = ts381_sine(total, 0.3);
        let shallow = 10 * TS381_RATE + TS381_RATE / 2; // 10.5s
        let shallow_end = shallow + 40 * 16; // 40ms
        let deep = 11 * TS381_RATE + TS381_RATE / 2; // 11.5s
        let deep_end = deep + 40 * 16;
        for x in &mut audio[shallow..shallow_end] {
            *x = 0.002; // 浅静音
        }
        for x in &mut audio[deep..deep_end] {
            *x = 0.0002; // 更深静音
        }
        let cuts = plan_gap_cuts(&audio, 0, total);
        assert_eq!(cuts.len(), 2, "20s ⇒ 一次切 + 尾巴：{cuts:?}");
        let cut = cuts[0].1;
        assert!(
            (shallow..shallow_end).contains(&cut),
            "必须切在较早的 10.5s 字缝内：cut={cut}"
        );
        assert!(cut < deep, "不得跳到更深的 11.5s 静音：cut={cut}");
        assert_eq!(cut, shallow + TS381_FRAME / 2, "取最早达标帧的中心样本");
        assert_eq!(cuts[1], (cut, total));
    }

    /// 逐位相等（`to_bits`）—— 比 `f32 ==` 更严（`-0.0` 与 `0.0` 也会被区分）。
    fn ts381_bits_eq(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b.iter())
                .all(|(x, y)| x.to_bits() == y.to_bits())
    }

    /// 构造期望段：`zeros(pre) ++ audio[main] ++ zeros(post)`。
    fn ts381_expected_segment(
        audio: &[f32],
        pre: usize,
        main: (usize, usize),
        post: usize,
    ) -> Vec<f32> {
        let mut v = Vec::with_capacity(pre + (main.1 - main.0) + post);
        v.extend(std::iter::repeat(0.0f32).take(pre));
        v.extend_from_slice(&audio[main.0..main.1]);
        v.extend(std::iter::repeat(0.0f32).take(post));
        v
    }

    /// #4 20s 路径（`build_padded_segments`）**逐位不变快照**（基线 = `1af7212^`，381 之前）。
    ///
    /// 固定输入写死每段的 `(前置 padding, 主段区间, 后置 padding)`；断言：
    /// ① 段数 / 每段长度与快照一致 ② 每段样本与「zeros ++ 原音频切片 ++ zeros」**逐位相等**。
    /// 覆盖：硬切(25s→20+5) / 相邻合并 / 不合并(含间隔 padding) / 越界丢弃 / end clamp。
    /// （本批只动本地 realtime 管线，此路径属离线 accuracy，须零变化。）
    #[test]
    fn ts381_padded_20s_snapshot_bit_identical_to_pre_381() {
        let pad = SEGMENT_PADDING_SAMPLES; // 3200
        let r = TS381_RATE;
        let total = 30 * r; // 480000
                            // 每样本非零（padding 恒 0，便于核对），并按位比对主段。
        let audio: Vec<f32> = (0..total).map(|i| ((i % 1000) as f32) + 1.0).collect();

        // (raw, 期望各段 (pre_pad, main_start, main_end, post_pad))
        let cases: Vec<(Vec<(usize, usize)>, Vec<(usize, usize, usize, usize)>)> = vec![
            // 25s 单段 ⇒ 硬切 20s + 5s；末段补 200ms 尾 padding
            (
                vec![(0, 25 * r)],
                vec![(0, 0, 20 * r, 0), (0, 20 * r, 25 * r, pad)],
            ),
            // 相邻两短段合并后 10.00625s ≤20s ⇒ 单段 + 尾 padding
            (
                vec![(0, 5 * r), (5 * r + 100, 5 * r)],
                vec![(0, 0, 10 * r + 100, pad)],
            ),
            // 不相邻且合并后 30s >20s ⇒ 不合并 ⇒ 两段，中间各补 11200 个 0
            (
                vec![(0, 15 * r), (16 * r, 15 * r)],
                vec![(0, 0, 15 * r, 11_200), (11_200, 16 * r, total, 0)],
            ),
            // start 越界 ⇒ 丢弃 ⇒ 无段
            (vec![(total, 1000)], vec![]),
            // end 越界 ⇒ clamp 到 total ⇒ 前向 padding 3200（末段无尾 padding）
            (
                vec![(total - 1000, 5000)],
                vec![(pad, total - 1000, total, 0)],
            ),
        ];
        for (raw, expected) in cases {
            let segs = build_padded_segments(&raw, total, &audio);
            assert_eq!(segs.len(), expected.len(), "段数快照（raw={raw:?}）");
            for (k, (seg, &(pre, ms, me, post))) in segs.iter().zip(expected.iter()).enumerate() {
                assert_eq!(
                    seg.len(),
                    pre + (me - ms) + post,
                    "段 #{k} 长度快照（raw={raw:?}）"
                );
                let want = ts381_expected_segment(&audio, pre, (ms, me), post);
                assert!(
                    ts381_bits_eq(seg, &want),
                    "段 #{k} 样本必须与 381 之前逐位相等（raw={raw:?}）"
                );
            }
        }
    }

    /// #4b 兜底路径 `naive_chunk`（381 未触及的其它管线）**逐位不变快照**：
    /// 固定输入按 20s 等分；断言每段样本与原音频对应切片逐位相等、区间首尾相接。
    #[test]
    fn ts381_naive_chunk_snapshot_bit_identical_to_pre_381() {
        let r = TS381_RATE;
        let audio: Vec<f32> = (0..30 * r).map(|i| ((i % 97) as f32) - 48.0).collect();

        let cases: Vec<(usize, Vec<(usize, usize)>)> = vec![
            (0, vec![]),
            (10 * r, vec![(0, 10 * r)]),
            (20 * r, vec![(0, 20 * r)]),
            (20 * r + 1, vec![(0, 20 * r), (20 * r, 20 * r + 1)]),
            (25 * r, vec![(0, 20 * r), (20 * r, 25 * r)]),
            (30 * r, vec![(0, 20 * r), (20 * r, 30 * r)]),
        ];
        for (n, expected) in cases {
            let segs = naive_chunk(&audio[..n]);
            assert_eq!(segs.len(), expected.len(), "naive_chunk 段数（n={n}）");
            let mut pos = 0usize;
            for (k, (seg, &(s, e))) in segs.iter().zip(expected.iter()).enumerate() {
                assert_eq!(s, pos, "naive_chunk 区间必须首尾相接（n={n}）");
                assert_eq!(e - s, seg.len(), "段 #{k} 长度（n={n}）");
                assert!(
                    ts381_bits_eq(seg, &audio[s..e]),
                    "段 #{k} 必须与原音频切片逐位相等（n={n}）"
                );
                pos = e;
            }
            assert_eq!(pos, n, "naive_chunk 必须覆盖到末尾（n={n}）");
        }
    }

    /// #5a 非零、且**非帧对齐**的 `start`：首片必须从 `start` 起（作者用例全从 0 起）。
    #[test]
    fn ts381_plan_gap_cuts_nonzero_unaligned_start() {
        let total = 40 * TS381_RATE;
        let mut audio = vec![0.5f32; total];
        for i in (5 * TS381_RATE)..(25 * TS381_RATE) {
            audio[i] = (2.0 * std::f32::consts::PI * 220.0 * (i - 5 * TS381_RATE) as f32
                / TS381_RATE as f32)
                .sin()
                * 0.3;
        }
        let start = 5 * TS381_RATE + 123; // 非 320 对齐
        let end = 25 * TS381_RATE; // 20s 跨度 ⇒ 会切
        let cuts = plan_gap_cuts(&audio, start, end);
        ts381_assert_contract(&cuts, start, end, "nonzero-unaligned-start");
    }

    /// #5b 空区间 / 倒置区间 / 空音频：必须返回空、不 panic。
    #[test]
    fn ts381_plan_gap_cuts_empty_or_inverted_range() {
        let audio = vec![0.1f32; 20 * TS381_RATE];
        assert!(
            plan_gap_cuts(&audio, 5 * TS381_RATE, 5 * TS381_RATE).is_empty(),
            "start == end ⇒ 空"
        );
        assert!(
            plan_gap_cuts(&audio, 8 * TS381_RATE, 3 * TS381_RATE).is_empty(),
            "start > end ⇒ 空"
        );
        assert!(plan_gap_cuts(&[], 0, 0).is_empty(), "空音频 + 空区间 ⇒ 空");
    }

    /// #5c 滑窗路径的**合并阈值 = 10s**，与 20s 路径不同（作者用例未覆盖该差异）：
    /// 两相邻段合并后 16.00625s —— ≤20s（20s 路径合并成 1 段）但 >10s（滑窗不合并 ⇒ 2 段）。
    #[test]
    fn ts381_sliding_merge_threshold_is_10s_not_20s() {
        let total = 30 * TS381_RATE;
        let audio = vec![0.2f32; total];
        let raw = vec![(0, 8 * TS381_RATE), (8 * TS381_RATE + 100, 8 * TS381_RATE)];
        assert_eq!(
            build_padded_segments(&raw, total, &audio).len(),
            1,
            "20s 路径：合并后 16.00625s ≤20s ⇒ 1 段"
        );
        assert_eq!(
            build_sliding_segments(&raw, total, &audio).len(),
            2,
            "滑窗路径：合并上限 10s ⇒ 16.00625s 不合并 ⇒ 2 段"
        );
    }

    /// LOCALRT-VAD-SILENCE-384 / FIX-391：`feed_is_speech`（391 起**逐 512 块喂入**）每 chunk
    /// 排空已完成段 ⇒ 长录音（300s）内部段队列**不增长**（内存有界）。需要 silero 模型 + full.wav。
    #[test]
    #[ignore = "requires silero model + full.wav; cargo test --bin feiyin-ime -- --ignored localrt_vad_feed_drains"]
    fn localrt_vad_feed_drains_queue_bounded() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = project_root.join("models");
        let seg = VadSegmenter::try_new_for_local_silence(&model_dir)
            .expect("local-silence VAD (needs models/silero-vad/silero_vad.onnx)");
        let wav = project_root.join("collab/research/audio-real-gavin/processed/full.wav");
        let wave = sherpa_onnx::Wave::read(wav.to_str().expect("utf8")).expect("read full.wav");
        let speech = wave.samples().to_vec();
        assert!(!speech.is_empty(), "full.wav 应有样本");
        // 300s：full.wav（~56s）循环拼接。
        let target = 300 * 16_000usize;
        let mut audio = Vec::with_capacity(target);
        while audio.len() < target {
            let remain = target - audio.len();
            let n = remain.min(speech.len());
            audio.extend_from_slice(&speech[..n]);
        }

        // 对照：不 pop 时段队列会累积（证明确有段产生，排空不是空转）。
        let ctrl = VadSegmenter::try_new_for_local_silence(&model_dir).unwrap();
        for c in audio.chunks(1600) {
            ctrl.detector.accept_waveform(c);
        }
        assert!(
            !ctrl.detector.is_empty(),
            "对照：不排空时段队列应已累积（否则本测无鉴别力）"
        );
        ctrl.detector.clear();

        // 被测：feed_is_speech 每个 chunk 排空 ⇒ 队列恒空，且确实检测到人声。
        let mut n_speech = 0usize;
        for c in audio.chunks(1600) {
            if seg.feed_is_speech(c) {
                n_speech += 1;
            }
            assert!(
                seg.detector.is_empty(),
                "feed_is_speech 后段队列必须为空（不随录音增长）"
            );
        }
        assert!(n_speech > 0, "应检测到人声 chunk（否则判定失效）");
        println!(
            "LOCALRT384 feed 300s: chunks={} speech_chunks={}",
            audio.len() / 1600,
            n_speech
        );
    }

    // ========================================================================
    // FIX-VAD-FEED-BY-WINDOW-391：逐 512 块喂入（speech_ranges / feed_is_speech）
    // ========================================================================

    /// FIX-391-④（纯逻辑）：`feed_in_vad_windows` 喂入次数 = `ceil(len/512)`、每块 ∈ (0,512]；
    /// 源码级护栏：`speech_ranges` 与 `feed_is_speech` 都走它（不再一次性 `accept` 整块）。
    #[test]
    fn vad391_feed_in_vad_windows_counts_ceil() {
        fn plan(len: usize) -> (usize, Vec<usize>) {
            let mut sizes = Vec::new();
            let n = feed_in_vad_windows(&vec![0.0f32; len], |b| sizes.push(b.len()));
            (n, sizes)
        }
        assert_eq!(plan(0), (0, vec![]));
        assert_eq!(plan(512), (1, vec![512]));
        assert_eq!(plan(513), (2, vec![512, 1]));
        let (n, sizes) = plan(1250);
        assert_eq!(n, 3, "ceil(1250/512)=3");
        assert_eq!(sizes, vec![512, 512, 226]);
        assert_eq!(n, (1250 + 511) / 512);
        assert!(sizes.iter().all(|&s| s > 0 && s <= 512));
        // 源码级护栏（needle 用 concat! 拆串，避免命中本测试自身字符串）。
        let src = include_str!("vad.rs");
        assert!(
            src.contains(concat!("feed_in_vad_windows(audio, |", "block|")),
            "speech_ranges 必须逐块喂入"
        );
        assert!(
            src.contains(concat!("feed_in_vad_windows(samples, |", "block|")),
            "feed_is_speech 必须逐块喂入"
        );
    }

    /// FIX-391-①②（必须实际运行，需 silero 模型 + full.wav）：4 段各 6s 语音 + 前后各 3s 静音：
    /// ① `speech_ranges` + `trim_to_speech` 后时长 ≥ 原片段 × 0.9（不吞语音）；
    /// ② ≤ 原片段 + 0.8s（静音被剪）；③ 对照：同一段**整块一次**喂入（旧写法，本测复刻）⇒ 显著更短。
    #[test]
    #[ignore = "requires silero model + full.wav; cargo test --bin feiyin-ime -- --ignored --nocapture vad391"]
    fn vad391_speech_ranges_keeps_speech_cuts_silence() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = root.join("models");
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        let Some(wave) = sherpa_onnx::Wave::read(wav.to_str().unwrap()) else {
            eprintln!("skip: full.wav 缺失");
            return;
        };
        let all = wave.samples();
        let rate = 16_000usize;
        let clip = 6 * rate;
        let pad = 3 * rate;
        let pad_samples = (LOCALRT_TRIM_PAD_SECS * rate as f32) as usize;
        let starts = [2 * rate, 9 * rate, 16 * rate, 23 * rate];
        for (i, &s) in starts.iter().enumerate() {
            let src = &all[s..(s + clip).min(all.len())];
            let mut audio = vec![0.0f32; pad];
            audio.extend_from_slice(src);
            audio.extend(std::iter::repeat(0.0f32).take(pad));
            let src_secs = src.len() as f32 / rate as f32;

            let Some(vad) = VadSegmenter::try_new_for_local_trim(&model_dir) else {
                eprintln!("skip: silero VAD 不可用");
                return;
            };
            let ranges = vad.speech_ranges(&audio);
            let out = crate::transcription::trim_to_speech(&audio, &ranges, pad_samples);
            let out_secs = out.len() as f32 / rate as f32;
            println!(
                "vad391 clip#{i} start={}s in={src_secs:.2}s ranges={ranges:?} out={out_secs:.2}s",
                s / rate
            );
            assert!(
                out_secs >= src_secs * 0.9,
                "clip#{i} 不得吞语音（out={out_secs:.2} src={src_secs:.2}）"
            );
            assert!(
                out_secs <= src_secs + 0.8,
                "clip#{i} 静音应被剪（out={out_secs:.2}）"
            );

            // 对照：旧「整块一次 accept_waveform」（本测复刻，不经生产函数）。
            let d = VadSegmenter::try_new_for_local_trim(&model_dir).unwrap();
            d.detector.reset();
            d.detector.accept_waveform(&audio);
            d.detector.flush();
            let mut old_ranges: Vec<(usize, usize)> = Vec::new();
            while let Some(seg) = d.detector.front() {
                let st = seg.start() as usize;
                let n = seg.n() as usize;
                d.detector.pop();
                old_ranges.push((st, st + n));
            }
            d.detector.clear();
            d.detector.reset();
            let old_out = crate::transcription::trim_to_speech(&audio, &old_ranges, pad_samples);
            let old_secs = old_out.len() as f32 / rate as f32;
            println!("vad391 clip#{i} OLD-whole-feed ranges={old_ranges:?} out={old_secs:.2}s");
            assert!(
                old_secs < src_secs * 0.5,
                "旧整块写法应显著更短（证根因）：old={old_secs:.2} src={src_secs:.2}"
            );
        }
    }

    /// FIX-391-③（必须实际运行，需 silero 模型 + full.wav）：同一音频按 160 / 512 / 1600 / 16000
    /// 样本块喂入 `feed_is_speech`、每块后取结果，人声开始 / 结束的判定差异 ≤ 一个喂入块
    ///（≤512 的块数 ≤ 512 ⇒ 与 VAD 窗口同粒度；块越大，观测粒度=块大小）。
    #[test]
    #[ignore = "requires silero model + full.wav; cargo test --bin feiyin-ime -- --ignored --nocapture vad391"]
    fn vad391_feed_is_speech_block_size_invariant() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = root.join("models");
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        let Some(wave) = sherpa_onnx::Wave::read(wav.to_str().unwrap()) else {
            eprintln!("skip: full.wav 缺失");
            return;
        };
        let audio: Vec<f32> = wave.samples().to_vec();
        let sizes = [160usize, 512, 1600, 16000];
        let mut ranges = Vec::new();
        for &bs in &sizes {
            let Some(vad) = VadSegmenter::try_new_for_local_trim(&model_dir) else {
                eprintln!("skip: silero VAD 不可用");
                return;
            };
            let mut first: Option<usize> = None;
            let mut last = 0usize;
            let mut off = 0usize;
            while off < audio.len() {
                let end = (off + bs).min(audio.len());
                if vad.feed_is_speech(&audio[off..end]) {
                    if first.is_none() {
                        first = Some(off);
                    }
                    last = end;
                }
                off = end;
            }
            let f = first.unwrap_or(0);
            println!("vad391 feed bs={bs} first_speech={f} last_speech={last}");
            ranges.push((f, last));
        }
        // 以 512 为基准（VAD 窗口粒度）。
        let (rf, rl) = ranges[1];
        for (i, &(f, l)) in ranges.iter().enumerate() {
            let bs = sizes[i];
            assert!(
                f.abs_diff(rf) <= bs.max(512),
                "bs={bs} 开始判定差异 {} 应 ≤ 喂入块",
                f.abs_diff(rf)
            );
            assert!(
                l.abs_diff(rl) <= bs.max(512),
                "bs={bs} 结束判定差异 {} 应 ≤ 喂入块",
                l.abs_diff(rl)
            );
        }
    }
}

// ========================================================================
// TEST-SYNC-391（阶段三 · 非作者护栏）：逐块喂入覆盖性质 / 源码护栏 / 旧写法反例
// ========================================================================
#[cfg(test)]
mod testsync391_tests {
    use super::{feed_in_vad_windows, VadSegmenter};

    struct Lcg(u64);
    impl Lcg {
        fn n(&mut self, lo: usize, hi: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (lo as u64 + (self.0 >> 33) % ((hi - lo + 1) as u64)) as usize
        }
    }

    /// 1. 逐块覆盖性质：300 组随机长度 0~200000，收集 accept 收到的块 —— 每块长度 ∈ (0,512]、
    ///    除末块外均 512、按序拼接**逐样本等于**原音频、次数 = `ceil(len/512)`。
    #[test]
    fn ts391_feed_windows_covers_all_samples_in_order() {
        let mut rng = Lcg(0x391_5EED);
        for case in 0..300usize {
            let len = rng.n(0, 200_000);
            // 唯一值 = 原下标（f32 对 ≤2^24 整数精确）⇒ 可逐样本校验拼接。
            let audio: Vec<f32> = (0..len).map(|i| i as f32).collect();
            let mut blocks: Vec<Vec<f32>> = Vec::new();
            let calls = feed_in_vad_windows(&audio, |b| blocks.push(b.to_vec()));
            assert_eq!(
                calls,
                len.div_ceil(512),
                "case {case}: 次数 = ceil(len/512)"
            );
            assert_eq!(blocks.len(), calls, "case {case}: 接受块数 = 调用次数");
            for (j, b) in blocks.iter().enumerate() {
                assert!(
                    !b.is_empty() && b.len() <= 512,
                    "case {case}: 块 {j} 长度 {} 越界（应 ∈(0,512]）",
                    b.len()
                );
                if j + 1 < blocks.len() {
                    assert_eq!(b.len(), 512, "case {case}: 非末块必须满 512");
                }
            }
            let flat: Vec<f32> = blocks.iter().flatten().copied().collect();
            assert_eq!(flat, audio, "case {case}: 按序拼接必须逐样本等于原音频");
        }
    }

    /// 2. 源码护栏：**逐块喂入口**（`speech_ranges` / `feed_speech`）的函数体（**剔除注释行**）内
    ///    **不得**直接对整段 `accept_waveform(audio)` / `accept_waveform(samples)`，必须经
    ///    `feed_in_vad_windows(`；VAD-393（A1）后 `feed_is_speech` 改为薄包装 ⇒ 另断言其委托
    ///    `feed_speech(`（同样逐块），且自身不直接整块喂。
    #[test]
    fn ts391_entry_points_feed_by_window_only() {
        let src = include_str!("vad.rs");
        for (anchor, arg) in [
            ("pub fn speech_ranges(", "audio"),
            ("pub fn feed_speech(", "samples"),
        ] {
            let body = src.split(anchor).nth(1).expect("锚点缺失");
            // 函数体：截到下一个 `pub fn` 或 impl 结束。
            let body = body.split("\n    pub fn ").next().unwrap();
            let body = body.split("\n}\n").next().unwrap();
            let code: String = body
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            let direct = format!("accept_waveform({arg})");
            assert!(
                !code.contains(&direct),
                "{anchor} 函数体内不得直接 `{direct}`（必须逐块）"
            );
            assert!(
                code.contains("feed_in_vad_windows("),
                "{anchor} 必须经 feed_in_vad_windows 逐块喂入"
            );
        }
        // 393：`feed_is_speech` 薄包装 ⇒ 委托 `feed_speech(`（逐块），自身不直接整块喂。
        let body = src
            .split("pub fn feed_is_speech(")
            .nth(1)
            .expect("feed_is_speech 锚点缺失");
        let body = body.split("\n    pub fn ").next().unwrap();
        let code: String = body
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains("accept_waveform(samples)"),
            "feed_is_speech 不得直接整块 accept_waveform"
        );
        assert!(
            code.contains("feed_speech("),
            "feed_is_speech 必须委托 feed_speech（逐块喂）"
        );
    }

    /// 3. 旧写法反例留证（`#[ignore]`，需 silero + full.wav）：同一「3s 静音 + 6s 语音 + 3s 静音」
    ///    —— **整块一次**喂入 ⇒ 区间只落在**末尾**（总长 ≤0.3s、起点贴近末尾）；
    ///    **逐块**喂入（生产 `speech_ranges`）⇒ 覆盖语音主体（起点在补静音之后 0.5s 内）。
    #[test]
    #[ignore = "requires silero model + full.wav; cargo test --bin feiyin-ime -- --ignored --nocapture ts391"]
    fn ts391_whole_feed_tail_only_vs_windowed_covers_speech() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = root.join("models");
        let wav = root.join("collab/research/audio-real-gavin/processed/full.wav");
        let Some(wave) = sherpa_onnx::Wave::read(wav.to_str().unwrap()) else {
            eprintln!("skip: full.wav 缺失");
            return;
        };
        let all = wave.samples();
        let rate = 16_000usize;
        let clip = 6 * rate;
        let pad = 3 * rate;
        let src = &all[2 * rate..(2 * rate + clip).min(all.len())];
        let mut audio = vec![0.0f32; pad];
        audio.extend_from_slice(src);
        audio.extend(std::iter::repeat(0.0f32).take(pad));
        let audio_secs = audio.len() as f32 / rate as f32;
        let speech_secs = src.len() as f32 / rate as f32;

        let Some(vad) = VadSegmenter::try_new_for_local_trim(&model_dir) else {
            eprintln!("skip: silero VAD 不可用");
            return;
        };
        // 逐块（生产路径）：起点在补静音之后 0.5s 内 + 覆盖语音主体。
        let ranges = vad.speech_ranges(&audio);
        assert!(!ranges.is_empty(), "逐块喂入应检出语音区间");
        let first_start = ranges[0].0 as f32 / rate as f32;
        assert!(
            first_start <= (pad as f32 / rate as f32) + 0.5,
            "逐块：起点应在补静音(3s)之后 0.5s 内，实测 {first_start:.2}s"
        );
        let covered = ranges
            .iter()
            .map(|&(s, e)| e.saturating_sub(s))
            .sum::<usize>() as f32
            / rate as f32;
        assert!(
            covered >= speech_secs * 0.9,
            "逐块：应覆盖语音主体（covered={covered:.2}s / speech={speech_secs:.2}s）"
        );

        // 对照：旧「整块一次」（本测复刻，不经生产函数）。
        let d = VadSegmenter::try_new_for_local_trim(&model_dir).unwrap();
        d.detector.reset();
        d.detector.accept_waveform(&audio);
        d.detector.flush();
        let mut old: Vec<(usize, usize)> = Vec::new();
        while let Some(seg) = d.detector.front() {
            let s = seg.start() as usize;
            let n = seg.n() as usize;
            d.detector.pop();
            old.push((s, s + n));
        }
        d.detector.clear();
        d.detector.reset();
        let old_secs =
            old.iter().map(|&(s, e)| e.saturating_sub(s)).sum::<usize>() as f32 / rate as f32;
        let old_start = old
            .first()
            .map(|&(s, _)| s as f32 / rate as f32)
            .unwrap_or(audio_secs);
        println!(
            "ts391 windowed first_start={first_start:.2}s covered={covered:.2}s | whole old_start={old_start:.2}s old_total={old_secs:.2}s (audio={audio_secs:.2}s)"
        );
        assert!(
            old_secs <= 0.3,
            "整块一次：区间总长应 ≤0.3s（只落末尾），实测 {old_secs:.2}s"
        );
        assert!(
            old_start >= audio_secs - 0.3,
            "整块一次：区间起点应贴近末尾，实测 {old_start:.2}s（末尾 {audio_secs:.2}s）"
        );
    }

    /// 393-A1⑨（TEST-SYNC）：`pad_and_extract`（20s 路径共用）与 `build_sliding_segments_with_spans`
    /// 在**相邻段**（`prev_end == start`）输入下：不 panic；音频输出逐位相等；首片 `pad_before == 0`。
    /// （重叠输入不在契约内：上游合并保证段不重叠，故不测。）
    #[test]
    fn ts393c_adjacent_segments_bitequal_and_pad_before_zero() {
        use super::{build_sliding_segments_with_spans, pad_and_extract};
        let total = 2000usize;
        let audio: Vec<f32> = (0..total).map(|i| (i as f32) * 0.01).collect();
        // 相邻段：seg0 [0,1000)、seg1 [1000,2000)（prev_end == start）。
        let raw = [(0usize, 1000usize), (1000, 1000)];
        let (padded_full, spans) = build_sliding_segments_with_spans(&raw, total, &audio);
        // plan_sliding_cuts 把相邻短段并成一片 [0,2000) ⇒ 首片 pad_before = 0 - 0 = 0。
        assert_eq!(
            spans.len(),
            1,
            "相邻段应合并为一片，实测 {} 片",
            spans.len()
        );
        assert_eq!(spans[0].2, 0, "首片 pad_before 应为 0");
        // 直接以合并后的区间走 20s 路径 ⇒ 与滑窗入口音频逐位相等。
        let merged = [(0usize, 2000usize)];
        let padded_direct = pad_and_extract(&merged, total, &audio);
        assert_eq!(
            padded_direct, padded_full,
            "pad_and_extract 与 build_sliding_segments_with_spans 音频输出必须逐位相等"
        );
    }
}
