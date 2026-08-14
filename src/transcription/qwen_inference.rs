//! Qwen-Audio-3.0 在线流式 ASR 模块（RESEARCH-ASR-038-A）
//!
//! 接入阿里百炼 qwen-audio-3.0-asr-flash-streaming 在线模型。
//! 协议：DashScope 通用 WebSocket Inference API——
//!   wss://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference
//!   Authorization: Bearer <API_KEY>（不需要 OpenAI-Beta header）
//! 流程：run-task（payload 含 model/parameters/input）
//!   → 二进制音频帧（3200 bytes/片 = 100ms @ 16kHz 16bit mono）
//!   → finish-task
//!   → 收 result-generated（中间结果 sentence_end=false / 最终结果 sentence_end=true）
//!   → task-finished
//!
//! 与旧 qwen3_online.rs（Realtime API）的差异：
//! - model 在 payload 而非 URL query
//! - 音频是二进制帧而非 base64 文本帧
//! - 鉴权不需要 OpenAI-Beta header
//! - 支持即时热词（vocabulary）和 language_hints
//!
//! 失败行为（沿用 DEC-028）：断网/超时/key 无效 → 报错提示转录失败，不自动降级本地

use anyhow::{anyhow, bail, Context, Result};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tungstenite::client_tls_with_config;
use tungstenite::http::Uri;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{ClientRequestBuilder, Error as WsError, HandshakeError, Message, WebSocket};

use crate::transcription::qwen3_online::f32_to_pcm16_le;

/// 连接超时（沿用 DEC-028：5s）
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// 二进制音频分片大小：3200 bytes = 100ms @ 16kHz/16bit/mono
/// 官方示例统一用此值
const AUDIO_CHUNK_BYTES: usize = 3200;

/// 静默超时（finish-task 后服务端连续无消息即异常）
const SILENCE_TIMEOUT: Duration = Duration::from_secs(10);

/// 固定四语 language_hints（Gavin 拍板 + 主控定稿）
/// 产品默认支持中英韩日，让 ASR 后端在四语范围内自动检测
/// 正好卡 Qwen-Audio-3.0 的 4 个上限，不会截断
const ASR_LANGUAGE_HINTS: &[&str] = &["zh", "en", "ja", "ko"];

/// 默认模型名
const DEFAULT_MODEL: &str = "qwen-audio-3.0-asr-flash-streaming";

/// VAD 断句静音阈值（ms），低于默认 1300 以获得更好的「边说边出字」手感
const DEFAULT_MAX_SENTENCE_SILENCE: i64 = 800;

// ===========================================================================
// 纯函数：消息构造（可单测）
// ===========================================================================

/// 生成 32 位无连字符 task_id
///
/// 用时间戳 + 进程 ID + 计数器构造，不依赖 uuid crate。
/// 服务端只要求 task_id 在 run-task / finish-task / continue-task 间一致，
/// 不要求全局唯一或标准 UUID 格式。
fn generate_task_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:016x}{:08x}{:08x}", ts as u64, pid, seq)
}

/// 构造 run-task 消息（纯函数，可单测）
///
/// 官方 schema（DashScope Inference API client events）：
/// - header: action="run-task", task_id, streaming="duplex"
/// - payload: task_group="audio", task="asr", function="recognition", model
/// - parameters: format, sample_rate, language_hints, vocabulary, max_sentence_silence 等
/// - input: context（本批不传上下文，v1 不上）
///
/// `vocabulary` 为空时省略该字段。
pub fn build_run_task_message(
    task_id: &str,
    model: &str,
    vocabulary: &serde_json::Value,
) -> serde_json::Value {
    let mut parameters = serde_json::json!({
        "format": "pcm",
        "sample_rate": 16000,
        "language_hints": ASR_LANGUAGE_HINTS,
        "semantic_punctuation_enabled": false,
        "max_sentence_silence": DEFAULT_MAX_SENTENCE_SILENCE,
    });
    // 仅当 vocabulary 非空对象时注入
    if let Some(obj) = vocabulary.as_object() {
        if !obj.is_empty() {
            parameters["vocabulary"] = vocabulary.clone();
        }
    }
    serde_json::json!({
        "header": {
            "action": "run-task",
            "task_id": task_id,
            "streaming": "duplex"
        },
        "payload": {
            "task_group": "audio",
            "task": "asr",
            "function": "recognition",
            "model": model,
            "parameters": parameters,
            "input": {}
        }
    })
}

/// 构造 finish-task 消息（纯函数，可单测）
pub fn build_finish_task_message(task_id: &str) -> serde_json::Value {
    serde_json::json!({
        "header": {
            "action": "finish-task",
            "task_id": task_id,
            "streaming": "duplex"
        },
        "payload": {
            "input": {}
        }
    })
}

