//! In-memory semantic cache (embeddings + cosine similarity) with SQLite write-through.
//!
//! The cache is **global** across UI chats: `search` never filters by `chat_id`.
//! `chat_id` is metadata for which session first stored the entry.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::db::{self, DbError, SharedDb};

/// Default cosine-similarity threshold for a cache hit.
pub const DEFAULT_THRESHOLD: f32 = 0.92;

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub embedding: Vec<f32>,
    pub prompt: String,
    pub response: String,
    pub timestamp: DateTime<Utc>,
    /// UI chat that first inserted this entry (not a search filter).
    pub chat_id: Uuid,
}

/// Brute-force in-memory store of prompt/response pairs keyed by embedding.
///
/// Mutations write through to SQLite so entries survive restarts.
pub struct SemanticCache {
    entries: Vec<CacheEntry>,
    db: SharedDb,
}

impl SemanticCache {
    /// Load every cache row from SQLite into memory.
    pub fn load(db: SharedDb) -> Result<Self, DbError> {
        let entries = {
            let conn = db.lock().expect("db lock poisoned");
            db::load_cache_entries(&conn)?
        };
        Ok(Self { entries, db })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Used by tests today; settings/clear-cache API in a later step.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Find the best global match at or above `threshold` (all chats).
    pub fn search(&self, query_embedding: &[f32], threshold: f32) -> Option<CacheEntry> {
        let mut best: Option<(f32, &CacheEntry)> = None;

        for entry in &self.entries {
            let Ok(score) = cosine_similarity(query_embedding, &entry.embedding) else {
                continue;
            };
            if score < threshold {
                continue;
            }
            match best {
                Some((best_score, _)) if score <= best_score => {}
                _ => best = Some((score, entry)),
            }
        }

        best.map(|(_, entry)| entry.clone())
    }

    /// Store a new prompt/response pair: SQLite first, then memory.
    pub fn insert(
        &mut self,
        embedding: Vec<f32>,
        prompt: impl Into<String>,
        response: impl Into<String>,
        chat_id: Uuid,
    ) -> Result<(), DbError> {
        let entry = CacheEntry {
            embedding,
            prompt: prompt.into(),
            response: response.into(),
            timestamp: Utc::now(),
            chat_id,
        };

        {
            let conn = self.db.lock().expect("db lock poisoned");
            db::insert_cache_entry(&conn, &entry)?;
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Remove all entries from SQLite, then from memory.
    /// Wired to the settings clear-cache endpoint in a later step.
    #[allow(dead_code)]
    pub fn clear(&mut self) -> Result<(), DbError> {
        {
            let conn = self.db.lock().expect("db lock poisoned");
            db::clear_cache_entries(&conn)?;
        }
        self.entries.clear();
        Ok(())
    }
}

/// Process-wide shared cache.
pub type SharedSemanticCache = Arc<RwLock<SemanticCache>>;

/// Load the shared cache from SQLite.
pub fn load_shared(db: SharedDb) -> Result<SharedSemanticCache, DbError> {
    Ok(Arc::new(RwLock::new(SemanticCache::load(db)?)))
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, ()> {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return Err(());
    }

    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        return Ok(0.0);
    }
    Ok(dot / denom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn temp_cache() -> (SemanticCache, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("semantic-chat-sc-{}", Uuid::new_v4()));
        let path = dir.join("semantic-chat.db");
        let shared = db::open(&path).expect("open db");
        let cache = SemanticCache::load(shared).expect("load cache");
        (cache, dir)
    }

    #[test]
    fn search_finds_near_duplicate() {
        let (mut cache, dir) = temp_cache();
        let embedding = vec![1.0, 0.0, 0.0];
        cache
            .insert(embedding.clone(), "hello", "world", Uuid::nil())
            .unwrap();

        let hit = cache.search(&embedding, DEFAULT_THRESHOLD);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().response, "world");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_misses_below_threshold() {
        let (mut cache, dir) = temp_cache();
        cache
            .insert(vec![1.0, 0.0, 0.0], "a", "b", Uuid::nil())
            .unwrap();

        let miss = cache.search(&[0.0, 1.0, 0.0], DEFAULT_THRESHOLD);
        assert!(miss.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_hits_are_shared_across_chats() {
        let chat_a = Uuid::new_v4();
        let chat_b = Uuid::new_v4();
        let embedding = vec![0.0, 1.0, 0.0];

        let (mut cache, dir) = temp_cache();
        cache
            .insert(embedding.clone(), "what is rust?", "A language.", chat_a)
            .unwrap();

        let hit = cache
            .search(&embedding, DEFAULT_THRESHOLD)
            .expect("expected a global cache hit");
        assert_eq!(hit.response, "A language.");
        assert_eq!(hit.chat_id, chat_a);
        assert_ne!(hit.chat_id, chat_b);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn insert_survives_reload_and_clear_empties_db() {
        let dir = std::env::temp_dir().join(format!("semantic-chat-persist-{}", Uuid::new_v4()));
        let path = dir.join("semantic-chat.db");
        let shared = db::open(&path).expect("open db");

        {
            let mut cache = SemanticCache::load(shared.clone()).unwrap();
            cache
                .insert(vec![0.0, 0.0, 1.0], "q", "a", Uuid::new_v4())
                .unwrap();
            assert_eq!(cache.len(), 1);
        }

        {
            let cache = SemanticCache::load(shared.clone()).unwrap();
            assert_eq!(cache.len(), 1);
            let hit = cache.search(&[0.0, 0.0, 1.0], DEFAULT_THRESHOLD);
            assert_eq!(hit.unwrap().response, "a");
        }

        {
            let mut cache = SemanticCache::load(shared.clone()).unwrap();
            cache.clear().unwrap();
            assert!(cache.is_empty());
        }

        let cache = SemanticCache::load(shared).unwrap();
        assert!(cache.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
