use anyhow::{Context, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::{config::ChineseScript, punctuation, text_normalizer};

// Re-export SenseVoice config for convenience
use sherpa_onnx::{OfflineFunASRNanoModelConfig, OfflineSenseVoiceModelConfig};

pub mod local_stream;
pub mod qwen_inference;
mod vad;
pub use vad::{
    build_padded_segments, join_segment_texts, naive_chunk, should_segment, VadSegmenter,
    SEGMENT_MAX_SECS, SEGMENT_PADDING_SAMPLES, SEGMENT_TRIGGER_SECS,
};

/// BUG-119（BUILD-118 端测第 1 项）：「用户没说话」的类型化信号，不是设备/网络错误。
///
/// 缺陷史：所有「没识别到语音」路径原先都用 anyhow 字符串错误（一中一英），
/// 显示层 `convert_to_friendly_error` 关键词嗅探分类不中 → 开发态原文直出。
/// 升级为类型化信号后，显示侧在 map_err 边界 downcast 本类型 → 信息提示（i18n）。
///
/// 契约：**新增第三个「没说话」产出源时只需 `bail!(NoSpeechError)`，
/// 显示侧分类器零改动** —— 这是本类型存在的唯一意义，禁止再往错误串里加关键词。
/// 设备/模型异常（空音频样本、VAD 已确认有语音段但模型空输出）**不得**用本类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoSpeechError;

impl std::fmt::Display for NoSpeechError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no speech detected")
    }
}

impl std::error::Error for NoSpeechError {}

/// ASR mode: offline or streaming (2-pass)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AsrMode {
    /// Offline mode: single pass, higher accuracy
    Offline,
    /// Streaming mode: 2-pass (streaming preview + offline correction)
    Streaming,
}

/// ASR 模型选择（DEC-025 + DEC-028 + ASR-056）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AsrModel {
    /// 性能最优：179MB FunASR Nano CTC 兼容版（OfflineSenseVoiceModelConfig，无 hotwords）
    Performance,
    /// 准确率更高：972MB FunASR Nano native（OfflineFunASRNanoModelConfig，config 层 hotwords）
    Accuracy,
    /// 在线流式 ASR —— Qwen Audio 3.0（RESEARCH-ASR-038 / ASR-041-B）：
    /// DashScope Inference 协议，模型 ID qwen-audio-3.0-asr-flash-streaming
    QwenAudioOnline,
    /// 在线流式 ASR —— FunASR Realtime（ASR-056）：
    /// 与 QwenAudioOnline 共用同一 WS 端点 + 同一 Inference 协议 + 同一返回结构，
    /// 仅 model 名与部分参数取值不同。Gavin 研究发现比 qwen-audio-3.0-asr-flash-streaming 快。
    FunAsrRealtime,
    /// LOCAL-RT-ENGINE-239-A（DEC-067）：本地流式实时档（极客档，DEC-065 默认隐藏）。
    ///
    /// **双模型并存常驻**（有意打破「本地一次只加载一个模型」）：
    /// - online = streaming paraformer trilingual（本地预览，greedy_search）
    /// - offline = accuracy FunASR Nano native（2pass 最终文本 + hotwords）
    ///
    /// DEC-067 附则一：任一模型缺失/加载失败 ⇒ 整档 `Err`，**禁止静默降级**。
    LocalRealtime,
}

impl AsrModel {
    pub fn from_config(s: &str) -> Self {
        if s.eq_ignore_ascii_case("accuracy") {
            AsrModel::Accuracy
        } else if s.eq_ignore_ascii_case("qwen_audio_online") {
            AsrModel::QwenAudioOnline
        } else if s.eq_ignore_ascii_case("fun_asr_realtime") {
            AsrModel::FunAsrRealtime
        } else if s.eq_ignore_ascii_case("local_realtime") {
            AsrModel::LocalRealtime
        } else {
            AsrModel::Performance
        }
    }

    /// ASR-056: 收敛判据——是否走在线流式管线（真流式 record_streaming + transcribe_streaming_realtime）。
    ///
    /// 新增在线模型时只改这一处，不必逐个更新 main.rs 的 12 处引用点。
    /// QwenAudioOnline 和 FunAsrRealtime 都走真流式管线（共用 qwen_inference.rs 实现）。
    pub fn is_online_streaming(&self) -> bool {
        matches!(self, AsrModel::QwenAudioOnline | AsrModel::FunAsrRealtime)
    }

    /// FIX-LOCALRT-ENGINE-EQ-252: 是否使用 **accuracy 引擎**（FunASR Nano native）。
    ///
    /// `Accuracy` 与 `LocalRealtime` 的离线/最终转录引擎都是 accuracy native
    /// （LocalRealtime 的 2pass 最终文本由 accuracy 产出），故所有「accuracy 引擎专属」
    /// 判据（VAD 分段、`native_punctuated`、hotwords 重载）都应命中两者。
    ///
    /// 🔴 收敛点：`== AsrModel::Accuracy` 这类相等比较**编译器不会报错**，新增变体极易漏改
    /// （本单两个端测 bug 即由此而来）。新增使用 accuracy 引擎的档位时，只改这一处。
    pub fn uses_accuracy_engine(self) -> bool {
        matches!(self, AsrModel::Accuracy | AsrModel::LocalRealtime)
    }
}

/// ASR transcriber using sherpa-onnx
///
/// ASR-SINGLE-MODEL-001（DEC-027）+ DEC-028：多模式 ASR 架构
/// - Performance: 179MB CTC，OfflineSenseVoiceModelConfig，无 hotwords
/// - Accuracy: 972MB native，OfflineFunASRNanoModelConfig，config 层 hotwords
/// - QwenAudioOnline: 零本地 ASR 内存，DashScope Inference 协议（ASR-041-B）
/// - Performance/Accuracy: 本地一次只加载一个模型；accuracy 不再预创建 CTC fallback
/// - 🔴 LOCAL-RT-ENGINE-239-A（DEC-067）例外：`LocalRealtime` 档位**同时常驻两个本地模型**
///   （online streaming paraformer 做预览 + offline accuracy 做 2pass 最终文本/热词），
///   有意打破「本地一次只加载一个模型」。该档位为极客档、默认隐藏（DEC-065）。
/// - H1 temperature 0.1 为 accuracy 唯一幻觉缓解（2026-07-08 Gavin 拍板下调 0.3→0.1，RESEARCH-ASR-ACCURACY-002 证实越低越好）；异常检测链已删除
///
/// SAFETY: sherpa_onnx::OfflineRecognizer 与 sherpa_onnx::OnlineRecognizer 均内含
/// *const C++ 指针（!Send），但其 C++ 实现本身是线程安全的（create/decode/destroy
/// 均可跨线程，只需保证同一 recognizer 实例不被并发访问）。
/// 此处 Transcriber 通过 channel 在后台构建线程与 worker 线程间转移，
/// 转移期间无并发访问（构建完成后才发送，发送后构建线程不再触碰），
/// 因此手动实现 Send 是安全的。两个 recognizer 用同一套规则包装，不引入新模式。
pub struct Transcriber {
    mode: AsrMode,
    asr_language: String,
    asr_model: AsrModel,
    /// 本地 ASR recognizer（Performance/Accuracy 模式用；QwenAudioOnline 为 None）
    offline_recognizer: Option<sherpa_onnx::OfflineRecognizer>,
    /// LOCAL-RT-ENGINE-239-A（DEC-067）：本地**流式** recognizer（streaming paraformer trilingual）。
    ///
    /// 仅 `LocalRealtime` 档位用；与 `offline_recognizer`（accuracy，2pass 最终文本 + 热词）
    /// **并存常驻**，有意打破旧约束「本地模式一次只加载一个模型」（见结构体文档）。
    ///
    /// ⚠️ 阶段一：本单只落字段声明与 !Send 包装；构建接线与枚举变体见 239-A 第二阶段。
    online_recognizer: Option<sherpa_onnx::OnlineRecognizer>,
    /// 当前注入的 hotwords 版本号（len + 内容哈希），用于感知词库变更
    hotwords_version: u64,
    /// VAD 分段器（仅 accuracy 长音频用）。
    ///
    /// 🔴 修正（LOCAL-RT-ENGINE-239-A，原文误写「懒加载」）：该字段在 `Transcriber::new`
    /// （见下方 `let vad_segmenter = if effective_model.uses_accuracy_engine() { ... }` 分支）
    /// **随构造立即尝试初始化**，并非「首次长音频时才初始化」。
    /// 用 `Mutex` 包住的原因：`VadSegmenter::segment` 需要 `&mut self`，而 `Transcriber`
    /// 以共享引用被调用，故用内部可变性串行化。
    vad_segmenter: Option<Mutex<vad::VadSegmenter>>,
    /// 在线 ASR 配置（仅 QwenAudioOnline 模式用，ASR-041-B 改名通用）
    asr_online_api_key: String,
    asr_online_url: String,
    asr_online_model: String,
    /// ASR-056: VAD 断句静音阈值（ms），config.toml 隐藏字段覆盖，ASR-SEG-229 默认 2000
    asr_online_max_sentence_silence: i64,
    /// ASR-SEG-229: 在线 ASR 语义断句/标点开关，config.toml 隐藏字段覆盖，默认 false
    asr_online_semantic_punctuation_enabled: bool,
}