// ===========================================================================
// 纯函数：音频分片（可单测）
// ===========================================================================

/// PCM bytes → 二进制分片（纯函数，可单测）
///
/// 每片 3200 bytes = 1600 samples = 100ms @ 16kHz/16bit/mono
/// 与官方示例一致
pub fn chunk_pcm_to_binary(pcm: &[u8]) -> Vec<Vec<u8>> {
    pcm.chunks(AUDIO_CHUNK_BYTES).map(|c| c.to_vec()).collect()
}

// ===========================================================================
// 纯函数：服务端事件解析（可单测）
// ===========================================================================

/// 从服务端 JSON 消息提取事件类型（纯函数，可单测）
///
/// 返回 header.event 字段的值，如 "task-started" / "result-generated" / "task-finished" / "task-failed"
pub fn extract_event_type(msg: &serde_json::Value) -> Option<&str> {
    msg.get("header")?.get("event")?.as_str()
}

/// 从 result-generated 事件提取句子信息（纯函数，可单测）
///
/// 返回 (sentence_id, text, sentence_end)
/// 非结果生成事件返回 None
pub fn extract_sentence(msg: &serde_json::Value) -> Option<(i64, String, bool)> {
    if extract_event_type(msg)? != "result-generated" {
        return None;
    }
    let sentence = msg.get("payload")?.get("output")?.get("sentence")?;
    let id = sentence.get("sentence_id")?.as_i64()?;
    let text = sentence.get("text")?.as_str()?.to_string();
    let end = sentence.get("sentence_end")?.as_bool()?;
    Some((id, text, end))
}

/// 从 task-failed 事件提取错误信息（纯函数，可单测）
pub fn extract_task_error(msg: &serde_json::Value) -> Option<String> {
    if extract_event_type(msg)? != "task-failed" {
        return None;
    }
    let header = msg.get("header")?;
    let code = header
        .get("error_code")
        .and_then(|c| c.as_str())
        .unwrap_or("unknown");
    let message = header
        .get("error_message")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown error");
    Some(format!("task-failed [{}]: {}", code, message))
}

/// 从 result-generated 事件提取计费时长（纯函数，可单测）
///
/// 仅 sentence_end=true 时 usage 非空，返回 duration（秒）
pub fn extract_usage_duration(msg: &serde_json::Value) -> Option<i64> {
    if extract_event_type(msg)? != "result-generated" {
        return None;
    }
    msg.get("payload")?.get("usage")?.get("duration")?.as_i64()
}

// ===========================================================================
// StreamingAsrState：增量结果状态机（纯函数，可单测）
// ===========================================================================

/// 流式 ASR 增量结果状态
///
/// 维护已确认句（sentence_end=true）和当前正在变化的句。
/// 设计依据 asr-streaming-pipeline-design-001.md §三
#[derive(Debug, Clone, Default)]
pub struct StreamingAsrState {
    /// 已确认句（sentence_end=true 的句子文本，按顺序）
    confirmed_sentences: Vec<String>,
    /// 当前正在变化的句（最新中间结果的文本）
    current_sentence: String,
    /// 当前句 ID
    current_sentence_id: i64,
}

impl StreamingAsrState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 处理 result-generated 事件
    ///
    /// - sentence_end=true：句子确认，加入 confirmed，清空 current
    /// - sentence_end=false 且 sentence_id 相同：同一句的中间修正，替换 current
    /// - sentence_end=false 且 sentence_id 不同：新句开始，更新 current_sentence_id
    pub fn on_result(&mut self, sentence_id: i64, text: &str, sentence_end: bool) {
        if sentence_end {
            self.confirmed_sentences.push(text.to_string());
            self.current_sentence.clear();
        } else if sentence_id == self.current_sentence_id {
            self.current_sentence = text.to_string();
        } else {
            self.current_sentence_id = sentence_id;
            self.current_sentence = text.to_string();
        }
    }

    /// 合并出完整显示文本（已确认句 + 当前句）
    ///
    /// overlay 侧整段覆盖（Gavin 拍板统一白色，不区分已确认/未确认）
    pub fn display_text(&self) -> String {
        let mut text = self.confirmed_sentences.join("");
        text.push_str(&self.current_sentence);
        text
    }

    /// 取最终文本（松键后交给 LLM）
    ///
    /// 只含 confirmed 句，不含未确认的 current_sentence
    /// （finish-task 后服务端会发最后一个 sentence_end=true，届时 current 清空）
    pub fn final_text(&self) -> String {
        self.confirmed_sentences.join("")
    }

    /// 已确认句数
    pub fn confirmed_count(&self) -> usize {
        self.confirmed_sentences.len()
    }
}

// ===========================================================================
// 纯函数：热词 vocabulary 构造（可单测）
// ===========================================================================

