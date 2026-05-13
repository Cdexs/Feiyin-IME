use anyhow::{anyhow, ensure, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::LlmConfig;

const ATTEMPT_TIMEOUTS: [Duration; 1] = [Duration::from_secs(6)];
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_DELAY: Duration = Duration::from_millis(250);

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<RequestMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    /// 关闭推理模式（SiliconFlow/DeepSeek 等模型的 thinking 模式），大幅减少延迟
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_thinking: Option<bool>,
}

#[derive(Serialize)]
struct RequestMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Option<ResponseMessage>,
    delta: Option<ResponseDelta>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct ResponseDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
}

pub struct LlmClient {
    client: reqwest::Client,
    config: LlmConfig,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("Failed to build HTTP client");
        Self { client, config }
    }

    pub async fn probe(&self) -> Result<String> {
        ensure!(self.config.enabled, "LLM optimization is disabled");
        ensure!(!self.config.api_url.trim().is_empty(), "API URL is empty");
        ensure!(!self.config.api_key.trim().is_empty(), "API key is empty");
        ensure!(!self.config.model.trim().is_empty(), "Model is empty");

        let url = self.chat_completions_url();
        let body = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![RequestMessage {
                role: "user".to_string(),
                content: "Reply with OK only.".to_string(),
            }],
            temperature: None,
            max_tokens: Some(5),
            stream: Some(false),
            enable_thinking: Some(false), // 关闭推理模式，快速响应
        };

        let mut last_err: Option<reqwest::Error> = None;

        for (idx, timeout) in ATTEMPT_TIMEOUTS.iter().copied().enumerate() {
            let attempt = idx + 1;
            if attempt > 1 {
                tokio::time::sleep(RETRY_DELAY).await;
            }

            match self.try_once(&url, &body, timeout).await {
                Ok(result) => return Ok(result),
                Err(e) if is_retryable_error(&e) => {
                    last_err = Some(e);
                }
                Err(e) => {
                    let message = probe_error_message(&e);
                    return Err(anyhow!(message));
                }
            }
        }

        let err = last_err.expect("probe retries exhausted without capturing error");
        let message = probe_error_message(&err);
        Err(anyhow!(message))
    }

    fn chat_completions_url(&self) -> String {
        let base = self.config.api_url.trim_end_matches('/');
        if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{}/chat/completions", base)
        }
    }

    async fn try_once(
        &self,
        url: &str,
        body: &ChatRequest,
        timeout: Duration,
    ) -> std::result::Result<String, reqwest::Error> {
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.config.api_key)
            .json(body)
            .timeout(timeout)
            .send()
            .await?;

        let response = response.error_for_status()?;
        let chat: ChatResponse = response.json().await?;
        let result = extract_text(chat).unwrap_or_default();
        Ok(result)
    }
}

fn is_retryable_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout()
}

fn probe_error_message(error: &reqwest::Error) -> String {
    if error.is_timeout() || error.is_connect() {
        "连接超时，请检查 API 地址是否可达".to_string()
    } else if let Some(status) = error.status() {
        match status.as_u16() {
            401 | 403 => "认证失败，请检查 API Key".to_string(),
            400..=499 => format!("请求错误（{}），请检查模型名称", status),
            _ => error.to_string(),
        }
    } else {
        error.to_string()
    }
}

fn extract_text(chat: ChatResponse) -> Option<String> {
    let mut parts = Vec::new();

    for choice in chat.choices {
        if let Some(message) = choice.message {
            if let Some(content) = message.content.filter(|s| !s.trim().is_empty()) {
                parts.push(content);
            } else if let Some(reasoning) =
                message.reasoning_content.filter(|s| !s.trim().is_empty())
            {
                parts.push(reasoning);
            }
            continue;
        }

        if let Some(delta) = choice.delta {
            if let Some(content) = delta.content.filter(|s| !s.trim().is_empty()) {
                parts.push(content);
            } else if let Some(reasoning) = delta.reasoning_content.filter(|s| !s.trim().is_empty())
            {
                parts.push(reasoning);
            }
        }
    }

    let text = parts.join("").trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}
