//! Qwen3 在线 ASR 模块（DEC-028）
//!
//! 接入阿里百炼 qwen3-asr-flash-realtime 在线模型，作为第三个 ASR 选项。
//! 协议：OpenAI Realtime 风格 WebSocket——
//!   wss://dashscope.aliyuncs.com/api-ws/v1/realtime?model=qwen3-asr-flash-realtime
//!   Authorization: Bearer <API_KEY> + OpenAI-Beta: realtime=v1
//! 流程：session.update（pcm/16kHz，turn_detection=null 手动模式）
//!   → input_audio_buffer.append（base64 分块）
//!   → input_audio_buffer.commit
//!   → 收 conversation.item.input_audio_transcription.completed 取最终文本
//!
//! 失败行为（DEC-028 Gavin 拍板）：断网/超时/key 无效 → 报错提示转录失败，不自动降级本地

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tungstenite::client_tls_with_config;
use tungstenite::http::Uri;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{ClientRequestBuilder, Error as WsError, HandshakeError, Message, WebSocket};

/// 连接超时（DEC-028：5s）
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// 单块样本数（200ms @ 16kHz，DEC-028 建议）
const CHUNK_SAMPLES: usize = 3200;

/// 静默超时（R2：commit 后服务端连续无消息即异常）
const SILENCE_TIMEOUT: Duration = Duration::from_secs(10);

/// 构造 session.update 请求消息（纯函数，可单测）
///
/// 官方 schema（阿里云 Model Studio qwen-asr-realtime-client-events）：
/// - `modalities: ["text"]`（对齐官方示例）
/// - `input_audio_format: "pcm"` + 独立 `sample_rate: 16000`（拆开，非合并串 "pcm/16000"）
/// - `input_audio_transcription`: 仅语言明确时传 `{language}`，auto 时省略（服务端自动检测）
/// - `turn_detection: null` 手动模式
///
/// language = None 时省略 input_audio_transcription 整个字段（交服务端自动检测语言）。
pub fn build_session_update_message(language: Option<&str>) -> serde_json::Value {
    let mut session = serde_json::json!({
        "type": "session.update",
        "session": {
            "modalities": ["text"],
            "input_audio_format": "pcm",
            "sample_rate": 16000,
            "turn_detection": null,
        }
    });
    if let Some(lang) = language {
        session["session"]["input_audio_transcription"] = serde_json::json!({
            "language": lang
        });
    }
    session
}

/// 构造 input_audio_buffer.append 请求消息（纯函数，可单测）
pub fn build_append_message(audio_b64: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "input_audio_buffer.append",
        "audio": audio_b64
    })
}

/// 构造 input_audio_buffer.commit 请求消息（纯函数，可单测）
pub fn build_commit_message() -> serde_json::Value {
    serde_json::json!({
        "type": "input_audio_buffer.commit"
    })
}

/// f32 样本 → PCM16 LE bytes（纯函数，可单测）
///
/// 转换公式：`(s * 32768.0).clamp(-32768.0, 32767.0) as i16`
/// - 1.0 → 32767（clamp 上限）
/// - -1.0 → -32768
/// - 0.0 → 0
pub fn f32_to_pcm16_le(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let pcm = (s * 32768.0).clamp(-32768.0, 32767.0) as i16;
        bytes.extend_from_slice(&pcm.to_le_bytes());
    }
    bytes
}

/// PCM bytes → base64 分块（纯函数，可单测）
///
/// 每块 ~3200 samples=200ms（DEC-028 建议块大小）
pub fn chunk_pcm_to_base64(pcm: &[u8]) -> Vec<String> {
    let chunk_bytes = CHUNK_SAMPLES * 2; // 16-bit = 2 bytes/sample
    let engine = base64::engine::general_purpose::STANDARD;
    pcm.chunks(chunk_bytes)
        .map(|chunk| engine.encode(chunk))
        .collect()
}

