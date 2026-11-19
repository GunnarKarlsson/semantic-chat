//! OpenAI-compatible Chat Completions API client (URL, key, and model from env).
//! `complete` is exercised by API routes in a later step.

#![allow(dead_code)]

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
        let model = std::env::var(ENV_MODEL)
            .map_err(|_| ChatCompletionsError::MissingEnv(ENV_MODEL))?;

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
            .map_err(ChatCompletionsError::Request)?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string());
            return Err(ChatCompletionsError::Http { status, body });
        }

        let parsed: ChatCompletionResponse = response
            .json()
            .await
            .map_err(ChatCompletionsError::Parse)?;

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
