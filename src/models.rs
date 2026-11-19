//! UI chat session types and HTTP DTOs for the conversation API.
//!
//! This module is for **in-app chats** (sidebar threads). It is not the
//! remote Chat Completions client — that lives in `chat_completions_api`.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            timestamp: Utc::now(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub messages: Vec<Message>,
}

impl ChatSession {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            title: "New chat".to_string(),
            created_at: Utc::now(),
            messages: Vec::new(),
        }
    }

    pub fn append_message(&mut self, message: Message) {
        if self.messages.is_empty() && message.role == Role::User {
            self.title = truncate_title(&message.content);
        }
        self.messages.push(message);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSummary {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
}

impl From<&ChatSession> for ChatSummary {
    fn from(session: &ChatSession) -> Self {
        Self {
            id: session.id,
            title: session.title.clone(),
            created_at: session.created_at,
        }
    }
}

/// `POST /api/messages` body — send a user message in a UI chat session.
#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageRequest {
    pub chat_id: Uuid,
    pub message: String,
}

/// `POST /api/messages` response — assistant reply for a UI chat session.
#[derive(Debug, Clone, Serialize)]
pub struct SendMessageResponse {
    pub chat_id: Uuid,
    pub reply: String,
    pub cache_hit: bool,
}

/// `POST /api/chats` response — new empty UI chat session.
#[derive(Debug, Clone, Serialize)]
pub struct CreateChatResponse {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
}

impl From<&ChatSession> for CreateChatResponse {
    fn from(session: &ChatSession) -> Self {
        Self {
            id: session.id,
            title: session.title.clone(),
            created_at: session.created_at,
        }
    }
}

#[derive(Debug, Error)]
pub enum ChatStoreError {
    #[error("chat not found: {0}")]
    NotFound(Uuid),
}

/// In-memory chat sessions.
#[derive(Debug, Default)]
pub struct ChatStore {
    sessions: HashMap<Uuid, ChatSession>,
}

impl ChatStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn create(&mut self) -> ChatSession {
        let session = ChatSession::new();
        let id = session.id;
        self.sessions.insert(id, session.clone());
        session
    }

    pub fn get(&self, id: Uuid) -> Option<&ChatSession> {
        self.sessions.get(&id)
    }

    pub fn list(&self) -> Vec<ChatSummary> {
        let mut summaries: Vec<_> = self.sessions.values().map(ChatSummary::from).collect();
        summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        summaries
    }

    pub fn append_message(
        &mut self,
        id: Uuid,
        message: Message,
    ) -> Result<&ChatSession, ChatStoreError> {
        let session = self
            .sessions
            .get_mut(&id)
            .ok_or(ChatStoreError::NotFound(id))?;
        session.append_message(message);
        Ok(session)
    }
}

pub type SharedChatStore = Arc<RwLock<ChatStore>>;

pub fn new_shared_store() -> SharedChatStore {
    Arc::new(RwLock::new(ChatStore::new()))
}

fn truncate_title(content: &str) -> String {
    const MAX_CHARS: usize = 48;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return "New chat".to_string();
    }
    let mut title: String = trimmed.chars().take(MAX_CHARS).collect();
    if trimmed.chars().count() > MAX_CHARS {
        title.push('…');
    }
    title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_append_sets_title_from_first_user_message() {
        let mut store = ChatStore::new();
        let session = store.create();
        let id = session.id;

        store
            .append_message(id, Message::user("How does the semantic cache work?"))
            .unwrap();

        let session = store.get(id).unwrap();
        assert_eq!(session.title, "How does the semantic cache work?");
        assert_eq!(session.messages.len(), 1);
    }
}
