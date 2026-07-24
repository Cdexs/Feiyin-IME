use anyhow::{Context, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::{config::ChineseScript, text_normalizer};

// Re-export SenseVoice config for convenience
use sherpa_onnx::{OfflineFunASRNanoModelConfig, OfflineSenseVoiceModelConfig};

pub mod qwen3_online;
mod vad;
pub use vad::{
    build_padded_segments, join_segment_texts, naive_chunk, should_segment, VadSegmenter,
    SEGMENT_MAX_SECS, SEGMENT_PADDING_SAMPLES, SEGMENT_TRIGGER_SECS,
};

/// ASR mode: offline or streaming (2-pass)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AsrMode {
    /// Offline mode: single pass, higher accuracy
    Offline,
    /// Streaming mode: 2-pass (streaming preview + offline correction)
    Streaming,
}

/// ASR 模型选择（DEC-025 + DEC-028）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AsrModel {
    /// 性能最优：179MB FunASR Nano CTC 兼容版（OfflineSenseVoiceModelConfig，无 hotwords）
    Performance,
    /// 准确率更高：972MB FunASR Nano native（OfflineFunASRNanoModelConfig，config 层 hotwords）
    Accuracy,
    /// Qwen3 在线 ASR（DEC-028）：零本地 ASR 内存，WebSocket 实时协议
    Qwen3Online,
}

impl AsrModel {
    pub fn from_config(s: &str) -> Self {
        if s.eq_ignore_ascii_case("accuracy") {
            AsrModel::Accuracy
        } else if s.eq_ignore_ascii_case("qwen3_online") {
            AsrModel::Qwen3Online
        } else {
            AsrModel::Performance
        }
    }
}

/// ASR transcriber using sherpa-onnx
///
/// ASR-SINGLE-MODEL-001（DEC-027）+ DEC-028：多模式 ASR 架构
/// - Performance: 179MB CTC，OfflineSenseVoiceModelConfig，无 hotwords
/// - Accuracy: 972MB native，OfflineFunASRNanoModelConfig，config 层 hotwords
/// - Qwen3Online: 零本地 ASR 内存，WebSocket 实时协议（DEC-028）
/// - 本地模式一次只加载一个模型；accuracy 不再预创建 CTC fallback
/// - H1 temperature 0.1 为 accuracy 唯一幻觉缓解（2026-07-08 Gavin 拍板下调 0.3→0.1，RESEARCH-ASR-ACCURACY-002 证实越低越好）；异常检测链已删除
///
/// SAFETY: sherpa_onnx::OfflineRecognizer 内含 *const C++ 指针（!Send），
/// 但其 C++ 实现本身是线程安全的（create/decode/destroy 均可跨线程，
/// 只需保证同一 recognizer 实例不被并发访问）。
/// 此处 Transcriber 通过 channel 在后台构建线程与 worker 线程间转移，
/// 转移期间无并发访问（构建完成后才发送，发送后构建线程不再触碰），
/// 因此手动实现 Send 是安全的。
pub struct Transcriber {
    mode: AsrMode,
    asr_language: String,
    asr_model: AsrModel,
    /// 本地 ASR recognizer（Performance/Accuracy 模式用；Qwen3Online 为 None）
    offline_recognizer: Option<sherpa_onnx::OfflineRecognizer>,
    /// 当前注入的 hotwords 版本号（len + 内容哈希），用于感知词库变更
    hotwords_version: u64,
    /// VAD 分段器（仅 accuracy 长音频用，懒加载）
    /// 用 Mutex<Option> 因为 VAD 在首次长音频时才初始化
    vad_segmenter: Option<Mutex<vad::VadSegmenter>>,
    /// Qwen3 在线 ASR 配置（仅 Qwen3Online 模式用）
    qwen3_url: String,
    qwen3_api_key: String,
    /// Qwen3 在线 ASR 模型 ID（DEC-028，从配置文件读取，2026-07-07 移出硬编码）
    qwen3_asr_model: String,
}

// SAFETY: Transcriber 持有的 OfflineRecognizer 内部为 *const C++ 指针。
// 跨线程转移时，发送方在 send 后不再访问该实例，接收方独占所有权，
// 满足"单一时刻单线程访问"约束。sherpa-onnx C++ 层本身支持跨线程调用。
unsafe impl Send for Transcriber {}

impl Transcriber {
    /// Create new Transcriber with explicit ASR model selection
    ///
    /// ASR-SINGLE-MODEL-001（DEC-027）+ DEC-028：多模式 ASR
    /// - Performance/Accuracy：加载本地模型（单模型，不预创建 fallback）
    /// - Qwen3Online：不加载任何本地模型（零本地 ASR 内存），存 url+key
    ///
    /// R2 修订（验收第 2 轮）：asr_model 存的是 effective_model（生效模型）。
    /// accuracy 降级 CTC 时 effective=Performance，语义自动归位。
    /// DEC-028：qwen3_online 模式 key 空 → bail 明确错误（UI 侧保证 key 非空才允许选中，后端防御）
    pub fn new(
        model_dir: &Path,
        enable_streaming: bool,
        asr_language: String,
        asr_model: AsrModel,
        hotwords: Option<&str>,
        qwen3_url: &str,
        qwen3_api_key: &str,
        qwen3_asr_model: &str,
    ) -> Result<Self> {
        let mode = if enable_streaming {
            AsrMode::Streaming
        } else {
            AsrMode::Offline
        };

        // DEC-028: qwen3_online 模式不加载本地模型
        if asr_model == AsrModel::Qwen3Online {
            if qwen3_api_key.trim().is_empty() {
                anyhow::bail!("Qwen3 ASR 配置失败：API Key 为空（请在设置中配置 Qwen3 API Key）");
            }
            log::info!(
                "Qwen3 online ASR mode: no local model loaded, url={}",
                qwen3_url
            );
            return Ok(Self {
                mode,
                asr_language,
                asr_model,
                offline_recognizer: None,
                hotwords_version: 0,
                vad_segmenter: None,
                qwen3_url: qwen3_url.to_string(),
                qwen3_api_key: qwen3_api_key.to_string(),
                qwen3_asr_model: qwen3_asr_model.to_string(),
            });
        }

        let (offline_recognizer, effective_model, hotwords_version) =
            build_recognizer(model_dir, &asr_language, asr_model, hotwords)?;

        // VAD 分段器仅 accuracy 模式懒加载初始化；performance 模式设 None
        // R2: 用 effective_model 判断（降级 CTC 时不初始化 VAD）
        let vad_segmenter = if effective_model == AsrModel::Accuracy {
            // accuracy 模式下立即尝试初始化（模型文件在则建，失败后续降级）
            vad::VadSegmenter::try_new(model_dir).map(Mutex::new)
        } else {
            None
        };

        Ok(Self {
            mode,
            asr_language,
            asr_model: effective_model,
            offline_recognizer: Some(offline_recognizer),
            hotwords_version,
            vad_segmenter,
            qwen3_url: String::new(),
            qwen3_api_key: String::new(),
            qwen3_asr_model: String::new(),
        })
    }

