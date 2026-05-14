//! Tantivy-based Full-Text Search Store
//!
//! This module implements keyword-based code search using Tantivy's BM25 algorithm.
//!
//! **Why Tantivy?**
//! - Embedded (no external services) - maintains "local-first" principle
//! - Fast BM25 ranking (better than grep for relevance)
//! - Production-ready and battle-tested
//! - Zero-setup (just files in `.gluon/memory/tantivy_index/`)
//!
//! **MVP Strategy**:
//! This provides "good enough" search for MVP. Future versions (v3.1+) will add
//! semantic search with embeddings, but keyword search remains as a fallback.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Value, Schema, STRING, STORED, TEXT, FAST};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};
use crate::memory::types::{EnhancedCodeChunk, SearchFilters, SearchResult};
use std::collections::HashSet;

/// Result type for Tantivy operations
pub type TantivyResult<T> = Result<T, TantivyError>;

/// Errors that can occur in Tantivy store
#[derive(Debug, thiserror::Error)]
pub enum TantivyError {
    #[error("Index creation failed: {0}")]
    IndexCreationFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Tantivy error: {0}")]
    TantivyError(#[from] tantivy::TantivyError),

    #[error("Search error: {0}")]
    SearchError(String),

    #[error("Document not found: {0}")]
    DocumentNotFound(String),
}

/// Tantivy-based code search store
///
/// Provides BM25-ranked keyword search over code chunks.
pub struct TantivyStore {
    /// Path to the index directory
    index_path: PathBuf,

    /// Tantivy index
    index: Index,

    /// Index reader for searches
    reader: IndexReader,

    /// Index writer for adding documents
    writer: Arc<parking_lot::Mutex<IndexWriter>>,

    /// Schema fields
    schema: TantivySchema,
}

/// Schema field IDs for easy access
struct TantivySchema {
    chunk_id: Field,
    file_path: Field,
    content: Field,
    symbol_name: Field,
    commit_hash: Field,
    start_line: Field,
    end_line: Field,
    kind: Field,
}

impl TantivyStore {
    /// Create a new Tantivy store or open an existing one
    ///
    /// # Arguments
    /// * `index_path` - Directory path for the Tantivy index (e.g., `.gluon/memory/tantivy_index/`)
    ///
    /// # Returns
    /// Initialized TantivyStore
    pub fn new<P: AsRef<Path>>(index_path: P) -> TantivyResult<Self> {
        let index_path = index_path.as_ref().to_path_buf();

        // Create directory if it doesn't exist
        fs::create_dir_all(&index_path)?;

        // Define schema
        let mut schema_builder = Schema::builder();

        // Unique ID for the chunk (UUID)
        let chunk_id = schema_builder.add_text_field("chunk_id", STRING | STORED);

        // File path (stored for results, not searchable)
        let file_path = schema_builder.add_text_field("file_path", STRING | STORED);

        // Content (main searchable field with BM25 scoring)
        let content = schema_builder.add_text_field("content", TEXT | STORED);

        // Symbol name (function/class name - searchable)
        let symbol_name = schema_builder.add_text_field("symbol_name", TEXT | STORED);

        // Git commit hash for temporal consistency
        let commit_hash = schema_builder.add_text_field("commit_hash", STRING | STORED);

        // Line numbers (for exact location)
        let start_line = schema_builder.add_u64_field("start_line", STORED | FAST);
        let end_line = schema_builder.add_u64_field("end_line", STORED | FAST);

        // Symbol kind (function, class, method, etc.)
        let kind = schema_builder.add_text_field("kind", STRING | STORED);

        let schema = schema_builder.build();

        // Create or open index
        let index = if index_path.join("meta.json").exists() {
            // Open existing index
            Index::open_in_dir(&index_path)?
        } else {
            // Create new index
            Index::create_in_dir(&index_path, schema.clone())?
        };

        // Create reader with auto-reload policy
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        // Create writer with 50MB heap
        let writer = index.writer(50_000_000)?;

        Ok(Self {
            index_path,
            index,
            reader,
            writer: Arc::new(parking_lot::Mutex::new(writer)),
            schema: TantivySchema {
                chunk_id,
                file_path,
                content,
                symbol_name,
                commit_hash,
                start_line,
                end_line,
                kind,
            },
        })
    }

    /// Add a code chunk to the index
    ///
    /// # Arguments
    /// * `chunk` - The code chunk to index
    ///
    /// # Returns
    /// Ok(()) if successful
    pub fn add_chunk(&self, chunk: &EnhancedCodeChunk) -> TantivyResult<()> {
        let doc = doc!(
            self.schema.chunk_id => chunk.chunk_id.clone(),
            self.schema.file_path => chunk.file_path.clone(),
            self.schema.content => chunk.content.clone(),
            self.schema.symbol_name => chunk.symbol_name.clone().unwrap_or_default(),
            self.schema.commit_hash => chunk.commit_hash.clone(),
            self.schema.start_line => chunk.start_line as u64,
            self.schema.end_line => chunk.end_line as u64,
            self.schema.kind => chunk.symbol_kind.clone().unwrap_or_default(),
        );

        let writer = self.writer.lock();
        writer.add_document(doc)?;

        Ok(())
    }

