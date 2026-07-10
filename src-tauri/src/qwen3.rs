use futures_util::StreamExt;
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderValue, Request};
use tokio_tungstenite::tungstenite::protocol::Message;

const BETA_VALUE: &str = "realtime=v1";
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Build the full WebSocket URI from base URL and model name.
fn build_ws_uri(base_url: &str, model: &str) -> String {
    if base_url.contains('?') {
        format!("{}&model={}", base_url, model)
    } else {
        format!("{}?model={}", base_url, model)
    }
}

/// Build the WebSocket handshake request for the Qwen3 realtime endpoint.
fn build_ws_request(ws_uri: &str, api_key: &str) -> Result<Request<()>, String> {
    let mut request = ws_uri
        .into_client_request()
        .map_err(|e| format!("invalid WebSocket URL: {e}"))?;

    let bearer = format!("Bearer {}", api_key.trim());
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&bearer)
            .map_err(|e| format!("invalid API key header: {e}"))?,
    );
    request
        .headers_mut()
        .insert("OpenAI-Beta", HeaderValue::from_static(BETA_VALUE));

    Ok(request)
}

/// Test whether the provided Qwen3 API key can authenticate to the realtime
/// ASR endpoint. We open the WebSocket and wait for the server to emit
/// `session.created`. Any other terminal event or timeout is treated as failure.
pub async fn test_qwen3_asr_connection(
    api_key: String,
    url: &str,
    model: &str,
) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("API key is empty".to_string());
    }

    let ws_uri = build_ws_uri(url, model);
    let request = build_ws_request(&ws_uri, &api_key)
        .map_err(|e| format!("failed to build WebSocket request: {e}"))?;

    let result = timeout(TEST_TIMEOUT, async {
        let (mut ws_stream, _) = connect_async(request)
            .await
            .map_err(|e| format!("WebSocket connect failed: {e}"))?;

        while let Some(msg) = ws_stream.next().await {
            let msg = msg.map_err(|e| format!("WebSocket read failed: {e}"))?;

            match msg {
                Message::Text(text) => {
                    if let Some(event_type) = parse_event_type(&text) {
                        match event_type.as_str() {
                            "session.created" => {
                                let _ = ws_stream.close(None).await;
                                return Ok("Connection established".to_string());
                            }
                            "error" => {
                                let detail = parse_error_detail(&text);
                                return Err(format!("server error: {detail}"));
                            }
                            _ => {}
                        }
                    }
                }
                Message::Close(_) => {
                    return Err("connection closed before session.created".to_string());
                }
                _ => {}
            }
        }
        Err("connection closed before session.created".to_string())
    })
    .await
    .map_err(|_| "Qwen3 ASR connection timed out (5s)".to_string())?;

    result
}

fn parse_event_type(text: &str) -> Option<String> {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| v.get("type")?.as_str().map(|s| s.to_string()))
}

fn parse_error_detail(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message").and_then(|m| m.as_str().map(|s| s.to_string())))
        })
        .unwrap_or_else(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_event_type_extracts_session_created() {
        let text = r#"{"type":"session.created","session":{"id":"sess_123"}}"#;
        assert_eq!(parse_event_type(text), Some("session.created".to_string()));
    }

    #[test]
    fn parse_event_type_extracts_error() {
        let text = r#"{"type":"error","error":{"message":"invalid auth"}}"#;
        assert_eq!(parse_event_type(text), Some("error".to_string()));
    }

    #[test]
    fn parse_event_type_returns_none_for_invalid_json() {
        assert_eq!(parse_event_type("not json"), None);
    }

    #[test]
    fn parse_error_detail_extracts_message() {
        let text = r#"{"type":"error","error":{"message":"invalid auth"}}"#;
        assert_eq!(parse_error_detail(text), "invalid auth");
    }

    #[test]
    fn parse_error_detail_falls_back_to_raw_text() {
        let text = r#"{"type":"error"}"#;
        assert_eq!(parse_error_detail(text), text);
    }

    #[test]
    fn rejects_empty_api_key() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(test_qwen3_asr_connection(
            "".to_string(),
            "wss://example.com/ws",
            "test-model",
        ));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn build_ws_request_generates_ws_handshake_headers() {
        let req = build_ws_request("wss://example.com/ws?model=m", "sk-test-key").unwrap();
        assert!(req.headers().get("Authorization").is_some());
        assert!(req.headers().get("OpenAI-Beta").is_some());
        assert!(req.headers().get("Sec-WebSocket-Key").is_some());
        assert!(req.headers().get("Upgrade").is_some());
        assert!(req.headers().get("Connection").is_some());
    }
}