/// 计算硬上限（R2：仅 commit 后总时长保险丝，非超时策略）
///
/// 公式：`max(30s, 音频时长 × 0.5)`，防服务端持续发消息但永不给最终结果的病态情况。
pub fn compute_hard_cap(audio_samples: usize) -> Duration {
    let audio_secs = audio_samples as f64 / 16000.0;
    let cap_secs = (30.0_f64).max(audio_secs * 0.5);
    Duration::from_secs_f64(cap_secs)
}

/// 从服务端 JSON 消息提取转录文本（纯函数，可单测）
///
/// 识别 `conversation.item.input_audio_transcription.completed` 类型的消息，
/// 从 `transcript` 字段取文本。其他类型返回 None。
pub fn extract_transcript(msg: &serde_json::Value) -> Option<String> {
    let msg_type = msg.get("type")?.as_str()?;
    if msg_type == "conversation.item.input_audio_transcription.completed" {
        msg.get("transcript")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
    } else {
        None
    }
}

/// 从服务端 JSON 消息提取错误信息（纯函数，可单测）
///
/// 识别 `error` 类型的消息，返回格式化错误字符串。
pub fn extract_error(msg: &serde_json::Value) -> Option<String> {
    let msg_type = msg.get("type")?.as_str()?;
    if msg_type == "error" {
        let error = msg.get("error")?;
        let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
        let code = error.get("code").and_then(|c| c.as_str()).unwrap_or("unknown");
        Some(format!("server error [{}]: {}", code, message))
    } else {
        None
    }
}

