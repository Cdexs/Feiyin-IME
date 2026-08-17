use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderValue, Request};
use tokio_tungstenite::tungstenite::protocol::Message;

const TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// ASR-055: Build the WebSocket handshake request for the Qwen Audio Inference API.
///
/// Unlike the old Realtime API, the Inference API:
/// - does NOT use `?model=` in the URL query (model goes in the run-task payload)
/// - does NOT need the `OpenAI-Beta: realtime=v1` header
/// - uses `Authorization: Bearer <key>` (same as production)
fn build_inference_ws_request(ws_uri: &str, api_key: &str) -> Result<Request<()>, String> {
    let mut request = ws_uri
        .into_client_request()
        .map_err(|e| format!("URL 配置错误：无效的 WebSocket 地址 - {e}"))?;

    let bearer = format!("Bearer {}", api_key.trim());
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&bearer)
            .map_err(|e| format!("API key 无效：无法构造鉴权头 - {e}"))?,
    );

    Ok(request)
}

/// ASR-055: Build the run-task message for the Inference API.
///
/// Mirrors the production implementation in `src/transcription/qwen_inference.rs:build_run_task_message`
/// (lines 95-128). Model is in `payload.model`, NOT in the URL query.
/// We send an empty `input` and no audio — the goal is just to verify the server
/// accepts our credentials and responds with `task-started`.
fn build_run_task_message(task_id: &str, model: &str) -> Value {
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
            "parameters": {
                "format": "pcm",
                "sample_rate": 16000,
                "language_hints": ["zh", "en", "ja", "ko"],
                "semantic_punctuation_enabled": false,
            },
            "input": {}
        }
    })
}

/// ASR-055: Build the finish-task message to cleanly close the test session.
fn build_finish_task_message(task_id: &str) -> Value {
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

fn extract_event_type(msg: &Value) -> Option<String> {
    msg.get("header")
        .and_then(|h| h.get("event"))
        .and_then(|e| e.as_str())
        .map(|s| s.to_string())
}

fn extract_task_error(msg: &Value) -> Option<String> {
    msg.get("header")
        .and_then(|h| h.get("error"))
        .and_then(|e| e.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            msg.get("payload")
                .and_then(|p| p.get("output"))
                .and_then(|o| o.get("error"))
                .and_then(|e| e.as_str())
                .map(|s| s.to_string())
        })
}

