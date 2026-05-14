//! LanceDB-backed holographic memory store
//!
//! This module implements the main vector storage interface using LanceDB
//! with support for:
//! - Persistent vector storage
//! - Git-aware versioning
//! - Semantic search with filters
//! - Backward compatibility (DualWrite mode)

use crate::memory::types::{
    EnhancedCodeChunk, VectorMetadata, SearchFilters, SearchResult,
    IndexStats, MemoryStats,
};
use crate::memory::smart_chunker::SmartChunker;
use crate::memory::git_tracker::GitTracker;
use crate::local_ai::rag_engine::VectorStore;  // Legacy store
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use std::time::Instant;
use std::fs;

/// Store operation mode for gradual migration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreMode {
    /// Write to both LanceDB and legacy HashMap (safe migration)
    DualWrite,
    /// Only use LanceDB (full migration complete)
    LanceOnly,
}

pub struct HolographicStore {
    project_id: i64,
    project_path: PathBuf,
    mode: StoreMode,

    // LanceDB connection (TODO: Implement actual LanceDB once dependencies compile)
    // lance_db: Arc<lance::DB>,
    lance_path: PathBuf,

    // Legacy HashMap store for backward compatibility
    legacy_store: Option<Arc<TokioMutex<VectorStore>>>,

    // Git tracker for versioning
    git_tracker: Arc<GitTracker>,

    // Smart chunker
    chunker: SmartChunker,
}

impl HolographicStore {
    /// Initialize holographic store for a project
    ///
    /// Creates `.gluon/memory/` directory if it doesn't exist
    pub async fn new(project_id: i64, project_path: &Path) -> Result<Self, String> {
        // Create memory directory
        let memory_dir = project_path.join(".gluon").join("memory");
        tokio::fs::create_dir_all(&memory_dir).await
            .map_err(|e| format!("Failed to create memory directory: {}", e))?;

        let lance_path = memory_dir.join("vectors.lance");

        // Initialize Git tracker
        let git_tracker = Arc::new(GitTracker::new(project_path)?);

        println!("[HolographicStore] Initialized for project {} at {:?}", project_id, project_path);
        println!("[HolographicStore] LanceDB path: {:?}", lance_path);
        println!("[HolographicStore] Git repo: {}", git_tracker.is_git_repo());

        // TODO: Initialize actual LanceDB connection
        // For now, create placeholder
        // let lance_db = lance::connect(&lance_path).await
        //     .map_err(|e| format!("Failed to connect to LanceDB: {}", e))?;

        Ok(Self {
            project_id,
            project_path: project_path.to_path_buf(),
            mode: StoreMode::DualWrite,  // Start in safe mode
            lance_path,
            legacy_store: None,  // Will be set via set_legacy_store()
            git_tracker,
            chunker: SmartChunker::new(),
        })
    }

    /// Set legacy store for backward compatibility (DualWrite mode)
    pub fn set_legacy_store(&mut self, legacy: Arc<TokioMutex<VectorStore>>) {
        self.legacy_store = Some(legacy);
    }

    /// Switch to LanceDB-only mode (after migration complete)
    pub fn set_mode(&mut self, mode: StoreMode) {
        self.mode = mode;
        println!("[HolographicStore] Switched to mode: {:?}", mode);
    }

    /// Index a single file
    ///
    /// - Chunks file using SmartChunker
    /// - Generates embeddings (via external API for now)
    /// - Stores in LanceDB (and legacy if DualWrite)
    /// - Tags with current commit hash
    pub async fn index_file(
        &mut self,
        file_path: &Path,
        content: &str,
        _commit_hash: Option<&str>,  // TODO: Use this
    ) -> Result<Vec<String>, String> {
        println!("[HolographicStore] Indexing file: {:?}", file_path);

        // Get current commit
        let commit_hash = self.git_tracker.current_commit()?;
        let commit_info = self.git_tracker.commit_info(&commit_hash)?;

        // Chunk the file using SmartChunker
        let mut chunks = self.chunker.chunk_file(file_path, content)?;

        // Populate git and project metadata
        for chunk in &mut chunks {
            chunk.commit_hash = commit_hash.clone();
            chunk.committed_at = commit_info.timestamp;
            chunk.author = commit_info.author.clone();
            chunk.project_id = self.project_id;
            chunk.project_path = self.project_path.to_string_lossy().to_string();
        }

        println!("[HolographicStore] Created {} chunks", chunks.len());

        // TODO: Generate embeddings and store in LanceDB
        // For now, just return chunk IDs as placeholder
        let chunk_ids: Vec<String> = chunks.iter().map(|c| c.chunk_id.clone()).collect();

        // If DualWrite mode, also write to legacy store
        if self.mode == StoreMode::DualWrite {
            if let Some(legacy) = &self.legacy_store {
                let mut legacy_guard = legacy.lock().await;
                let _ = legacy_guard.index_file(
                    file_path.to_string_lossy().to_string(),
                    content.to_string(),
                ).await;
                println!("[HolographicStore] Also wrote to legacy store");
            }
        }

        Ok(chunk_ids)
    }