/// 在线 ASR 转录主入口
///
/// 协议流程（DEC-028 + R2）：
/// 1. connect（url + ?model=<model_id>，Header 鉴权，连接超时 5s）
/// 2. send session.update（官方 schema：pcm + sample_rate + modalities + language）
/// 3. 逐块 input_audio_buffer.append（base64 分块 200ms）
/// 4. send input_audio_buffer.commit
/// 5. 等 conversation.item.input_audio_transcription.completed 取最终文本
///    - 静默超时 10s（commit 后任何消息重置计时器）
///    - 硬上限 max(30s, 音频时长×0.5) 总保险丝
///
/// `model_id` 从配置文件读取（DEC-028 移出硬编码，2026-07-07）。
/// `language` = None 时 session 中省略 language 字段（服务端自动检测），
/// 明确值（zh/en 等）传入。
///
/// 失败路径映射 anyhow Err（Gavin 拍板：报错不降级）：
/// - 鉴权失败（401/403）→ "鉴权失败"
/// - 网络失败（连接/读写错误）→ "网络失败"
/// - 超时（服务端 10s 无响应或超过硬上限）→ "超时"
pub fn transcribe_online(
    url: &str,
    api_key: &str,
    model_id: &str,
    samples_16k: &[f32],
    language: Option<&str>,
) -> Result<String> {
    if api_key.trim().is_empty() {
        bail!("Qwen3 ASR 鉴权失败：API Key 为空");
    }
    if samples_16k.is_empty() {
        bail!("Qwen3 ASR 转录失败：音频样本为空");
    }

    // URL 追加 model 查询参数（model_id 从配置文件读取）
    let full_url = if url.contains('?') {
        format!("{}&model={}", url, model_id)
    } else {
        format!("{}?model={}", url, model_id)
    };

    let hard_cap = compute_hard_cap(samples_16k.len());
    log::info!(
        "Qwen3 ASR connecting to {} ({} samples = {:.1}s, hard_cap={:.0}s, language={:?})",
        full_url,
        samples_16k.len(),
        samples_16k.len() as f64 / 16000.0,
        hard_cap.as_secs_f64(),
        language,
    );

    // DNS 解析 + TCP 连接（CONNECT_TIMEOUT=5s，DEC-028）
    let uri: Uri = full_url.parse().context("Invalid Qwen3 ASR URL")?;
    let host = uri.host().context("Qwen3 ASR 网络失败：URL 缺少 host")?;
    let port = uri.port_u16().unwrap_or(443);
    let addrs = format!("{}:{}", host, port)
        .to_socket_addrs()
        .context("Qwen3 ASR 网络失败：DNS 解析失败")?;
    let socket_addrs: Vec<_> = addrs.collect();
    let addr = socket_addrs.first().context("Qwen3 ASR 网络失败：DNS 未返回地址")?;
    let tcp = TcpStream::connect_timeout(addr, CONNECT_TIMEOUT)
        .context("Qwen3 ASR 网络失败：连接超时")?;
    tcp.set_read_timeout(Some(CONNECT_TIMEOUT))
        .context("Qwen3 ASR 网络失败：设置读取超时失败")?;
    tcp.set_write_timeout(Some(CONNECT_TIMEOUT))
        .context("Qwen3 ASR 网络失败：设置写入超时失败")?;

    // 构造请求 + TLS/WS 握手
    let request = ClientRequestBuilder::new(uri)
        .with_header("Authorization", format!("Bearer {}", api_key))
        .with_header("OpenAI-Beta", "realtime=v1");
    let (mut ws_socket, response) = client_tls_with_config(request, tcp, None, None)
        .map_err(|e| match e {
            HandshakeError::Failure(e) => map_connect_error(&e),
            HandshakeError::Interrupted(_) => {
                anyhow!("Qwen3 ASR 网络失败：TLS 握手意外中断")
            }
        })?;

    // WS 握手成功的状态码是 101 Switching Protocols（非 2xx）。
    // 鉴权失败（401/403）等异常由 tungstenite 握手层以 Err 返回（map_connect_error 处理），
    // 能走到这里 status 必为 101。曾误用 is_success()（仅认 2xx）把成功握手判为
    // "HTTP 101 - 连接被拒绝"（BUG-QWEN3-STATUS-001，2026-07-08 Gavin 端测暴露）。
    debug_assert_eq!(response.status().as_u16(), 101, "post-handshake status must be 101");
    let _ = &response;

    // 设置 socket 读写超时（10s tick），作为静默超时 + 硬上限的计时粒度
    set_socket_timeouts(&mut ws_socket, Duration::from_secs(10), Duration::from_secs(10))
        .context("Qwen3 ASR 网络失败：设置 socket 超时失败")?;

    // 1. send session.update（官方 schema 对齐）
    let session_update = build_session_update_message(language);
    send_json(&mut ws_socket, &session_update)?;

    // 2. 逐块 input_audio_buffer.append
    let pcm = f32_to_pcm16_le(samples_16k);
    let chunks = chunk_pcm_to_base64(&pcm);
    log::info!("Qwen3 ASR uploading {} chunks ({}ms each)", chunks.len(), CHUNK_SAMPLES as u64 * 1000 / 16000);
    for chunk in &chunks {
        let append_msg = build_append_message(chunk);
        send_json(&mut ws_socket, &append_msg)?;
    }

    // 3. send commit — 从此起计时
    let commit_msg = build_commit_message();
    send_json(&mut ws_socket, &commit_msg)?;
    let commit_time = std::time::Instant::now();
    let hard_cap_deadline = commit_time + hard_cap;
    let mut last_activity = commit_time;

    // 4. 等待 conversation.item.input_audio_transcription.completed
    loop {
        let now = std::time::Instant::now();
        if now > hard_cap_deadline {
            bail!(
                "Qwen3 ASR 超时：超过硬上限 {:.0}s 仍未收到转录结果",
                hard_cap.as_secs_f64()
            );
        }
        let msg = match ws_socket.read() {
            Ok(m) => m,
            Err(e) if is_read_timeout(&e) => {
                // 10s tick 到期——检查静默超时。
                // 必须取新时间戳：循环头的 now 是本次阻塞 read 之前的值，
                // 用它会让静默报错延迟一个 tick（~20s 才触发）。
                if last_activity.elapsed() >= SILENCE_TIMEOUT {
                    bail!("Qwen3 ASR 超时：服务端 10s 无响应");
                }
                continue;
            }
            Err(e) => bail!("Qwen3 ASR 网络失败：读取消息失败 - {}", e),
        };
        // 任何消息收到即重置静默计时
        last_activity = std::time::Instant::now();
        match msg {
            Message::Text(text) => {
                let parsed: serde_json::Value = serde_json::from_str(&text)
                    .context("Qwen3 ASR 网络失败：服务端返回非 JSON 文本")?;
                if let Some(err_msg) = extract_error(&parsed) {
                    bail!("Qwen3 ASR 服务端错误：{}", err_msg);
                }
                if let Some(transcript) = extract_transcript(&parsed) {
                    log::info!("Qwen3 ASR transcription completed: {}", transcript);
                    return Ok(transcript);
                }
                // 其他消息类型（session.updated / input_audio_buffer.committed 等）忽略继续等
            }
            Message::Binary(_) => {
                // ASR 服务端不应发 binary，忽略
            }
            Message::Ping(_) | Message::Pong(_) => {
                // WebSocket keepalive 重置静默计时
            }
            Message::Close(_) => {
                bail!("Qwen3 ASR 网络失败：服务端关闭连接（未发送转录结果）");
            }
            Message::Frame(_) => {
                // Raw frame，忽略
            }
        }
    }
}