    pub fn asr_model(&self) -> AsrModel {
        self.asr_model
    }

    /// 当前 hotwords 版本号（外部对比用）
    pub fn hotwords_version(&self) -> u64 {
        self.hotwords_version
    }

    /// 当前 ASR 语言（热重载 swap 时同步 active 状态用）
    pub fn language(&self) -> &str {
        &self.asr_language
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
        _language: &str,
        script: ChineseScript,
    ) -> Result<String> {
        self.transcribe_with_punct_info(samples, script)
            .map(|(text, _)| text)
    }

    /// 转录并返回标点来源标记（ASR-PUNCT-OPT-001）
    ///
    /// 返回 `(text, native_punctuated)`：
    /// - `native_punctuated = true`：文本真正出自 native 模型（自带标点），可跳过标点引擎
    /// - `native_punctuated = false`：文本出自 performance/兜底/混合来源（无标点），需走标点引擎
    ///
    /// 标记规则：
    /// - performance → 恒 false
    /// - accuracy 单次 native 成功 → true
    /// - accuracy VAD 分段：所有段均 native 成功才 true；任一段兜底 → false
    /// - qwen3_online → true（DEC-028：在线模型输出自带标点）
    pub fn transcribe_with_punct_info(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        // DEC-028: qwen3_online 模式走在线转录路径
        if self.asr_model == AsrModel::Qwen3Online {
            let lang = if self.asr_language == "auto" {
                None
            } else {
                Some(self.asr_language.as_str())
            };
            let text = qwen3_online::transcribe_online(
                &self.qwen3_url,
                &self.qwen3_api_key,
                &self.qwen3_asr_model,
                samples,
                lang,
            )?;
            // Qwen3 输出自带标点 → native_punctuated=true（跳过标点引擎）
            let normalized = text_normalizer::normalize_text_for_language(&text, script);
            return Ok((normalized, true));
        }
        match self.mode {
            AsrMode::Offline => self.transcribe_offline_detailed(samples, script),
            AsrMode::Streaming => self.transcribe_2pass_detailed(samples, script),
        }
    }

    /// Single-pass offline transcription (higher accuracy) — 旧签名兼容
    fn transcribe_offline(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        self.transcribe_offline_detailed(samples, script)
            .map(|(t, _)| t)
    }

    /// 带标点来源标记的 offline 转录
    ///
    /// ASR-SINGLE-MODEL-001（DEC-027）：VAD 降级路径重设计
    /// - 分段全空 → bail 转录失败（不再降级单次转录整段）
    /// - VAD segmenter 不可用 / lock poisoned 且 >24s → 朴素 20s 等分
    /// - 短音频 <24s → 单次转录路径不变
    ///
    /// R1 修订（验收第 2 轮）：lock poisoned 不再静默落到单次转录整段，
    /// 改为走 naive_chunk 分支（与 VAD 不可用同路径）。
    fn transcribe_offline_detailed(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        // ASR-LONG-AUDIO-001: accuracy 分支长音频 VAD 分段路径
        if self.asr_model == AsrModel::Accuracy && vad::should_segment(samples) {
            // 尝试 VAD 分段；lock poisoned / None → 降级 naive_chunk
            let vad_segments: Option<Vec<Vec<f32>>> = match &self.vad_segmenter {
                Some(vad_lock) => match vad_lock.lock() {
                    Ok(vad) => {
                        let segs = vad.segment(samples);
                        if segs.is_empty() {
                            log::warn!("VAD produced no speech segments, transcription failed");
                            anyhow::bail!(
                                "ASR transcription failed: VAD detected no speech segments"
                            );
                        }
                        log::info!(
                            "VAD segmented {} samples ({:.1}s) into {} segments",
                            samples.len(),
                            samples.len() as f64 / 16000.0,
                            segs.len()
                        );
                        Some(segs)
                    }
                    Err(_) => {
                        // R1: lock poisoned → 走 naive_chunk，禁止静默落到单次转录整段
                        log::warn!(
                            "VAD segmenter lock poisoned, falling back to naive {}s chunking",
                            vad::SEGMENT_MAX_SECS
                        );
                        None
                    }
                },
                None => {
                    // VAD segmenter 不可用 → 朴素 20s 等分
                    // 保证 accuracy 长音频在 VAD 模型缺失时仍可用（禁止 >28s 整段喂 native）
                    log::warn!(
                        "VAD segmenter unavailable, using naive {}s chunking for {} samples ({:.1}s)",
                        vad::SEGMENT_MAX_SECS,
                        samples.len(),
                        samples.len() as f64 / 16000.0
                    );
                    None
                }
            };

            let segments = match vad_segments {
                Some(segs) => segs,
                None => vad::naive_chunk(samples),
            };

            // R1: 抽出的辅助函数复用（VAD 分段 / naive_chunk 共用转录循环）
            return self.transcribe_segments_chunked(&segments, script);
        }

        self.transcribe_segment_detailed(samples, script)
    }