    /// Semantic search with filters
    pub async fn search(
        &self,
        query: &str,
        _filters: SearchFilters,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, String> {
        println!("[HolographicStore] Searching for: '{}' (top_k={})", query, top_k);

        // TODO: Implement actual LanceDB vector search
        // For now, delegate to legacy store if available
        if let Some(legacy) = &self.legacy_store {
            let legacy_guard = legacy.lock().await;
            let results = legacy_guard.search_by_query(query.to_string(), top_k).await?;

            // Convert legacy results to SearchResult format
            // This is a placeholder - we'll need actual chunk data
            return Ok(vec![]);  // Empty for now
        }

        Ok(vec![])
    }

    /// Git-aware search (temporal consistency)
    pub async fn search_at_commit(
        &self,
        query: &str,
        commit_hash: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, String> {
        let filters = SearchFilters {
            commit_hash: Some(commit_hash.to_string()),
            ..Default::default()
        };

        self.search(query, filters, top_k).await
    }

    /// Get memory statistics
    pub async fn stats(&self) -> Result<MemoryStats, String> {
        // TODO: Implement actual stats from LanceDB
        Ok(MemoryStats {
            total_chunks: 0,
            total_vectors: 0,
            disk_usage_bytes: self.disk_usage()?,
            last_indexed: None,
            current_commit: Some(self.git_tracker.current_commit()?),
            projects_count: 1,
        })
    }

    /// Get disk usage in bytes
    fn disk_usage(&self) -> Result<u64, String> {
        // TODO: Implement proper directory size calculation
        if self.lance_path.exists() {
            let metadata = fs::metadata(&self.lance_path)
                .map_err(|e| format!("Failed to get metadata: {}", e))?;
            Ok(metadata.len())
        } else {
            Ok(0)
        }
    }

    /// Garbage collection - remove orphaned vectors
    pub async fn gc(&mut self, _current_commit: &str) -> Result<usize, String> {
        // TODO: Implement GC
        println!("[HolographicStore] Garbage collection not yet implemented");
        Ok(0)
    }

    /// Index entire directory
    pub async fn index_directory(
        &mut self,
        root_path: &Path,
        excluded_patterns: &[String],
    ) -> Result<IndexStats, String> {
        use walkdir::WalkDir;

        let start = Instant::now();
        let mut stats = IndexStats {
            files_processed: 0,
            chunks_created: 0,
            chunks_updated: 0,
            chunks_deleted: 0,
            duration_ms: 0,
            errors: vec![],
        };

        println!("[HolographicStore] Indexing directory: {:?}", root_path);

        for entry in WalkDir::new(root_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();

            // Skip ignored files
            if crate::memory::file_watcher::should_ignore(path, excluded_patterns) {
                continue;
            }

            // Only index supported files
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if !["rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "cpp", "c", "h"].contains(&ext) {
                    continue;
                }
            } else {
                continue;
            }

            // Read and index file
            match fs::read_to_string(path) {
                Ok(content) => {
                    match self.index_file(path, &content, None).await {
                        Ok(chunk_ids) => {
                            stats.files_processed += 1;
                            stats.chunks_created += chunk_ids.len();
                        }
                        Err(e) => {
                            stats.errors.push(format!("{}: {}", path.display(), e));
                        }
                    }
                }
                Err(e) => {
                    stats.errors.push(format!("{}: {}", path.display(), e));
                }
            }
        }

        stats.duration_ms = start.elapsed().as_millis() as u64;
        println!("[HolographicStore] Indexing complete: {} files, {} chunks, {} errors",
            stats.files_processed, stats.chunks_created, stats.errors.len());

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_holographic_store_init() {
        let temp_dir = TempDir::new().unwrap();
        let store = HolographicStore::new(1, temp_dir.path()).await.unwrap();

        assert_eq!(store.project_id, 1);
        assert_eq!(store.mode, StoreMode::DualWrite);
        assert!(temp_dir.path().join(".gluon/memory").exists());
    }

    #[tokio::test]
    async fn test_store_mode_switch() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = HolographicStore::new(1, temp_dir.path()).await.unwrap();

        assert_eq!(store.mode, StoreMode::DualWrite);
        store.set_mode(StoreMode::LanceOnly);
        assert_eq!(store.mode, StoreMode::LanceOnly);
    }
}