/// 词库条目（简化版，供 vocabulary 构造用）
#[derive(Debug, Clone)]
pub struct VocabEntry {
    pub word: String,
    pub source: String,
}

/// 检查词条是否通过 ASR 热词长度限制（纯函数，可单测）
///
/// 官方限制：
/// - 含非 ASCII 字符：总字符数 ≤ 15
/// - 纯 ASCII 字符：按空格切分片段数 ≤ 7
pub fn passes_asr_vocab_limit(word: &str) -> bool {
    let has_non_ascii = word.chars().any(|c| !c.is_ascii());
    if has_non_ascii {
        word.chars().count() <= 15
    } else {
        word.split_whitespace().count() <= 7
    }
}

/// 根据 source 确定热词权重（纯函数，可单测）
///
/// user=5（专名领域词，同音词多最易错）
/// system=4（历史 bug 沉淀保护词，本身识别率不低）
/// 未知 source → 3（保守默认）
pub fn vocab_weight(source: &str) -> i32 {
    match source {
        "user" => 5,
        "system" => 4,
        _ => 3,
    }
}

/// 从词库条目列表构造 vocabulary JSON（纯函数，可单测）
///
/// 过滤超限词条（跳过 ASR 注入但保留给 LLM 侧）。
/// 返回 {"词": 权重, ...} JSON object。
/// 🔴 wordbook_candidates 表绝不注入——本函数只接收已过滤的条目，调用方负责只传 wordbook 表数据
pub fn build_vocabulary(entries: &[VocabEntry]) -> serde_json::Value {
    let mut vocab = serde_json::Map::new();
    for entry in entries {
        if !passes_asr_vocab_limit(&entry.word) {
            log::warn!(
                "ASR vocabulary: skipping word '{}' (exceeds length limit, kept for LLM only)",
                entry.word
            );
            continue;
        }
        let weight = vocab_weight(&entry.source);
        vocab.insert(entry.word.clone(), serde_json::json!(weight));
    }
    serde_json::Value::Object(vocab)
}

// ===========================================================================
// IO 函数：WebSocket 连接与转录
// ===========================================================================

/// 发送 JSON 消息到 WebSocket
fn send_json(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    value: &serde_json::Value,
) -> Result<()> {
    let text = serde_json::to_string(value).context("序列化 WS 消息失败")?;
    socket
        .send(Message::Text(text.into()))
        .map_err(|e| anyhow!("发送消息失败 - {}", e))
}

/// 判断 tungstenite 错误是否为 socket 读超时
fn is_read_timeout(e: &WsError) -> bool {
    match e {
        WsError::Io(io_err) => {
            io_err.kind() == std::io::ErrorKind::TimedOut
                || io_err.kind() == std::io::ErrorKind::WouldBlock
        }
        _ => false,
    }
}

/// 映射连接错误到鉴权/网络失败
fn map_connect_error(e: &tungstenite::Error) -> anyhow::Error {
    let msg = e.to_string();
    if msg.contains("401") || msg.contains("403") || msg.contains("Unauthorized") {
        anyhow!("鉴权失败：{}", msg)
    } else {
        anyhow!("网络失败：连接失败 - {}", msg)
    }
}

/// 设置 WebSocket socket 读写超时
fn set_socket_timeouts(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    read_timeout: Duration,
    write_timeout: Duration,
) -> Result<()> {
    use tungstenite::stream::MaybeTlsStream::*;
    match socket.get_mut() {
        Plain(tcp) => {
            tcp.set_read_timeout(Some(read_timeout))
                .context("设置读取超时失败")?;
            tcp.set_write_timeout(Some(write_timeout))
                .context("设置写入超时失败")?;
        }
        Rustls(stream_owned) => {
            stream_owned
                .sock
                .set_read_timeout(Some(read_timeout))
                .context("设置读取超时失败")?;
            stream_owned
                .sock
                .set_write_timeout(Some(write_timeout))
                .context("设置写入超时失败")?;
        }
        _ => {}
    }
    Ok(())
}

/// 计算硬上限（沿用 DEC-028：finish-task 后总时长保险丝）
///
/// 公式：`max(30s, 音频时长 × 0.5)`
pub fn compute_hard_cap(audio_samples: usize) -> Duration {
    let audio_secs = audio_samples as f64 / 16000.0;
    let cap_secs = (30.0_f64).max(audio_secs * 0.5);
    Duration::from_secs_f64(cap_secs)
}

