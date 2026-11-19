//! OpenAI-compatible Chat Completions API client (URL, key, and model from env).
//! `complete` is exercised by API routes in a later step.

#![allow(dead_code)]

use std::sync::Arc;

use reqwest::Client;
use serde::{Deserialize, Serialize};

const ENV_BASE_URL: &str = "CHAT_API_BASE_URL";
const ENV_API_KEY: &str = "CHAT_API_KEY";
const ENV_MODEL: &str = "CHAT_MODEL";

#[derive(Debug)]
pub struct ChatCompletionsError(String);

impl std::fmt::Display for ChatCompletionsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ChatCompletionsError {}

impl From<String> for ChatCompletionsError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ChatCompletionsError {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// HTTP client for an OpenAI-compatible `POST /v1/chat/completions` API.
#[derive(Clone)]
pub struct ChatCompletionsApiClient {
    http: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl ChatCompletionsApiClient {
    /// Build a client from `CHAT_API_BASE_URL`, `CHAT_API_KEY`, and `CHAT_MODEL`.
    pub fn from_env() -> Result<Self, ChatCompletionsError> {
        let base_url = std::env::var(ENV_BASE_URL).map_err(|_| {
            ChatCompletionsError(format!("missing environment variable {ENV_BASE_URL}"))
        })?;
        let api_key = std::env::var(ENV_API_KEY).map_err(|_| {
            ChatCompletionsError(format!("missing environment variable {ENV_API_KEY}"))
        })?;
        let model = std::env::var(ENV_MODEL).map_err(|_| {
            ChatCompletionsError(format!("missing environment variable {ENV_MODEL}"))
        })?;

        if base_url.trim().is_empty() {
            return Err(ChatCompletionsError(format!("{ENV_BASE_URL} is empty")));
        }
        if api_key.trim().is_empty() {
            return Err(ChatCompletionsError(format!("{ENV_API_KEY} is empty")));
        }
        if model.trim().is_empty() {
            return Err(ChatCompletionsError(format!("{ENV_MODEL} is empty")));
        }

        Ok(Self {
            http: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Send a single user message and return the assistant reply text.
    pub async fn complete(&self, user_message: &str) -> Result<String, ChatCompletionsError> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: user_message.to_string(),
            }],
        };

        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                ChatCompletionsError(format!("chat completions request failed: {e}"))
            })?;

        let status = response.status();
        if !status.is_success() {
            let detail = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string());
            return Err(ChatCompletionsError(format!(
                "chat completions HTTP {status}: {detail}"
            )));
        }

        let parsed: ChatCompletionResponse = response.json().await.map_err(|e| {
            ChatCompletionsError(format!("failed to parse chat completions response: {e}"))
        })?;

        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message)
            .and_then(|message| message.content)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| {
                ChatCompletionsError(
                    "chat completions response had no assistant content".into(),
                )
            })
    }
}

pub type SharedChatCompletionsApiClient = Arc<ChatCompletionsApiClient>;

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: Option<ChatCompletionMessage>,
}

#[derive(Deserialize)]
struct ChatCompletionMessage {
    #[serde(default)]
    content: Option<String>,
}
