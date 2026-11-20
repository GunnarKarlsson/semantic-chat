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

use crate::db::{self, DbError, SharedDb};

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
    /// Session title after append (from first user message when new).
    pub title: String,
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
    #[error(transparent)]
    Db(#[from] DbError),
}

/// In-memory chat sessions with SQLite write-through.
pub struct ChatStore {
    sessions: HashMap<Uuid, ChatSession>,
    db: SharedDb,
}

impl ChatStore {
    /// Load every chat and its messages from SQLite into memory.
    pub fn load(db: SharedDb) -> Result<Self, DbError> {
        let sessions = {
            let conn = db.lock().expect("db lock poisoned");
            let list = db::load_chat_sessions(&conn)?;
            list.into_iter().map(|s| (s.id, s)).collect()
        };
        Ok(Self { sessions, db })
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn create(&mut self) -> Result<ChatSession, DbError> {
        let session = ChatSession::new();
        {
            let conn = self.db.lock().expect("db lock poisoned");
            db::insert_chat(&conn, &session)?;
        }
        let id = session.id;
        self.sessions.insert(id, session.clone());
        Ok(session)
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
            .get(&id)
            .ok_or(ChatStoreError::NotFound(id))?;

        let position = session.messages.len() as i64;
        let title = if session.messages.is_empty() && message.role == Role::User {
            truncate_title(&message.content)
        } else {
            session.title.clone()
        };

        {
            let mut conn = self.db.lock().expect("db lock poisoned");
            db::insert_message(&mut conn, id, &message, position, &title)?;
        }

        let session = self
            .sessions
            .get_mut(&id)
            .expect("session must exist after lookup");
        session.append_message(message);
        Ok(session)
    }
}

pub type SharedChatStore = Arc<RwLock<ChatStore>>;

/// Load the shared chat store from SQLite.
pub fn load_shared_store(db: SharedDb) -> Result<SharedChatStore, DbError> {
    Ok(Arc::new(RwLock::new(ChatStore::load(db)?)))
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

    fn temp_store() -> (ChatStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("semantic-chat-store-{}", Uuid::new_v4()));
        let path = dir.join("semantic-chat.db");
        let shared = db::open(&path).expect("open db");
        let store = ChatStore::load(shared).expect("load store");
        (store, dir)
    }

    #[test]
    fn create_and_append_sets_title_from_first_user_message() {
        let (mut store, dir) = temp_store();
        let session = store.create().unwrap();
        let id = session.id;

        store
            .append_message(id, Message::user("How does the semantic cache work?"))
            .unwrap();

        let session = store.get(id).unwrap();
        assert_eq!(session.title, "How does the semantic cache work?");
        assert_eq!(session.messages.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn long_first_message_is_truncated_for_sidebar_title() {
        let (mut store, dir) = temp_store();
        let id = store.create().unwrap().id;
        let long = "x".repeat(60);
        store
            .append_message(id, Message::user(long))
            .unwrap();
        let title = &store.get(id).unwrap().title;
        assert!(title.ends_with('…'));
        assert_eq!(title.chars().count(), 49);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chats_and_messages_survive_reload() {
        let dir = std::env::temp_dir().join(format!("semantic-chat-reload-{}", Uuid::new_v4()));
        let path = dir.join("semantic-chat.db");
        let shared = db::open(&path).expect("open db");

        let chat_id = {
            let mut store = ChatStore::load(shared.clone()).unwrap();
            let session = store.create().unwrap();
            let id = session.id;
            store
                .append_message(id, Message::user("persist me"))
                .unwrap();
            store
                .append_message(id, Message::assistant("ok"))
                .unwrap();
            id
        };

        let store = ChatStore::load(shared).unwrap();
        assert_eq!(store.len(), 1);
        let session = store.get(chat_id).unwrap();
        assert_eq!(session.title, "persist me");
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].content, "persist me");
        assert_eq!(session.messages[1].content, "ok");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