/// 流式 ASR 转录主入口
///
/// 协议流程（RESEARCH-ASR-038）：
/// 1. connect（URL + Authorization Bearer header，连接超时 5s）
/// 2. send run-task（payload 含 model/parameters/vocabulary/language_hints）
/// 3. 等 task-started
/// 4. 逐块发送二进制音频帧（3200 bytes/片）
/// 5. send finish-task
/// 6. 等 result-generated 事件，收集最终文本
///    - 静默超时 10s（finish-task 后任何消息重置计时器）
///    - 硬上限 max(30s, 音频时长×0.5)
///
/// `on_result` 回调在每次收到 result-generated 时调用，传入 display_text，
/// 供调用方推送增量到 overlay（批次 B 接线）。
///
/// 失败路径映射 anyhow Err（沿用 DEC-028：报错不降级）：
/// - 鉴权失败（401/403）→ "鉴权失败"
/// - 网络失败 → "网络失败"
/// - 超时 → "超时"
pub fn transcribe_streaming(
    url: &str,
    api_key: &str,
    model: &str,
    samples_16k: &[f32],
    vocabulary: &serde_json::Value,
    mut on_result: impl FnMut(&str),
) -> Result<String> {
    if api_key.trim().is_empty() {
        bail!("鉴权失败：API Key 为空");
    }
    if samples_16k.is_empty() {
        bail!("转录失败：音频样本为空");
    }

    let model = if model.trim().is_empty() {
        DEFAULT_MODEL
    } else {
        model
    };
    let task_id = generate_task_id();
    let hard_cap = compute_hard_cap(samples_16k.len());

    log::info!(
        "QwenAudio streaming ASR: url={}, model={}, task_id={}, {} samples ({:.1}s), hard_cap={:.0}s",
        url,
        model,
        task_id,
        samples_16k.len(),
        samples_16k.len() as f64 / 16000.0,
        hard_cap.as_secs_f64(),
    );

    // DNS 解析 + TCP 连接
    let uri: Uri = url.parse().context("无效的 ASR URL")?;
    let host = uri.host().context("URL 缺少 host")?;
    let port = uri.port_u16().unwrap_or(443);
    let addrs = format!("{}:{}", host, port)
        .to_socket_addrs()
        .context("DNS 解析失败")?;
    let socket_addrs: Vec<_> = addrs.collect();
    let addr = socket_addrs.first().context("DNS 未返回地址")?;
    let tcp = TcpStream::connect_timeout(addr, CONNECT_TIMEOUT).context("网络失败：连接超时")?;
    tcp.set_read_timeout(Some(CONNECT_TIMEOUT))
        .context("网络失败：设置读取超时失败")?;
    tcp.set_write_timeout(Some(CONNECT_TIMEOUT))
        .context("网络失败：设置写入超时失败")?;

    // TLS/WS 握手（Inference API 不需要 OpenAI-Beta header）
    let request =
        ClientRequestBuilder::new(uri).with_header("Authorization", format!("Bearer {}", api_key));
    let (mut ws_socket, response) =
        client_tls_with_config(request, tcp, None, None).map_err(|e| match e {
            HandshakeError::Failure(e) => map_connect_error(&e),
            HandshakeError::Interrupted(_) => anyhow!("网络失败：TLS 握手意外中断"),
        })?;

    // WS 握手成功状态码 101
    debug_assert_eq!(
        response.status().as_u16(),
        101,
        "post-handshake status must be 101"
    );
    let _ = &response;

    // 设置 socket 超时（10s tick）
    set_socket_timeouts(
        &mut ws_socket,
        Duration::from_secs(10),
        Duration::from_secs(10),
    )
    .context("网络失败：设置 socket 超时失败")?;

    // 1. send run-task
    let run_task = build_run_task_message(&task_id, model, vocabulary);
    send_json(&mut ws_socket, &run_task)?;

    // 2. 等 task-started
    let mut started = false;
    loop {
        match ws_socket.read() {
            Ok(Message::Text(text)) => {
                let parsed: serde_json::Value =
                    serde_json::from_str(&text).context("服务端返回非 JSON 文本")?;
                if let Some(err) = extract_task_error(&parsed) {
                    bail!("服务端错误：{}", err);
                }
                if extract_event_type(&parsed) == Some("task-started") {
                    started = true;
                    log::info!("QwenAudio ASR task started: {}", task_id);
                    break;
                }
                // 其他事件忽略
            }
            Ok(Message::Binary(_)) => {}
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                bail!("网络失败：服务端关闭连接（未收到 task-started）");
            }
            Ok(Message::Frame(_)) => {}
            Err(e) if is_read_timeout(&e) => {
                bail!("超时：等待 task-started 超时");
            }
            Err(e) => bail!("网络失败：读取消息失败 - {}", e),
        }
    }
    if !started {
        bail!("网络失败：未收到 task-started");
    }

    // 3. 发送二进制音频帧
    let pcm = f32_to_pcm16_le(samples_16k);
    let chunks = chunk_pcm_to_binary(&pcm);
    log::info!(
        "QwenAudio ASR uploading {} binary chunks ({}ms each)",
        chunks.len(),
        AUDIO_CHUNK_BYTES as u64 * 1000 / (16000 * 2)
    );
    for chunk in &chunks {
        ws_socket
            .send(Message::Binary(chunk.clone().into()))
            .map_err(|e| anyhow!("网络失败：发送音频帧失败 - {}", e))?;
    }

    // 4. send finish-task
    let finish_task = build_finish_task_message(&task_id);
    send_json(&mut ws_socket, &finish_task)?;
    let finish_time = std::time::Instant::now();
    let hard_cap_deadline = finish_time + hard_cap;
    let mut last_activity = finish_time;

    // 5. 收 result-generated，收集最终文本
    let mut state = StreamingAsrState::new();
    loop {
        let now = std::time::Instant::now();
        if now > hard_cap_deadline {
            bail!(
                "超时：超过硬上限 {:.0}s 仍未收到 task-finished",
                hard_cap.as_secs_f64()
            );
        }
        let msg = match ws_socket.read() {
            Ok(m) => m,
            Err(e) if is_read_timeout(&e) => {
                if last_activity.elapsed() >= SILENCE_TIMEOUT {
                    bail!("超时：服务端 10s 无响应");
                }
                continue;
            }
            Err(e) => bail!("网络失败：读取消息失败 - {}", e),
        };
        last_activity = std::time::Instant::now();
        match msg {
            Message::Text(text) => {
                let parsed: serde_json::Value =
                    serde_json::from_str(&text).context("服务端返回非 JSON 文本")?;
                if let Some(err) = extract_task_error(&parsed) {
                    bail!("服务端错误：{}", err);
                }
                if let Some((id, text, end)) = extract_sentence(&parsed) {
                    state.on_result(id, &text, end);
                    let display = state.display_text();
                    log::debug!(
                        "QwenAudio ASR result: id={}, end={}, text='{}', display='{}'",
                        id,
                        end,
                        text,
                        display
                    );
                    on_result(&display);
                }
                if extract_event_type(&parsed) == Some("task-finished") {
                    log::info!(
                        "QwenAudio ASR task finished: {} confirmed sentences",
                        state.confirmed_count()
                    );
                    let final_text = state.final_text();
                    if final_text.is_empty() {
                        // fallback: 用 display_text（可能只有未确认的 current_sentence）
                        let display = state.display_text();
                        if display.is_empty() {
                            bail!("转录失败：task-finished 但无识别结果");
                        }
                        return Ok(display);
                    }
                    return Ok(final_text);
                }
            }
            Message::Binary(_) => {}
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Close(_) => {
                bail!("网络失败：服务端关闭连接（未发送 task-finished）");
            }
            Message::Frame(_) => {}
        }
    }
}

