//! Local embedding model wrapper (`all-MiniLM-L6-v2`).

use std::sync::Arc;

use candle_core::Device;
use sentence_transformers_rs::sentence_transformer::{
    SentenceTransformer, SentenceTransformerBuilder, Which,
};
use thiserror::Error;

/// Expected output size for `all-MiniLM-L6-v2`.
pub const EMBEDDING_DIM: usize = 384;

#[derive(Debug, Error)]
pub enum EmbedderError {
    #[error("failed to load embedding model: {0}")]
    Load(String),
    #[error("failed to embed text: {0}")]
    Embed(String),
    #[error("model returned no embedding")]
    EmptyOutput,
    #[error("expected {expected}-dim embedding, got {actual}")]
    UnexpectedDim { expected: usize, actual: usize },
}

/// CPU sentence embedder. Construct once and share with [`Arc`].
pub struct Embedder {
    model: SentenceTransformer,
}

impl Embedder {
    /// Download (on first use) and load `all-MiniLM-L6-v2` on CPU.
    pub fn load() -> Result<Self, EmbedderError> {
        let device = Device::Cpu;
        let model = SentenceTransformerBuilder::with_sentence_transformer(&Which::AllMiniLML6v2)
            .with_device(&device)
            .build()
            .map_err(|e| EmbedderError::Load(e.to_string()))?;
        Ok(Self { model })
    }

    /// Embed `text` into a 384-dimensional L2-normalized vector.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedderError> {
        let embeddings = self
            .model
            .embed(&[text])
            .map_err(|e| EmbedderError::Embed(e.to_string()))?;

        let vector = embeddings
            .into_iter()
            .next()
            .ok_or(EmbedderError::EmptyOutput)?;

        if vector.len() != EMBEDDING_DIM {
            return Err(EmbedderError::UnexpectedDim {
                expected: EMBEDDING_DIM,
                actual: vector.len(),
            });
        }

        Ok(vector)
    }
}

/// Shared handle for the process-wide embedder.
pub type SharedEmbedder = Arc<Embedder>;
