/// Tauri commands for Vector Map management
use crate::Project;
use gluon_desktop_lib::local_ai::vector_map_manager::{VectorMap, VectorMapStats};
use gluon_desktop_lib::AppState;
use sqlx::SqlitePool;
use tauri::State;
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;
use std::time::{SystemTime, UNIX_EPOCH};
use gluon_desktop_lib::local_ai::rag_engine::VectorStore;

#[derive(serde::Serialize)]
pub struct FileRagStatus {
    path: String,
    status: String, // "indexed", "outdated", "unindexed"
    last_indexed: Option<i64>,
    last_modified_disk: i64,
}

/// Get RAG status for all files in a project
#[tauri::command]
pub async fn get_project_rag_status(
    project_path: String,
    pool: State<'_, SqlitePool>,
) -> Result<Vec<FileRagStatus>, String> {
    // 1. Fetch project configuration including excluded_paths
    let project: (i64, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT COALESCE(vector_map_id, 1), excluded_paths, allowed_extensions FROM projects WHERE path = ?"
    )
    .bind(&project_path)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| format!("DB Error: {}", e))?
    .unwrap_or((1, None, None));

    let vector_map_id = project.0;
    let excluded_paths_str = project.1;
    let allowed_extensions_str = project.2;

    // 2. Fetch indexed files from DB
    let indexed_files: Vec<(String, i64)> = sqlx::query_as(
        "SELECT file_path, last_modified FROM indexed_files WHERE vector_map_id = ?"
    )
    .bind(vector_map_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("DB Error: {}", e))?;

    // [FIX] Normalize DB paths to forward slashes to ensure matching works on Windows
    let index_map: HashMap<String, i64> = indexed_files.into_iter()
        .map(|(path, time)| (path.replace('\\', "/"), time))
        .collect();

    // 3. Parse excluded paths and extensions from project config
    let mut ignore_dirs: Vec<String> = vec![
        "node_modules".to_string(), ".git".to_string(), "target".to_string(),
        "dist".to_string(), "build".to_string(), ".next".to_string(),
        ".vs".to_string(), "obj".to_string(), "bin".to_string(),
        "__pycache__".to_string(), ".venv".to_string(), "venv".to_string()
    ];

    // Add project-specific excluded paths (parse JSON array or comma-separated)
    if let Some(excluded) = excluded_paths_str {
        // Try to parse as JSON array first
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&excluded) {
            for path in parsed {
                let trimmed = path.trim().to_string();
                if !trimmed.is_empty() && !ignore_dirs.contains(&trimmed) {
                    ignore_dirs.push(trimmed);
                }
            }
        } else {
            // Fallback to comma-separated parsing
            for path in excluded.split(',') {
                let trimmed = path.trim().to_string();
                if !trimmed.is_empty() && !ignore_dirs.contains(&trimmed) {
                    ignore_dirs.push(trimmed);
                }
            }
        }
    }

    // 4. Scan disk
    let mut results = Vec::new();
    let mut total_files_seen = 0;
    let mut skipped_by_extension = 0;
    let mut skipped_context_files = 0;

    // Parse allowed extensions from project config or use defaults (parse JSON array or comma-separated)
    let allowed_exts: Vec<String> = if let Some(exts_str) = allowed_extensions_str {
        // Try to parse as JSON array first
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&exts_str) {
            parsed.iter().map(|s| s.trim().to_lowercase()).collect()
        } else {
            // Fallback to comma-separated parsing
            exts_str.split(',').map(|s| s.trim().to_lowercase()).collect()
        }
    } else {
        vec![
            "rs".to_string(), "toml".to_string(), "js".to_string(), "jsx".to_string(),
            "ts".to_string(), "tsx".to_string(), "html".to_string(), "css".to_string(),
            "json".to_string(), "md".to_string(), "yml".to_string(), "yaml".to_string(),
            "txt".to_string(), "py".to_string(), "java".to_string(), "go".to_string(),
            "sql".to_string(), "xml".to_string(), "pdf".to_string(), "docx".to_string()
        ]
    };

    println!("[RAG Status] Scanning project: {}", project_path);
    println!("[RAG Status] Excluded dirs: {:?}", ignore_dirs);
    println!("[RAG Status] Allowed extensions: {:?}", allowed_exts);

    for entry in WalkDir::new(&project_path)
        .into_iter()
        .filter_entry(|e| {
            // Only filter at directory level - skip hidden and common ignore dirs
            if !e.file_type().is_dir() {
                return true; // Don't filter files at this stage
            }
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && !ignore_dirs.iter().any(|d| d == &name.to_string())
        })
    {
        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                total_files_seen += 1;
                let full_path = entry.path().to_string_lossy().to_string().replace('\\', "/");

                // Skip Gluon context files
                if full_path.contains("-context-") {
                    skipped_context_files += 1;
                    continue;
                }

                // Check extension
                if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                    if !allowed_exts.contains(&ext.to_lowercase()) {
                        skipped_by_extension += 1;
                        continue;
                    }
                } else {
                    skipped_by_extension += 1;
                    continue;
                }

                let metadata = entry.metadata().map_err(|e| e.to_string())?;
                let modified_disk = metadata.modified().unwrap_or(SystemTime::now())
                    .duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

                let (status, last_indexed) = if let Some(&indexed_time) = index_map.get(&full_path) {
                    // Tolerance 2s for filesystem diffs
                    if modified_disk > indexed_time + 2 {
                        ("outdated", Some(indexed_time))
                    } else {
                        ("indexed", Some(indexed_time))
                    }
                } else {
                    ("unindexed", None)
                };

                results.push(FileRagStatus {
                    path: full_path,
                    status: status.to_string(),
                    last_indexed,
                    last_modified_disk: modified_disk
                });
            }
        }
    }

    println!("[RAG Status] Files seen: {}, Skipped (extension): {}, Skipped (context): {}, Added to results: {}",
             total_files_seen, skipped_by_extension, skipped_context_files, results.len());

    // Deduplicate results by path (keep first occurrence)
    let initial_count = results.len();
    let mut seen_paths = std::collections::HashSet::new();
    results.retain(|file| seen_paths.insert(file.path.clone()));

    if initial_count != results.len() {
        println!("[RAG Status] Warning: Found {} duplicate paths, deduplicated to {}",
                 initial_count - results.len(), results.len());
    }

    // Count by status for diagnostics
    let indexed_count = results.iter().filter(|f| f.status == "indexed").count();
    let outdated_count = results.iter().filter(|f| f.status == "outdated").count();
    let unindexed_count = results.iter().filter(|f| f.status == "unindexed").count();

    println!("[RAG Status] Final: Indexed: {}, Outdated: {}, Unindexed: {}, Total: {}",
             indexed_count, outdated_count, unindexed_count, results.len());

    // Sort: Outdated -> Unindexed -> Indexed
    results.sort_by(|a, b| {
        let score = |s: &str| match s { "outdated" => 0, "unindexed" => 1, _ => 2 };
        score(&a.status).cmp(&score(&b.status)).then(a.path.cmp(&b.path))
    });

    Ok(results)
}