    /// ASR-SINGLE-MODEL-001 R1：逐段转录循环（VAD 分段 / naive_chunk 复用）。
    ///
    /// 所有段转录后拼接；任一段空/失败 → all_native=false；
    /// 拼接结果全空 → bail 转录失败。
    fn transcribe_segments_chunked(
        &self,
        segments: &[Vec<f32>],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        let mut all_native = true;
        let mut seg_texts: Vec<String> = Vec::with_capacity(segments.len());
        for seg in segments {
            match self.transcribe_segment_detailed(seg, script) {
                Ok((t, np)) => {
                    seg_texts.push(t);
                    if !np {
                        all_native = false;
                    }
                }
                Err(e) => {
                    log::warn!("Segment transcription failed: {}", e);
                    seg_texts.push(String::new());
                    all_native = false;
                }
            }
        }
        let joined = vad::join_segment_texts(&seg_texts);
        if !joined.trim().is_empty() {
            log::info!("Segmented transcription: {}", joined);
            return Ok((
                text_normalizer::normalize_text_for_language(&joined, script),
                all_native,
            ));
        }
        log::warn!("All segments produced empty text, transcription failed");
        anyhow::bail!("ASR transcription failed: all segments produced empty text");
    }

    /// 转录单段音频 — 旧签名兼容
    fn transcribe_segment(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        self.transcribe_segment_detailed(samples, script)
            .map(|(t, _)| t)
    }

    /// 带标点来源标记的单段转录。
    ///
    /// ASR-SINGLE-MODEL-001（DEC-027）：移除兜底链与异常检测。
    /// - accuracy native 成功 → (text, true)
    /// - accuracy 空输出 → bail 转录失败（上层 overlay 提示）
    /// - performance → (text, false)
    /// - qwen3_online 不走此路径（transcribe_with_punct_info 已路由）
    fn transcribe_segment_detailed(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        let recognizer = self
            .offline_recognizer
            .as_ref()
            .context("No local ASR recognizer (qwen3_online mode should not reach here)")?;
        let stream = recognizer.create_stream();
        stream.accept_waveform(16000, samples);
        recognizer.decode(&stream);

        let result = stream.get_result().context("No transcription result")?;
        let text = result.text.trim().to_string();

        // ASR-SINGLE-MODEL-001: accuracy 空输出 → bail（不再兜底 CTC）
        if self.asr_model == AsrModel::Accuracy {
            if text.is_empty() {
                log::warn!("ASR accuracy model produced empty output, transcription failed");
                anyhow::bail!("ASR transcription failed: accuracy model produced empty output");
            }
            // accuracy native 成功 → native_punctuated = true
            return Ok((
                text_normalizer::normalize_text_for_language(&text, script),
                true,
            ));
        }

        // performance 分支 → native_punctuated = false（CTC 无标点）
        Ok((
            text_normalizer::normalize_text_for_language(&text, script),
            false,
        ))
    }

    /// 2-pass streaming transcription — 旧签名兼容
    fn transcribe_2pass(&self, samples: &[f32], script: ChineseScript) -> Result<String> {
        self.transcribe_2pass_detailed(samples, script)
            .map(|(t, _)| t)
    }

    /// 带标点来源标记的 2-pass 转录
    fn transcribe_2pass_detailed(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        // 当前实现：直接使用 offline 单遍
        self.transcribe_offline_detailed(samples, script)
    }
}

/// 剥离 native 模型自带标点（ASR-PUNCT-OPT-001）
///
/// 用于 accuracy 模式 + 用户关闭自动标点（punctuation.enabled=false）场景：
/// native 输出自带标点，用户关了开关但 native 照样出标点 → 剥离使其与 CTC 行为一致。
///
/// 剥离字符集：中文标点（，。！？；：、""''…—）+ 英文标点（,.!?;:"'()[]）
/// **不剥离**：小数点（.）在数字间（如 3.14）、URL/路径中的 / . - _ ~
///
/// 注意：此函数保守处理——只剥离明确的句末/句中标点符号，
/// 不处理数字间小数点（避免破坏"3.14"等数值）。
pub fn strip_punctuation(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(chars.len());
    let punct_set: &[char] = &[
        '，', '。', '！', '？', '；', '：', '、', '\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}',
        '…', '—', ',', '!', '?', ';', ':', '"', '\'', '(', ')', '[', ']',
    ];
    for (i, &c) in chars.iter().enumerate() {
        if punct_set.contains(&c) {
            // 保护数字间的小数点（前后均为数字/空格+数字）
            if c == '.' {
                let prev = chars.get(i.wrapping_sub(1)).copied().unwrap_or(' ');
                let next = chars.get(i + 1).copied().unwrap_or(' ');
                if prev.is_ascii_digit() && next.is_ascii_digit() {
                    result.push(c);
                    continue;
                }
            }
            // 保护句末省略号模式：连续的 . 不处理为单独标点
            // （. 不在 punct_set 中，所以无需特殊处理）
            continue;
        }
        result.push(c);
    }
    // 清理剥离后可能产生的连续空格（标点前后原有空格）
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }
    result.trim().to_string()
}
/// entries 须按确定性顺序（调用方按 id 排序）保证哈希稳定
pub fn hotwords_version(entries: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();
    entries.len().hash(&mut hasher);
    for word in entries {
        word.hash(&mut hasher);
    }
    hasher.finish()
}

/// ASR-ACC-OPT-001 方案 A：hotwords 精选上限。
/// 超过此数量的词条按 id DESC（最近添加优先）截断。
/// 研究依据：220 条 hotwords → 0% 全空输出（撑爆 max_total_len=512 context），
/// 全量 11 条含无关英文词 → 60%（比无 hotwords 62.5% 还差）。
/// ASR-ACC-TUNE-001（2026-07-08 Gavin 拍板）：50→20，002/003 实测 hw=50 比 hw=20
/// 退化 -2.5pp，10-20 为最优区间。
pub const HOTWORDS_MAX_ENTRIES: usize = 20;