/// ASR-055: Test the online ASR connection using the Inference API protocol.
///
/// Flow: connect WebSocket -> send run-task -> wait for `task-started` -> send finish-task -> close.
/// Does NOT send any audio data (saves cost, faster).
///
/// Error messages are categorized into three human-readable types:
/// 1. "API key 无效/鉴权失败" — handshake rejected (HTTP 401/403) or auth header error
/// 2. "网络不通/超时" — connection failure or timeout
/// 3. "URL 配置错误" — invalid WebSocket URL format
pub async fn test_qwen3_asr_connection(
    api_key: String,
    url: &str,
    model: &str,
) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("API key 未填写".to_string());
    }
    if url.trim().is_empty() {
        return Err("URL 配置错误：ASR 在线地址为空，请在配置中填写".to_string());
    }
    if model.trim().is_empty() {
        return Err("URL 配置错误：ASR 模型名称为空，请在配置中填写".to_string());
    }

    let request = build_inference_ws_request(url, &api_key).map_err(|e| e.to_string())?;

    let task_id = format!(
        "test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    let result = timeout(TEST_TIMEOUT, async {
        // 1. Connect WebSocket
        let (mut ws_stream, response) = connect_async(request)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("401")
                    || msg.contains("403")
                    || msg.contains("Unauthorized")
                    || msg.contains("Forbidden")
                {
                    "API key 无效或鉴权失败：服务端拒绝连接（HTTP 401/403）".to_string()
                } else if msg.contains("dns") || msg.contains("DNS") || msg.contains("resolve") {
                    format!("网络不通：无法解析域名 - {msg}")
                } else if msg.contains("timeout") || msg.contains("Timeout") || msg.contains("timed out") {
                    format!("网络不通：连接超时 - {msg}")
                } else if msg.contains("connect")
                    || msg.contains("Connect")
                    || msg.contains("connection refused")
                {
                    format!("网络不通：无法连接到服务端 - {msg}")
                } else if msg.contains("tls") || msg.contains("TLS") || msg.contains("certificate") {
                    format!("网络不通：TLS 握手失败 - {msg}")
                } else {
                    format!("网络不通：WebSocket 连接失败 - {msg}")
                }
            })?;

        // Verify handshake success (HTTP 101 Switching Protocols)
        if response.status().as_u16() != 101 {
            let status = response.status().as_u16();
            if status == 401 || status == 403 {
                return Err(format!("API key 无效或鉴权失败：服务端返回 HTTP {status}"));
            }
            return Err(format!("网络不通：服务端返回 HTTP {status}（预期 101）"));
        }

        // 2. Send run-task (model in payload, not URL)
        let run_task = build_run_task_message(&task_id, model);
        let run_task_str =
            serde_json::to_string(&run_task).map_err(|e| format!("内部错误：序列化 run-task 失败 - {e}"))?;
        ws_stream
            .send(Message::Text(run_task_str.into()))
            .await
            .map_err(|e| format!("网络不通：发送 run-task 失败 - {e}"))?;

        // 3. Wait for task-started (or task-failed / error)
        while let Some(msg) = ws_stream.next().await {
            let msg = msg.map_err(|e| format!("网络不通：读取消息失败 - {e}"))?;

            match msg {
                Message::Text(text) => {
                    let parsed: Value = serde_json::from_str(&text)
                        .map_err(|e| format!("网络不通：服务端返回非 JSON 文本 - {e}"))?;

                    if let Some(err) = extract_task_error(&parsed) {
                        let err_lower = err.to_lowercase();
                        if err_lower.contains("auth")
                            || err_lower.contains("key")
                            || err_lower.contains("token")
                            || err_lower.contains("permission")
                            || err_lower.contains("unauthorized")
                            || err_lower.contains("forbidden")
                        {
                            return Err(format!("API key 无效或鉴权失败：服务端报错 - {err}"));
                        }
                        if err_lower.contains("model") {
                            return Err(format!("URL 配置错误：模型名称无效 - {err}"));
                        }
                        return Err(format!("服务端错误：{err}"));
                    }

                    if extract_event_type(&parsed).as_deref() == Some("task-started") {
                        // 4. Send finish-task to cleanly close
                        let finish = build_finish_task_message(&task_id);
                        let finish_str = serde_json::to_string(&finish)
                            .map_err(|e| format!("内部错误：序列化 finish-task 失败 - {e}"))?;
                        let _ = ws_stream.send(Message::Text(finish_str.into())).await;
                        let _ = ws_stream.close(None).await;
                        return Ok("连接成功：已通过鉴权并收到 task-started".to_string());
                    }
                    // Other events (e.g. task-finished) — keep waiting
                }
                Message::Close(_) => {
                    return Err("网络不通：服务端在收到 task-started 前关闭连接".to_string());
                }
                _ => {}
            }
        }

        Err("网络不通：未收到 task-started 即连接关闭".to_string())
    })
    .await
    .map_err(|_| format!("网络不通：测试连接超时（{}秒）", TEST_TIMEOUT.as_secs()))?;

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_inference_ws_request_generates_auth_header() {
        let req = build_inference_ws_request("wss://example.com/api-ws/v1/inference", "sk-test-key");
        assert!(req.is_ok());
        let req = req.unwrap();
        assert!(req.headers().get("Authorization").is_some());
        // Inference API must NOT have OpenAI-Beta header (that was Realtime API)
        assert!(req.headers().get("OpenAI-Beta").is_none());
    }

    #[test]
    fn build_inference_ws_request_rejects_invalid_url() {
        // Empty string fails into_client_request parsing
        let req = build_inference_ws_request("", "sk-test-key");
        assert!(req.is_err());
        let err = req.unwrap_err();
        assert!(err.contains("URL 配置错误"));
    }

    #[test]
    fn build_run_task_message_puts_model_in_payload() {
        let msg = build_run_task_message("test-123", "qwen-audio-3.0-asr-flash-streaming");
        // model must be in payload, NOT in URL query
        assert_eq!(
            msg["payload"]["model"],
            serde_json::json!("qwen-audio-3.0-asr-flash-streaming")
        );
        // header.action must be run-task
        assert_eq!(msg["header"]["action"], "run-task");
        assert_eq!(msg["header"]["task_id"], "test-123");
        // task_group/task/function must match production
        assert_eq!(msg["payload"]["task_group"], "audio");
        assert_eq!(msg["payload"]["task"], "asr");
        assert_eq!(msg["payload"]["function"], "recognition");
        // parameters must have format/sample_rate/language_hints
        assert_eq!(msg["payload"]["parameters"]["format"], "pcm");
        assert_eq!(msg["payload"]["parameters"]["sample_rate"], 16000);
    }

    #[test]
    fn build_finish_task_message_has_correct_action() {
        let msg = build_finish_task_message("test-123");
        assert_eq!(msg["header"]["action"], "finish-task");
        assert_eq!(msg["header"]["task_id"], "test-123");
    }

    #[test]
    fn extract_event_type_recognizes_task_started() {
        let v: Value = serde_json::json!({
            "header": {"event": "task-started", "task_id": "t1"}
        });
        assert_eq!(extract_event_type(&v), Some("task-started".to_string()));
    }

    #[test]
    fn extract_event_type_returns_none_for_missing() {
        let v: Value = serde_json::json!({"header": {"task_id": "t1"}});
        assert_eq!(extract_event_type(&v), None);
    }

    #[test]
    fn extract_task_error_extracts_header_error() {
        let v: Value = serde_json::json!({
            "header": {"error": "invalid model"}
        });
        assert_eq!(extract_task_error(&v), Some("invalid model".to_string()));
    }

    #[test]
    fn rejects_empty_api_key() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(test_qwen3_asr_connection(
            "".to_string(),
            "wss://example.com/api-ws/v1/inference",
            "test-model",
        ));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key"));
    }

    #[test]
    fn rejects_empty_url() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(test_qwen3_asr_connection(
            "sk-test".to_string(),
            "",
            "test-model",
        ));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("URL"));
    }

    #[test]
    fn rejects_empty_model() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(test_qwen3_asr_connection(
            "sk-test".to_string(),
            "wss://example.com/api-ws/v1/inference",
            "",
        ));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("模型"));
    }
}