/// 发送 JSON 消息到 WebSocket
fn send_json(
    socket: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    value: &serde_json::Value,
) -> Result<()> {
    let text = serde_json::to_string(value).context("序列化 WS 消息失败")?;
    socket
        .send(Message::Text(text.into()))
        .map_err(|e| anyhow!("Qwen3 ASR 网络失败：发送消息失败 - {}", e))
}

/// 判断 tungstenite 错误是否为 socket 读超时
///
/// P0 修复（2026-07-07 R1）：超时不是失败，read 循环应 continue 回到循环头重查 deadline。
/// Windows 阻塞 socket 超时返回 TimedOut，Unix 返回 WouldBlock，两者都要覆盖。
pub fn is_read_timeout(e: &WsError) -> bool {
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
        anyhow!("Qwen3 ASR 鉴权失败：{}", msg)
    } else {
        anyhow!("Qwen3 ASR 网络失败：连接失败 - {}", msg)
    }
}

/// 设置 WebSocket socket 读写超时（防止 worker 线程无限阻塞）
///
/// 每个 read/write 操作最多阻塞此时间，配合 transcribe_online 的 deadline 循环
/// 实现整体超时（DEC-028：不能在网络异常时无限阻塞 worker 线程）。
fn set_socket_timeouts(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    read_timeout: Duration,
    write_timeout: Duration,
) -> Result<()> {
    use tungstenite::stream::MaybeTlsStream::*;
    match socket.get_mut() {
        Plain(tcp) => {
            tcp.set_read_timeout(Some(read_timeout))
                .context("Qwen3 ASR 网络失败：设置读取超时失败")?;
            tcp.set_write_timeout(Some(write_timeout))
                .context("Qwen3 ASR 网络失败：设置写入超时失败")?;
        }
        Rustls(stream_owned) => {
            stream_owned
                .sock
                .set_read_timeout(Some(read_timeout))
                .context("Qwen3 ASR 网络失败：设置读取超时失败")?;
            stream_owned
                .sock
                .set_write_timeout(Some(write_timeout))
                .context("Qwen3 ASR 网络失败：设置写入超时失败")?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_session_update_has_official_schema() {
        let msg = build_session_update_message(Some("zh"));
        assert_eq!(msg["type"], "session.update");
        assert_eq!(msg["session"]["modalities"][0], "text");
        assert_eq!(msg["session"]["input_audio_format"], "pcm");
        assert_eq!(msg["session"]["sample_rate"], 16000);
        assert!(msg["session"]["turn_detection"].is_null());
        assert_eq!(msg["session"]["input_audio_transcription"]["language"], "zh");
    }

    #[test]
    fn build_session_update_omits_language_when_auto() {
        let msg = build_session_update_message(None);
        assert_eq!(msg["type"], "session.update");
        assert_eq!(msg["session"]["input_audio_format"], "pcm");
        assert_eq!(msg["session"]["sample_rate"], 16000);
        assert_eq!(msg["session"]["modalities"][0], "text");
        // language=None 时应有 modalities/format/sample_rate，无 input_audio_transcription
        assert!(msg["session"].get("input_audio_transcription").is_none(),
            "language=None must omit input_audio_transcription entirely");
    }

    #[test]
    fn build_session_update_uses_specified_language() {
        let msg = build_session_update_message(Some("en"));
        assert_eq!(msg["session"]["input_audio_transcription"]["language"], "en");
    }

    #[test]
    fn build_append_message_has_audio_field() {
        let msg = build_append_message("dGVzdA==");
        assert_eq!(msg["type"], "input_audio_buffer.append");
        assert_eq!(msg["audio"], "dGVzdA==");
    }

    #[test]
    fn build_commit_message_has_only_type() {
        let msg = build_commit_message();
        assert_eq!(msg["type"], "input_audio_buffer.commit");
        assert_eq!(msg.as_object().unwrap().len(), 1, "commit msg must have only type field");
    }

    #[test]
    fn f32_to_pcm16_le_converts_correctly() {
        // 0.0 → 0, 1.0 → 32767 (clamp), -1.0 → -32768, 0.5 → 16384
        let samples = vec![0.0f32, 1.0, -1.0, 0.5];
        let bytes = f32_to_pcm16_le(&samples);
        assert_eq!(bytes.len(), 8); // 4 samples × 2 bytes
        assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 0);
        assert_eq!(i16::from_le_bytes([bytes[2], bytes[3]]), 32767);
        assert_eq!(i16::from_le_bytes([bytes[4], bytes[5]]), -32768);
        assert_eq!(i16::from_le_bytes([bytes[6], bytes[7]]), 16384); // 0.5 × 32768 = 16384
    }

    #[test]
    fn f32_to_pcm16_clamps_overshoot() {
        // 超过 1.0 的值应 clamp 到 32767
        let samples = vec![2.0f32, -2.0];
        let bytes = f32_to_pcm16_le(&samples);
        assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 32767);
        assert_eq!(i16::from_le_bytes([bytes[2], bytes[3]]), -32768);
    }

    #[test]
    fn chunk_pcm_to_base64_splits_correctly() {
        // 6400 samples = 12800 bytes = 2 chunks (6400 bytes each)
        let samples: Vec<f32> = vec![0.0; 6400];
        let pcm = f32_to_pcm16_le(&samples);
        assert_eq!(pcm.len(), 12800);
        let chunks = chunk_pcm_to_base64(&pcm);
        assert_eq!(chunks.len(), 2, "6400 samples must split into 2 chunks of 3200 samples");
        // 每块 base64 解码后应 6400 bytes
        let engine = base64::engine::general_purpose::STANDARD;
        let decoded0 = engine.decode(&chunks[0]).unwrap();
        assert_eq!(decoded0.len(), 6400);
    }

    #[test]
    fn chunk_pcm_to_base64_exact_one_chunk() {
        // 3200 samples = 1 chunk
        let samples: Vec<f32> = vec![0.0; 3200];
        let pcm = f32_to_pcm16_le(&samples);
        let chunks = chunk_pcm_to_base64(&pcm);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn chunk_pcm_to_base64_remainder_chunk() {
        // 4000 samples = 1 full chunk (3200) + 1 remainder (800)
        let samples: Vec<f32> = vec![0.0; 4000];
        let pcm = f32_to_pcm16_le(&samples);
        let chunks = chunk_pcm_to_base64(&pcm);
        assert_eq!(chunks.len(), 2);
        let engine = base64::engine::general_purpose::STANDARD;
        let decoded1 = engine.decode(&chunks[1]).unwrap();
        assert_eq!(decoded1.len(), 1600, "remainder chunk = 800 samples × 2 bytes");
    }

    #[test]
    fn chunk_pcm_empty_returns_empty() {
        let chunks = chunk_pcm_to_base64(&[]);
        assert!(chunks.is_empty());
    }

    #[test]
    fn compute_hard_cap_short_audio_uses_30s_floor() {
        // 1s 音频 → max(30, 0.5) = 30s
        let cap = compute_hard_cap(16000);
        assert_eq!(cap, Duration::from_secs(30));
    }

    #[test]
    fn compute_hard_cap_60s_audio_30s() {
        // 60s 音频 → max(30, 30) = 30s（边界）
        let cap = compute_hard_cap(16000 * 60);
        assert_eq!(cap, Duration::from_secs(30));
    }

    #[test]
    fn compute_hard_cap_120s_audio_60s() {
        // 120s 音频 → max(30, 60) = 60s
        let cap = compute_hard_cap(16000 * 120);
        assert_eq!(cap, Duration::from_secs(60));
    }

    #[test]
    fn compute_hard_cap_boundary_15s_audio() {
        // 15s 音频 → max(30, 7.5) = 30s
        let cap = compute_hard_cap(16000 * 15);
        assert_eq!(cap, Duration::from_secs(30));
    }

    #[test]
    fn extract_transcript_completed_message() {
        let msg = serde_json::json!({
            "type": "conversation.item.input_audio_transcription.completed",
            "transcript": "你好世界"
        });
        assert_eq!(extract_transcript(&msg), Some("你好世界".to_string()));
    }

    #[test]
    fn extract_transcript_other_message_returns_none() {
        let msg = serde_json::json!({
            "type": "session.updated"
        });
        assert_eq!(extract_transcript(&msg), None);
    }

    #[test]
    fn extract_transcript_missing_transcript_field() {
        let msg = serde_json::json!({
            "type": "conversation.item.input_audio_transcription.completed"
        });
        assert_eq!(extract_transcript(&msg), None);
    }

    #[test]
    fn extract_error_message() {
        let msg = serde_json::json!({
            "type": "error",
            "error": {
                "code": "invalid_api_key",
                "message": "API key is invalid"
            }
        });
        let result = extract_error(&msg);
        assert!(result.is_some());
        let err = result.unwrap();
        assert!(err.contains("invalid_api_key"));
        assert!(err.contains("API key is invalid"));
    }

    #[test]
    fn extract_error_non_error_message_returns_none() {
        let msg = serde_json::json!({"type": "session.updated"});
        assert_eq!(extract_error(&msg), None);
    }

    #[test]
    fn extract_error_missing_fields_uses_defaults() {
        let msg = serde_json::json!({
            "type": "error",
            "error": {}
        });
        let result = extract_error(&msg);
        assert!(result.is_some());
        assert!(result.unwrap().contains("unknown"));
    }

    #[test]
    fn is_read_timeout_detects_timed_out() {
        let err = WsError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out"));
        assert!(is_read_timeout(&err), "TimedOut must be detected as read timeout");
    }

    #[test]
    fn is_read_timeout_detects_would_block() {
        let err = WsError::Io(std::io::Error::new(std::io::ErrorKind::WouldBlock, "would block"));
        assert!(is_read_timeout(&err), "WouldBlock must be detected as read timeout");
    }

    #[test]
    fn is_read_timeout_returns_false_for_other_io_errors() {
        let err = WsError::Io(std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset"));
        assert!(!is_read_timeout(&err), "ConnectionReset must NOT be detected as read timeout");
    }

    #[test]
    fn is_read_timeout_returns_false_for_non_io_errors() {
        let err = WsError::Protocol(tungstenite::error::ProtocolError::WrongHttpMethod);
        assert!(!is_read_timeout(&err), "Protocol error must NOT be detected as read timeout");
    }

    #[test]
    fn transcribe_online_empty_api_key_bails() {
        let result = transcribe_online("wss://example.com", "", "qwen3-asr-flash-realtime", &[0.0; 16000], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("鉴权失败"), "empty key must bail with auth failure, got: {}", err);
        assert!(err.contains("API Key 为空"));
    }

    #[test]
    fn transcribe_online_empty_samples_bails() {
        let result = transcribe_online("wss://example.com", "sk-test", "qwen3-asr-flash-realtime", &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("音频样本为空"));
    }

    #[test]
    #[ignore = "requires network + valid API key (Gavin 端测)"]
    fn transcribe_online_integration() {
        // 真实联网调用，留给 Gavin 端测
    }
}