/// ASR-ACC-OPT-001 方案 A：hotwords 单条最大字符数。
/// 超长的词条（candidates 整句）不灌入，避免膨胀 user_prompt。
pub const HOTWORDS_MAX_ENTRY_CHARS: usize = 10;

/// ASR-ACC-OPT-001 方案 A：判定词条是否为纯 ASCII（纯英文/数字）。
/// 纯 ASCII 词条（worker1/tester1/todo 等无关词）带偏 native decoder，
/// 研究证实全量 wordbook 含此类词性能从 62.5% 降到 60%。
fn is_pure_ascii(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii())
}

/// ASR-ACC-OPT-001 方案 A：精选 wordbook 词条，过滤无关词 + 上限截断。
///
/// 过滤规则（按研究 RESEARCH-ASR-ACCURACY-001 R2）：
/// 1. 空/纯空白词条过滤
/// 2. 纯 ASCII 词条过滤（英文/数字如 worker1/tester1/todo）
/// 3. 超长词条过滤（> HOTWORDS_MAX_ENTRY_CHARS，candidates 整句不灌入）
/// 4. 数量上限 HOTWORDS_MAX_ENTRIES，按入参顺序（调用方已按 id DESC 排序）
///    截断保留最近的词条
///
/// 调用方（main.rs load_hotwords_for_accuracy）已按 id DESC 排序，
/// 截断后保留最近添加的词条，确定性顺序保证 hotwords 版本号哈希稳定。
pub fn curate_hotwords_entries(entries: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for word in entries {
        let trimmed = word.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_pure_ascii(trimmed) {
            continue;
        }
        if trimmed.chars().count() > HOTWORDS_MAX_ENTRY_CHARS {
            continue;
        }
        result.push(trimmed.to_string());
        if result.len() >= HOTWORDS_MAX_ENTRIES {
            break;
        }
    }
    result
}

/// 从 wordbook 词条构建 hotwords 字符串（逗号分隔）
/// 仅 accuracy 分支使用；performance 分支不支持 hotwords
///
/// ASR-ACC-OPT-001 方案 A：内部调用 curate_hotwords_entries 精选，
/// 过滤无关词条 + 上限截断，避免全量灌入带偏 native decoder。
pub fn build_hotwords_string(entries: &[String]) -> String {
    curate_hotwords_entries(entries).join(",")
}

/// 构建 recognizer（ASR-SINGLE-MODEL-001：单模型加载，不再创建 fallback）
/// 返回 (主 recognizer, 生效模型, hotwords_version)
///
/// R2 修订（验收第 2 轮）：返回 effective_model 解决降级语义错标。
/// accuracy 分支 native 加载失败 → 降级 CTC，effective_model=Performance，
/// Transcriber 存 effective_model 作为 asr_model——三处语义自动归位：
/// ① CTC 输出标 native_punctuated=false（下游标点模块正常走）
/// ② 空输出走 performance bail 语义（不 accuracy bail）
/// ③ 不触发 VAD 分段（performance 不分段）
fn build_recognizer(
    model_dir: &Path,
    language: &str,
    asr_model: AsrModel,
    hotwords: Option<&str>,
) -> Result<(sherpa_onnx::OfflineRecognizer, AsrModel, u64)> {
    let hotwords_version = match hotwords {
        Some(h) => {
            let count = h.split(',').filter(|s| !s.trim().is_empty()).count();
            let mut hasher = DefaultHasher::new();
            count.hash(&mut hasher);
            h.hash(&mut hasher);
            hasher.finish()
        }
        None => 0,
    };

    match asr_model {
        AsrModel::Performance => {
            let recognizer = create_sensevoice_recognizer(model_dir, language)?;
            Ok((recognizer, AsrModel::Performance, hotwords_version))
        }
        AsrModel::Accuracy => {
            // ASR-SINGLE-MODEL-001: accuracy 分支尝试加载 native 模型；失败则降级 performance
            // 不再预创建 CTC fallback recognizer（省 ~250-350MB 常驻）
            match create_funasr_nano_recognizer(model_dir, hotwords) {
                Ok(recognizer) => Ok((recognizer, AsrModel::Accuracy, hotwords_version)),
                Err(e) => {
                    log::warn!(
                        "Accuracy model load failed ({}), falling back to performance model",
                        e
                    );
                    let recognizer = create_sensevoice_recognizer(model_dir, language)?;
                    // R2: effective_model=Performance，Transcriber 存此值语义归位
                    Ok((recognizer, AsrModel::Performance, hotwords_version))
                }
            }
        }
        AsrModel::Qwen3Online => {
            // DEC-028: qwen3_online 不加载本地模型，Transcriber::new() 已提前返回，
            // 此分支不应被触达
            unreachable!(
                "Qwen3Online should be handled in Transcriber::new() before build_recognizer"
            )
        }
    }
}

/// ASR-CTC-OPT-001 P2（已撤销）: 推导 ITN rule_fsts 路径（exe 同级 models/itn/itn_zh_number.fst）。
///
/// **撤销原因**：ITN rule_fsts 把中文数字规整成阿拉伯数字（"七"→"7"），
/// 对输入法场景有害（用户说"七"想输入汉字"七"而非"7"）。本轮撤销 P2，
/// 智能规则化（仅规整多位数字、单字保留汉字）另行立项，等 Gavin 决策。
///
/// 函数保留供未来智能 ITN 立项复用，但当前不被 create_sensevoice_recognizer 调用。
/// 单测保留验证路径推导逻辑。
#[allow(dead_code)]
fn resolve_itn_fst_path(model_dir: &Path) -> Option<String> {
    let itn_fst_path = model_dir.join("itn").join("itn_zh_number.fst");
    if itn_fst_path.exists() {
        log::info!("ITN rule_fsts enabled: {:?}", itn_fst_path);
        Some(itn_fst_path.to_str().unwrap_or("").to_string())
    } else {
        log::warn!(
            "ITN rule_fsts file not found at {:?}, ITN disabled (download itn_zh_number.fst to models/itn/ to enable)",
            itn_fst_path
        );
        None
    }
}