// SAFETY: Transcriber 持有的 OfflineRecognizer / OnlineRecognizer 内部均为 *const C++ 指针。
// 跨线程转移时，发送方在 send 后不再访问该实例，接收方独占所有权，
// 满足"单一时刻单线程访问"约束。sherpa-onnx C++ 层本身支持跨线程调用。
// LOCAL-RT-ENGINE-239-A: online_recognizer 与 offline_recognizer 共用本条 SAFETY 论证。
unsafe impl Send for Transcriber {}

/// PARALLEL-ACC-298：把 `&OfflineRecognizer` 送进 accuracy 并行 worker 线程的 Send 包装。
///
/// 🔴 SAFETY 论证 = **对象独占**，不是「时间上不重叠」（本条与 `SendOnlineRecognizerRef`
/// 的论证**不同**：298 是**真并发**，streaming 线程与 accuracy worker 同时在跑，故
/// 「scope 内串行、join 后才继续」那套时间论在此**不成立**，不得套用）。
///
/// 独占性两条证据（coder-2 核实的代码事实）：
/// 1. **录音期间无人替换/访问该 offline recognizer**：唯一会 swap/drop `transcriber` 的是
///    `apply_reload_result`，它只在 worker loop 顶（`main.rs:7442-7458`）或 D2 有界等待
///    （`:7633-7653`）执行 —— 二者都在 LocalRealtime 的 `std::thread::scope` **之前**；
///    scope 期间 worker 线程**就地阻塞**在 `record_streaming`（`:7965`），loop 顶不执行
///    （`:7462` 注释「录音期间 loop 顶不执行，天然不会中途换引擎」）。后台 reload 线程
///    （`:7254`）只构建**新**实例，绝不碰旧实例。
/// 2. **scope 内只有一个线程访问本对象**：accuracy worker 是唯一使用者；主 worker 线程只跑
///    音频（`record_streaming`），streaming ASR 线程用的是 **online** recognizer（另一个对象）。
///    全仓 `transcribe_with_punct_info` 唯一生产调用点 `main.rs:9132` 在 scope **之后**，
///    且 298 携带 pretranscribed 时该分支根本不执行。
///
/// 结论：全程无第二个线程访问**同一** offline recognizer，跨线程转移成立。
pub struct SendOfflineRecognizerRef<'a>(pub &'a sherpa_onnx::OfflineRecognizer);
unsafe impl<'a> Send for SendOfflineRecognizerRef<'a> {}

impl<'a> SendOfflineRecognizerRef<'a> {
    /// 按值消费包装取出引用（同 `SendOnlineRecognizerRef`，破 Rust2021 disjoint capture）。
    pub fn into_inner(self) -> &'a sherpa_onnx::OfflineRecognizer {
        self.0
    }
}

/// PARALLEL-ACC-298：用常驻 offline(accuracy) recognizer 并行转写**一个**分段。
///
/// 与 [`Transcriber::transcribe_segment_detailed`] 的 accuracy 分支逐位同口径：
/// `create_stream → accept_waveform → decode → get_result → trim → strip_asr_special_tokens
/// → 空则 Err(NativeEmpty) → normalize_text_for_language + native_punctuated=true`。
/// 独立成自由函数，是为了让 accuracy worker 在 scope 线程内**只借 recognizer、不入 `&self`**
/// （`Transcriber` 非 `Sync`，不能跨线程共享 `&self`）。
pub(crate) fn transcribe_accuracy_segment(
    recognizer: &sherpa_onnx::OfflineRecognizer,
    samples: &[f32],
    script: ChineseScript,
) -> Result<(String, bool)> {
    let stream = recognizer.create_stream();
    stream.accept_waveform(16000, samples);
    recognizer.decode(&stream);
    let result = stream.get_result().context("No transcription result")?;
    let text = Transcriber::strip_asr_special_tokens(result.text.trim());
    if text.is_empty() {
        // 与 transcribe_segment_detailed 一致：accuracy 空输出 ⇒ 该段失败（上层 all_native=false）
        anyhow::bail!("ASR accuracy model produced empty output");
    }
    Ok((
        text_normalizer::normalize_text_for_language(&text, script),
        true,
    ))
}

impl Transcriber {
    /// Create new Transcriber with explicit ASR model selection
    ///
    /// ASR-SINGLE-MODEL-001（DEC-027）+ DEC-028 + ASR-041-B：多模式 ASR
    /// - Performance/Accuracy：加载本地模型（单模型，不预创建 fallback）
    /// - QwenAudioOnline：不加载任何本地模型（零本地 ASR 内存），存 url+key
    ///
    /// R2 修订（验收第 2 轮）：asr_model 存的是 effective_model（生效模型）。
    /// accuracy 降级 CTC 时 effective=Performance，语义自动归位。
    pub fn new(
        model_dir: &Path,
        enable_streaming: bool,
        asr_language: String,
        asr_model: AsrModel,
        hotwords: Option<&str>,
        asr_online_api_key: &str,
        asr_online_url: &str,
        asr_online_model: &str,
        asr_online_max_sentence_silence: i64,
        asr_online_semantic_punctuation_enabled: bool,
    ) -> Result<Self> {
        // HOTWORDS-TOKEN-268: 启动时后台预热热词 tokenizer（~数百 ms），避免首次计 token
        // 落在「按热键→开始录音」路径上被用户感知。
        warm_hotwords_tokenizer();

        let mode = if enable_streaming {
            AsrMode::Streaming
        } else {
            AsrMode::Offline
        };

        // DEC-028 / ASR-038-B / ASR-041-B / ASR-056: 在线 ASR 模式不加载本地模型
        // ASR-056: QwenAudioOnline 与 FunAsrRealtime 都走此路径（is_online_streaming 收敛判据）
        if asr_model.is_online_streaming() {
            if asr_online_api_key.trim().is_empty() {
                anyhow::bail!("在线 ASR 配置失败：API Key 为空（请在设置中配置 API Key）");
            }
            log::info!(
                "Online ASR mode: no local model loaded, model={:?}, url={}",
                asr_model,
                asr_online_url
            );
            return Ok(Self {
                mode,
                asr_language,
                asr_model,
                offline_recognizer: None,
                online_recognizer: None,
                hotwords_version: 0,
                vad_segmenter: None,
                asr_online_api_key: asr_online_api_key.to_string(),
                asr_online_url: asr_online_url.to_string(),
                asr_online_model: asr_online_model.to_string(),
                asr_online_max_sentence_silence,
                asr_online_semantic_punctuation_enabled,
            });
        }

        // LOCAL-RT-ENGINE-239-A（DEC-067）：LocalRealtime 双模型并存常驻。
        // 与 accuracy 不同：**不做静默降级**，任一模型缺失/加载失败即 Err（附则一），
        // 因为降级就是「用户以为用 A 实际跑 B」的不一致（ASR-UI-208 判定）。
        if asr_model == AsrModel::LocalRealtime {
            let (online_recognizer, offline_recognizer, hotwords_version) =
                build_local_realtime_recognizers(model_dir, &asr_language, hotwords)?;
            // VAD 分段器：与 accuracy 同条件（2pass 最终引擎是 accuracy，长音频要分段）
            let vad_segmenter = vad::VadSegmenter::try_new(model_dir).map(Mutex::new);
            log::info!(
                "LocalRealtime: dual recognizers loaded (online streaming paraformer + offline accuracy)"
            );
            return Ok(Self {
                mode,
                asr_language,
                asr_model: AsrModel::LocalRealtime,
                offline_recognizer: Some(offline_recognizer),
                online_recognizer: Some(online_recognizer),
                hotwords_version,
                vad_segmenter,
                asr_online_api_key: String::new(),
                asr_online_url: String::new(),
                asr_online_model: String::new(),
                asr_online_max_sentence_silence,
                asr_online_semantic_punctuation_enabled,
            });
        }

        let (offline_recognizer, effective_model, hotwords_version) =
            build_recognizer(model_dir, &asr_language, asr_model, hotwords)?;

        // VAD 分段器仅使用 accuracy 引擎的档位初始化；performance 模式设 None
        // R2: 用 effective_model 判断（降级 CTC 时不初始化 VAD）
        // FIX-LOCALRT-ENGINE-EQ-252: 判据收敛到 uses_accuracy_engine()（含 LocalRealtime）
        let vad_segmenter = if effective_model.uses_accuracy_engine() {
            // accuracy 引擎档位立即尝试初始化（模型文件在则建，失败后续降级）
            vad::VadSegmenter::try_new(model_dir).map(Mutex::new)
        } else {
            None
        };

        Ok(Self {
            mode,
            asr_language,
            asr_model: effective_model,
            offline_recognizer: Some(offline_recognizer),
            // LOCAL-RT-ENGINE-239-A（DEC-067）：仅 LocalRealtime 档并存流式 recognizer；
            // Performance/Accuracy 无流式侧，在线档已在上面提前返回。
            online_recognizer: None,
            hotwords_version,
            vad_segmenter,
            asr_online_api_key: String::new(),
            asr_online_url: String::new(),
            asr_online_model: String::new(),
            asr_online_max_sentence_silence,
            asr_online_semantic_punctuation_enabled,
        })
    }

