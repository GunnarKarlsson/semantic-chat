//! Remote OpenAI-compatible **Chat Completions** HTTP client (URL, key, model from env).
//!
//! This is not the UI chat/session layer — that lives in `models` (`ChatSession`, etc.).

use std::sync::Arc;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const ENV_BASE_URL: &str = "CHAT_API_BASE_URL";
const ENV_API_KEY: &str = "CHAT_API_KEY";
const ENV_MODEL: &str = "CHAT_MODEL";

#[derive(Debug, Error)]
pub enum ChatCompletionsError {
    #[error("missing environment variable {0}")]
    MissingEnv(&'static str),
    #[error("{0} is empty")]
    EmptyEnv(&'static str),
    #[error("chat completions messages list is empty")]
    EmptyMessages,
    #[error("chat completions request failed")]
    Request(#[source] reqwest::Error),
    #[error("chat completions HTTP {status}: {body}")]
    Http { status: StatusCode, body: String },
    #[error("failed to parse chat completions response")]
    Parse(#[source] reqwest::Error),
    #[error("chat completions response had no assistant content")]
    EmptyContent,
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
        let base_url = std::env::var(ENV_BASE_URL)
            .map_err(|_| ChatCompletionsError::MissingEnv(ENV_BASE_URL))?;
        let api_key = std::env::var(ENV_API_KEY)
            .map_err(|_| ChatCompletionsError::MissingEnv(ENV_API_KEY))?;
        let model =
            std::env::var(ENV_MODEL).map_err(|_| ChatCompletionsError::MissingEnv(ENV_MODEL))?;

        if base_url.trim().is_empty() {
            return Err(ChatCompletionsError::EmptyEnv(ENV_BASE_URL));
        }
        if api_key.trim().is_empty() {
            return Err(ChatCompletionsError::EmptyEnv(ENV_API_KEY));
        }
        if model.trim().is_empty() {
            return Err(ChatCompletionsError::EmptyEnv(ENV_MODEL));
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

    /// Send an ordered conversation (oldest → newest) and return the assistant reply.
    pub async fn complete(
        &self,
        messages: &[CompletionsMessage],
    ) -> Result<String, ChatCompletionsError> {
        if messages.is_empty() {
            return Err(ChatCompletionsError::EmptyMessages);
        }

        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = CompletionsRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
        };

        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(ChatCompletionsError::Request)?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string());
            return Err(ChatCompletionsError::Http { status, body });
        }

        let parsed: CompletionsResponse =
            response.json().await.map_err(ChatCompletionsError::Parse)?;

        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message)
            .and_then(|message| message.content)
            .filter(|content| !content.is_empty())
            .ok_or(ChatCompletionsError::EmptyContent)
    }
}

pub type SharedChatCompletionsApiClient = Arc<ChatCompletionsApiClient>;

/// One turn in the Completions `messages` array (order = conversation time).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionsMessage {
    pub role: String,
    pub content: String,
}

impl CompletionsMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

#[derive(Serialize)]
struct CompletionsRequest {
    model: String,
    messages: Vec<CompletionsMessage>,
}

#[derive(Deserialize)]
struct CompletionsResponse {
    choices: Vec<CompletionsChoice>,
}

#[derive(Deserialize)]
struct CompletionsChoice {
    message: Option<CompletionsAssistantMessage>,
}

#[derive(Deserialize)]
struct CompletionsAssistantMessage {
    #[serde(default)]
    content: Option<String>,
}
