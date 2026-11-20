//! Local SQLite persistence (`data/semantic-chat.db`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use thiserror::Error;
use uuid::Uuid;

use crate::cache::CacheEntry;

/// Default cosine-similarity threshold stored in the `settings` table.
pub const DEFAULT_SIMILARITY_THRESHOLD: &str = "0.92";

/// Setting key for the semantic-cache similarity threshold.
pub const SETTING_SIMILARITY_THRESHOLD: &str = "similarity_threshold";

#[derive(Debug, Error)]
pub enum DbError {
    #[error("failed to create data directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid embedding blob length {0} (must be a multiple of 4)")]
    InvalidEmbeddingBlob(usize),
    #[error("invalid cache timestamp {value}: {source}")]
    InvalidTimestamp {
        value: String,
        source: chrono::ParseError,
    },
    #[error("invalid chat_id {value}: {source}")]
    InvalidChatId {
        value: String,
        source: uuid::Error,
    },
}

/// Process-wide shared SQLite connection.
pub type SharedDb = Arc<Mutex<Connection>>;

/// Open (or create) the database, migrate schema, and seed defaults.
pub fn open_default() -> Result<(SharedDb, PathBuf), DbError> {
    let path = default_db_path();
    let db = open(&path)?;
    Ok((db, path))
}

/// Path to `data/semantic-chat.db` under the current working directory.
pub fn default_db_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("data")
        .join("semantic-chat.db")
}

/// Open the database at `path`, creating parent dirs and running migrations.
pub fn open(path: &Path) -> Result<SharedDb, DbError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| DbError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let conn = Connection::open(path)?;
    migrate(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}

fn migrate(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS cache_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            embedding BLOB NOT NULL,
            prompt TEXT NOT NULL,
            response TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            chat_id TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS chats (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            position INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )?;

    conn.execute(
        "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
        params![SETTING_SIMILARITY_THRESHOLD, DEFAULT_SIMILARITY_THRESHOLD],
    )?;

    Ok(())
}

/// Load all semantic-cache rows (oldest first).
pub fn load_cache_entries(conn: &Connection) -> Result<Vec<CacheEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT embedding, prompt, response, timestamp, chat_id
         FROM cache_entries
         ORDER BY id ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let embedding: Vec<u8> = row.get(0)?;
        let prompt: String = row.get(1)?;
        let response: String = row.get(2)?;
        let timestamp: String = row.get(3)?;
        let chat_id: String = row.get(4)?;
        Ok((embedding, prompt, response, timestamp, chat_id))
    })?;

    let mut entries = Vec::new();
    for row in rows {
        let (embedding_blob, prompt, response, timestamp, chat_id) = row?;
        entries.push(CacheEntry {
            embedding: blob_to_embedding(&embedding_blob)?,
            prompt,
            response,
            timestamp: parse_timestamp(&timestamp)?,
            chat_id: parse_chat_id(&chat_id)?,
        });
    }
    Ok(entries)
}

/// Insert one cache entry. Returns the new row id.
pub fn insert_cache_entry(conn: &Connection, entry: &CacheEntry) -> Result<i64, DbError> {
    conn.execute(
        "INSERT INTO cache_entries (embedding, prompt, response, timestamp, chat_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            embedding_to_blob(&entry.embedding),
            entry.prompt,
            entry.response,
            entry.timestamp.to_rfc3339(),
            entry.chat_id.to_string(),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Delete every semantic-cache row.
pub fn clear_cache_entries(conn: &Connection) -> Result<(), DbError> {
    conn.execute("DELETE FROM cache_entries", [])?;
    Ok(())
}

pub fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(embedding.len() * 4);
    for value in embedding {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub fn blob_to_embedding(blob: &[u8]) -> Result<Vec<f32>, DbError> {
    if blob.len() % 4 != 0 {
        return Err(DbError::InvalidEmbeddingBlob(blob.len()));
    }
    let mut embedding = Vec::with_capacity(blob.len() / 4);
    for chunk in blob.chunks_exact(4) {
        embedding.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(embedding)
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, DbError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|source| DbError::InvalidTimestamp {
            value: value.to_string(),
            source,
        })
}

fn parse_chat_id(value: &str) -> Result<Uuid, DbError> {
    Uuid::parse_str(value).map_err(|source| DbError::InvalidChatId {
        value: value.to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_schema_and_seeds_threshold() {
        let dir = std::env::temp_dir().join(format!("semantic-chat-db-{}", uuid::Uuid::new_v4()));
        let path = dir.join("semantic-chat.db");

        let db = open(&path).expect("open db");
        {
            let conn = db.lock().expect("db lock");
            let threshold: String = conn
                .query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    params![SETTING_SIMILARITY_THRESHOLD],
                    |row| row.get(0),
                )
                .expect("threshold row");
            assert_eq!(threshold, DEFAULT_SIMILARITY_THRESHOLD);

            let tables: Vec<String> = {
                let mut stmt = conn
                    .prepare(
                        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
                    )
                    .unwrap();
                stmt.query_map([], |row| row.get(0))
                    .unwrap()
                    .map(|r| r.unwrap())
                    .collect()
            };
            assert!(tables.iter().any(|t| t == "cache_entries"));
            assert!(tables.iter().any(|t| t == "chats"));
            assert!(tables.iter().any(|t| t == "messages"));
            assert!(tables.iter().any(|t| t == "settings"));
        }

        // Re-open should not duplicate the seed row.
        drop(db);
        let db = open(&path).expect("re-open db");
        let conn = db.lock().expect("db lock");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM settings WHERE key = ?1",
                params![SETTING_SIMILARITY_THRESHOLD],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_entry_round_trips_through_sqlite() {
        let dir = std::env::temp_dir().join(format!("semantic-chat-cache-{}", Uuid::new_v4()));
        let path = dir.join("semantic-chat.db");
        let db = open(&path).expect("open db");

        let entry = CacheEntry {
            embedding: vec![1.0, 0.0, 0.5],
            prompt: "hello".into(),
            response: "world".into(),
            timestamp: Utc::now(),
            chat_id: Uuid::new_v4(),
        };

        {
            let conn = db.lock().expect("db lock");
            insert_cache_entry(&conn, &entry).expect("insert");
            let loaded = load_cache_entries(&conn).expect("load");
            assert_eq!(loaded.len(), 1);
            assert_eq!(loaded[0].prompt, "hello");
            assert_eq!(loaded[0].response, "world");
            assert_eq!(loaded[0].embedding, entry.embedding);
            assert_eq!(loaded[0].chat_id, entry.chat_id);
            clear_cache_entries(&conn).expect("clear");
            assert!(load_cache_entries(&conn).unwrap().is_empty());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