    /// Add multiple chunks in batch (more efficient)
    ///
    /// # Arguments
    /// * `chunks` - Slice of code chunks to index
    ///
    /// # Returns
    /// Ok(()) if successful
    pub fn add_chunks(&self, chunks: &[EnhancedCodeChunk]) -> TantivyResult<()> {
        let writer = self.writer.lock();

        // KROK 1: Deduplikacja - usuń stare wpisy dla aktualizowanych plików
        // Zbieramy unikalne ścieżki plików z nowych chunków
        let paths_to_update: HashSet<&String> = chunks.iter().map(|c| &c.file_path).collect();
        
        for path in paths_to_update {
            // Usuwamy WSZYSTKIE dokumenty powiązane z tą ścieżką pliku
            let term = Term::from_field_text(self.schema.file_path, path);
            writer.delete_term(term);
        }

        // KROK 2: Dodaj nowe chunki
        for chunk in chunks {
            let doc = doc!(
                self.schema.chunk_id => chunk.chunk_id.clone(),
                self.schema.file_path => chunk.file_path.clone(),
                self.schema.content => chunk.content.clone(),
                self.schema.symbol_name => chunk.symbol_name.clone().unwrap_or_default(),
                self.schema.commit_hash => chunk.commit_hash.clone(),
                self.schema.start_line => chunk.start_line as u64,
                self.schema.end_line => chunk.end_line as u64,
                self.schema.kind => chunk.symbol_kind.clone().unwrap_or_default(),
            );

            writer.add_document(doc)?;
        }

        Ok(())
    }

    /// Commit pending changes to the index
    ///
    /// Must be called after add_chunk/add_chunks for changes to be searchable.
    pub fn commit(&self) -> TantivyResult<()> {
        let mut writer = self.writer.lock();
        writer.commit()?;
        Ok(())
    }

    /// Search the index for code chunks
    ///
    /// # Arguments
    /// * `query` - Search query string (supports boolean operators: AND, OR, NOT)
    /// * `limit` - Maximum number of results to return (default: 10)
    /// * `filters` - Optional filters (commit hash, file path pattern, etc.)
    ///
    /// # Returns
    /// Vector of search results, ranked by BM25 score
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        filters: Option<&SearchFilters>,
    ) -> TantivyResult<Vec<SearchResult>> {
        let searcher = self.reader.searcher();

        // Create query parser (search in content and symbol_name fields)
        let fields: Vec<Field> = vec![self.schema.content, self.schema.symbol_name];
        let query_parser = QueryParser::for_index(&self.index, fields);

        let query = query_parser
            .parse_query(query)
            .map_err(|e| TantivyError::SearchError(format!("Query parse error: {}", e)))?;

        // Execute search
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| TantivyError::SearchError(format!("Search failed: {}", e)))?;

        // Convert results
        let mut results: Vec<SearchResult> = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = match searcher.doc(doc_address) {
                Ok(d) => d,
                Err(e) => return Err(TantivyError::SearchError(format!("Doc retrieval failed: {}", e))),
            };

            // Extract fields
            let chunk_id = doc
                .get_first(self.schema.chunk_id)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let file_path = doc
                .get_first(self.schema.file_path)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let content = doc
                .get_first(self.schema.content)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let symbol_name = doc
                .get_first(self.schema.symbol_name)
                .and_then(|v| v.as_str())
                .map(String::from);

            let commit_hash = doc
                .get_first(self.schema.commit_hash)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let start_line = doc
                .get_first(self.schema.start_line)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            let end_line = doc
                .get_first(self.schema.end_line)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;

            // Apply filters if provided
            if let Some(f) = filters {
                // Filter by commit hash
                if let Some(ref filter_commit) = f.commit_hash {
                    if &commit_hash != filter_commit {
                        continue;
                    }
                }

                // Filter by file path patterns (glob check)
                if !f.file_patterns.is_empty() {
                    let matches = f.file_patterns.iter().any(|pattern| file_path.contains(pattern));
                    if !matches {
                        continue;
                    }
                }
            }

            // Create a minimal EnhancedCodeChunk for the result
            // Note: We don't have all fields in the index, so we fill with defaults
            let chunk = EnhancedCodeChunk {
                chunk_id,
                file_path,
                content,
                symbol_name,
                symbol_kind: None, // Not stored in simplified index
                parent_name: None,
                start_line,
                end_line,
                commit_hash,
                cyclomatic_complexity: 0,
                security_alerts: vec![],
                type_coverage: 0.0,
                has_docstring: false,
                committed_at: chrono::Utc::now(), // Placeholder
                author: String::new(),             // Placeholder
                project_id: 0,                     // Placeholder
                project_path: String::new(),       // Placeholder
                embedding_model: "none".to_string(),
                indexed_at: chrono::Utc::now(),
            };