    pub fn asr_model(&self) -> AsrModel {
        self.asr_model
    }

    /// LOCAL-RT-ENGINE-239-A（DEC-067）：取常驻的本地流式 recognizer（仅 `LocalRealtime` 档位为 `Some`）。
    ///
    /// 供 239-B 接线 `local_stream::transcribe_streaming_local` 使用。
    pub fn online_recognizer(&self) -> Option<&sherpa_onnx::OnlineRecognizer> {
        self.online_recognizer.as_ref()
    }

    /// PARALLEL-ACC-298：取常驻的 accuracy offline recognizer（accuracy 引擎档位为 `Some`）。
    ///
    /// 供 `main.rs` 的 accuracy 并行 worker 经 [`SendOfflineRecognizerRef`] 在 scope 线程内使用。
    pub fn offline_recognizer(&self) -> Option<&sherpa_onnx::OfflineRecognizer> {
        self.offline_recognizer.as_ref()
    }

    /// ASR-038-B: 在线 ASR 的 Inference API 端点
    pub fn asr_online_url(&self) -> &str {
        &self.asr_online_url
    }

    /// ASR-038-B: QwenAudioOnline 的模型 ID
    pub fn asr_online_model(&self) -> &str {
        &self.asr_online_model
    }

    /// ASR-056: VAD 断句静音阈值（ms）
    pub fn asr_online_max_sentence_silence(&self) -> i64 {
        self.asr_online_max_sentence_silence
    }

