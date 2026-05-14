use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex as TokioMutex;

use super::rag_engine::VectorStore;

/// Vector Map metadata stored in database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VectorMap {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub total_chunks: i64,
    pub total_files: i64,
    pub size_bytes: i64,
}

/// Statistics about a vector map
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMapStats {
    pub id: i64,
    pub name: String,
    pub total_chunks: i64,
    pub total_files: i64,
    pub size_bytes: i64,
    pub projects_using: i64,
}

/// Record of an indexed file for incremental indexing
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IndexedFileRecord {
    pub id: i64,
    pub vector_map_id: i64,
    pub file_path: String,
    pub last_modified: i64,        // Unix timestamp (system mtime)
    pub last_indexed_at: String,   // SQLite datetime
    pub chunk_count: i64,
    pub file_size_bytes: i64,
}

/// Manages multiple vector maps with SQLite backend
pub struct VectorMapManager {
    pool: SqlitePool,
    /// In-memory cache: vector_map_id -> VectorStore
    cache: Arc<TokioMutex<HashMap<i64, VectorStore>>>,
}

impl VectorMapManager {
    /// Create a new VectorMapManager
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            cache: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    /// Load a vector map from database into memory (with caching)
    pub async fn load_map(&self, map_id: i64) -> Result<VectorStore, String> {
        // Check cache first
        let mut cache = self.cache.lock().await;
        if let Some(store) = cache.get(&map_id) {
            println!("[VectorMapManager] Cache hit for map {}", map_id);
            return Ok(store.clone());
        }

        println!("[VectorMapManager] Loading map {} from database...", map_id);

        // Load from database
        let embeddings: Vec<(String, Vec<u8>, String)> = sqlx::query_as(
            "SELECT chunk_key, embedding, content
             FROM vector_embeddings
             WHERE vector_map_id = ?"
        )
        .bind(map_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to load embeddings: {}", e))?;

        let mut store = VectorStore::new();
        let mut failed_count = 0;

        for (chunk_key, embedding_blob, content) in embeddings {
            // Deserialize Vec<f32> from BLOB
            match bincode::deserialize::<Vec<f32>>(&embedding_blob) {
                Ok(embedding) => {
                    store.insert(chunk_key, embedding, content);
                }
                Err(e) => {
                    eprintln!("[VectorMapManager] Failed to deserialize embedding: {}", e);
                    failed_count += 1;
                }
            }
        }

        if failed_count > 0 {
            eprintln!("[VectorMapManager] Warning: {} embeddings failed to deserialize", failed_count);
        }

        let loaded_count = store.len();
        println!("[VectorMapManager] Loaded {} embeddings into map {}", loaded_count, map_id);

        // Cache the loaded store
        cache.insert(map_id, store.clone());

        Ok(store)
    }

    /// Save a single chunk to the database
    pub async fn save_chunk(
        &self,
        map_id: i64,
        chunk_key: String,
        embedding: Vec<f32>,
        content: String,
        file_path: String,
        start_line: i64,
    ) -> Result<(), String> {
        // Serialize embedding to BLOB
        let embedding_blob = bincode::serialize(&embedding)
            .map_err(|e| format!("Failed to serialize embedding: {}", e))?;

        sqlx::query(
            "INSERT OR REPLACE INTO vector_embeddings
             (vector_map_id, chunk_key, embedding, content, file_path, start_line)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(map_id)
        .bind(&chunk_key)
        .bind(&embedding_blob)
        .bind(&content)
        .bind(&file_path)
        .bind(start_line)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to save chunk: {}", e))?;

        // Update cache if loaded
        let mut cache = self.cache.lock().await;
        if let Some(store) = cache.get_mut(&map_id) {
            store.insert(chunk_key, embedding, content);
        }

        Ok(())
    }

    /// Delete all chunks for a specific file (before re-indexing)
    pub async fn delete_file_chunks(&self, map_id: i64, file_path: &str) -> Result<(), String> {
        sqlx::query(
            "DELETE FROM vector_embeddings
             WHERE vector_map_id = ? AND file_path = ?"
        )
        .bind(map_id)
        .bind(file_path)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to delete chunks: {}", e))?;

        // Invalidate cache for this map (force reload next time)
        let mut cache = self.cache.lock().await;
        cache.remove(&map_id);

        Ok(())
    }

    /// Get list of files that need re-indexing (changed since last index)
    pub async fn get_files_to_reindex(
        &self,
        map_id: i64,
        project_path: &str,
        candidate_files: Vec<String>,
    ) -> Result<Vec<String>, String> {
        let mut files_to_index = Vec::new();
        let total_candidates = candidate_files.len();

        for relative_path in candidate_files {
            let full_path = Path::new(project_path).join(&relative_path);
            let full_path_str = full_path.to_string_lossy().to_string();

            // Get file's current mtime
            let current_mtime = match std::fs::metadata(&full_path) {
                Ok(metadata) => match metadata.modified() {
                    Ok(time) => match time.duration_since(UNIX_EPOCH) {
                        Ok(duration) => duration.as_secs() as i64,
                        Err(_) => {
                            // If time is before UNIX epoch, skip this file
                            continue;
                        }
                    },
                    Err(_) => {
                        // Can't get modification time, skip
                        continue;
                    }
                },
                Err(_) => {
                    // File doesn't exist, skip
                    continue;
                }
            };

            // Check if file is in indexed_files table
            let record: Option<(i64,)> = sqlx::query_as(
                "SELECT last_modified FROM indexed_files
                 WHERE vector_map_id = ? AND file_path = ?"
            )
            .bind(map_id)
            .bind(&full_path_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Failed to query indexed_files: {}", e))?;

            match record {
                None => {
                    // File was never indexed -> needs indexing
                    println!("[VectorMapManager] File never indexed: {}", relative_path);
                    files_to_index.push(relative_path);
                }
                Some((last_modified,)) => {
                    if current_mtime > last_modified {
                        // File modified since last index -> needs re-indexing
                        println!("[VectorMapManager] File modified: {} (current: {}, last: {})",
                                 relative_path, current_mtime, last_modified);
                        files_to_index.push(relative_path);
                    } else {
                        // File unchanged -> skip
                        println!("[VectorMapManager] File unchanged: {}", relative_path);
                    }
                }
            }
        }

        println!("[VectorMapManager] {} / {} files need (re-)indexing",
                 files_to_index.len(), total_candidates);

        Ok(files_to_index)
    }

    /// Update indexed_files record after successful indexing
    pub async fn update_file_index_record(
        &self,
        map_id: i64,
        file_path: &str,
        mtime: i64,
        chunk_count: i64,
        file_size_bytes: i64,
    ) -> Result<(), String> {
        sqlx::query(
            "INSERT OR REPLACE INTO indexed_files
             (vector_map_id, file_path, last_modified, last_indexed_at, chunk_count, file_size_bytes)
             VALUES (?, ?, ?, datetime('now'), ?, ?)"
        )
        .bind(map_id)
        .bind(file_path)
        .bind(mtime)
        .bind(chunk_count)
        .bind(file_size_bytes)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update indexed_files: {}", e))?;

        Ok(())
    }

    /// Update vector map metadata (total_chunks, total_files, size_bytes, etc.)
    pub async fn update_map_metadata(&self, map_id: i64) -> Result<(), String> {
        // Count chunks
        let chunk_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM vector_embeddings WHERE vector_map_id = ?"
        )
        .bind(map_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to count chunks: {}", e))?;

        // Count files
        let file_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM indexed_files WHERE vector_map_id = ?"
        )
        .bind(map_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to count files: {}", e))?;

        // Calculate total size of embeddings (sum of embedding BLOB sizes)
        let size_bytes: (Option<i64>,) = sqlx::query_as(
            "SELECT SUM(LENGTH(embedding)) FROM vector_embeddings WHERE vector_map_id = ?"
        )
        .bind(map_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to calculate size: {}", e))?;

        let total_size = size_bytes.0.unwrap_or(0);

        // Update map metadata
        sqlx::query(
            "UPDATE vector_maps
             SET total_chunks = ?, total_files = ?, size_bytes = ?, updated_at = datetime('now')
             WHERE id = ?"
        )
        .bind(chunk_count.0)
        .bind(file_count.0)
        .bind(total_size)
        .bind(map_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update map metadata: {}", e))?;

        Ok(())
    }

    /// Search within a specific vector map
    pub async fn search(
        &self,
        map_id: i64,
        query_embedding: Vec<f32>,
        top_k: usize,
    ) -> Result<Vec<(String, f32, String)>, String> {
        // Load map into memory if not already cached
        let store = self.load_map(map_id).await?;

        // Perform cosine similarity search
        let results = store.search(&query_embedding, top_k);

        Ok(results)
    }

    /// Clear cache for a specific map (force reload next time)
    pub async fn invalidate_cache(&self, map_id: i64) {
        let mut cache = self.cache.lock().await;
        cache.remove(&map_id);
        println!("[VectorMapManager] Cache invalidated for map {}", map_id);
    }

    /// Clear all caches
    pub async fn clear_all_caches(&self) {
        let mut cache = self.cache.lock().await;
        cache.clear();
        println!("[VectorMapManager] All caches cleared");
    }
}