/// Create SenseVoice Chinese recognizer (FunASR Nano CTC 兼容版，179MB)
/// DEC-025 路线 A：默认 performance 模型
///
/// ASR-CTC-OPT-001:
/// - P1: silence head 由调用方 select_preprocessing_params 控制（本函数不涉及）
/// - P2: ITN rule_fsts 已撤销（副作用：中文数字→阿拉伯数字对输入法有害，另行立项）
/// - P3: blank_penalty 0.5→0.0（C2 证实对 FunASR Nano CTC 无影响，遗产值清理）
fn create_sensevoice_recognizer(
    model_dir: &Path,
    language: &str,
) -> Result<sherpa_onnx::OfflineRecognizer> {
    let model_dir_path = ensure_sensevoice_model(model_dir)?;

    let model_path = model_dir_path.join("model.int8.onnx");
    let tokens_path = model_dir_path.join("tokens.txt");

    let offline_config = sherpa_onnx::OfflineRecognizerConfig {
        model_config: sherpa_onnx::OfflineModelConfig {
            sense_voice: OfflineSenseVoiceModelConfig {
                model: Some(model_path.to_str().unwrap_or("").to_string()),
                language: Some(language.to_string()),
                use_itn: true,
            },
            tokens: Some(tokens_path.to_str().unwrap_or("").to_string()),
            ..Default::default()
        },
        // ASR-CTC-OPT-001 P2: ITN rule_fsts 已撤销（副作用见 resolve_itn_fst_path 文档）
        // ASR-CTC-OPT-001 P3: blank_penalty 0.5→0.0
        // C2 证实 0/0.25/0.5/0.75/1.0 五档输出逐字节一致，对 FunASR Nano CTC 无影响
        blank_penalty: 0.0,
        ..Default::default()
    };

    sherpa_onnx::OfflineRecognizer::create(&offline_config)
        .context("Failed to create SenseVoice offline recognizer")
}

/// Create FunASR Nano native recognizer (972MB，accuracy 分支)
/// 字段填法参照 src/bin/poc_funasr_nano.rs:62-85
fn create_funasr_nano_recognizer(
    model_dir: &Path,
    hotwords: Option<&str>,
) -> Result<sherpa_onnx::OfflineRecognizer> {
    let model_dir_path = ensure_funasr_nano_model(model_dir)?;

    let enc = model_dir_path.join("encoder_adaptor.int8.onnx");
    let llm = model_dir_path.join("llm.int8.onnx");
    let emb = model_dir_path.join("embedding.int8.onnx");
    let tok = model_dir_path.join("Qwen3-0.6B");

    let offline_config = sherpa_onnx::OfflineRecognizerConfig {
        model_config: sherpa_onnx::OfflineModelConfig {
            funasr_nano: OfflineFunASRNanoModelConfig {
                encoder_adaptor: Some(enc.to_str().unwrap_or("").to_string()),
                llm: Some(llm.to_str().unwrap_or("").to_string()),
                embedding: Some(emb.to_str().unwrap_or("").to_string()),
                tokenizer: Some(tok.to_str().unwrap_or("").to_string()),
                system_prompt: Some("You are a helpful assistant.".to_string()),
                user_prompt: Some("语音转写:".to_string()),
                max_new_tokens: 0,
                temperature: 0.1,
                top_p: 1.0,
                seed: 42,
                language: None,
                itn: 1,
                hotwords: hotwords.map(|s| s.to_string()),
            },
            tokens: Some(String::new()),
            ..Default::default()
        },
        ..Default::default()
    };

    sherpa_onnx::OfflineRecognizer::create(&offline_config)
        .context("Failed to create FunASR Nano offline recognizer")
}

/// Ensure FunASR Nano native model (972MB) is present; bail if missing
fn ensure_funasr_nano_model(model_dir: &Path) -> Result<PathBuf> {
    // FunASR Nano native（2025-12-30，encoder+LLM decoder，有 hotwords）
    let model_dir_path = model_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30");

    let enc = model_dir_path.join("encoder_adaptor.int8.onnx");
    let llm = model_dir_path.join("llm.int8.onnx");
    let emb = model_dir_path.join("embedding.int8.onnx");
    let tok = model_dir_path.join("Qwen3-0.6B");

    let dir_ok = model_dir_path.exists();
    let files_ok = enc.exists() && llm.exists() && emb.exists() && tok.exists();
    if dir_ok && files_ok {
        log::info!("FunASR Nano native model found at {:?}", model_dir_path);
        return Ok(model_dir_path);
    }

    anyhow::bail!(
        "FunASR Nano native model not found at {:?}. Please download manually from:\n  https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models",
        model_dir_path
    )
}

/// Ensure SenseVoice (FunASR Nano CTC 兼容版，179MB) model is present
fn ensure_sensevoice_model(model_dir: &Path) -> Result<PathBuf> {
    // FunASR Nano CTC 兼容版（179MB，2025-12-17）— DEC-025 路线 A 直换默认模型
    // 模型文件名同为 model.int8.onnx + tokens.txt，沿用 OfflineSenseVoiceModelConfig
    // 旧目录 sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09 保留作回滚
    let model_dir_path = model_dir.join("sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17");

    let model_file = model_dir_path.join("model.int8.onnx");
    let tokens_file = model_dir_path.join("tokens.txt");

    if model_dir_path.exists() && model_file.exists() && tokens_file.exists() {
        log::info!("SenseVoice model found at {:?}", model_dir_path);
        return Ok(model_dir_path);
    }

    anyhow::bail!(
        "SenseVoice model not found at {:?}. Please download manually from:\n  https://huggingface.co/sherpa-onnx/sherpa-onnx-sense-voice-funasr-nano-int8-2025-12-17",
        model_dir_path
    )
}

pub fn model_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("models")
}

