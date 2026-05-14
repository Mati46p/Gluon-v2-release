//! Holographic Memory System (Phase 3)
//!
//! This module implements the production-grade memory system with:
//! - **MVP**: Tantivy BM25 keyword search (embedded, local-first)
//! - **Future**: LanceDB for semantic vector search (when dependency conflict resolved)
//! - Git-aware versioning for temporal consistency
//! - Smart Tree-sitter chunking for semantic boundaries
//! - Background file watching for automatic re-indexing
//! - Project isolation (.gluon/memory/ per project)
//!
//! ## Search Strategy
//! - **MVP (v3.0)**: Tantivy BM25 keyword search - fast, accurate, no embeddings needed
//! - **v3.1**: Add OpenAI/Ollama embeddings for semantic search
//! - **v3.2**: LanceDB vector store (when arrow/chrono conflict resolved)

pub mod types;
pub mod smart_chunker;
pub mod store;
pub mod git_tracker;
pub mod file_watcher;
pub mod embedding_provider; // MVP: trait for future embedding support
pub mod tantivy_store;      // MVP: BM25 keyword search

// Re-export main types
pub use types::{EnhancedCodeChunk, VectorMetadata, SearchFilters, SearchResult, IndexStats, MemoryStats};
pub use smart_chunker::SmartChunker;
pub use store::{HolographicStore, StoreMode};
pub use git_tracker::{GitTracker, CommitInfo};
pub use file_watcher::FileWatcher;
pub use embedding_provider::{EmbeddingProvider, NoEmbeddingProvider, EmbeddingModelInfo, EmbeddingError};
pub use tantivy_store::{TantivyStore, TantivyError};