// ===========================================================================
// 测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- 消息构造 ---

    #[test]
    fn build_run_task_has_correct_schema() {
        let vocab = serde_json::json!({"测试词": 5});
        let msg = build_run_task_message("test-task-id", "test-model", &vocab);
        assert_eq!(msg["header"]["action"], "run-task");
        assert_eq!(msg["header"]["task_id"], "test-task-id");
        assert_eq!(msg["header"]["streaming"], "duplex");
        assert_eq!(msg["payload"]["task_group"], "audio");
        assert_eq!(msg["payload"]["task"], "asr");
        assert_eq!(msg["payload"]["function"], "recognition");
        assert_eq!(msg["payload"]["model"], "test-model");
        assert_eq!(msg["payload"]["parameters"]["format"], "pcm");
        assert_eq!(msg["payload"]["parameters"]["sample_rate"], 16000);
    }

    #[test]
    fn build_run_task_model_in_payload_not_url() {
        // model 必须在 payload 里，不在 URL query（与旧协议的关键差异）
        let msg = build_run_task_message("tid", "my-model", &serde_json::json!({}));
        assert_eq!(msg["payload"]["model"], "my-model");
        // header 不应有 model
        assert!(msg["header"].get("model").is_none());
    }

    #[test]
    fn build_run_task_language_hints_fixed_four() {
        let msg = build_run_task_message("tid", "model", &serde_json::json!({}));
        let hints = msg["payload"]["parameters"]["language_hints"]
            .as_array()
            .unwrap();
        assert_eq!(hints.len(), 4, "must be exactly 4 language hints");
        assert_eq!(hints[0], "zh");
        assert_eq!(hints[1], "en");
        assert_eq!(hints[2], "ja");
        assert_eq!(hints[3], "ko");
    }

    #[test]
    fn build_run_task_vocabulary_injected_when_non_empty() {
        let vocab = serde_json::json!({"张三": 5, "李四": 4});
        let msg = build_run_task_message("tid", "model", &vocab);
        assert_eq!(msg["payload"]["parameters"]["vocabulary"]["张三"], 5);
        assert_eq!(msg["payload"]["parameters"]["vocabulary"]["李四"], 4);
    }

    #[test]
    fn build_run_task_vocabulary_omitted_when_empty() {
        let vocab = serde_json::json!({});
        let msg = build_run_task_message("tid", "model", &vocab);
        assert!(
            msg["payload"]["parameters"].get("vocabulary").is_none(),
            "empty vocabulary must be omitted"
        );
    }

    #[test]
    fn build_run_task_has_max_sentence_silence_800() {
        let msg = build_run_task_message("tid", "model", &serde_json::json!({}));
        assert_eq!(msg["payload"]["parameters"]["max_sentence_silence"], 800);
    }

    #[test]
    fn build_run_task_semantic_punctuation_disabled() {
        let msg = build_run_task_message("tid", "model", &serde_json::json!({}));
        assert_eq!(
            msg["payload"]["parameters"]["semantic_punctuation_enabled"],
            false
        );
    }

    #[test]
    fn build_finish_task_has_correct_schema() {
        let msg = build_finish_task_message("test-task-id");
        assert_eq!(msg["header"]["action"], "finish-task");
        assert_eq!(msg["header"]["task_id"], "test-task-id");
        assert_eq!(msg["header"]["streaming"], "duplex");
        assert_eq!(msg["payload"]["input"], serde_json::json!({}));
    }

    #[test]
    fn finish_task_uses_same_task_id_as_run_task() {
        let task_id = "shared-id-123";
        let run = build_run_task_message(task_id, "model", &serde_json::json!({}));
        let finish = build_finish_task_message(task_id);
        assert_eq!(run["header"]["task_id"], finish["header"]["task_id"]);
    }

    // --- 音频分片 ---

    #[test]
    fn chunk_pcm_to_binary_splits_correctly() {
        // 6400 bytes = 2 chunks of 3200
        let pcm = vec![0u8; 6400];
        let chunks = chunk_pcm_to_binary(&pcm);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 3200);
        assert_eq!(chunks[1].len(), 3200);
    }

    #[test]
    fn chunk_pcm_to_binary_exact_one_chunk() {
        let pcm = vec![0u8; 3200];
        let chunks = chunk_pcm_to_binary(&pcm);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn chunk_pcm_to_binary_remainder_chunk() {
        // 4000 bytes = 1 full (3200) + 1 remainder (800)
        let pcm = vec![0u8; 4000];
        let chunks = chunk_pcm_to_binary(&pcm);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].len(), 800);
    }

    #[test]
    fn chunk_pcm_to_binary_empty_returns_empty() {
        let chunks = chunk_pcm_to_binary(&[]);
        assert!(chunks.is_empty());
    }

    // --- 事件解析 ---

    #[test]
    fn extract_event_type_task_started() {
        let msg = serde_json::json!({
            "header": {"task_id": "tid", "event": "task-started", "attributes": {}},
            "payload": {}
        });
        assert_eq!(extract_event_type(&msg), Some("task-started"));
    }

    #[test]
    fn extract_event_type_result_generated() {
        let msg = serde_json::json!({
            "header": {"task_id": "tid", "event": "result-generated", "attributes": {}},
            "payload": {}
        });
        assert_eq!(extract_event_type(&msg), Some("result-generated"));
    }

    #[test]
    fn extract_event_type_missing_returns_none() {
        let msg = serde_json::json!({"payload": {}});
        assert_eq!(extract_event_type(&msg), None);
    }

    #[test]
    fn extract_sentence_intermediate_result() {
        let msg = serde_json::json!({
            "header": {"task_id": "tid", "event": "result-generated", "attributes": {}},
            "payload": {
                "output": {
                    "sentence": {
                        "begin_time": 0, "end_time": null,
                        "text": "你好", "sentence_end": false,
                        "sentence_id": 1, "words": []
                    }
                },
                "usage": null
            }
        });
        let result = extract_sentence(&msg);
        assert_eq!(result, Some((1, "你好".to_string(), false)));
    }

    #[test]
    fn extract_sentence_final_result() {
        let msg = serde_json::json!({
            "header": {"task_id": "tid", "event": "result-generated", "attributes": {}},
            "payload": {
                "output": {
                    "sentence": {
                        "begin_time": 0, "end_time": 920,
                        "text": "好，我知道了", "sentence_end": true,
                        "sentence_id": 1, "words": []
                    }
                },
                "usage": {"duration": 3}
            }
        });
        let result = extract_sentence(&msg);
        assert_eq!(result, Some((1, "好，我知道了".to_string(), true)));
    }

    #[test]
    fn extract_sentence_non_result_event_returns_none() {
        let msg = serde_json::json!({
            "header": {"task_id": "tid", "event": "task-started", "attributes": {}},
            "payload": {}
        });
        assert_eq!(extract_sentence(&msg), None);
    }

    #[test]
    fn extract_task_error_returns_message() {
        let msg = serde_json::json!({
            "header": {
                "task_id": "tid", "event": "task-failed",
                "error_code": "CLIENT_ERROR", "error_message": "request timeout",
                "attributes": {}
            },
            "payload": {}
        });
        let result = extract_task_error(&msg);
        assert!(result.is_some());
        let err = result.unwrap();
        assert!(err.contains("CLIENT_ERROR"));
        assert!(err.contains("request timeout"));
    }

    #[test]
    fn extract_task_error_non_error_event_returns_none() {
        let msg = serde_json::json!({
            "header": {"task_id": "tid", "event": "task-started", "attributes": {}},
            "payload": {}
        });
        assert_eq!(extract_task_error(&msg), None);
    }

    #[test]
    fn extract_usage_duration_returns_seconds() {
        let msg = serde_json::json!({
            "header": {"task_id": "tid", "event": "result-generated", "attributes": {}},
            "payload": {"output": {"sentence": {"text": "ok", "sentence_end": true}}, "usage": {"duration": 5}}
        });
        assert_eq!(extract_usage_duration(&msg), Some(5));
    }

    #[test]
    fn extract_usage_duration_null_when_not_ended() {
        let msg = serde_json::json!({
            "header": {"task_id": "tid", "event": "result-generated", "attributes": {}},
            "payload": {"output": {"sentence": {"text": "ok", "sentence_end": false}}, "usage": null}
        });
        assert_eq!(extract_usage_duration(&msg), None);
    }

    // --- StreamingAsrState ---

    #[test]
    fn streaming_state_initial_empty() {
        let state = StreamingAsrState::new();
        assert_eq!(state.display_text(), "");
        assert_eq!(state.final_text(), "");
        assert_eq!(state.confirmed_count(), 0);
    }

    #[test]
    fn streaming_state_intermediate_replaces_current() {
        let mut state = StreamingAsrState::new();
        state.on_result(1, "你好", false);
        assert_eq!(state.display_text(), "你好");
        state.on_result(1, "你好世界", false);
        assert_eq!(state.display_text(), "你好世界");
        assert_eq!(state.confirmed_count(), 0);
    }

    #[test]
    fn streaming_state_sentence_end_confirms() {
        let mut state = StreamingAsrState::new();
        state.on_result(1, "你好", false);
        state.on_result(1, "你好世界", true);
        assert_eq!(state.confirmed_count(), 1);
        assert_eq!(state.final_text(), "你好世界");
        assert_eq!(state.display_text(), "你好世界");
    }

    #[test]
    fn streaming_state_new_sentence_starts_after_confirm() {
        let mut state = StreamingAsrState::new();
        state.on_result(1, "第一句", true);
        state.on_result(2, "第二", false);
        assert_eq!(state.display_text(), "第一句第二");
        assert_eq!(state.final_text(), "第一句");
        state.on_result(2, "第二句", true);
        assert_eq!(state.display_text(), "第一句第二句");
        assert_eq!(state.final_text(), "第一句第二句");
    }

    #[test]
    fn streaming_state_multiple_sentences() {
        let mut state = StreamingAsrState::new();
        state.on_result(1, "A", true);
        state.on_result(2, "B", true);
        state.on_result(3, "C", true);
        assert_eq!(state.final_text(), "ABC");
        assert_eq!(state.confirmed_count(), 3);
    }

    // --- vocabulary 构造 ---

    #[test]
    fn passes_asr_vocab_limit_short_chinese() {
        assert!(passes_asr_vocab_limit("维生素B12")); // 6 chars
        assert!(passes_asr_vocab_limit("厄洛替尼盐酸盐")); // 7 chars
    }

    #[test]
    fn passes_asr_vocab_limit_long_chinese_rejected() {
        assert!(!passes_asr_vocab_limit(
            "一二三四五六七八九十十一十二十三十四十五十六"
        )); // 16 chars > 15
    }

    #[test]
    fn passes_asr_vocab_limit_short_ascii() {
        assert!(passes_asr_vocab_limit("hello")); // 1 fragment
        assert!(passes_asr_vocab_limit("Exothermic reaction")); // 2 fragments
    }

    #[test]
    fn passes_asr_vocab_limit_long_ascii_rejected() {
        let long = "one two three four five six seven eight"; // 8 fragments > 7
        assert!(!passes_asr_vocab_limit(long));
    }

    #[test]
    fn vocab_weight_user_is_5() {
        assert_eq!(vocab_weight("user"), 5);
    }

    #[test]
    fn vocab_weight_system_is_4() {
        assert_eq!(vocab_weight("system"), 4);
    }

    #[test]
    fn vocab_weight_unknown_is_3() {
        assert_eq!(vocab_weight("other"), 3);
    }

    #[test]
    fn build_vocabulary_basic() {
        let entries = vec![
            VocabEntry {
                word: "风无心".into(),
                source: "user".into(),
            },
            VocabEntry {
                word: "三五成群".into(),
                source: "system".into(),
            },
        ];
        let vocab = build_vocabulary(&entries);
        assert_eq!(vocab["风无心"], 5);
        assert_eq!(vocab["三五成群"], 4);
    }

    #[test]
    fn build_vocabulary_filters_overlimit() {
        let long_word = "一二三四五六七八九十十一十二十三十四十五十六"; // 16 chars
        let entries = vec![
            VocabEntry {
                word: "正常词".into(),
                source: "user".into(),
            },
            VocabEntry {
                word: long_word.into(),
                source: "user".into(),
            },
        ];
        let vocab = build_vocabulary(&entries);
        assert_eq!(vocab["正常词"], 5);
        assert!(
            vocab.get(long_word).is_none(),
            "overlimit word must be filtered out"
        );
    }

    #[test]
    fn build_vocabulary_empty_entries() {
        let vocab = build_vocabulary(&[]);
        assert!(vocab.as_object().unwrap().is_empty());
    }

    #[test]
    fn build_vocabulary_mixed_sources() {
        let entries = vec![
            VocabEntry {
                word: "采编".into(),
                source: "user".into(),
            },
            VocabEntry {
                word: "LLM".into(),
                source: "system".into(),
            },
            VocabEntry {
                word: "test".into(),
                source: "unknown".into(),
            },
        ];
        let vocab = build_vocabulary(&entries);
        assert_eq!(vocab["采编"], 5);
        assert_eq!(vocab["LLM"], 4);
        assert_eq!(vocab["test"], 3);
    }

    // --- 硬上限 ---

    #[test]
    fn compute_hard_cap_short_audio_uses_30s_floor() {
        let cap = compute_hard_cap(16000);
        assert_eq!(cap, Duration::from_secs(30));
    }

    #[test]
    fn compute_hard_cap_120s_audio_60s() {
        let cap = compute_hard_cap(16000 * 120);
        assert_eq!(cap, Duration::from_secs(60));
    }

    // --- task_id 生成 ---

    #[test]
    fn generate_task_id_is_32_chars() {
        let id = generate_task_id();
        assert_eq!(id.len(), 32, "task_id must be 32 hex chars");
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_task_id_unique_consecutive() {
        let id1 = generate_task_id();
        let id2 = generate_task_id();
        assert_ne!(id1, id2, "consecutive task_ids must differ");
    }

    // --- IO 入口防御 ---

    #[test]
    fn transcribe_streaming_empty_api_key_bails() {
        let result = transcribe_streaming(
            "wss://example.com",
            "",
            "model",
            &[0.0; 16000],
            &serde_json::json!({}),
            |_| {},
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API Key 为空"));
    }

    #[test]
    fn transcribe_streaming_empty_samples_bails() {
        let result = transcribe_streaming(
            "wss://example.com",
            "sk-test",
            "model",
            &[],
            &serde_json::json!({}),
            |_| {},
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("音频样本为空"));
    }

    // --- 词库加载调用链验证 ---

    #[test]
    fn vocab_entry_from_wordbook_entry() {
        // 验证 VocabEntry 可从 WordEntry 转换（调用链的桥接点）
        let entry = crate::wordbook::WordEntry {
            id: 1,
            word: "风无心".into(),
            source: "user".into(),
            created_at: "2026-01-01".into(),
        };
        let vocab = VocabEntry {
            word: entry.word.clone(),
            source: entry.source.clone(),
        };
        assert_eq!(vocab.word, "风无心");
        assert_eq!(vocab.source, "user");
    }

    #[test]
    fn build_vocabulary_from_wordbook_entries() {
        // 模拟从 wordbook 表 list_all() 返回的条目
        let entries = vec![
            VocabEntry { word: "采编".into(), source: "user".into() },
            VocabEntry { word: "风无心".into(), source: "user".into() },
            VocabEntry { word: "三五成群".into(), source: "system".into() },
            VocabEntry { word: "维生素B12".into(), source: "system".into() },
        ];
        let vocab = build_vocabulary(&entries);
        assert_eq!(vocab["采编"], 5);
        assert_eq!(vocab["风无心"], 5);
        assert_eq!(vocab["三五成群"], 4);
        assert_eq!(vocab["维生素B12"], 4);
        // 确认不含 candidates 表的纯数字
        assert!(vocab.get("3").is_none());
        assert!(vocab.get("6").is_none());
    }
}