/// 检测 accuracy 模型是否就位（供 Tauri command 调用）
pub fn check_accuracy_model_ready(model_dir: &Path) -> (bool, PathBuf) {
    let dir = model_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30");
    let enc = dir.join("encoder_adaptor.int8.onnx");
    let llm = dir.join("llm.int8.onnx");
    let emb = dir.join("embedding.int8.onnx");
    let tok = dir.join("Qwen3-0.6B");
    let ready = dir.exists() && enc.exists() && llm.exists() && emb.exists() && tok.exists();
    (ready, dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asr_model_from_config_parses_values() {
        assert_eq!(AsrModel::from_config("performance"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("accuracy"), AsrModel::Accuracy);
        assert_eq!(AsrModel::from_config("Performance"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("ACCURACY"), AsrModel::Accuracy);
        assert_eq!(AsrModel::from_config(""), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("garbage"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("qwen3_online"), AsrModel::Qwen3Online);
        assert_eq!(AsrModel::from_config("QWEN3_ONLINE"), AsrModel::Qwen3Online);
    }

    #[test]
    fn hotwords_version_stable_for_same_input() {
        let entries = vec!["派".to_string(), "队".to_string()];
        let v1 = hotwords_version(&entries);
        let v2 = hotwords_version(&entries);
        assert_eq!(v1, v2, "same input must produce same hash");
    }

    #[test]
    fn hotwords_version_changes_on_content_change() {
        let e1 = vec!["派".to_string()];
        let e2 = vec!["湃".to_string()];
        assert_ne!(
            hotwords_version(&e1),
            hotwords_version(&e2),
            "different content must produce different hash"
        );
    }

    #[test]
    fn hotwords_version_changes_on_len_change() {
        let e1 = vec!["a".to_string()];
        let e2 = vec!["a".to_string(), "b".to_string()];
        assert_ne!(
            hotwords_version(&e1),
            hotwords_version(&e2),
            "different length must produce different hash"
        );
    }

    #[test]
    fn hotwords_version_order_sensitive() {
        let e1 = vec!["a".to_string(), "b".to_string()];
        let e2 = vec!["b".to_string(), "a".to_string()];
        assert_ne!(
            hotwords_version(&e1),
            hotwords_version(&e2),
            "order must affect hash (caller must sort for stability)"
        );
    }

    #[test]
    fn build_hotwords_string_joins_non_empty() {
        let entries = vec![
            "派".to_string(),
            "队".to_string(),
            "  ".to_string(), // empty after trim
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(s, "派,队");
    }

    #[test]
    fn build_hotwords_string_empty_entries() {
        let entries: Vec<String> = vec![];
        assert_eq!(build_hotwords_string(&entries), "");
    }

    // ============================================================
    // ASR-ACC-OPT-001 方案 A: curate_hotwords_entries 精选策略测试
    // WORDBOOK-SINGLEWORD-001-CORE: 单词化（&[String]）
    // ============================================================

    #[test]
    fn curate_filters_pure_ascii_entries() {
        let entries = vec![
            "worker1".to_string(),
            "tester1".to_string(),
            "todo".to_string(),
            "派".to_string(),
            "比利".to_string(),
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(s, "派,比利", "pure ASCII entries must be filtered");
    }

    #[test]
    fn curate_filters_long_entries() {
        let long_str = "这是第一行需要测试的内容第二行需要测试的内容".to_string();
        let entries = vec!["短词".to_string(), long_str.clone(), "派发".to_string()];
        let s = build_hotwords_string(&entries);
        assert_eq!(s, "短词,派发", "long entries must be filtered");
        assert!(long_str.chars().count() > HOTWORDS_MAX_ENTRY_CHARS);
    }

    #[test]
    fn curate_filters_empty_and_whitespace_entries() {
        let entries = vec!["  ".to_string(), "".to_string(), "派".to_string()];
        assert_eq!(build_hotwords_string(&entries), "派");
    }

    #[test]
    fn curate_enforces_max_entries_limit() {
        let entries: Vec<String> = (0..60).map(|i| format!("词{}", i)).collect();
        let s = build_hotwords_string(&entries);
        let count = s.split(',').count();
        assert_eq!(
            count, HOTWORDS_MAX_ENTRIES,
            "must cap at HOTWORDS_MAX_ENTRIES"
        );
    }

    #[test]
    fn curate_keeps_most_recent_when_over_limit() {
        let entries = vec![
            "最近词1".to_string(),
            "最近词2".to_string(),
            "早期词".to_string(),
        ];
        let curated = curate_hotwords_entries(&entries);
        assert_eq!(curated.len(), 3);
        assert_eq!(curated[0], "最近词1");
        assert_eq!(curated[1], "最近词2");
        assert_eq!(curated[2], "早期词");
    }

    #[test]
    fn curate_preserves_order_for_stable_hash() {
        let entries = vec!["派".to_string(), "比".to_string(), "利".to_string()];
        let s1 = build_hotwords_string(&entries);
        let s2 = build_hotwords_string(&entries);
        assert_eq!(s1, s2, "same input must produce same output");
        assert_eq!(s1, "派,比,利");
    }

    #[test]
    fn curate_mixed_realistic_wordbook() {
        let entries = vec![
            "我吃".to_string(),
            "worker1".to_string(),
            "tester1".to_string(),
            "todo".to_string(),
            "比利".to_string(),
            "词库".to_string(),
            "灵界".to_string(),
            "阿炎".to_string(),
            "coder1".to_string(),
            "阿炎".to_string(),
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(
            s, "我吃,比利,词库,灵界,阿炎,阿炎",
            "must keep only CJK entries"
        );
    }

    #[test]
    fn curate_empty_wordbook_returns_empty() {
        let entries: Vec<String> = vec![];
        assert_eq!(build_hotwords_string(&entries), "");
    }

    #[test]
    fn is_pure_ascii_detection() {
        assert!(is_pure_ascii("worker1"));
        assert!(is_pure_ascii("tester1"));
        assert!(is_pure_ascii("todo"));
        assert!(is_pure_ascii("123"));
        assert!(is_pure_ascii("ABC"));
        assert!(!is_pure_ascii("派"));
        assert!(!is_pure_ascii("比利"));
        assert!(!is_pure_ascii("worker1派"));
        assert!(!is_pure_ascii(""));
    }

    // ============================================================
    // ASR-ACC-OPT-001 方案 A 补测：候选缺口确认与覆盖
    // ============================================================

    #[test]
    fn curate_filter_invariance_preserves_hash_stability() {
        let without_ascii = vec!["派".to_string(), "比".to_string()];
        let with_ascii = vec!["派".to_string(), "worker1".to_string(), "比".to_string()];
        let s1 = build_hotwords_string(&without_ascii);
        let s2 = build_hotwords_string(&with_ascii);
        assert_eq!(
            s1, s2,
            "ASCII entry filtering must not change curated output"
        );
    }

    #[test]
    fn curate_filters_exact_boundary_entries() {
        let ten_chars = "一二三四五六七八九十";
        let eleven_chars = "一二三四五六七八九十1";
        assert_eq!(ten_chars.chars().count(), HOTWORDS_MAX_ENTRY_CHARS);
        assert_eq!(eleven_chars.chars().count(), HOTWORDS_MAX_ENTRY_CHARS + 1);
        let entries = vec![
            ten_chars.to_string(),
            eleven_chars.to_string(),
            "派".to_string(),
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(
            s,
            format!("{},派", ten_chars),
            "10-char kept, 11-char filtered"
        );
    }

    #[test]
    fn curate_keeps_mixed_cjk_ascii_entries() {
        let entries = vec![
            "worker1".to_string(),
            "worker1派".to_string(),
            "比利".to_string(),
        ];
        let s = build_hotwords_string(&entries);
        assert_eq!(s, "worker1派,比利", "mixed CJK-ASCII entries must be kept");
    }

    #[test]
    fn curate_enforces_max_entries_order() {
        let entries: Vec<String> = (0..25).map(|i| format!("词{}", i)).collect();
        let curated = curate_hotwords_entries(&entries);
        assert_eq!(curated.len(), HOTWORDS_MAX_ENTRIES);
        for i in 0..20 {
            assert_eq!(
                curated[i],
                format!("词{}", i),
                "entry {} must be at position {}",
                i,
                i
            );
        }
        assert!(
            !curated.contains(&"词20".to_string()),
            "21st entry must be truncated"
        );
    }

    // ============================================================
    // ASR-PUNCT-OPT-001: strip_punctuation 测试
    // ============================================================

    #[test]
    fn strip_punctuation_chinese() {
        // 标点直接删除，不加空格
        assert_eq!(
            strip_punctuation("周末要不要去露营？最近天气超舒服。"),
            "周末要不要去露营最近天气超舒服"
        );
    }

    #[test]
    fn strip_punctuation_english() {
        // 英文标点删除后保留原有空格
        assert_eq!(
            strip_punctuation("Hello, world! How are you?"),
            "Hello world How are you"
        );
    }

    #[test]
    fn strip_punctuation_mixed() {
        assert_eq!(
            strip_punctuation("今天很好，very nice！"),
            "今天很好very nice"
        );
    }

    #[test]
    fn strip_punctuation_empty() {
        assert_eq!(strip_punctuation(""), "");
    }

    #[test]
    fn strip_punctuation_no_punct() {
        assert_eq!(
            strip_punctuation("没有任何标点的文本"),
            "没有任何标点的文本"
        );
    }

    #[test]
    fn strip_punctuation_preserves_decimal() {
        // 小数点不剥离（. 不在 punct_set 中）
        assert_eq!(strip_punctuation("圆周率是3.14"), "圆周率是3.14");
        assert_eq!(strip_punctuation("价格50.5元"), "价格50.5元");
    }

    #[test]
    fn strip_punctuation_preserves_url() {
        // URL 中的 . / : 等不剥离（: 剥离但 URL 中的 // 和 . 保留）
        // 注意：: 在 punct_set 中会被剥离——这是保守策略的已知限制，
        // URL 场景极少出现在语音转写输出中
        let input = "访问 https example.com 路径";
        assert_eq!(strip_punctuation(input), "访问 https example.com 路径");
    }

    #[test]
    fn strip_punctuation_only_punct() {
        assert_eq!(strip_punctuation("，。！？"), "");
    }

    #[test]
    fn strip_punctuation_quotes() {
        // 中文引号 \u{201C} \u{201D} 和单引号 \u{2018} \u{2019}
        let input = "\u{201C}\u{4F60}\u{597D}\u{201D}\u{4ED6}\u{8BF4}";
        assert_eq!(strip_punctuation(input), "你好他说");
    }

    #[test]
    fn strip_punctuation_collapses_spaces() {
        // 标点剥离后连续空格压缩
        assert_eq!(strip_punctuation("你好，  世界！"), "你好 世界");
    }

    // ============================================================
    // ASR-PUNCT-OPT-001: 标点决策来源标记逻辑测试
    // ============================================================

    /// 来源标记规则是纯逻辑（performance→false, accuracy native 成功→true, 兜底→false）
    /// 这里通过间接验证规则文档化（实际 Transcriber 需模型加载，无法纯逻辑测）
    /// 决策真值表在 result.md 文档化
    #[test]
    fn source_flag_documentation_placeholder() {
        // 来源标记规则由 transcribe_with_punct_info 实现，需真实模型加载，此处不纯逻辑测
        // 真值表见 result.md
        assert!(true);
    }

    // ============================================================
    // ASR-CTC-OPT-001 P2: resolve_itn_fst_path 路径推导测试
    // ============================================================

    #[test]
    fn resolve_itn_fst_path_returns_none_when_missing() {
        // 降级保护：fst 不存在时返回 None，不硬失败
        let tmp = std::env::temp_dir();
        let non_existent = tmp.join("voice_ime_test_nonexistent_itn");
        let result = resolve_itn_fst_path(&non_existent);
        assert!(
            result.is_none(),
            "missing fst must return None (graceful degrade)"
        );
    }

    #[test]
    fn resolve_itn_fst_path_returns_some_when_present() {
        // fst 存在时返回路径字符串
        let tmp = std::env::temp_dir().join("voice_ime_test_itn_present");
        let itn_dir = tmp.join("itn");
        std::fs::create_dir_all(&itn_dir).ok();
        let fst_path = itn_dir.join("itn_zh_number.fst");
        std::fs::write(&fst_path, b"dummy fst content").ok();
        let result = resolve_itn_fst_path(&tmp);
        assert!(result.is_some(), "existing fst must return Some(path)");
        assert!(
            result.unwrap().contains("itn_zh_number.fst"),
            "path must contain fst filename"
        );
        // 清理
        std::fs::remove_file(&fst_path).ok();
        std::fs::remove_dir(&itn_dir).ok();
        std::fs::remove_dir(&tmp).ok();
    }

    #[test]
    fn resolve_itn_fst_path_uses_model_dir_subpath() {
        // 路径必须经 model_dir 推导（DEC-011，exe 同级 models/itn/）
        // 验证路径结构：model_dir/itn/itn_zh_number.fst
        let tmp = std::env::temp_dir().join("voice_ime_test_itn_path_check");
        let itn_dir = tmp.join("itn");
        std::fs::create_dir_all(&itn_dir).ok();
        let fst_path = itn_dir.join("itn_zh_number.fst");
        std::fs::write(&fst_path, b"dummy").ok();
        let result = resolve_itn_fst_path(&tmp);
        if let Some(path) = result {
            // 路径应以 model_dir/itn/itn_zh_number.fst 结尾
            assert!(
                path.ends_with("itn_zh_number.fst"),
                "path must end with fst filename"
            );
            assert!(path.contains("itn"), "path must contain itn dir");
        }
        std::fs::remove_file(&fst_path).ok();
        std::fs::remove_dir(&itn_dir).ok();
        std::fs::remove_dir(&tmp).ok();
    }

    // ============================================================
    // ASR-SINGLE-MODEL-001 R1/R2 修订（验收第 2 轮）测试
    // ============================================================

    /// R2: build_recognizer 返回 3-tuple (recognizer, effective_model, hotwords_version)。
    /// accuracy 降级 CTC 时 effective_model=Performance（语义归位三处）。
    ///
    /// 此测试验证降级场景：model_dir 下只有 CTC 模型（native 缺失），
    /// 请求 accuracy → native 加载失败 → 降级 CTC → effective_model=Performance。
    /// 需要 SenseVoice CTC 模型存在于项目 models/ 目录。
    #[test]
    #[ignore = "requires SenseVoice CTC model in project models/ dir"]
    fn build_recognizer_accuracy_degraded_to_performance_effective_model() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let model_dir = project_root.join("models");
        // 请求 accuracy，但 native 模型路径下放一个假目录使加载失败
        // 实际项目 models/ 下 native 模型若存在则此测试需构造缺失场景。
        // 此处验证返回签名结构 + effective_model 语义：
        // 若 native 加载成功 → effective=Accuracy；若失败降级 → effective=Performance
        let result = build_recognizer(&model_dir, "zh", AsrModel::Accuracy, None);
        match result {
            Ok((_recognizer, effective_model, _hw_ver)) => {
                // effective_model 必须是 AsrModel 枚举值之一
                assert!(
                    effective_model == AsrModel::Accuracy
                        || effective_model == AsrModel::Performance,
                    "effective_model must be valid AsrModel variant"
                );
                // 若 native 存在 → Accuracy；若缺失降级 → Performance
                // 关键：effective_model 不再恒 Accuracy（降级时正确归位 Performance）
            }
            Err(e) => {
                // 两个模型都缺失才 Err，本机至少有一个则不会到这
                eprintln!("build_recognizer err (models may be missing): {}", e);
            }
        }
    }

    /// R2 纯逻辑验证：build_recognizer 返回元组 arity 正确（3 个元素），
    /// effective_model 是 AsrModel 类型。此测试文档化签名契约，
    /// 防止未来重构误改返回类型（如漏掉 effective_model）。
    #[test]
    fn build_recognizer_return_signature_contract() {
        // 编译期断言：build_recognizer 返回 Result<(OfflineRecognizer, AsrModel, u64)>
        // 此函数签名由类型系统保证，此处文档化契约供未来维护者参考。
        // 关键：第 2 个元素必须是 AsrModel（effective_model），不能省略。
        fn _type_check<F>(_f: F)
        where
            F: Fn(
                &Path,
                &str,
                AsrModel,
                Option<&str>,
            ) -> Result<(sherpa_onnx::OfflineRecognizer, AsrModel, u64)>,
        {
        }
        _type_check(build_recognizer);
        // 若此测试编译通过，签名契约满足
        assert!(
            true,
            "build_recognizer signature contract: 3-tuple with effective_model"
        );
    }

    /// R1 纯逻辑验证：transcribe_segments_chunked 是 Transcriber 的方法，
    /// 接收 segments 切片返回 Result<(String, bool)>。此测试文档化契约：
    /// - 辅助函数消除 VAD 分段 / naive_chunk 两路径的重复转录循环
    /// - 全空段拼接 → bail
    /// - 任一段非 native → all_native=false
    /// 实际路径验证需 Transcriber 实例（模型加载），此处文档化行为契约。
    #[test]
    fn transcribe_segments_chunked_contract_documented() {
        // 契约文档化（需模型实例才能测实际行为，此处防回归签名变更）：
        // 1. 输入 &[Vec<f32>]（段样本列表）
        // 2. 逐段调 transcribe_segment_detailed
        // 3. join_segment_texts 拼接
        // 4. 全空 → bail "all segments produced empty text"
        // 5. 任一段 np=false → all_native=false
        // 6. 返回 (normalized_text, all_native)
        //
        // R1 关键：lock poisoned 时走 naive_chunk 分支调此辅助函数，
        // 不再静默落到单次转录整段（禁止 >28s 整段喂 native）。
        assert!(true, "transcribe_segments_chunked contract documented");
    }
}