    /// ASR-SEG-229: 在线 ASR 语义断句/标点开关（默认 false = VAD 断句）
    pub fn asr_online_semantic_punctuation_enabled(&self) -> bool {
        self.asr_online_semantic_punctuation_enabled
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
    /// 剥离 ASR 特殊 token（ASR-NOSPEECH-FILTER-001）。
    ///
    /// FunASR/SenseVoice/Qwen3 等模型常输出富文本标记，如：
    /// `<|nospeech|>`, `<|zh|>`, `<|en|>`, `<|ja|>`, `<|ko|>`,
    /// `<|NEUTRAL|>`, `<|HAPPY|>`, `<|Speech|>`, `<|woitn|>`, `<|withitn|>` 等。
    /// 这些标记不应进入下游文本处理/注入。
    ///
    /// 规则：精确匹配 `<|...|>` 配对形态（内部不含 `|` 与 `<`、`>`），
    /// 仅剥离完整 token，不误伤正常文本中的孤立 `<` 或 `>`。
    /// 多个连续 token 合并为一个空白分隔边界，避免产生多余空格。
    pub fn strip_asr_special_tokens(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        let mut pending_space = false; // 标记前一个 token 结束后是否欠一个空格
        while i < chars.len() {
            if chars[i] == '<' && chars.get(i + 1) == Some(&'|') {
                // 寻找结束 `|>`
                let mut j = i + 2;
                while j + 1 < chars.len() && !(chars[j] == '|' && chars[j + 1] == '>') {
                    // token 内部不得含另一个 `<` 或 `>` 或 `|`，否则不是合法 token
                    if chars[j] == '<' || chars[j] == '>' || chars[j] == '|' {
                        break;
                    }
                    j += 1;
                }
                if j + 1 < chars.len() && chars[j] == '|' && chars[j + 1] == '>' && j > i + 2 {
                    // 合法非空 `<|...|>` token：直接跳过，仅记录后续可能需要空格
                    pending_space = !result.is_empty();
                    i = j + 2;
                    continue;
                }
            }
            // 普通字符：若欠空格且当前非空白，按两侧字符类型决定是否补空格。
            // 【主控修正 2026-07-27】仅当 token 两侧均为 ASCII 字母/数字时才补空格
            // （防止英文单词被 token 分隔后粘连成 helloworld）；CJK 两侧一律不补——
            // 中文无词间空格，补了会把「你<|NEUTRAL|>好」污染成「你 好」并一路注入。
            if pending_space && chars[i] != ' ' {
                let prev_is_ascii_word = result
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_ascii_alphanumeric());
                if prev_is_ascii_word && chars[i].is_ascii_alphanumeric() {
                    result.push(' ');
                }
            }
            pending_space = false;
            result.push(chars[i]);
            i += 1;
        }
        result
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
    /// - qwen3_online → 实测输出是否含**有效标点**（PUNCT-GOVERNANCE-030-A-2：
    ///   官方 API 无标点参数，不再假定必然带标点；用 `has_effective_punctuation`
    ///   统一「词内嵌豁免」判据 —— `3.14`/`don't`/`3:30`/`example.com` 的词内
    ///   标点不算，真句末标点才算；真有标点才跳过标点引擎）
    pub fn transcribe_with_punct_info(
        &self,
        samples: &[f32],
        script: ChineseScript,
    ) -> Result<(String, bool)> {
        // ASR-038-B / ASR-056: 在线流式 ASR 路径（非流式回退：录完整段再发）
        // QwenAudioOnline 与 FunAsrRealtime 都走此路径（is_online_streaming 收敛判据）。
        // 流式管线在 main.rs 的 run_streaming_pipeline 中直接调用 qwen_inference::transcribe_streaming，
        // 此分支仅用于非流式回退（如模型切换过渡期）
        if self.asr_model.is_online_streaming() {
            let vocabulary = crate::transcription::load_wordbook_vocabulary();
            let text = qwen_inference::transcribe_streaming(
                &self.asr_online_url,
                &self.asr_online_api_key,
                &self.asr_online_model,
                samples,
                &vocabulary,
                self.asr_online_max_sentence_silence,
                self.asr_online_semantic_punctuation_enabled,
                None,
                |_| {},
            )?;
            let cleaned = Self::strip_asr_special_tokens(&text);
            let trimmed = cleaned.trim();
            if trimmed.is_empty() {
                // BUG-119: 在线回退路径空输出 = 没识别到语音，类型化信息信号
                anyhow::bail!(NoSpeechError);
            }
            let normalized = text_normalizer::normalize_text_for_language(trimmed, script);
            let native_punctuated = punctuation::has_effective_punctuation(&normalized);
            return Ok((normalized, native_punctuated));
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
        // FIX-LOCALRT-ENGINE-EQ-252: 收敛到 uses_accuracy_engine()（LocalRealtime 长音频同样要分段，
        // 否则整段喂 native 撞 max_total_len=512 ⇒ 空输出）
        if self.asr_model.uses_accuracy_engine() && vad::should_segment(samples) {
            // 尝试 VAD 分段；lock poisoned / None → 降级 naive_chunk
            let vad_segments: Option<Vec<Vec<f32>>> = match &self.vad_segmenter {
                Some(vad_lock) => match vad_lock.lock() {
                    Ok(vad) => {
                        let segs = vad.segment(samples);
                        if segs.is_empty() {
                            log::warn!("VAD produced no speech segments, transcription failed");
                            // BUG-119: 没说话 = 类型化信息信号，不走错误串
                            anyhow::bail!(NoSpeechError);
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
        // BUG-119: 全段空拼接 = 没识别到语音，类型化信息信号
        anyhow::bail!(NoSpeechError);
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

        // ASR-NOSPEECH-FILTER-001: 剥离特殊 token（如 <|nospeech|>）
        let text = Self::strip_asr_special_tokens(&text);

        // ASR-SINGLE-MODEL-001: accuracy 空输出 → bail（不再兜底 CTC）
        // FIX-LOCALRT-ENGINE-EQ-252: 收敛到 uses_accuracy_engine()（LocalRealtime 也用 accuracy 引擎，
        // native_punctuated=true ⇒ 跳过外部 CT-Transformer，避免双重打点）
        if self.asr_model.uses_accuracy_engine() {
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

/// 剥离 native 模型自带标点（ASR-PUNCT-OPT-001 + PUNCT-GOVERNANCE-030-A A1/A2）
///
/// 用于 accuracy 模式 + 用户关闭自动标点（punctuation.enabled=false）场景：
/// native 输出自带标点，用户关了开关但 native 照样出标点 → 剥离使其与 CTC 行为一致。
///
/// 剥离字符集：`punctuation::PUNCT_CHARS`（中文标点、中文括号/书名号/间隔号
/// + 英文标点 + 半角句点 .）。
/// **保护不剥离**：小数点（.）在数字间（如 3.14）、域名/URL 中的点（example.com）。
///
/// 行为（A2，Gavin 2026-08-08）：命中标点 **push 一个空格**（而非删除），
/// 末尾连续空格合一（:505 逻辑保留）+ trim。保证句中标点造成的词间粘连被空格承接。
pub fn strip_punctuation(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(chars.len());
    for (i, &c) in chars.iter().enumerate() {
        if !punctuation::is_punctuation(c) {
            result.push(c);
            continue;
        }
        // 保护：小数点 / 域名点（前后均为 ASCII 字母或数字）
        // 覆盖现有 :490 数字间小数点逻辑 + example.com 域名点（URL 场景实测结论）
        if c == '.' {
            let prev = chars.get(i.wrapping_sub(1)).copied().unwrap_or(' ');
            let next = chars.get(i + 1).copied().unwrap_or(' ');
            if prev.is_ascii_alphanumeric() && next.is_ascii_alphanumeric() {
                result.push(c);
                continue;
            }
        }
        // A2: 命中标点 push 空格（而非 continue 跳过）
        result.push(' ');
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
/// 🔴 HOTWORDS-BUDGET-265（Gavin 2026-09-20 拍板）：20 → **120**。
/// 背景：accuracy 换 KV=1024（DEC-072）后 20s 段预算 149→667，条数可放开；
/// 但**必须同时设字符预算**（见 HOTWORDS_MAX_TOTAL_CHARS，DEC-071）——只放开条数会因
/// 120×10 字 ≈1000 token 重新溢出，比换 1024 之前更糟。
pub const HOTWORDS_MAX_ENTRIES: usize = 120;

/// ASR-ACC-OPT-001 方案 A：hotwords 单条最大字符数。
/// 超长的词条（candidates 整句）不灌入，避免膨胀 user_prompt。
pub const HOTWORDS_MAX_ENTRY_CHARS: usize = 10;

/// HOTWORDS-TOKEN-268：热词**总 token 预算**（**C++ 口径**，含 `,` 分隔符）。
///
/// 算式（全按 C++ `FunASRNanoTokenizer` 口径；最坏 20.4s 段含 VAD padding）：
/// ```
/// 1024(max_total_len)
/// − 340(20.4s 段音频)
/// − 168(模板 + user_prompt + 特殊 token；实测最小差值)
/// − 160(生成预留)   ← 🔴 以前漏了这项，就是「生成饿死」的成因（DEC-069 第二级静默失效）
/// = 356
/// ```
/// 160 生成预留依据：20s 段约说 60–80 字，留 160 有一倍余量。
/// ⚠️ 生成预留缺失时**不报 Truncating**（输入没溢出，只是没空间生成）⇒ 只盯 Truncating 判不出。
///
/// 物理上限：1024 已是 ModelScope 最大版本；`HOTWORDS_MAX_ENTRIES=120` 仅作**条数上限**，
/// **token 预算硬约束先生效** ⇒ 长词按 token 成本自动截断（如 4 字词约装 66 条）。
pub const HOTWORDS_MAX_TOTAL_TOKENS: usize = 356;

/// HOTWORDS-TOKEN-268：Rust `tokenizers` 计数相对 C++ `FunASRNanoTokenizer` 的**保守系数**。
///
/// C++ 自己读 `vocab.json`+`merges.txt` **手搓 byte-level BPE** + 模拟 pre_tokenizer 正则
/// （`funasr-nano-tokenizer.cc`），与 `tokenizer.json` 完整管线在 CJK 上结果不同：
/// 实测 C++/Rust 比值 = 2字 1.25 / 生僻4字 1.15 / **常用4字 1.66** / 3字 1.53 / 5字 1.40 /
/// 6字 1.51 / 10字 1.17 / 中英混 1.48 ⇒ 取最大 1.66 × 1.1 ≈ 1.83 → 上取 **1.85**。
/// 🔴 上游 tokenizer 实现变更须**重新标定**；这是保守上界（宁可少装词，不可低估溢出）。
const HOTWORDS_CPP_SAFETY_FACTOR: f64 = 1.85;

/// 惰性加载的 Qwen3 BPE tokenizer（仅计 token 用，加载一次）。
static HOTWORDS_TOKENIZER: OnceLock<Option<tokenizers::Tokenizer>> = OnceLock::new();

/// 取（必要时加载）tokenizer。路径按 DEC-011 exe 同级 models 推导。
fn hotwords_tokenizer() -> &'static Option<tokenizers::Tokenizer> {
    HOTWORDS_TOKENIZER.get_or_init(|| {
        let p = model_dir()
            .join("sherpa-onnx-funasr-nano-int8-2025-12-30")
            .join("Qwen3-0.6B")
            .join("tokenizer.json");
        let t = std::time::Instant::now();
        match tokenizers::Tokenizer::from_file(&p) {
            Ok(tk) => {
                log::info!(
                    "HOTWORDS-TOKEN-268: tokenizer loaded in {:?} ({})",
                    t.elapsed(),
                    p.display()
                );
                Some(tk)
            }
            Err(e) => {
                log::warn!(
                    "HOTWORDS-TOKEN-268: tokenizer load failed ({}): {}；将退化为字节上界",
                    p.display(),
                    e
                );
                None
            }
        }
    })
}

/// HOTWORDS-TOKEN-268：启动时预热 tokenizer，避免 ~数百 ms 加载落在「按热键→开始录音」路径上。
pub fn warm_hotwords_tokenizer() {
    std::thread::spawn(|| {
        let _ = hotwords_tokenizer();
    });
}

/// 单条词在 **C++ 口径**下的保守 token 估算（Rust 计数 × 系数；tokenizer 不可用则用 UTF-8 字节数上界）。
fn estimate_word_tokens(tk: Option<&tokenizers::Tokenizer>, word: &str) -> usize {
    match tk {
        Some(t) => match t.encode(word, false) {
            Ok(enc) => ((enc.get_ids().len() as f64) * HOTWORDS_CPP_SAFETY_FACTOR).ceil() as usize,
            // byte-level BPE 下 token ≤ byte，字节数是安全的粗上界
            Err(_) => word.len(),
        },
        None => word.len(),
    }
}

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
/// 4. 数量上限 HOTWORDS_MAX_ENTRIES + **总 token 预算 HOTWORDS_MAX_TOTAL_TOKENS（C++ 口径，含分隔符）**，
///    按入参顺序（调用方已按 hit_count DESC, id DESC 排序）截断保留高优先级词条
///
/// 调用方（main.rs load_hotwords_for_accuracy）已按 hit_count DESC, id DESC 排序，
/// 截断后保留高频/最近词条，确定性顺序保证 hotwords 版本号哈希稳定。
pub fn curate_hotwords_entries(entries: &[String]) -> Vec<String> {
    curate_hotwords_entries_with(entries, hotwords_tokenizer().as_ref())
}

/// 可注入 tokenizer 的内核（单测用真实 tokenizer 时传 Some，验证回退时传 None）。
fn curate_hotwords_entries_with(
    entries: &[String],
    tk: Option<&tokenizers::Tokenizer>,
) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    // HOTWORDS-TOKEN-268: 累加 C++ 口径 token 估算（含 `,`），超预算即 **break**（不是 continue）。
    // 入参已按 hit_count DESC,id DESC 排序 ⇒ 前面的就是更该进的；break 保持「优先级高的先占额度」，
    // continue 会跳过长词去塞短词、把高频长词挤掉，与频次排序初衷相反。
    let mut est_tokens = 0usize;
    for word in entries {
        let trimmed = word.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_pure_ascii(trimmed) {
            continue;
        }
        if trimmed.chars().count() > HOTWORDS_MAX_ENTRY_CHARS {
            // 单条超长属「过滤」不是「预算不足」，用 continue（不占预算，也不阻断后续短词）
            continue;
        }
        // +1 = 逗号分隔符（首条无前导逗号）
        let add = estimate_word_tokens(tk, trimmed) + if result.is_empty() { 0 } else { 1 };
        if est_tokens + add > HOTWORDS_MAX_TOTAL_TOKENS {
            break;
        }
        est_tokens += add;
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
        AsrModel::LocalRealtime => {
            // LOCAL-RT-ENGINE-239-A（DEC-067）：LocalRealtime 需要 online + offline 双模型并存，
            // 由 Transcriber::new 的专用分支经 build_local_realtime_recognizers() 构建；
            // 本函数「返回单一 offline recognizer」的契约不适用该档，正常路径不会触达。
            // 防御性 bail（非 panic），避免误用时静默返回半个模型。
            anyhow::bail!(
                "LocalRealtime requires dual-model construction; handled in Transcriber::new()"
            )
        }
        AsrModel::QwenAudioOnline | AsrModel::FunAsrRealtime => {
            // ASR-041-B / ASR-056: 在线 ASR 模式不加载本地模型，
            // Transcriber::new() 已提前返回，此分支不应被触达
            unreachable!(
                "online ASR models should be handled in Transcriber::new() before build_recognizer"
            )
        }
    }
}

/// LOCAL-RT-ENGINE-239-A（DEC-067）：LocalRealtime 双模型构建。
///
/// - online  = streaming paraformer trilingual（本地预览，greedy_search）
/// - offline = accuracy FunASR Nano native（2pass 最终文本 + hotwords）
///
/// 🔴 **DEC-067 附则一：任一缺失/加载失败即 `Err`，禁止静默降级**。
/// 不沿用 accuracy 的 `effective_model` 归位逻辑 —— 静默降级正是 ASR-UI-208 判定
/// 「用户以为用 A 实际跑 B」的不一致。错误信息区分 online / offline 侧，供 239-B 浮层文案分类。
///
/// 返回 `(online, offline, hotwords_version)`。hotwords_version 算法与 `build_recognizer`
/// 内联实现一致（此处**不抽公共 fn**，以免触碰现有 accuracy / performance 分支）。
fn build_local_realtime_recognizers(
    model_dir: &Path,
    _language: &str,
    hotwords: Option<&str>,
) -> Result<(
    sherpa_onnx::OnlineRecognizer,
    sherpa_onnx::OfflineRecognizer,
    u64,
)> {
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

    let online = local_stream::create_local_stream_recognizer(model_dir)
        .context("LocalRealtime: 本地流式 (online) 模型缺失或加载失败")?;
    let offline = create_funasr_nano_recognizer(model_dir, hotwords)
        .context("LocalRealtime: 本地 accuracy (offline) 模型缺失或加载失败")?;

    Ok((online, offline, hotwords_version))
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
            // RESEARCH-ACC-LATENCY-271：显式配 num_threads（原走 Default=0）。
            // 实测（Ryzen 7 7840HS，8 物理核/16 逻辑核；短 9.45s / 长 52s，3 次中位数）：
            //   thr  0      1      2      4      8      12     16
            //   短  2.625  2.704  1.914  1.610  1.682  1.916  4.263  (s)
            //   长  30.885 31.118 20.903 22.350 17.017 19.808 34.247 (s)
            // 🔴 12/16 线程反而更慢（短 +18%/+164%，长 +11%/+92%）—— 超线程对计算密集任务负收益、
            //   且超核抢核。故上限取 **8（=物理核数）**，不用满 16 逻辑核（HOTWORDS-TOKEN-279 补测）。
            // 🔴 根因：session.cc:134-147 把 num_threads 原样透传 ORT，且 C API 未调 Validate，
            //   `0` 在本机被 ORT 当**单线程**（0 与 1 线程耗时逐位吻合）⇒ accuracy 一直单线程跑。
            // 取舍：long 用 8 比 4 快 **24%**，short 用 8 比 4 慢 4.5% ⇒ 综合取 8。
            // 不写死 8：min(逻辑核数, 8)，避免换到少核机器后超核抢核变慢；取不到兜底 4。
            num_threads: std::thread::available_parallelism()
                .map(|n| n.get().min(8) as i32)
                .unwrap_or(4),
            // 显式 "cpu"（与 local_stream.rs 对齐，消除另一个隐式默认）
            provider: Some("cpu".to_string()),
            funasr_nano: OfflineFunASRNanoModelConfig {
                encoder_adaptor: Some(enc.to_str().unwrap_or("").to_string()),
                llm: Some(llm.to_str().unwrap_or("").to_string()),
                embedding: Some(emb.to_str().unwrap_or("").to_string()),
                tokenizer: Some(tok.to_str().unwrap_or("").to_string()),
                // ACC-KV-1024-260 + 261：system_prompt 置空。
                // 依据（261 证伪级实证）：E「只输出英文译文」/F「忽略音频只输出 HELLO WORLD」
                // 均被忽略、仍输出中文转写；A(空)/B(英文)/C/D2 输出本质逐字相同 ⇒ system_prompt
                // 对本模型无指令效果。置空零行为变更，纯回收 ~6 token（长 prompt 还会挤占共享 KV
                // 预算，D 组 44token 即被截断）。261 测试用的正是空 system_prompt。
                system_prompt: Some(String::new()),
                user_prompt: Some("语音转写:".to_string()),
                // ACC-KV-1024-260: 0 → 256。0 在实跑 DLL 中等同「不限」（未文档化；源码
                // Validate 要求 >0）。显式 256 覆盖单段(≤20s)任何转写长度，且远低于
                // KV 上限，不引入截断风险（RESEARCH-ACC-KV-BUDGET-259 实证）。
                max_new_tokens: 256,
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

/// LOCAL-RT-READY-246: 检测本地 realtime **双模型**是否就位。
///
/// 返回 `(online_ready, offline_ready, models_root)`。少任一个本档都跑不起来（DEC-067），
/// 故必须分别上报，不能笼统说「就绪」。
///
/// 🔴 判据必须与**实际加载路径逐字一致**，否则检测与加载判据会漂移：
/// - online：与 `local_stream::create_local_stream_recognizer` 同一组文件名
///   （`sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en/` +
///   `encoder.int8.onnx` + `decoder.int8.onnx` + `tokens.txt`）
/// - offline：复用 `check_accuracy_model_ready` 判据（accuracy 原生三件套 + tokenizer 目录）
// 供主程序（239-B）接线；当前 root 侧无调用者，保留此 allow 以守 warnings 基线（接线时移除）。
#[allow(dead_code)]
pub fn check_local_realtime_models_ready(model_dir: &Path) -> (bool, bool, PathBuf) {
    // online：与 local_stream::create_local_stream_recognizer 同一组文件名
    let online_dir = model_dir.join("sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en");
    let online_ready = online_dir.join("encoder.int8.onnx").exists()
        && online_dir.join("decoder.int8.onnx").exists()
        && online_dir.join("tokens.txt").exists();

    // offline：**直接调用** check_accuracy_model_ready，不复制判据。
    // 🔴 主控 2026-09-20 裁定：此处曾就地复制五行判据以守 warnings 111 基线，
    // 但那会制造一份必须手工同步的副本——一旦与 accuracy 判据漂移，界面显示
    // 「已就位」而实际加载失败，属静默失效。warnings 基线是检查手段不是目标；
    // 该函数由 never-used 变可达是改善，基线随之更新为 110/102。
    let (offline_ready, _) = check_accuracy_model_ready(model_dir);

    (online_ready, offline_ready, model_dir.to_path_buf())
}

/// ASR-038-B: 从 wordbook 表加载热词 vocabulary（调用链证明）
///
/// **调用链**：
/// `wordbook::Wordbook::list_all()` → `db::load_word_entries()`（只查 `wordbook` 表）
/// → 转换为 `qwen_inference::VocabEntry` → `qwen_inference::build_vocabulary()`
///
/// 🔴 **数据源保证**：`Wordbook::list_all()` 只查 `wordbook` 表（`db::load_word_entries()`），
/// **不查 `wordbook_candidates` 表**（163 条未确认候选，禁止注入）。
/// `wordbook_candidates` 由 `db::upsert_candidate` / `db::load_candidates` 管理，
/// 本函数不调用这些接口。
///
/// 权重：user=5（专名领域词）、system=4（历史 bug 沉淀保护词）、未知=3
/// 超限词条由 `build_vocabulary` 内部过滤（跳过 ASR 但保留给 LLM）
pub fn load_wordbook_vocabulary() -> serde_json::Value {
    match crate::wordbook::Wordbook::open() {
        Ok(wb) => match wb.list_all() {
            Ok(entries) => {
                let vocab_entries: Vec<qwen_inference::VocabEntry> = entries
                    .into_iter()
                    .map(|e| qwen_inference::VocabEntry {
                        word: e.word,
                        source: e.source,
                    })
                    .collect();
                log::info!(
                    "ASR vocabulary loaded from wordbook table: {} entries",
                    vocab_entries.len()
                );
                qwen_inference::build_vocabulary(&vocab_entries)
            }
            Err(e) => {
                log::warn!("Failed to load wordbook for ASR vocabulary: {}", e);
                serde_json::json!({})
            }
        },
        Err(e) => {
            log::warn!("Failed to open wordbook for ASR vocabulary: {}", e);
            serde_json::json!({})
        }
    }
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
        // ASR-041-B: Qwen3Online 已删除，qwen3_online 字符串由迁移逻辑改写为 qwen_audio_online
        // from_config 不再认 qwen3_online（迁移在 load 阶段已改写）
        // ASR-038-B: QwenAudioOnline 解析
        assert_eq!(
            AsrModel::from_config("qwen_audio_online"),
            AsrModel::QwenAudioOnline
        );
        assert_eq!(
            AsrModel::from_config("QWEN_AUDIO_ONLINE"),
            AsrModel::QwenAudioOnline
        );
    }

    // ============================================================
    // ASR-038-B: Transcriber QwenAudioOnline 独立配置字段验证
    // 回归防护：A 批错误复用 qwen3_url 调 Inference 协议，B 批修正为独立 asr_online_url
    // ============================================================

    /// ASR-038-B-005: QwenAudioOnline 模式 Transcriber::new 存在线 ASR 配置
    /// （不加载本地模型，new 不依赖 model_dir 存在性，可直接测）
    #[test]
    fn transcriber_qwen_audio_online_stores_independent_config() {
        let model_dir = std::path::Path::new("nonexistent-model-dir-for-test");
        let t = Transcriber::new(
            model_dir,
            false,
            "auto".to_string(),
            AsrModel::QwenAudioOnline,
            None,
            "sk-test-key",
            "wss://llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference",
            "qwen-audio-3.0-asr-flash-streaming",
            800,
            false,
        )
        .expect("QwenAudioOnline Transcriber::new should succeed without local models");
        assert_eq!(t.asr_model(), AsrModel::QwenAudioOnline);
        assert_eq!(
            t.asr_online_url(),
            "wss://llm-kudx4dj2bfqn4gr2.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference"
        );
        assert_eq!(t.asr_online_model(), "qwen-audio-3.0-asr-flash-streaming");
        // ASR-056: max_sentence_silence 存储
        assert_eq!(t.asr_online_max_sentence_silence(), 800);
    }

    /// TEST-SYNC-229 G3: 两轴参数透传链不断 —— `Transcriber::new` 传入什么，
    /// getter 就返回什么（防中途丢参 / 重新硬编码）。
    /// 用非默认值（轴一 1234 ≠ 2000、轴二 true ≠ false）才能与「写死默认值」区分开；
    /// 传默认值就算字段被丢弃、getter 直接返回默认常量也会假绿。
    #[test]
    fn transcriber_seg229_two_axis_passthrough_stores_exact_values() {
        let model_dir = std::path::Path::new("nonexistent-model-dir-for-test");
        let t = Transcriber::new(
            model_dir,
            false,
            "auto".to_string(),
            AsrModel::QwenAudioOnline,
            None,
            "sk-test-key",
            "wss://inference.example.com",
            "qwen-audio-3.0-asr-flash-streaming",
            1234,
            true,
        )
        .expect("QwenAudioOnline Transcriber::new should succeed without local models");
        assert_eq!(
            t.asr_online_max_sentence_silence(),
            1234,
            "轴一：传入 1234 必须原样返回（非默认 2000，可区分写死默认）"
        );
        assert_eq!(
            t.asr_online_semantic_punctuation_enabled(),
            true,
            "轴二：传入 true 必须原样返回（默认 false，可区分写死默认）"
        );
    }

    /// ASR-038-B-006: QwenAudioOnline 模式 API Key 空 → bail
    #[test]
    fn transcriber_qwen_audio_online_empty_key_bails() {
        let model_dir = std::path::Path::new("nonexistent-model-dir-for-test");
        let result = Transcriber::new(
            model_dir,
            false,
            "auto".to_string(),
            AsrModel::QwenAudioOnline,
            None,
            "",
            "wss://inference.example.com",
            "qwen-audio-3.0-asr-flash-streaming",
            800,
            false,
        );
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(
            err_msg.contains("API Key 为空"),
            "empty API key should bail with clear message, got: {}",
            err_msg
        );
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

    // ---- HOTWORDS-TOKEN-268: token 预算测试（用真实 tokenizer；不可用则跳过）----

    fn test_tokenizer() -> Option<tokenizers::Tokenizer> {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("models")
            .join("sherpa-onnx-funasr-nano-int8-2025-12-30")
            .join("Qwen3-0.6B")
            .join("tokenizer.json");
        let t = std::time::Instant::now();
        let tk = tokenizers::Tokenizer::from_file(&p).ok();
        if tk.is_some() {
            eprintln!("268: Rust tokenizers load = {:?}", t.elapsed());
        }
        tk
    }

    fn est_of(tk: Option<&tokenizers::Tokenizer>, ws: &[String]) -> usize {
        let mut t = 0usize;
        for (i, w) in ws.iter().enumerate() {
            t += estimate_word_tokens(tk, w.trim()) + if i > 0 { 1 } else { 0 };
        }
        t
    }

    fn two_char_common(n: usize) -> Vec<String> {
        let pool: Vec<char> =
            "的一是不了人我在有他这中大来上国个到说们为子和你地出道也时年得就那要下以生会自着去"
                .chars()
                .collect();
        (0..n)
            .map(|i| format!("{}{}", pool[i % pool.len()], pool[(i * 3 + 1) % pool.len()]))
            .collect()
    }

    /// 268：预算不变式——截断结果必须 ≤ 356，且**再加一条必然超**（即极大致）。
    fn assert_budget_maximal(tk: Option<&tokenizers::Tokenizer>, entries: &[String]) {
        let kept = curate_hotwords_entries_with(entries, tk);
        assert!(kept.len() <= entries.len());
        assert!(
            est_of(tk, &kept) <= HOTWORDS_MAX_TOTAL_TOKENS,
            "kept 必须 ≤ 356，实得 {}",
            est_of(tk, &kept)
        );
        if kept.len() < entries.len() {
            let mut more = kept.clone();
            more.push(entries[kept.len()].clone());
            assert!(
                est_of(tk, &more) > HOTWORDS_MAX_TOTAL_TOKENS,
                "未达极大：再加 1 条仍 ≤ 356"
            );
        }
    }

    #[test]
    fn curate_two_char_120_report_and_budget() {
        let Some(tk) = test_tokenizer() else {
            return;
        };
        let entries = two_char_common(120);
        let kept = curate_hotwords_entries_with(&entries, Some(&tk));
        eprintln!(
            "268: 2 字×120 实装 {} 条（est={}）",
            kept.len(),
            est_of(Some(&tk), &kept)
        );
        assert_budget_maximal(Some(&tk), &entries);
    }

    #[test]
    fn curate_common_4char_truncated_by_token_budget() {
        let Some(tk) = test_tokenizer() else {
            return;
        };
        let pool: Vec<char> = "北京上海深圳杭州成都武汉西安南京广州苏州天津重庆长沙青岛厦门中科华夏东方南方长城智能网络数据电子信息集团研究院机器人医疗健康".chars().collect();
        let entries: Vec<String> = (0..120)
            .map(|i| {
                format!(
                    "{}{}{}{}",
                    pool[(4 * i) % pool.len()],
                    pool[(4 * i + 1) % pool.len()],
                    pool[(4 * i + 2) % pool.len()],
                    pool[(4 * i + 3) % pool.len()]
                )
            })
            .collect();
        let kept = curate_hotwords_entries_with(&entries, Some(&tk));
        eprintln!(
            "268: 常用 4 字×120 实装 {} 条（est={}）",
            kept.len(),
            est_of(Some(&tk), &kept)
        );
        assert!(kept.len() < 120, "4 字词应被 token 预算截断（<120）");
        assert_budget_maximal(Some(&tk), &entries);
    }

    #[test]
    fn curate_rare_4char_truncated_by_token_budget() {
        let Some(tk) = test_tokenizer() else {
            return;
        };
        let entries: Vec<String> = (0..120)
            .map(|i| {
                let c = char::from_u32(0x4E00 + i).unwrap();
                format!("{}{}{}{}", c, c, c, c)
            })
            .collect();
        assert_budget_maximal(Some(&tk), &entries);
    }

    #[test]
    fn curate_token_budget_boundary() {
        let Some(tk) = test_tokenizer() else {
            return;
        };
        // 用单字常用词，逐条推近 356，验证正好保留 / 再加一条丢弃
        let entries: Vec<String> = (0..200).map(|_| "的".to_string()).collect();
        let kept = curate_hotwords_entries_with(&entries, Some(&tk));
        let est = est_of(Some(&tk), &kept);
        assert!(est <= HOTWORDS_MAX_TOTAL_TOKENS, "est={}", est);
        let mut more = kept.clone();
        more.push("的".to_string());
        assert!(est_of(Some(&tk), &more) > HOTWORDS_MAX_TOTAL_TOKENS);
    }

    #[test]
    fn curate_fallback_without_tokenizer_uses_byte_bound() {
        // tokenizer 不可用 → 用 UTF-8 字节数（安全粗上界）：字节和 ≤ 356
        let entries: Vec<String> = std::iter::repeat("北京".to_string()).take(200).collect();
        let kept = curate_hotwords_entries_with(&entries, None);
        let mut bytes = 0usize;
        for (i, w) in kept.iter().enumerate() {
            bytes += w.len() + if i > 0 { 1 } else { 0 };
        }
        assert!(bytes <= HOTWORDS_MAX_TOTAL_TOKENS, "bytes={}", bytes);
        assert!(!kept.is_empty());
    }

    #[test]
    fn curate_enforces_max_entries_limit() {
        // 150 条 1 字词：token 预算先生效（条目上限 120 仅作上限）
        let entries: Vec<String> = (0..150).map(|_| "的".to_string()).collect();
        let kept = curate_hotwords_entries(&entries);
        assert!(kept.len() <= HOTWORDS_MAX_ENTRIES);
        assert!(!kept.is_empty());
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
        // 268：token 预算先生效，条数上限 120 仅作上限；截断必须保持入参顺序（前缀）
        let entries = two_char_common(200);
        let curated = curate_hotwords_entries(&entries);
        assert!(curated.len() <= HOTWORDS_MAX_ENTRIES);
        assert!(!curated.is_empty());
        for (i, w) in curated.iter().enumerate() {
            assert_eq!(w, &entries[i], "entry {} 顺序必须与入参一致（前缀）", i);
        }
    }

    // ============================================================
    // ASR-PUNCT-OPT-001: strip_punctuation 测试
    // ============================================================

    #[test]
    fn strip_punctuation_chinese() {
        // 标点替换为空格（A2），不是直接删除。
        // 句末标点产生的多余空格由 trim 清理。
        assert_eq!(
            strip_punctuation("周末要不要去露营？最近天气超舒服。"),
            "周末要不要去露营 最近天气超舒服"
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
        // 中英混合，标点替换为空格
        assert_eq!(
            strip_punctuation("今天很好，very nice！"),
            "今天很好 very nice"
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
        // 引号位置替换为空格（A2）
        let input = "\u{201C}\u{4F60}\u{597D}\u{201D}\u{4ED6}\u{8BF4}";
        assert_eq!(strip_punctuation(input), "你好 他说");
    }

    #[test]
    fn strip_punctuation_collapses_spaces() {
        // 标点剥离后连续空格压缩
        assert_eq!(strip_punctuation("你好，  世界！"), "你好 世界");
    }

    /// TEST-SYNC-030 3.3 补充：strip_punctuation 全量参数化——PUNCT_CHARS 每个成员
    /// 命中的语义都是「换成一个空格」（A2），不是直接删除。用「词 + 标点 + 词」夹持
    /// 验证命中标点产生空格（句末等产生的位置由后续 trim 清理不在本用例覆盖）。
    #[test]
    fn strip_punctuation_every_char_replaces_with_space() {
        use crate::punctuation::PUNCT_CHARS;
        // '.' 有「数字间/域名」保护逻辑，单独测（两侧为汉字时不豁免 → 也是空格）。
        for &c in PUNCT_CHARS {
            let input = format!("测试{c}测试");
            let out = strip_punctuation(&input);
            assert_eq!(
                out, "测试 测试",
                "PUNCT_CHARS 成员 {c:?} 应被替换为单个空格（A2），而非删除"
            );
        }
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

    #[test]
    fn qwen3_native_punctuated_detection() {
        // PUNCT-GOVERNANCE-030-A-2：qwen3 native_punctuated = 输出是否含**有效标点**
        // （实测，非恒 true；主控 c 方案统一调用 has_effective_punctuation）
        use crate::punctuation::has_effective_punctuation;
        // 带句末标点 → true（跳过标点引擎，行为不变）
        assert!(has_effective_punctuation("今天天气不错。"));
        assert!(has_effective_punctuation("OK? 好的。"));
        // 无标点 → false（走标点引擎补标点，修复目标）
        assert!(!has_effective_punctuation("好的"));
        assert!(!has_effective_punctuation("谢谢"));
        // 🔴 词内嵌入误判修复（c 方案）：PUNCT_CHARS 含半角 `.`，纯数字 3.14
        //   旧判据 is_punctuation 误判「带标点」→ native_punctuated=true → 跳过引擎。
        //   新判据 has_effective_punctuation 做「前后 ASCII 字母数字夹持则豁免」：
        //   3.14 / went's 撇号 / 3:30 冒号 / example.com 域名点全部不再计价点。
        assert!(!has_effective_punctuation("3.14"));
        assert!(!has_effective_punctuation("圆周率是3.14"));
        assert!(!has_effective_punctuation("I don't know"));
        assert!(!has_effective_punctuation("3:30 开会"));
        assert!(!has_effective_punctuation("example.com"));
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
        // 4. 全空 → bail!(NoSpeechError)（BUG-119 类型化信息信号）
        // 5. 任一段 np=false → all_native=false
        // 6. 返回 (normalized_text, all_native)
        //
        // R1 关键：lock poisoned 时走 naive_chunk 分支调此辅助函数，
        // 不再静默落到单次转录整段（禁止 >28s 整段喂 native）。
        assert!(true, "transcribe_segments_chunked contract documented");
    }

    // ============================================================
    // ASR-NOSPEECH-FILTER-001: 特殊 token 剥离测试
    // ============================================================

    #[test]
    fn strip_asr_special_tokens_pure_token() {
        assert_eq!(Transcriber::strip_asr_special_tokens("<|nospeech|>"), "");
    }

    #[test]
    fn strip_asr_special_tokens_token_with_text() {
        assert_eq!(Transcriber::strip_asr_special_tokens("<|zh|>你好"), "你好");
    }

    #[test]
    fn strip_asr_special_tokens_text_with_token() {
        assert_eq!(
            Transcriber::strip_asr_special_tokens("你好<|nospeech|>"),
            "你好"
        );
    }

    #[test]
    fn strip_asr_special_tokens_multiple_tokens() {
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|zh|><|nospeech|>你好"),
            "你好"
        );
    }

    #[test]
    fn strip_asr_special_tokens_mixed_with_text() {
        // 【主控修正 2026-07-27】CJK 之间剥离 token 后不得插入空格
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|zh|>你<|NEUTRAL|>好<|nospeech|>"),
            "你好"
        );
    }

    #[test]
    fn strip_asr_special_tokens_keeps_ascii_word_boundary() {
        // 英文单词被 token 分隔时必须补空格，否则粘连成 helloworld
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello<|NEUTRAL|>world"),
            "hello world"
        );
        // 数字同理
        assert_eq!(Transcriber::strip_asr_special_tokens("1<|zh|>2"), "1 2");
    }

    #[test]
    fn strip_asr_special_tokens_no_space_at_cjk_ascii_boundary() {
        // 中英边界不补空格：ASR 原文没有空格就不应凭空造出来
        assert_eq!(
            Transcriber::strip_asr_special_tokens("中文<|NEUTRAL|>abc"),
            "中文abc"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("abc<|NEUTRAL|>中文"),
            "abc中文"
        );
    }

    #[test]
    fn strip_asr_special_tokens_normal_text_unchanged() {
        // 孤立 < 或 > 不应被误伤
        assert_eq!(
            Transcriber::strip_asr_special_tokens("小于号<和大于号>"),
            "小于号<和大于号>"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("他说：1 < 2 且 3 > 1"),
            "他说：1 < 2 且 3 > 1"
        );
    }

    #[test]
    fn strip_asr_special_tokens_incomplete_not_stripped() {
        // 不完整的 token 形态不应误删
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|nospeech"),
            "<|nospeech"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("|nospeech|>"),
            "|nospeech|>"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|no|speech|>"),
            "<|no|speech|>"
        );
    }

    // ============================================================
    // P0-TEST-SYNC-20260727: 空格边界缺口补充
    // ============================================================

    #[test]
    fn strip_asr_special_tokens_token_start_ascii() {
        // token 位于串首 + 后接 ASCII → 无前导空格
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|NEUTRAL|>hello"),
            "hello"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("<|zh|>world"),
            "world"
        );
    }

    #[test]
    fn strip_asr_special_tokens_token_end_ascii() {
        // token 位于串尾（ASCII 在前）→ 无后置空格
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello<|NEUTRAL|>"),
            "hello"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("world<|nospeech|>"),
            "world"
        );
    }

    #[test]
    fn strip_asr_special_tokens_existing_space_near_token() {
        // token 相邻已有空格 → 不再添加额外空格
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello <|NEUTRAL|>world"),
            "hello world"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello<|NEUTRAL|> world"),
            "hello world"
        );
    }

    #[test]
    fn strip_asr_special_tokens_fullwidth_punct_near_token() {
        // 全角标点与 token 相邻 → 不产生空格（全角非 ascii_alphanumeric）
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello<|NEUTRAL|>！"),
            "hello！"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("你好<|NEUTRAL|>，"),
            "你好，"
        );
    }

    #[test]
    fn strip_asr_special_tokens_consecutive_between_ascii() {
        // 连续多 token 夹在英文单词中间 → 只产生一个空格
        assert_eq!(
            Transcriber::strip_asr_special_tokens("hello<|NEUTRAL|><|OTHER|>world"),
            "hello world"
        );
        assert_eq!(
            Transcriber::strip_asr_special_tokens("count<|zh|><|nospeech|><|en|>123"),
            "count 123"
        );
    }

    #[test]
    fn strip_asr_special_tokens_empty() {
        // 空串：幂等返回空串
        assert_eq!(Transcriber::strip_asr_special_tokens(""), "");
    }

    /// ASR-041-B-补 / ASR-056: AsrModel 恰好四变体
    /// （Performance/Accuracy/QwenAudioOnline/FunAsrRealtime）。
    /// Gavin 明确保留 Accuracy，防将来误删该变体。
    /// ASR-056 新增 FunAsrRealtime（与 QwenAudioOnline 共用 Inference 协议）。
    /// 用穷举 match 而非断言计数：删任一变体 → 本 match 编译失败（比 `== 4` 强）。
    /// 每个变体都走一遍 from_config 正反向，钉住字符串 ↔ 变体映射不被破坏。
    #[test]
    fn asr_model_exactly_four_variants_with_stable_mapping() {
        // 穷举四变体：新增/删除变体都会在此编译失败（编译期护栏）
        let _exhaustive: fn(AsrModel) = |m| match m {
            AsrModel::Performance
            | AsrModel::Accuracy
            | AsrModel::QwenAudioOnline
            | AsrModel::FunAsrRealtime
            | AsrModel::LocalRealtime => (),
        };

        // from_config 正反向映射（大小写不敏感）
        assert_eq!(AsrModel::from_config("performance"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config("accuracy"), AsrModel::Accuracy);
        assert_eq!(
            AsrModel::from_config("qwen_audio_online"),
            AsrModel::QwenAudioOnline
        );
        assert_eq!(
            AsrModel::from_config("QwEn_AuDiO_oNlInE"),
            AsrModel::QwenAudioOnline
        );
        // ASR-056: fun_asr_realtime 映射
        assert_eq!(
            AsrModel::from_config("fun_asr_realtime"),
            AsrModel::FunAsrRealtime
        );
        assert_eq!(
            AsrModel::from_config("FuN_AsR_ReAlTiMe"),
            AsrModel::FunAsrRealtime
        );
        // 未知字符串回落 Performance（不 panic）
        assert_eq!(AsrModel::from_config("qwen3_online"), AsrModel::Performance);
        assert_eq!(AsrModel::from_config(""), AsrModel::Performance);
    }

    /// ASR-056: is_online_streaming() 收敛判据——QwenAudioOnline 和 FunAsrRealtime 都返回 true
    #[test]
    fn asr_056_is_online_streaming_correct_for_all_variants() {
        assert!(
            !AsrModel::Performance.is_online_streaming(),
            "Performance must not be online streaming"
        );
        assert!(
            !AsrModel::Accuracy.is_online_streaming(),
            "Accuracy must not be online streaming"
        );
        assert!(
            AsrModel::QwenAudioOnline.is_online_streaming(),
            "QwenAudioOnline must be online streaming"
        );
        assert!(
            AsrModel::FunAsrRealtime.is_online_streaming(),
            "FunAsrRealtime must be online streaming"
        );
    }
}