            results.push(SearchResult {
                chunk,
                score: score as f32,
                distance: 1.0 - score as f32, // BM25 score converted to distance
            });
        }

        Ok(results)
    }

    /// Get the number of documents in the index
    pub fn doc_count(&self) -> TantivyResult<usize> {
        let searcher = self.reader.searcher();
        Ok(searcher.num_docs() as usize)
    }

    /// Clear all documents from the index
    ///
    /// WARNING: This is destructive and cannot be undone!
    pub fn clear(&self) -> TantivyResult<()> {
        let mut writer = self.writer.lock();
        writer.delete_all_documents()?;
        writer.commit()?;
        Ok(())
    }

    /// Get the index path
    pub fn index_path(&self) -> &Path {
        &self.index_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_chunk(id: &str, content: &str, symbol: Option<&str>) -> EnhancedCodeChunk {
        EnhancedCodeChunk {
            chunk_id: id.to_string(),
            file_path: format!("src/{}.rs", id),
            content: content.to_string(),
            symbol_name: symbol.map(String::from),
            symbol_kind: Some("function".to_string()),
            start_line: 1,
            end_line: 10,
            commit_hash: "abc123".to_string(),
            author: "test".to_string(),
            committed_at: chrono::Utc::now(),
            cyclomatic_complexity: 1,
            security_alerts: vec![],
            type_coverage: 0.5,
            has_docstring: false,
            parent_name: None,
            project_id: 1,
            project_path: "/test".to_string(),
            embedding_model: "none".to_string(),
            indexed_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_tantivy_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = TantivyStore::new(temp_dir.path()).unwrap();

        assert_eq!(store.doc_count().unwrap(), 0);
        assert_eq!(store.index_path(), temp_dir.path());
    }

    #[test]
    fn test_add_and_search_chunks() {
        let temp_dir = TempDir::new().unwrap();
        let store = TantivyStore::new(temp_dir.path()).unwrap();

        // Add test chunks
        let chunk1 = create_test_chunk("1", "fn authenticate_user() {}", Some("authenticate_user"));
        let chunk2 = create_test_chunk("2", "fn login_handler() {}", Some("login_handler"));
        let chunk3 = create_test_chunk("3", "fn parse_json() {}", Some("parse_json"));

        store.add_chunk(&chunk1).unwrap();
        store.add_chunk(&chunk2).unwrap();
        store.add_chunk(&chunk3).unwrap();
        store.commit().unwrap();

        assert_eq!(store.doc_count().unwrap(), 3);

        // Search for authentication
        let results = store.search("authenticate", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk.chunk_id, "1");
        assert!(results[0].score > 0.0);

        // Search for login
        let results = store.search("login", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk.chunk_id, "2");

        // Search with limit
        let results = store.search("fn", 2, None).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_batch_add_chunks() {
        let temp_dir = TempDir::new().unwrap();
        let store = TantivyStore::new(temp_dir.path()).unwrap();

        let chunks = vec![
            create_test_chunk("1", "test content 1", Some("func1")),
            create_test_chunk("2", "test content 2", Some("func2")),
            create_test_chunk("3", "test content 3", Some("func3")),
        ];

        store.add_chunks(&chunks).unwrap();
        store.commit().unwrap();

        assert_eq!(store.doc_count().unwrap(), 3);
    }

    #[test]
    fn test_search_with_filters() {
        let temp_dir = TempDir::new().unwrap();
        let store = TantivyStore::new(temp_dir.path()).unwrap();

        let mut chunk = create_test_chunk("1", "test function", Some("test_func"));
        chunk.commit_hash = "commit123".to_string();
        chunk.file_path = "src/auth/login.rs".to_string();

        store.add_chunk(&chunk).unwrap();
        store.commit().unwrap();

        // Filter by commit hash
        let filters = SearchFilters {
            commit_hash: Some("commit123".to_string()),
            file_patterns: vec![],
            ..Default::default()
        };
        let results = store.search("test", 10, Some(&filters)).unwrap();
        assert_eq!(results.len(), 1);

        // Filter by wrong commit hash
        let filters = SearchFilters {
            commit_hash: Some("wrongcommit".to_string()),
            ..Default::default()
        };
        let results = store.search("test", 10, Some(&filters)).unwrap();
        assert_eq!(results.len(), 0);

        // Filter by file path
        let filters = SearchFilters {
            file_patterns: vec!["auth".to_string()],
            ..Default::default()
        };
        let results = store.search("test", 10, Some(&filters)).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_clear() {
        let temp_dir = TempDir::new().unwrap();
        let store = TantivyStore::new(temp_dir.path()).unwrap();

        let chunk = create_test_chunk("1", "test", Some("func"));
        store.add_chunk(&chunk).unwrap();
        store.commit().unwrap();

        assert_eq!(store.doc_count().unwrap(), 1);

        store.clear().unwrap();
        assert_eq!(store.doc_count().unwrap(), 0);
    }
}