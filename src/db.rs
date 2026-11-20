//! Local SQLite persistence (`data/semantic-chat.db`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection};
use thiserror::Error;

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
}
