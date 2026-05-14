//! Embedding Provider Interface
//!
//! This module defines the trait for embedding generation providers.
//!
//! **MVP Strategy**: For MVP, we use Tantivy keyword search (no embeddings).
//! This trait provides the architecture for v3.1+ when we add semantic search.
//!
//! **Supported Providers (Post-MVP)**:
//! - OpenAI: text-embedding-3-small/large (highest quality, paid)
//! - Ollama: nomic-embed-text (local, free, good quality)
//! - Fallback: Tantivy BM25 (keyword search, no embeddings needed)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Result type for embedding operations
pub type EmbeddingResult<T> = Result<T, EmbeddingError>;

/// Errors that can occur during embedding generation
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Provider not available: {0}")]
    ProviderUnavailable(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),
}

/// Metadata about the embedding model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModelInfo {
    /// Provider name (e.g., "openai", "ollama", "tantivy")
    pub provider: String,

    /// Model name (e.g., "text-embedding-3-small", "nomic-embed-text")
    pub model: String,

    /// Embedding dimension (e.g., 1536 for OpenAI, 768 for nomic)
    pub dimension: usize,

    /// Maximum tokens per request
    pub max_tokens: usize,

    /// Cost per 1M tokens (in USD cents), None if free/local
    pub cost_per_million_tokens: Option<f64>,
}

/// Trait for embedding generation providers
///
/// Implementations handle the specific API calls and model management
/// for different embedding providers.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Get information about the embedding model
    fn model_info(&self) -> &EmbeddingModelInfo;

    /// Generate embedding for a single text
    ///
    /// # Arguments
    /// * `text` - The text to embed
    ///
    /// # Returns
    /// Vector of floats representing the embedding
    async fn embed(&self, text: &str) -> EmbeddingResult<Vec<f32>>;

    /// Generate embeddings for multiple texts (batch operation)
    ///
    /// More efficient than calling embed() multiple times.
    ///
    /// # Arguments
    /// * `texts` - Slice of texts to embed
    ///
    /// # Returns
    /// Vector of embeddings, one per input text
    async fn embed_batch(&self, texts: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>>;

    /// Check if the provider is available and properly configured
    ///
    /// # Returns
    /// Ok(()) if provider is ready, Err otherwise
    async fn health_check(&self) -> EmbeddingResult<()>;
}

/// Fallback "provider" that returns empty embeddings
///
/// Used when semantic search is disabled (MVP mode with Tantivy only)
pub struct NoEmbeddingProvider;

impl NoEmbeddingProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EmbeddingProvider for NoEmbeddingProvider {
    fn model_info(&self) -> &EmbeddingModelInfo {
        // Static reference to avoid allocation
        static MODEL_INFO: once_cell::sync::Lazy<EmbeddingModelInfo> =
            once_cell::sync::Lazy::new(|| EmbeddingModelInfo {
                provider: "none".to_string(),
                model: "keyword-search-only".to_string(),
                dimension: 0,
                max_tokens: 0,
                cost_per_million_tokens: Some(0.0),
            });
        &MODEL_INFO
    }

    async fn embed(&self, _text: &str) -> EmbeddingResult<Vec<f32>> {
        // Return empty vector - Tantivy doesn't need embeddings
        Ok(Vec::new())
    }

    async fn embed_batch(&self, texts: &[&str]) -> EmbeddingResult<Vec<Vec<f32>>> {
        // Return empty vectors for all inputs
        Ok(vec![Vec::new(); texts.len()])
    }

    async fn health_check(&self) -> EmbeddingResult<()> {
        // Always healthy - no external dependencies
        Ok(())
    }
}

impl Default for NoEmbeddingProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_embedding_provider() {
        let provider = NoEmbeddingProvider::new();

        // Check model info
        let info = provider.model_info();
        assert_eq!(info.provider, "none");
        assert_eq!(info.dimension, 0);

        // Test single embedding
        let embedding = provider.embed("test text").await.unwrap();
        assert!(embedding.is_empty());

        // Test batch embedding
        let texts = vec!["text1", "text2", "text3"];
        let embeddings = provider.embed_batch(&texts).await.unwrap();
        assert_eq!(embeddings.len(), 3);
        assert!(embeddings.iter().all(|e| e.is_empty()));

        // Test health check
        assert!(provider.health_check().await.is_ok());
    }
}
