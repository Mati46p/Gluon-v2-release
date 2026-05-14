//! Data structures for Holographic Memory System

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Enhanced code chunk with semantic metadata from Tree-sitter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedCodeChunk {
    // Core identity
    pub chunk_id: String,              // UUID or hash-based ID
    pub file_path: String,             // Relative to project root
    pub start_line: usize,             // 1-based line number
    pub end_line: usize,
    pub content: String,               // Actual code content

    // Semantic context (from Tree-sitter SemanticSignature)
    pub symbol_name: Option<String>,   // e.g., "login", "UserController"
    pub symbol_kind: Option<String>,   // e.g., "function", "class", "method"
    pub parent_name: Option<String>,   // e.g., class name for methods

    // Code quality metrics (from QueryMatcher)
    pub cyclomatic_complexity: usize,  // Control flow complexity
    pub security_alerts: Vec<String>,  // Detected security issues
    pub type_coverage: f32,            // 0.0-1.0 typed parameters ratio
    pub has_docstring: bool,           // Documentation present

    // Git versioning (NEW - Temporal Consistency)
    pub commit_hash: String,           // Git SHA
    pub committed_at: DateTime<Utc>,   // Commit timestamp
    pub author: String,                // Commit author

    // Project isolation (NEW)
    pub project_id: i64,               // FK to projects table
    pub project_path: String,          // Project root path

    // Metadata
    pub embedding_model: String,       // e.g., "nomic-embed-text-v2-moe.Q8_0"
    pub indexed_at: DateTime<Utc>,     // When this chunk was indexed
}

/// Metadata stored alongside vectors in LanceDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMetadata {
    pub chunk_id: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub symbol_name: Option<String>,
    pub symbol_kind: Option<String>,
    pub parent_name: Option<String>,
    pub commit_hash: String,
    pub project_id: i64,
    pub indexed_at: DateTime<Utc>,
}

/// Filters for semantic search queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilters {
    pub commit_hash: Option<String>,      // Filter by specific commit
    pub file_patterns: Vec<String>,       // Glob patterns (e.g., "src/**/*.rs")
    pub min_complexity: Option<usize>,    // Minimum cyclomatic complexity
    pub max_complexity: Option<usize>,    // Maximum cyclomatic complexity
    pub symbol_kind: Option<String>,      // Filter by kind (function, class, etc.)
    pub has_security_alerts: Option<bool>,// Only chunks with/without security issues
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            commit_hash: None,
            file_patterns: vec![],
            min_complexity: None,
            max_complexity: None,
            symbol_kind: None,
            has_security_alerts: None,
        }
    }
}

/// Result from semantic search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk: EnhancedCodeChunk,
    pub score: f32,                       // Similarity score (0.0-1.0)
    pub distance: f32,                    // Vector distance
}

/// Statistics from indexing operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub files_processed: usize,
    pub chunks_created: usize,
    pub chunks_updated: usize,
    pub chunks_deleted: usize,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

/// Memory statistics for a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_chunks: usize,
    pub total_vectors: usize,
    pub disk_usage_bytes: u64,
    pub last_indexed: Option<DateTime<Utc>>,
    pub current_commit: Option<String>,
    pub projects_count: usize,
}

/// Migration statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStats {
    pub legacy_vectors_count: usize,
    pub migrated_count: usize,
    pub failed_count: usize,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}
