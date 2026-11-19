//! In-memory semantic cache (embeddings + cosine similarity).
//!
//! The cache is **global** across UI chats: `search` never filters by `chat_id`.
//! `chat_id` is metadata for which session first stored the entry.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use uuid::Uuid;

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
#[derive(Debug, Default)]
pub struct SemanticCache {
    entries: Vec<CacheEntry>,
}

impl SemanticCache {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
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

    /// Store a new prompt/response pair with its embedding.
    pub fn insert(
        &mut self,
        embedding: Vec<f32>,
        prompt: impl Into<String>,
        response: impl Into<String>,
        chat_id: Uuid,
    ) {
        self.entries.push(CacheEntry {
            embedding,
            prompt: prompt.into(),
            response: response.into(),
            timestamp: Utc::now(),
            chat_id,
        });
    }
}

/// Process-wide shared cache.
pub type SharedSemanticCache = Arc<RwLock<SemanticCache>>;

pub fn new_shared() -> SharedSemanticCache {
    Arc::new(RwLock::new(SemanticCache::new()))
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

    #[test]
    fn search_finds_near_duplicate() {
        let mut cache = SemanticCache::new();
        let embedding = vec![1.0, 0.0, 0.0];
        cache.insert(embedding.clone(), "hello", "world", Uuid::nil());

        let hit = cache.search(&embedding, DEFAULT_THRESHOLD);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().response, "world");
    }

    #[test]
    fn search_misses_below_threshold() {
        let mut cache = SemanticCache::new();
        cache.insert(vec![1.0, 0.0, 0.0], "a", "b", Uuid::nil());

        let miss = cache.search(&[0.0, 1.0, 0.0], DEFAULT_THRESHOLD);
        assert!(miss.is_none());
    }

    #[test]
    fn cache_hits_are_shared_across_chats() {
        let chat_a = Uuid::new_v4();
        let chat_b = Uuid::new_v4();
        let embedding = vec![0.0, 1.0, 0.0];

        let mut cache = SemanticCache::new();
        cache.insert(embedding.clone(), "what is rust?", "A language.", chat_a);

        let hit = cache
            .search(&embedding, DEFAULT_THRESHOLD)
            .expect("expected a global cache hit");
        assert_eq!(hit.response, "A language.");
        assert_eq!(hit.chat_id, chat_a);
        assert_ne!(hit.chat_id, chat_b);
    }
}