/// Get all vector maps
#[tauri::command]
pub async fn get_vector_maps(pool: State<'_, SqlitePool>) -> Result<Vec<VectorMap>, String> {
    let mut maps: Vec<VectorMap> = sqlx::query_as(
        "SELECT id, name, description, created_at, updated_at, total_chunks, total_files, size_bytes
         FROM vector_maps
         ORDER BY name"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("Failed to get vector maps: {}", e))?;

    // Recalculate sizes for any maps that have 0 size but chunks
    for map in &mut maps {
        if map.size_bytes == 0 && map.total_chunks > 0 {
            let size_result: (Option<i64>,) = sqlx::query_as(
                "SELECT SUM(LENGTH(embedding)) FROM vector_embeddings WHERE vector_map_id = ?"
            )
            .bind(map.id)
            .fetch_one(pool.inner())
            .await
            .map_err(|e| format!("Failed to calculate size for map {}: {}", map.id, e))?;

            map.size_bytes = size_result.0.unwrap_or(0);
        }
    }

    Ok(maps)
}

/// Get statistics for a specific vector map
#[tauri::command]
pub async fn get_vector_map_stats(
    map_id: i64,
    pool: State<'_, SqlitePool>,
) -> Result<VectorMapStats, String> {
    // Get map info
    let mut map: VectorMap = sqlx::query_as(
        "SELECT id, name, description, created_at, updated_at, total_chunks, total_files, size_bytes
         FROM vector_maps
         WHERE id = ?"
    )
    .bind(map_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Failed to get vector map: {}", e))?;

    // If size_bytes is 0 but we have chunks, recalculate it
    if map.size_bytes == 0 && map.total_chunks > 0 {
        let size_result: (Option<i64>,) = sqlx::query_as(
            "SELECT SUM(LENGTH(embedding)) FROM vector_embeddings WHERE vector_map_id = ?"
        )
        .bind(map_id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| format!("Failed to calculate size: {}", e))?;

        map.size_bytes = size_result.0.unwrap_or(0);
    }

    // Count projects using this map
    let (projects_using,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM projects WHERE vector_map_id = ?"
    )
    .bind(map_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Failed to count projects: {}", e))?;

    Ok(VectorMapStats {
        id: map.id,
        name: map.name,
        total_chunks: map.total_chunks,
        total_files: map.total_files,
        size_bytes: map.size_bytes,
        projects_using,
    })
}

/// Get list of projects using a specific vector map
#[tauri::command]
pub async fn get_shared_projects(
    map_id: i64,
    pool: State<'_, SqlitePool>,
) -> Result<Vec<String>, String> {
    let projects: Vec<(String,)> = sqlx::query_as(
        "SELECT path FROM projects WHERE vector_map_id = ? ORDER BY path"
    )
    .bind(map_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("Failed to get shared projects: {}", e))?;

    Ok(projects.into_iter().map(|(path,)| path).collect())
}

/// Create a new vector map
#[tauri::command]
pub async fn create_vector_map(
    name: String,
    description: Option<String>,
    pool: State<'_, SqlitePool>,
) -> Result<VectorMap, String> {
    // Insert new map
    let result = sqlx::query(
        "INSERT INTO vector_maps (name, description) VALUES (?, ?)"
    )
    .bind(&name)
    .bind(&description)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("Failed to create vector map: {}", e))?;

    let new_map_id = result.last_insert_rowid();

    // Fetch the created map
    let map: VectorMap = sqlx::query_as(
        "SELECT id, name, description, created_at, updated_at, total_chunks, total_files, size_bytes
         FROM vector_maps
         WHERE id = ?"
    )
    .bind(new_map_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Failed to fetch created map: {}", e))?;

    println!("[VectorMaps] Created new vector map: {} (id: {})", name, new_map_id);

    Ok(map)
}

/// Update a project's vector map
#[tauri::command]
pub async fn update_project_vector_map(
    project_path: String,
    vector_map_id: i64,
    pool: State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE projects SET vector_map_id = ? WHERE path = ?")
        .bind(vector_map_id)
        .bind(&project_path)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Failed to update project vector map: {}", e))?;

    println!("[VectorMaps] Updated project '{}' to use map {}", project_path, vector_map_id);

    Ok(())
}

/// Delete a vector map (moves projects to default map first)
#[tauri::command]
pub async fn delete_vector_map(
    map_id: i64,
    pool: State<'_, SqlitePool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Prevent deleting the default map
    if map_id == 1 {
        return Err("Cannot delete the default vector map".to_string());
    }

    // Move all projects using this map to default map (id=1)
    sqlx::query("UPDATE projects SET vector_map_id = 1 WHERE vector_map_id = ?")
        .bind(map_id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Failed to move projects to default map: {}", e))?;

    // Delete the map (CASCADE will delete all embeddings and indexed_files)
    sqlx::query("DELETE FROM vector_maps WHERE id = ?")
        .bind(map_id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Failed to delete vector map: {}", e))?;

    // Invalidate cache for this map
    state.vector_map_manager.invalidate_cache(map_id).await;

    println!("[VectorMaps] Deleted vector map {}", map_id);

    Ok(())
}

/// Clear all embeddings from a vector map (keep the map itself)
#[tauri::command]
pub async fn clear_vector_map(
    map_id: i64,
    pool: State<'_, SqlitePool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Delete all embeddings
    sqlx::query("DELETE FROM vector_embeddings WHERE vector_map_id = ?")
        .bind(map_id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Failed to clear embeddings: {}", e))?;

    // Delete all indexed_files records
    sqlx::query("DELETE FROM indexed_files WHERE vector_map_id = ?")
        .bind(map_id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Failed to clear indexed files: {}", e))?;

    // Update map metadata
    state.vector_map_manager.update_map_metadata(map_id).await?;

    // Invalidate cache
    state.vector_map_manager.invalidate_cache(map_id).await;

    println!("[VectorMaps] Cleared all embeddings from map {}", map_id);

    Ok(())
}

/// Manual RAG search result
#[derive(serde::Serialize, Clone)]
pub struct RagSearchResult {
    pub file_path: String,
    pub content: String,
    pub score: f32,
}

/// Perform manual RAG search with user query
#[tauri::command]
pub async fn rag_search_manual(
    query: String,
    top_k: usize,
    project_path: Option<String>,
    state: State<'_, AppState>,
    pool: State<'_, SqlitePool>,
) -> Result<Vec<RagSearchResult>, String> {
    println!("[RAG Search] Manual search: query='{}', top_k={}, project='{:?}'", query, top_k, project_path);

    // Check if RAG service is running
    if !state.local_ai.is_running() {
        return Err("RAG service is not running".to_string());
    }

    // Create temporary VectorStore for searching
    let mut temp_store = VectorStore::new();

    // Determine which vector_map_id to use based on project_path
    let vector_map_id: i64 = if let Some(ref path) = project_path {
        // Get vector_map_id for this project
        sqlx::query_scalar("SELECT COALESCE(vector_map_id, 1) FROM projects WHERE path = ?")
            .bind(path)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| format!("DB Error: {}", e))?
            .unwrap_or(1)
    } else {
        // Default to map 1 if no project specified
        1i64
    };

    println!("[RAG Search] Loading embeddings from vector_map_id={} for project: {:?}", vector_map_id, project_path);

    // Fetch chunks from database
    let chunks: Vec<(String, Vec<u8>, String)> = sqlx::query_as(
        "SELECT file_path, embedding, content FROM vector_embeddings WHERE vector_map_id = ? LIMIT 10000"
    )
    .bind(vector_map_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("Failed to load embeddings from database: {}", e))?;

    if chunks.is_empty() {
        return Err("No embeddings found in database. Please index some files first.".to_string());
    }

    println!("[RAG Search] Loaded {} chunks from database", chunks.len());

    // Convert database chunks to VectorStore format
    for (file_path, embedding_bytes, content) in chunks {
        // Deserialize embedding from BLOB
        let embedding: Vec<f32> = bincode::deserialize(&embedding_bytes)
            .map_err(|e| format!("Failed to deserialize embedding: {}", e))?;

        // Use unique key format (file_path is already unique from DB query)
        temp_store.insert(file_path, embedding, content);
    }

    // Perform the search
    let results = temp_store
        .search_structured(&query, top_k)
        .await
        .map_err(|e| format!("Search failed: {}", e))?;

    // Convert to serializable format
    let search_results: Vec<RagSearchResult> = results
        .into_iter()
        .map(|chunk| RagSearchResult {
            file_path: chunk.file_path.clone(),
            content: chunk.content.clone(),
            score: chunk.score,
        })
        .collect();

    println!("[RAG Search] Found {} results", search_results.len());

    Ok(search_results)
}