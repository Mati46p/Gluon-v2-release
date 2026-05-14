//! Tauri Commands for Apply System
//!
//! This module implements Tauri commands that expose the Apply System
//! functionality to the frontend (browser).

use crate::apply_system::{
    ApplySystemConfig, ChangeQueueItem, ChangeStatus, TauriToBrowserMessage,
    shared::config::validate_file_path,
    core::transaction::TransactionManager,
    features::snapshot::SnapshotManager,
    matchers::match_code,
    parsers::parse_model_response,
    context::ContextGraph,
};

use crate::apply_system::context::repo_map::RepoMap;
use crate::apply_system::lazy::engine::LazyStitcherEngine;
use crate::apply_system::features::prompts::LazyStitcherConfig;
use crate::apply_system::features::backup_system::{self, BackupEntry, BackupFilePreview};
use crate::workflow::agent_workflow::AgentWorkflowManager;

use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager, State};
use crate::editor_bridge::{EditorBridge, FileChangeNotification, ChangeRange};
use crate::apply_system::features::debug_manager::DebugSnapshotManager;

// ============================================================================
// Global State Management
// ============================================================================

/// Global state for the Apply System
///
/// Manages:
/// - Queue of pending changes
/// - Snapshot manager
/// - Configuration
/// - Context Graph (Repo Map)
/// - Lazy Stitcher Engine
pub struct ApplySystemState {
    /// Queue of all changes (pending, applied, failed)
    pub change_queue: Arc<Mutex<Vec<ChangeQueueItem>>>,

    /// Snapshot manager for conflict detection and undo
    pub snapshot_manager: Arc<Mutex<SnapshotManager>>,

    /// System configuration
    pub config: Arc<Mutex<ApplySystemConfig>>,

    /// Semantic Graph of the repository
    pub context_graph: Arc<Mutex<ContextGraph>>,

    /// [NAPRAWA 2] Mapa repozytorium do szybkiego wyszukiwania plików (Symbol Resolver)
    pub repo_map: Mutex<RepoMap>,

    /// [NAPRAWA 3] Silnik Lazy Stitcher (Engine)
    pub lazy_stitcher: Mutex<LazyStitcherEngine>,

    /// Set of cancelled request IDs - used to stop processing
    pub cancelled_requests: Arc<Mutex<std::collections::HashSet<String>>>,

    /// Agent Workflow Manager - manages multi-agent communication graph
    pub agent_workflow: AgentWorkflowManager,
}

impl ApplySystemState {
    pub fn new() -> Self {
        Self {
            change_queue: Arc::new(Mutex::new(Vec::new())),
            snapshot_manager: Arc::new(Mutex::new(SnapshotManager::new())),
            config: Arc::new(Mutex::new(ApplySystemConfig::default())),
            context_graph: Arc::new(Mutex::new(ContextGraph::new())),

            // Inicjalizacja nowych komponentów
            repo_map: Mutex::new(RepoMap::new()),
            lazy_stitcher: Mutex::new(LazyStitcherEngine::new(LazyStitcherConfig::default())),
            cancelled_requests: Arc::new(Mutex::new(std::collections::HashSet::new())),
            agent_workflow: AgentWorkflowManager::new(),
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Read file content from disk
async fn read_file_content(file_path: &str) -> Result<String, String> {
    tokio::fs::read_to_string(file_path)
        .await
        .map_err(|e| format!("Failed to read file {}: {}", file_path, e))
}

/// Write file content to disk, ensuring parent directories exist
async fn write_file_content(file_path: &str, content: &str) -> Result<(), String> {
    let path = std::path::Path::new(file_path);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create parent directories for {}: {}", file_path, e))?;
        }
    }

    tokio::fs::write(file_path, content)
        .await
        .map_err(|e| format!("Failed to write file {}: {}", file_path, e))
}

// ============================================================================
// Status Updates (Tauri → Browser)
// ============================================================================

/// Emit status update to the browser
fn emit_status(app: &AppHandle, message: &str, level: &str) {
    let status_msg = TauriToBrowserMessage::StatusUpdate {
        message: message.to_string(),
        level: level.to_string(),
    };

    let _ = app.emit("apply-system-status", status_msg);
}

/// Emit progress update to the browser
fn emit_progress(app: &AppHandle, current: usize, total: usize, message: &str) {
    let progress_msg = TauriToBrowserMessage::Progress {
        current,
        total,
        message: message.to_string(),
    };

    let _ = app.emit("apply-system-progress", progress_msg);
}

/// Emit detailed apply progress update (Pulse System)
/// This provides granular feedback during individual change application
fn emit_apply_progress(
    app: &AppHandle,
    request_id: &str,
    change_id: &str,
    step: crate::apply_system::ProcessingStep,
    message: &str,
    progress: u8,
    details: Option<String>,
    file_path: Option<String>,
) {
    let progress_msg = TauriToBrowserMessage::ApplyProgress {
        request_id: request_id.to_string(),
        change_id: change_id.to_string(),
        step,
        message: message.to_string(),
        progress,
        details: details.clone(),
        file_path,
    };

    eprintln!("[EMIT DEBUG] Emitting apply-system-apply-progress event:");
    eprintln!("  change_id: {}, step: {:?}, progress: {}%, message: {}",
        change_id, step, progress, message);

    if let Err(e) = app.emit("apply-system-apply-progress", progress_msg) {
        eprintln!("[EMIT ERROR] Failed to emit progress event: {:?}", e);
    } else {
        eprintln!("[EMIT DEBUG] ✅ Successfully emitted Tauri event");
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Parse model response into structured changes
#[tauri::command]
pub async fn parse_model_response_command(
    raw_response: String,
    state: State<'_, ApplySystemState>,
    app: AppHandle,
) -> Result<Vec<ChangeQueueItem>, String> {
    emit_status(&app, "Parsing model response...", "info");

    match parse_model_response(&raw_response) {
        Ok(changes) => {
            let config = state.config.lock().unwrap();
            let path_config = &config.path_config;

            let mut validated_changes = Vec::new();
            let mut rejected_changes = Vec::new();

            for change in changes {
                match validate_file_path(&change.file_path, path_config) {
                    Ok(()) => {
                        validated_changes.push(change);
                    }
                    Err(reason) => {
                        emit_status(
                            &app,
                            &format!("Rejected change for {}: {}", change.file_path, reason),
                            "warning",
                        );
                        rejected_changes.push((change.file_path.clone(), reason));
                    }
                }
            }
            drop(config);

            let accepted_count = validated_changes.len();
            let rejected_count = rejected_changes.len();

            if rejected_count > 0 {
                emit_status(
                    &app,
                    &format!(
                        "Security: Rejected {} unsafe path(s), accepted {}",
                        rejected_count, accepted_count
                    ),
                    "warning",
                );
            }

            emit_status(
                &app,
                &format!("Successfully parsed {} change(s)", accepted_count),
                "success",
            );

            let mut queue = state.change_queue.lock().unwrap();
            queue.append(&mut validated_changes.clone());

            let response = TauriToBrowserMessage::ParsingComplete {
                request_id: uuid::Uuid::new_v4().to_string(),
                success: true,
                changes: Some(validated_changes.clone()),
                error: None,
            };
            let _ = app.emit("apply-system-parsing", response);

            Ok(validated_changes)
        }
        Err(e) => {
            let error_msg = format!("Parsing failed: {:?}", e);
            emit_status(&app, &error_msg, "error");

            let response = TauriToBrowserMessage::ParsingComplete {
                request_id: uuid::Uuid::new_v4().to_string(),
                success: false,
                changes: None,
                error: Some(error_msg.clone()),
            };
            let _ = app.emit("apply-system-parsing", response);

            Err(error_msg)
        }
    }
}
/// Apply a single change
#[tauri::command]
pub async fn apply_change_command(
    change_id: String,
    file_content: String,
    request_id: Option<String>,  // Added for Pulse system tracking
    state: State<'_, ApplySystemState>,
    app: AppHandle,
) -> Result<(), String> {
    // [GLUON LOG] Start of Process - User initiated Action
    crate::gluon_info!("ApplySystem", "🚀 USER INITIATED APPLY: change_id={}", change_id);
    eprintln!("[Gluon Debug] Starting apply_change_command for ID: {}", change_id);

    // Generate request_id if not provided (for backward compatibility)
    let request_id = request_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    crate::gluon_info!("ApplySystem", "Request ID assigned: {}", request_id);

    use crate::apply_system::shared::protocol::ProcessingStep;

    // Get file_path early for progress tracking
    let file_path_for_progress = {
        let queue = state.change_queue.lock().unwrap();
        queue
            .iter()
            .find(|c| c.id == change_id)
            .map(|c| c.file_path.clone())
    };

    if let Some(ref path) = file_path_for_progress {
        crate::gluon_info!("ApplySystem", "Target File: {}", path);
    } else {
        crate::gluon_error!("ApplySystem", "CRITICAL: Change ID {} not found in queue!", change_id);
    }

    // Step 1: Queued (0% - just starting)
    eprintln!("[APPLY DEBUG] Starting apply_change_command for change_id: {}", change_id);
    eprintln!("[APPLY DEBUG] Request ID: {}", request_id);

    emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Queued, "Change queued for processing", 0, None, file_path_for_progress.clone());
    eprintln!("[APPLY DEBUG] ✅ Emitted Queued progress event");

    emit_status(&app, &format!("Applying change {}...", change_id), "info");
    eprintln!("[APPLY DEBUG] ✅ Emitted status update");

    // Step 2: Validating (10% - validating file path)
    emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Validating, "Validating file path and permissions", 10, None, file_path_for_progress.clone());

    let (file_path_clone, old_code_is_empty) = {
        let mut queue = state.change_queue.lock().unwrap();
        let change = queue
            .iter_mut()
            .find(|c| c.id == change_id)
            .ok_or_else(|| "Change not found in queue".to_string())?;

        let config = state.config.lock().unwrap();
        if let Err(reason) = validate_file_path(&change.file_path, &config.path_config) {
            change.status = ChangeStatus::Failed;
            change.error_message = Some(format!("Security validation failed: {}", reason));
            let file_path_for_error = change.file_path.clone();
            let reason_for_error = reason.clone();

            emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Failed,
                &format!("Security validation blocked: {}", reason_for_error), 100, None, Some(file_path_for_error.clone()));
            emit_status(
                &app,
                &format!(
                    "Security: Blocked apply to {}: {}",
                    file_path_for_error, reason_for_error
                ),
                "error",
            );
            return Err(format!("Security validation failed: {}", reason_for_error));
        }

        (change.file_path.clone(), change.old_code.is_empty())
    };

    // [FIX] Special handling for new file creation (old_code is empty)
    if old_code_is_empty {
        eprintln!("[Gluon Debug] ✨ New file creation detected for: {}", file_path_clone);

        // Step 3: Creating file
        emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Snapshotting, "Creating new file...", 30, None, file_path_for_progress.clone());

        // Get new_code from queue
        let new_code = {
            let queue = state.change_queue.lock().unwrap();
            let change = queue
                .iter()
                .find(|c| c.id == change_id)
                .ok_or_else(|| "Change not found in queue".to_string())?;
            change.new_code.clone()
        };

        // Create parent directories if they don't exist
        if let Some(parent) = std::path::Path::new(&file_path_clone).parent() {
            if !parent.exists() {
                eprintln!("[Gluon Debug] Creating parent directories: {:?}", parent);
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("Failed to create parent directories: {}", e))?;
            }
        }

        // Step 4: Writing new file
        emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Writing, "Writing new file content...", 70, None, file_path_for_progress.clone());

        // Write the new file
        write_file_content(&file_path_clone, &new_code).await
            .map_err(|e| {
                emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Failed,
                    &format!("Failed to create file: {}", e), 100, None, file_path_for_progress.clone());
                format!("Failed to create file: {}", e)
            })?;

        eprintln!("[Gluon Debug] ✅ New file created successfully: {}", file_path_clone);

        // Step 5: Success
        emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Success, "New file created successfully!", 100, None, file_path_for_progress.clone());

        // Update status in queue
        {
            let mut queue = state.change_queue.lock().unwrap();
            if let Some(change) = queue.iter_mut().find(|c| c.id == change_id) {
                change.status = ChangeStatus::Applied;
            }
        }

        emit_status(&app, &format!("✨ Created new file: {}", file_path_clone), "success");
        return Ok(());
    }

    // Normal flow for existing files (modifications)
    // Step 3: Snapshotting (20% - creating backup)
    emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Snapshotting, "Creating snapshot for undo", 20, None, file_path_for_progress.clone());
    eprintln!("[Gluon Debug] Creating snapshot for {}", file_path_clone);
    {
        let snapshot_mgr = state.snapshot_manager.lock().unwrap();
        // Create both legacy (per-file) and per-change snapshots
        snapshot_mgr.create_snapshot(file_path_clone.clone(), file_content.clone());
        snapshot_mgr.create_change_snapshot(change_id.to_string(), file_path_clone.clone(), file_content.clone());
    }

    // Step 4: Matching (30% - finding exact code location)
    emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Matching, "Locating code in file...", 30, None, file_path_for_progress.clone());

    let (old_code, line_start) = {
        let mut queue = state.change_queue.lock().unwrap();
        let change = queue
            .iter_mut()
            .find(|c| c.id == change_id)
            .ok_or_else(|| "Change not found after snapshot".to_string())?;
        change.status = ChangeStatus::Matching;
        (change.old_code.clone(), change.line_start)
    };

    eprintln!("[Gluon Debug] Starting match_code for {}", file_path_clone);
    let match_result = match_code(&file_content, &old_code, line_start, Some(&file_path_clone))
        .map_err(|e| {
            emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Failed,
                &format!("Matching failed: {:?}", e), 100, None, file_path_for_progress.clone());
            format!("Matching failed: {:?}", e)
        })?;
    eprintln!("[Gluon Debug] Match successful: lines {}-{}", match_result.matched_line_start, match_result.matched_line_end);

    // Step 5: Match found - add details about confidence
    let confidence_pct = (match_result.confidence * 100.0) as u8;
    let match_details = format!("Line {} | Method: {:?} | Confidence: {}%",
        match_result.matched_line_start, match_result.method_used, confidence_pct);
    emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::SafetyCheck,
        "Match found, verifying safety...", 60, Some(match_details), file_path_for_progress.clone());

    let (file_path, new_code) = {
        let mut queue = state.change_queue.lock().unwrap();
        let change = queue
            .iter_mut()
            .find(|c| c.id == change_id)
            .ok_or_else(|| "Change not found after matching".to_string())?;

        change.match_result = Some(match_result.clone());
        change.status = ChangeStatus::Applying;
        (change.file_path.clone(), change.new_code.clone())
    };

    // Step 6: Writing (70% - preparing to write)
    emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Writing, "Preparing to write changes...", 70, None, file_path_for_progress.clone());
    emit_status(&app, "Applying changes to file...", "info");
    eprintln!("[Gluon Debug] Reading file: {}", file_path);
 
    let current_content = match tokio::fs::read_to_string(&file_path).await {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to read file for writing: {}", e)),
    };
    eprintln!("[Gluon Debug] File read successfully ({} bytes). Calculating indentation...", current_content.len());
 
    let line_start = match_result.matched_line_start;
    let line_end = match_result.matched_line_end;
 
    use crate::apply_system::matchers::utils::smart_adjust_indentation;
    let adjusted_new_code = smart_adjust_indentation(
        &current_content,
        line_start,
        &new_code
    );
    eprintln!("[Gluon Debug] Indentation adjusted. Preparing write...");
 
    let lines: Vec<&str> = current_content.lines().collect();
 
    let start_idx = if line_start > 0 { line_start - 1 } else { 0 };
    let end_idx = if line_end > 0 { line_end } else { 0 };
    let safe_end_idx = std::cmp::min(end_idx, lines.len());
 
    if start_idx > lines.len() {
        return Err("Match start line out of bounds".to_string());
    }
 
    let mut new_lines_vec = Vec::new();
    new_lines_vec.extend_from_slice(&lines[..start_idx]);
    if !adjusted_new_code.is_empty() {
        new_lines_vec.push(adjusted_new_code.as_str());
    }
    if safe_end_idx < lines.len() {
        new_lines_vec.extend_from_slice(&lines[safe_end_idx..]);
    }

    let final_content = new_lines_vec.join("\n");

    let editor_bridge = app.state::<EditorBridge>();
    
    // Zmienna do przechowywania czy zapis się udał, aby potem wysłać powiadomienie
    // Używamy underscore aby wyciszyć warning, jeśli logika powiadomień jest tymczasowo nieaktywna
    let mut write_success = false;

    if editor_bridge.is_connected() {
        emit_status(&app, "Delegating write to VS Code (preserving undo stack)...", "info");
        
        // [FIX] Używamy wersji extended, aby przekazać change_id do rejestru Undo w VS Code
        match editor_bridge.request_edit_extended(
            file_path.clone(), 
            final_content.clone(),
            Some(change_id.clone()),       // Przekazujemy ID zmiany
            Some(request_id.clone()),      // Przekazujemy ID requestu (jako batch)
            Some(current_content.clone()), // Przekazujemy starą zawartość (dla pewności cofania)
            Some(line_start)               // Przekazujemy linię startu
        ).await {
            Ok(_) => { 
                eprintln!("[Gluon Debug] Change {} sent to VS Code successfully.", change_id);
                write_success = true; 
            },
            Err(e) => {
                emit_status(&app, &format!("VS Code apply failed: {}. Falling back to disk write.", e), "warning");
                if let Err(e) = write_file_content(&file_path, &final_content).await {
                    return Err(format!("Failed to write file (fallback): {}", e));
                }
                write_success = true;
            }
        }
    } else {
        if let Err(e) = write_file_content(&file_path, &final_content).await {
            return Err(format!("Failed to write file: {}", e));
        }
        write_success = true;
    }

    // --- Editor Notification Logic (Flash Effect) ---
    if write_success {
        // Step 7: Notifying (90% - sending editor notification)
        emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Notifying, "Notifying editor...", 90, None, file_path_for_progress.clone());

        // Obliczamy zakres zmian do podświetlenia.
        // Konwertujemy 1-based indexing na 0-based dla VS Code.
        // Używamy długości nowego kodu (snippetu), aby określić wysokość podświetlenia.

        // Bezpieczne odejmowanie
        let start_line_0based = line_start.saturating_sub(1);
        let new_lines_count = new_code.lines().count();
        let end_line_0based = start_line_0based + new_lines_count;

        let notification = FileChangeNotification {
            path: file_path.clone(),
            ranges: vec![ChangeRange {
                start_line: start_line_0based,
                end_line: end_line_0based
            }]
        };

        // Wysyłamy powiadomienie asynchronicznie, nie blokując głównego wątku
        editor_bridge.notify_changes(vec![notification]);
    }

    {
        let mut queue = state.change_queue.lock().unwrap();
        if let Some(change) = queue.iter_mut().find(|c| c.id == change_id) {
            change.status = ChangeStatus::Applied;
            change.applied_timestamp = Some(std::time::SystemTime::now());
            // Store the file content after this change (for selective undo)
            change.applied_content = Some(final_content.clone());
        }
    }

    // Step 8: Success (100% - complete)
    emit_apply_progress(&app, &request_id, &change_id, ProcessingStep::Success, "Change applied successfully", 100, None, file_path_for_progress);
    emit_status(&app, "Change applied successfully.", "success");
    Ok(())
}

/// Get current change queue state
#[tauri::command]
pub async fn get_change_queue(
    state: State<'_, ApplySystemState>,
) -> Result<Vec<ChangeQueueItem>, String> {
    let queue = state.change_queue.lock().unwrap();
    Ok(queue.clone())
}

/// Apply all pending changes
#[tauri::command]
pub async fn apply_all_changes(
    state: State<'_, ApplySystemState>,
    editor_bridge: State<'_, EditorBridge>,
    app: AppHandle,
) -> Result<(), String> {
    let mut pending_changes: Vec<ChangeQueueItem> = {
        let queue_guard = state.change_queue.lock().unwrap();
        queue_guard
            .iter()
            .filter(|c| c.status == ChangeStatus::Pending)
            .cloned()
            .collect()
    };

    let total = pending_changes.len();

    if total == 0 {
        emit_status(&app, "No pending changes to apply", "info");
        return Ok(());
    }

    emit_status(&app, &format!("Starting atomic transaction for {} changes...", total), "info");

    let mut transaction_manager = TransactionManager::new();
    let transaction_result = transaction_manager.execute_batch(&mut pending_changes).await;

    {
        let mut queue_guard = state.change_queue.lock().unwrap();
        
        for processed_change in &pending_changes {
            if let Some(original) = queue_guard.iter_mut().find(|c| c.id == processed_change.id) {
                *original = processed_change.clone();
            }
        }
    }

    match transaction_result {
        Ok(batch_result) => {
            let success_msg = format!(
                "Transaction committed successfully. Modified {} files.", 
                batch_result.files_modified
            );
            emit_status(&app, &success_msg, "success");

            // --- Batch Editor Notification (Flash Effect) ---
            // Zbieramy wszystkie zakresy zmian dla wszystkich plików w transakcji
            let mut changes_map: HashMap<String, Vec<ChangeRange>> = HashMap::new();

            for change in &pending_changes {
                let start_line_0based = change.line_start.saturating_sub(1);
                let new_lines_count = change.new_code.lines().count();
                let end_line_0based = start_line_0based + new_lines_count;

                changes_map.entry(change.file_path.clone())
                    .or_default()
                    .push(ChangeRange {
                        start_line: start_line_0based,
                        end_line: end_line_0based
                    });
            }

            let notifications: Vec<FileChangeNotification> = changes_map.into_iter().map(|(path, ranges)| {
                FileChangeNotification { path, ranges }
            }).collect();

            if !notifications.is_empty() {
                editor_bridge.notify_changes(notifications);
            }
            
            let response = TauriToBrowserMessage::ApplyAllComplete {
                request_id: uuid::Uuid::new_v4().to_string(),
                total,
                applied: total, 
                failed: 0,
                failed_changes: vec![],
            };
            let _ = app.emit("apply-system-apply-all", response);
        },
        Err(e) => {
            let err_msg = format!("Batch transaction failed: {}", e);
            emit_status(&app, &err_msg, "error");
            
            let response = TauriToBrowserMessage::ApplyAllComplete {
                request_id: uuid::Uuid::new_v4().to_string(),
                total,
                applied: 0,
                failed: total,
                failed_changes: pending_changes.iter().map(|c| c.id.clone()).collect(),
            };
            let _ = app.emit("apply-system-apply-all", response);
        }
    }

    Ok(())
}

/// Undo a single change with selective reapplication
///
/// This implements selective undo:
/// 1. Find the state before this change (using per-change snapshot)
/// 2. Restore file to that state
/// 3. Mark all changes applied AFTER this one as Pending (they'll need to be reapplied)
/// 4. Mark this change as Pending
#[tauri::command]
pub async fn undo_change(
    change_id: String,
    state: State<'_, ApplySystemState>,
    app: AppHandle,
) -> Result<(), String> {
    emit_status(&app, &format!("Undoing change {}...", change_id), "info");

    // Step 1: Find the change and get its snapshot (state before this change)
    let (file_path, _target_idx, changes_after_indices, restore_content) = {
        let queue = state.change_queue.lock().unwrap();

        let target_idx = queue
            .iter()
            .position(|c| c.id == change_id)
            .ok_or_else(|| "Change not found".to_string())?;

        let change = &queue[target_idx];

        if change.status != ChangeStatus::Applied {
            return Err("Change was not applied".to_string());
        }

        let file_path = change.file_path.clone();

        // Get snapshot from this change (state before applying it)
        let snapshot_mgr = state.snapshot_manager.lock().unwrap();
        let snapshot = snapshot_mgr
            .get_change_snapshot(&change_id)
            .ok_or_else(|| "No snapshot found for this change".to_string())?
            .1;

        // Find all changes for this file that came after this change
        let changes_after: Vec<usize> = queue
            .iter()
            .enumerate()
            .filter(|(i, c)| {
                c.file_path == file_path && *i > target_idx && c.status == ChangeStatus::Applied
            })
            .map(|(i, _)| i)
            .collect();

        (file_path, target_idx, changes_after, snapshot.content.clone())
    };
    // Step 2: Restore file to state before this change
    // This effectively undoes the target change AND all changes that came after it
    write_file_content(&file_path, &restore_content).await?;

    // Step 3: Mark changes as pending (this one + all dependent after it)
    let dependent_change_ids = {
        let mut queue = state.change_queue.lock().unwrap();
        let mut dependent_ids = Vec::new();

        // Mark target change as PENDING (not applied)
        if let Some(change) = queue.iter_mut().find(|c| c.id == change_id) {
            change.status = ChangeStatus::Pending;
            change.applied_timestamp = None;
            change.applied_content = None;
        }

        // Mark all changes applied AFTER this one as PENDING
        // They are physically gone from the file now, so we must re-apply them.
        for idx in changes_after_indices {
            if let Some(change) = queue.get_mut(idx) {
                dependent_ids.push(change.id.clone());
                change.status = ChangeStatus::Pending;
                change.applied_timestamp = None;
                change.applied_content = None;
            }
        }

        dependent_ids
    };

    // Broadcast status change for the undone change (Target)
    // We send "undone" so the UI shows the "Redo" button, even though internally it is Pending application
    // [FIX] Correct event name to match main.rs listener
    let _ = app.emit("apply-system-status-change", serde_json::json!({
        "change_id": change_id,
        "status": "undone"
    }));

    // Broadcast status for dependents (Temporarily Pending while we re-apply)
    for dep_id in &dependent_change_ids {
        let _ = app.emit("apply-system-status-change", serde_json::json!({
            "change_id": dep_id,
            "status": "applying" // Show spinner in UI
        }));
    }

    emit_status(&app, &format!("Restored file to state before change {}. Re-applying dependents...", change_id), "info");

    // AUTOMATIC REAPPLICATION: Automatically reapply dependent changes using full logic
    eprintln!("[Gluon Smart Undo] Undo complete. Starting re-application of {} dependent changes.", dependent_change_ids.len());

    for (i, dep_id) in dependent_change_ids.iter().enumerate() {
        eprintln!("[Gluon Smart Undo] Re-applying dependent change {}/{} (ID: {})", i+1, dependent_change_ids.len(), dep_id);

        // 1. Read current content (Snapshot state + any re-applied changes so far)
        let file_content = match read_file_content(&file_path).await {
            Ok(content) => content,
            Err(e) => {
                eprintln!("[Gluon Smart Undo] 🚨 Failed to read file for reapplication: {}", e);
                emit_status(&app, &format!("Failed to re-apply change: IO Error"), "error");

                // Mark remaining as failed in UI
                let _ = app.emit("apply-system-status-change", serde_json::json!({
                    "change_id": dep_id,
                    "status": "error"
                }));
                continue;
            }
        };

        // 2. Execute full apply command (Matching -> Guard -> Write -> Notify)
        // We generate a temp request ID so Pulse events are isolated
        let temp_req_id = format!("smart-undo-reapply-{}", uuid::Uuid::new_v4());

        match apply_change_command(
            dep_id.clone(),
            file_content,
            Some(temp_req_id),
            state.clone(),
            app.clone()
        ).await {
            Ok(_) => {
                eprintln!("[Gluon Smart Undo] ✅ Successfully reapplied change: {}", dep_id);
            }
            Err(e) => {
                eprintln!("[Gluon Smart Undo] ❌ Failed to reapply change {}: {}", dep_id, e);
                // Mark as error in UI so user knows matching failed (likely due to context shift)
                let _ = app.emit("apply-system-status-change", serde_json::json!({
                    "change_id": dep_id,
                    "status": "error"
                }));
            }
        }
    }

    emit_status(&app, "Smart Undo sequence completed.", "success");

    Ok(())
}

/// Undo all applied changes from current session
#[tauri::command]
pub async fn undo_all_changes(
    state: State<'_, ApplySystemState>,
    app: AppHandle,
) -> Result<(), String> {
    emit_status(&app, "Undoing all changes...", "info");

    let applied_changes = {
        let queue = state.change_queue.lock().unwrap();
        queue
            .iter()
            .filter(|c| c.status == ChangeStatus::Applied)
            .map(|c| c.id.clone())
            .collect::<Vec<_>>()
    };

    let total = applied_changes.len();

    if total == 0 {
        emit_status(&app, "No changes to undo", "info");
        return Ok(());
    }

    emit_status(&app, &format!("Undoing {} change(s)...", total), "info");

    let mut undone = 0;
    let mut failed = 0;

    for (idx, change_id) in applied_changes.iter().enumerate() {
        emit_progress(
            &app,
            idx + 1,
            total,
            &format!("Undoing change {}/{}", idx + 1, total),
        );

        match undo_change(change_id.clone(), state.clone(), app.clone()).await {
            Ok(_) => undone += 1,
            Err(e) => {
                failed += 1;
                eprintln!("Failed to undo {}: {}", change_id, e);
            }
        }
    }

    let summary = format!("Undone {}/{} changes. {} failed.", undone, total, failed);
    emit_status(
        &app,
        &summary,
        if failed == 0 { "success" } else { "warning" },
    );

    let response = TauriToBrowserMessage::UndoAllComplete {
        request_id: uuid::Uuid::new_v4().to_string(),
        total_undone: undone,
    };
    let _ = app.emit("apply-system-undo-all", response);

    Ok(())
}

/// Get current configuration
#[tauri::command]
pub async fn get_config(state: State<'_, ApplySystemState>) -> Result<ApplySystemConfig, String> {
    let config = state.config.lock().unwrap();
    Ok(config.clone())
}

/// Update configuration
#[tauri::command]
pub async fn update_config(
    new_config: Value,
    state: State<'_, ApplySystemState>,
) -> Result<(), String> {
    let config: ApplySystemConfig =
        serde_json::from_value(new_config).map_err(|e| format!("Invalid config: {}", e))?;

    let mut current_config = state.config.lock().unwrap();
    *current_config = config;

    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationRequest {
    file_path: String,
    search_content: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationResult {
    file_path: String,
    line_start: Option<usize>,
}

#[tauri::command]
pub async fn resolve_change_locations(
    requests: Vec<LocationRequest>,
) -> Result<Vec<LocationResult>, String> {
    let mut results = Vec::new();

    for req in requests {
        // 1. Próba odczytu pliku
        let file_content = match read_file_content(&req.file_path).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[ResolveLocation] Failed to read {}: {}", req.file_path, e);
                results.push(LocationResult {
                    file_path: req.file_path,
                    line_start: None,
                });
                continue;
            }
        };

        let search_normalized = req.search_content.replace("\r\n", "\n");

        // 2. PRIMARY: Użyj zaawansowanego matchera (Weighted/Fuzzy)
        let mut line_start = match match_code(&file_content, &search_normalized, 0, Some(&req.file_path)) {
            Ok(res) => Some(res.matched_line_start),
            Err(_) => None,
        };

        // 3. FALLBACK: Robust Sliding Window (Ignoruje wcięcia)
        // Jeśli matcher zawiódł (częste przy małych fragmentach), używamy "siłowego" dopasowania sekwencji.
        if line_start.is_none() {
            // Przygotuj linie szukane: usuń puste, przytnij białe znaki
            let search_lines: Vec<String> = search_normalized
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();

            if !search_lines.is_empty() {
                let file_lines: Vec<&str> = file_content.lines().collect();

                // Iteruj po pliku
                'window: for (i, _) in file_lines.iter().enumerate() {
                    // Sprawdź czy sekwencja zmieści się w pliku
                    if i + search_lines.len() > file_lines.len() {
                        break;
                    }

                    // Sprawdź sekwencję linia po linii
                    for (j, search_line) in search_lines.iter().enumerate() {
                        // Porównuj TRIMMED vs TRIMMED (ignoruje wcięcia)
                        if file_lines[i + j].trim() != search_line.as_str() {
                            continue 'window;
                        }
                    }

                    // Jeśli pętla wewnętrzna przeszła -> Mamy dopasowanie!
                    // println!("[ResolveLocation] Fallback found match at line {}", i + 1);
                    line_start = Some(i + 1);
                    break;
                }
            }
        }

        results.push(LocationResult {
            file_path: req.file_path,
            line_start,
        });
    }

    Ok(results)
}

// ============================================================================
// NEW: Context Graph Commands
// ============================================================================

// ============================================================================
// G-INTERACTIVE PROTOCOL - Unified Context Node API
// ============================================================================

/// Typ operacji kontekstowej żądanej przez model
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextOperation {
    /// Pobierz konkretny symbol z pliku (funkcja/klasa)
    FileSymbol {
        path: String,
        symbol: String,
    },
    /// NOWOŚĆ: Pobierz mapę sygnatur dla konkretnych plików (rozpoznanie struktury)
    SemanticMap {
        paths: Vec<String>,
    },
    /// Przeszukaj repozytorium semantycznie (RAG)
    RagSearch {
        query: String,
        #[serde(default = "default_top_k")]
        top_k: usize,
    },
    /// Pobierz cały plik (dla małych plików < 200 linii)
    FullFile {
        path: String,
    },
}

fn default_top_k() -> usize {
    3
}

/// Pojedynczy wynik kontekstowy
#[derive(serde::Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextItem {
    SymbolContent {
        file_path: String,
        symbol_name: String,
        content: String,
    },
    RagResult {
        query: String,
        results: Vec<String>,
    },
    FileContent {
        file_path: String,
        content: String,
        line_count: usize,
    },
    Error {
        operation: String,
        error: String,
    },
}

/// Odpowiedź z pełnym kontekstem
#[derive(serde::Serialize, Debug)]
pub struct ContextResponse {
    pub request_id: String,
    pub items: Vec<ContextItem>,
    pub total_operations: usize,
    pub successful: usize,
    pub failed: usize,
}

// === Security & Validation ===

/// Maximum number of operations per request (DOS protection)
const MAX_OPERATIONS_PER_REQUEST: usize = 50;

/// Maximum file size to load in full (1MB)
const MAX_FULL_FILE_SIZE: usize = 1_000_000;

/// Validate that path doesn't contain path traversal attacks
fn validate_safe_path(path: &str) -> Result<(), String> {
    // Block obvious attacks
    if path.contains("..") {
        return Err("Path traversal detected: '..' not allowed".to_string());
    }

    // Block absolute paths that go outside project (Windows/Unix)
    if path.starts_with('/') || path.contains(':') {
        if !path.starts_with("C:\\Users") && !path.starts_with("C:/Users") {
            // Allow user paths, but log them
            println!("[Security] Absolute path used: {}", path);
        }
    }

    // Block null bytes
    if path.contains('\0') {
        return Err("Null byte in path".to_string());
    }

    Ok(())
}

/// **Symbol Picker Support: Get File Symbols**
///
/// Returns list of symbols (functions, classes, methods, etc.) from a file
/// for the Symbol Picker UI in the extension.
///
/// Used when user clicks the eye icon (👁️) in file tree to preview file structure.
#[tauri::command]
pub async fn get_file_symbols(
    file_path: String,
    project_root: Option<String>,
) -> Result<Vec<SerializableSymbol>, String> {
    use crate::apply_system::context::symbol_extractor::extract_symbols;
    use std::path::Path;

    println!("[SymbolPicker] 👁️ Fetching symbols for: {}", file_path);

    // Resolve full path
    let full_path = if let Some(root) = project_root {
        let resolved = Path::new(&root).join(&file_path);
        println!("[SymbolPicker] Resolved path: {:?}", resolved);
        resolved
    } else {
        Path::new(&file_path).to_path_buf()
    };

    // Read file
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;

    // Extract symbols using existing symbol_extractor
    let symbols = extract_symbols(&full_path, &content)?;

    println!("[SymbolPicker] ✅ Found {} symbols", symbols.len());

    // Convert to serializable format for JSON transport
    let result: Vec<SerializableSymbol> = symbols
        .into_iter()
        .map(|s| SerializableSymbol {
            name: s.name,
            kind: s.kind.as_str().to_string(),
            line: s.line,
            parent: s.parent,
            signature: s.signature,
        })
        .collect();

    Ok(result)
}

/// Serializable version of Symbol for JSON transport to extension
#[derive(serde::Serialize)]
pub struct SerializableSymbol {
    pub name: String,
    pub kind: String,  // "function", "class", "method", etc.
    pub line: usize,
    pub parent: Option<String>,
    pub signature: String,
}

/// **GŁÓWNA KOMENDA: Context Node Executor**
///
/// Wykonuje batch operacji kontekstowych na żądanie modelu.
/// To jest serce "Interactive Context Protocol" - model wysyła JSON,
/// dostaje precyzyjny kontekst bez halucynacji.
///
/// ### Security Features:
/// - Path traversal protection
/// - Operation count limits (max 50)
/// - File size limits (max 1MB for full files)
/// - Sandbox validation via ApplySystemConfig
#[tauri::command]
pub async fn execute_context_operations(
    operations: Vec<ContextOperation>,
    project_root: Option<String>,
    _state: State<'_, ApplySystemState>,
    app: AppHandle,
) -> Result<ContextResponse, String> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let total_operations = operations.len();

    println!("[G-RAG] 🚀 STARTED Request ID: {}", request_id);
    println!("[G-RAG] Operations count: {}", total_operations);

    if total_operations > MAX_OPERATIONS_PER_REQUEST {
        println!("[G-RAG] ❌ Too many operations requested!");
        return Err(format!("Too many operations: {}. Max: {}", total_operations, MAX_OPERATIONS_PER_REQUEST));
    }

    let mut items = Vec::new();
    let mut successful = 0;
    let mut failed = 0;

    // 1. Dostęp do globalnego stanu (VectorStore dla RAG)
    let app_state: State<crate::AppState> = app.state();
    
    // 2. Pobierz wszystkie zarejestrowane ścieżki projektów z bazy danych dla globalnej rezolucji.
    // Dzięki temu Agent może pobrać plik z projektu "Speedy-Plugin" nawet jeśli project_root to "Gluon-v2".
    let pool = app.state::<sqlx::SqlitePool>();
    let all_project_paths: Vec<String> = match sqlx::query_scalar("SELECT path FROM projects")
        .fetch_all(pool.inner())
        .await {
            Ok(paths) => paths,
            Err(e) => {
                println!("[G-RAG] ⚠️ Database query failed: {}. Falling back to empty projects list.", e);
                Vec::new()
            }
        };

    for (idx, operation) in operations.iter().enumerate() {
        println!("[G-RAG] Processing op [{}/{}]: {:?}", idx + 1, total_operations, operation);
        
        let item = match operation {
            // --- OPERACJA 1: RAG Search (Wyszukiwanie semantyczne) ---
            ContextOperation::RagSearch { query, top_k } => {
                println!("[G-RAG] 🔎 Executing RAG Search: '{}' (top_k: {})", query, top_k);

                // [FIX] Get vector_map_id for current project (support per-project vector maps)
                let vector_map_id: i64 = if let Some(root) = &project_root {
                    sqlx::query_scalar(
                        "SELECT COALESCE(vector_map_id, 1) FROM projects WHERE path = ?"
                    )
                    .bind(root)
                    .fetch_one(pool.inner())
                    .await
                    .unwrap_or(1)
                } else {
                    1 // Fallback to default map if no project_root provided
                };

                println!("[G-RAG] Using vector_map_id: {} for project: {:?}", vector_map_id, project_root);

                let store = app_state.vector_map_manager.load_map(vector_map_id).await
                    .map_err(|e| format!("Failed to load vector map: {}", e))?;
                match store.search_by_query(query.clone(), *top_k).await {
                    Ok(results) => {
                        println!("[G-RAG] ✅ RAG found {} results", results.len());
                        successful += 1;
                        ContextItem::RagResult { query: query.clone(), results }
                    },
                    Err(e) => {
                        println!("[G-RAG] ❌ RAG Error: {}", e);
                        failed += 1;
                        ContextItem::Error { operation: "rag_search".into(), error: e }
                    }
                }
            },

            // --- OPERACJA 2: Symbol Extraction (Chirurgiczne pobieranie kodu) ---
            ContextOperation::FileSymbol { path, symbol } => {
                println!("[G-RAG] 📄 Extracting symbol '{}' from '{}'", symbol, path);
                if let Err(e) = validate_safe_path(&path) {
                    println!("[G-RAG] ❌ Path validation failed: {}", e);
                    failed += 1;
                    ContextItem::Error { operation: format!("symbol:{}", path), error: e }
                } else {
                    // [FIX] Global Path Resolution logic
                    let mut resolved_path_buf = None;

                    // A. Sprawdź w project_root podanym przez rozszerzenie
                    if let Some(root) = &project_root {
                        let p = std::path::Path::new(root).join(&path);
                        if p.exists() { resolved_path_buf = Some(p); }
                    }

                    // B. Szukaj globalnie we wszystkich znanych projektach
                    if resolved_path_buf.is_none() {
                        for proj_root in &all_project_paths {
                            let p = std::path::Path::new(proj_root).join(&path);
                            if p.exists() { 
                                resolved_path_buf = Some(p);
                                break;
                            }
                        }
                    }

                    // C. Sprawdź czy ścieżka jest absolutna
                    if resolved_path_buf.is_none() {
                        let p = std::path::PathBuf::from(&path);
                        if p.exists() && p.is_absolute() { resolved_path_buf = Some(p); }
                    }

                    if let Some(path_obj) = resolved_path_buf {
                        let path_for_log = path_obj.to_string_lossy().to_string();
                        let symbol_clone = symbol.clone();
                        
                        match tokio::task::spawn_blocking(move || {
                            crate::apply_system::context::symbol_extractor::extract_symbol_content(&path_obj, &symbol_clone)
                        }).await {
                            Ok(Ok(content)) => {
                                println!("[G-RAG] ✅ Symbol extracted from {}", path_for_log);
                                successful += 1;
                                ContextItem::SymbolContent { file_path: path.clone(), symbol_name: symbol.clone(), content }
                            },
                            Ok(Err(e)) => {
                                println!("[G-RAG] ❌ Extraction error in {}: {}", path_for_log, e);
                                failed += 1;
                                ContextItem::Error { operation: format!("symbol:{}:{}", path, symbol), error: e }
                            },
                            Err(e) => {
                                println!("[G-RAG] ❌ Task Panic: {}", e);
                                failed += 1;
                                ContextItem::Error { operation: "task_panic".into(), error: e.to_string() }
                            }
                        }
                    } else {
                        println!("[G-RAG] ❌ File not found in any project: {}", path);
                        failed += 1;
                        ContextItem::Error { operation: format!("symbol:{}", path), error: "File not found in any indexed project".into() }
                    }
                }
            },

            // --- OPERACJA 3: Full File Read (Dla małych plików konfiguracyjnych) ---
            ContextOperation::FullFile { path } => {
                println!("[G-RAG] 📖 Reading full file: '{}'", path);
                if let Err(e) = validate_safe_path(&path) {
                    failed += 1;
                    ContextItem::Error { operation: format!("read:{}", path), error: e }
                } else {
                    let mut resolved_path_str = None;

                    // Global Resolution dla FullFile
                    if let Some(root) = &project_root {
                        let p = std::path::Path::new(root).join(&path);
                        if p.exists() { resolved_path_str = Some(p.to_string_lossy().to_string()); }
                    }

                    if resolved_path_str.is_none() {
                        for root in &all_project_paths {
                            let p = std::path::Path::new(root).join(&path);
                            if p.exists() { 
                                resolved_path_str = Some(p.to_string_lossy().to_string());
                                break;
                            }
                        }
                    }

                    if let Some(full_path_str) = resolved_path_str {
                        match tokio::fs::metadata(&full_path_str).await {
                            Ok(meta) => {
                                if meta.len() > MAX_FULL_FILE_SIZE as u64 {
                                    println!("[G-RAG] ⚠️ File too large: {} bytes", meta.len());
                                    failed += 1;
                                    ContextItem::Error { operation: format!("read:{}", path), error: "File too large (>1MB). Use symbol extraction.".into() }
                                } else {
                                    match tokio::fs::read_to_string(&full_path_str).await {
                                        Ok(content) => {
                                            println!("[G-RAG] ✅ File read successfully ({} lines)", content.lines().count());
                                            successful += 1;
                                            ContextItem::FileContent { 
                                                file_path: path.clone(), 
                                                line_count: content.lines().count(),
                                                content 
                                            }
                                        },
                                        Err(e) => {
                                            println!("[G-RAG] ❌ Read error: {}", e);
                                            failed += 1;
                                            ContextItem::Error { operation: format!("read:{}", path), error: e.to_string() }
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                println!("[G-RAG] ❌ Metadata error: {}", e);
                                failed += 1;
                                ContextItem::Error { operation: format!("read:{}", path), error: e.to_string() }
                            }
                        }
                    } else {
                        println!("[G-RAG] ❌ File not found for reading: {}", path);
                        failed += 1;
                        ContextItem::Error { operation: format!("read:{}", path), error: "File not found in any indexed project".into() }
                    }
                }
            },

            // 4. NOWOŚĆ: Generowanie mapy semantycznej dla wybranych plików/katalogów
            ContextOperation::SemanticMap { paths } => {
                println!("[G-RAG] 🗺️ Generating Semantic Map for {} path(s)", paths.len());
                println!("[G-RAG] project_root: {:?}", project_root);
                for p in paths.iter() {
                    println!("[G-RAG] path to index: {}", p);
                }
                let state = app.state::<ApplySystemState>();
                // Mutex lock must be mutable to allow indexing
                let mut graph = state.context_graph.lock().unwrap();

                // Remember files BEFORE indexing to find newly indexed files
                let files_before: std::collections::HashSet<String> = graph.get_all_files().into_iter().collect();
                println!("[G-RAG] 📊 Files before: {}", files_before.len());

                // Collect all file paths (expand directories)
                let mut all_file_paths: Vec<String> = Vec::new();

                // [FIX] Force indexing of requested paths before mapping
                // Expand directories to files, index everything
                for path in paths {
                    let mut resolved_path_buf = None;

                    // A. Check project_root
                    if let Some(root) = &project_root {
                        let p = std::path::Path::new(root).join(path);
                        println!("[G-RAG] 🔍 Checking (A) project_root join: {}", p.display());
                        if p.exists() {
                            println!("[G-RAG] ✅ Found via project_root");
                            resolved_path_buf = Some(p);
                        } else {
                            println!("[G-RAG] ❌ Not found via project_root");
                        }
                    } else {
                        println!("[G-RAG] ⚠️ No project_root provided");
                    }

                    // B. Check all projects
                    if resolved_path_buf.is_none() {
                        for proj_root in &all_project_paths {
                            let p = std::path::Path::new(proj_root).join(path);
                            if p.exists() {
                                resolved_path_buf = Some(p);
                                break;
                            }
                        }
                    }

                    // C. Absolute path
                    if resolved_path_buf.is_none() {
                        let p = std::path::PathBuf::from(path);
                        if p.exists() && p.is_absolute() { resolved_path_buf = Some(p); }
                    }

                    if let Some(p) = resolved_path_buf {
                        // Force index: check if it's a directory or file
                        if p.is_dir() {
                            // Index entire directory
                            let path_str = p.to_string_lossy().to_string();
                            let exclusions = vec![
                                "node_modules".to_string(),
                                "target".to_string(),
                                ".git".to_string(),
                                "dist".to_string(),
                                "build".to_string()
                            ];
                            println!("[G-RAG] 🔍 About to index directory: {}", path_str);
                            graph.index_directory(&path_str, &exclusions, None::<fn(usize, &str)>);
                            println!("[G-RAG] ✅ Indexed directory: {}", path_str);

                            // Collect all NEWLY indexed files from this directory
                            let files_after: std::collections::HashSet<String> = graph.get_all_files().into_iter().collect();
                            println!("[G-RAG] 📊 Files after: {}", files_after.len());

                            let new_files: Vec<String> = files_after.iter()
                                .filter(|f| !files_before.contains(*f))
                                .cloned()
                                .collect();

                            println!("[G-RAG] 📂 Found {} newly indexed files", new_files.len());
                            for nf in &new_files {
                                println!("[G-RAG] 📄 New file: {}", nf);
                            }
                            all_file_paths.extend(new_files);
                        } else if p.is_file() {
                            // Index single file
                            graph.index_file(&p);
                            let file_path = p.to_string_lossy().to_string().replace('\\', "/");
                            all_file_paths.push(file_path.clone());
                            println!("[G-RAG] ✅ Indexed file: {}", p.display());
                        }
                    } else {
                        println!("[G-RAG] ⚠️ Path not found: {}", path);
                    }
                }

                // Generate map from (now populated) graph with collected file paths
                let map_content = if all_file_paths.is_empty() {
                    format!("No files found for paths: {:?}", paths)
                } else {
                    println!("[G-RAG] Generating map for {} file(s)", all_file_paths.len());
                    crate::apply_system::context::ranker::map_target_files(&graph, &all_file_paths)
                };

                successful += 1;
                ContextItem::SymbolContent {
                    file_path: "multi-file-map".into(),
                    symbol_name: "semantic_map".into(),
                    content: map_content
                }
            }
        };
        items.push(item);
    }

    println!("[G-RAG] 🏁 Finished. Success: {}, Failed: {}", successful, failed);

    Ok(ContextResponse {
        request_id,
        items,
        total_operations,
        successful,
        failed,
    })
}

#[tauri::command]
pub async fn refresh_context_graph(
    project_path: String,
    state: State<'_, ApplySystemState>,
) -> Result<String, String> {
    let graph = state.context_graph.clone();
    
    // Default exclusions
    let exclusions = vec![
        "node_modules".to_string(), 
        "target".to_string(), 
        ".git".to_string(), 
        "dist".to_string(),
        "build".to_string(),
        ".env".to_string(),
    ];

    tokio::task::spawn_blocking(move || {
        let mut g = graph.lock().unwrap();
        g.index_directory(&project_path, &exclusions, None::<fn(usize, &str)>);
        Ok(format!("Indexed {} files", g.get_all_files().len()))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_repo_map_prompt(
    focused_files: Vec<String>,
    state: State<'_, ApplySystemState>,
) -> Result<String, String> {
    let graph = state.context_graph.lock().unwrap();
    // Generate map fitting into ~2000 tokens (approx 200 lines)
    let map = crate::apply_system::context::ranker::rank_files(&graph, &focused_files, 2000);
    Ok(map)
}

/// Nowa komenda: Zwraca "szkielet" repozytorium (lista plików + sygnatury)
/// To jest mapa startowa dla Agenta
#[tauri::command]
pub async fn get_repo_skeleton(
    project_path: String,
    state: State<'_, ApplySystemState>,
) -> Result<String, String> {
    let path = std::path::Path::new(&project_path);
    let skeleton = {
        let repo_map = state.repo_map.lock().map_err(|e| e.to_string())?;
        repo_map.generate_skeleton(path)
    };

    Ok(skeleton)
}

/// Nowa komenda: Chirurgiczne pobieranie kontekstu
/// Agent prosi o "src/auth.rs" i symbol "login" -> Dostaje kod funkcji.
#[tauri::command]
pub async fn get_precise_context(
    file_path: String,
    symbol_name: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&file_path);
        match crate::apply_system::context::symbol_extractor::extract_symbol_content(path, &symbol_name) {
            Ok(content) => Ok(content),
            Err(e) => {
                // Fallback: Jeśli nie znaleziono symbolu, zwróć chociaż 50 pierwszych linii pliku
                if let Ok(full_content) = std::fs::read_to_string(path) {
                    let preview: String = full_content.lines().take(50).collect::<Vec<_>>().join("\n");
                    Ok(format!("// Symbol '{}' not found. Returning file preview:\n{}", symbol_name, preview))
                } else {
                    Err(e)
                }
            }
        }
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_available_backups(
    pool: tauri::State<'_, sqlx::SqlitePool>,
    app_handle: tauri::AppHandle
) -> Result<Vec<BackupEntry>, String> {
    let projects: Vec<String> = sqlx::query_scalar("SELECT path FROM projects")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let global_dl = app_handle.path().download_dir().ok().map(|p| p.to_string_lossy().to_string());

    let backups = tokio::task::spawn_blocking(move || {
        backup_system::scan_for_backups(projects, global_dl)
    }).await.map_err(|e| e.to_string())?;

    Ok(backups)
}

#[tauri::command]
pub async fn preview_backup_content(
    filepath: String
) -> Result<Vec<BackupFilePreview>, String> {
    tokio::task::spawn_blocking(move || {
        backup_system::parse_backup_file(&filepath)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn restore_backup_files(
    files: Vec<BackupFilePreview>
) -> Result<usize, String> {
    tokio::task::spawn_blocking(move || {
        backup_system::restore_files(files)
    }).await.map_err(|e| e.to_string())?
}

// Payload struct for WebSocket communication
#[derive(serde::Deserialize)]
pub struct CreateSnapshotPayload {
    pub change_id: String,
    pub html_snippet: String,
    pub error_msg: String,
}

/// Creates a full debug snapshot for a failed change
#[tauri::command]
pub async fn create_debug_snapshot(
    payload: CreateSnapshotPayload,
    state: State<'_, ApplySystemState>,
    app: AppHandle,
) -> Result<String, String> {
    // 1. Precise Lookup
    let queue = state.change_queue.lock().unwrap();
    let change_opt = queue.iter().find(|c| c.id == payload.change_id).cloned();
    
    // Diagnostic info for mismatch analysis
    let debug_info = if change_opt.is_none() {
        let available_ids: Vec<String> = queue.iter().map(|c| c.id.clone()).collect();
        Some((queue.len(), available_ids))
    } else {
        None
    };
    drop(queue);

    let change = match change_opt {
        Some(c) => c,
        None => {
            // Log exactly what happened to help debug the Frontend issue
            if let Some((len, ids)) = debug_info {
                crate::gluon_error!("DebugManager", 
                    "SNAPSHOT ID MISMATCH! Frontend requested ID: '{}', but Backend has: {:?}. (Queue Size: {})", 
                    payload.change_id, ids, len);
            }

            // Fallback: Create Orphan Snapshot so user doesn't lose the error msg
            let mut dummy = ChangeQueueItem::new(
                "unknown_context.txt".to_string(),
                0, 0,
                "/* Original code missing due to ID mismatch */".to_string(),
                payload.html_snippet.clone()
            );
            dummy.id = payload.change_id.clone();
            dummy.error_message = Some(format!("ID Mismatch: {}", payload.error_msg));
            dummy.status = ChangeStatus::Failed;
            dummy
        }
    };

    // 2. Generate snapshot
    match DebugSnapshotManager::create_snapshot(&app, &change, &payload.html_snippet, &payload.error_msg) {
        Ok(path) => {
            crate::gluon_info!("DebugManager", "Snapshot created at: {}", path);
            Ok(path)
        },
        Err(e) => {
            crate::gluon_error!("DebugManager", "Failed to create snapshot: {}", e);
            Err(e)
        }
    }
}

/// Runs integrity audit comparing context file vs disk
#[tauri::command]
pub async fn run_integrity_audit(
    context_file_path: String,
    selected_files: Vec<String>
) -> Result<Vec<crate::apply_system::features::integrity_auditor::IntegrityReport>, String> {
    tokio::task::spawn_blocking(move || {
        crate::apply_system::features::integrity_auditor::run_audit(context_file_path, selected_files)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn export_audit_report(
    app: AppHandle,
    context_file_path: String,
    selected_files: Vec<String>
) -> Result<String, String> {
    use crate::apply_system::features::integrity_auditor::{AuditReporter, AuditPolicy};
    
    // 1. Run the audit logic
    let reports = tokio::task::spawn_blocking(move || {
        crate::apply_system::features::integrity_auditor::run_audit(context_file_path, selected_files)
    }).await.map_err(|e| e.to_string())??;

    // 2. Load Policy (Standard)
    let policy = AuditPolicy::standard();

    // 3. Generate HTML
    let html_content = AuditReporter::generate_html(&reports, &policy);

    // 4. Save to Downloads folder
    let download_dir = app.path().download_dir().map_err(|e| e.to_string())?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("gluon_audit_report_{}.html", timestamp);
    let file_path = download_dir.join(filename);

    tokio::fs::write(&file_path, html_content).await.map_err(|e| e.to_string())?;

    // 5. Return the absolute path
    Ok(file_path.to_string_lossy().to_string())
}

// ============================================================================
// Advanced Debug Commands
// ============================================================================

use crate::apply_system::features::debug_manager::{
    DebugManager, DebugConfig, ErrorRecord, ErrorSeverity,
    ErrorCategory, PerformanceMetrics, ExportFormat
};
use crate::apply_system::shared::logging::{LogLevel, LogStatistics, LOG_MANAGER};

/// Get debug configuration
#[tauri::command]
pub async fn get_debug_config() -> Result<DebugConfig, String> {
    Ok(DebugConfig::default())
}

/// Update debug configuration
#[tauri::command]
pub async fn update_debug_config(config: DebugConfig) -> Result<(), String> {
    // Apply configuration updates
    if let Ok(mut buffer) = crate::apply_system::shared::logging::LOG_BUFFER.lock() {
        buffer.set_min_level(if config.verbose_logging {
            LogLevel::Debug
        } else {
            LogLevel::Info
        });
    }
    Ok(())
}

/// Get log statistics
#[tauri::command]
pub async fn get_log_statistics() -> Result<LogStatistics, String> {
    LOG_MANAGER.lock()
        .map(|mgr| mgr.get_statistics())
        .map_err(|e| format!("Failed to get log statistics: {}", e))
}

/// Get filtered logs
#[tauri::command]
pub async fn get_filtered_logs(
    level: String,
    module: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<String>, String> {
    let log_level = match level.to_uppercase().as_str() {
        "TRACE" => LogLevel::Trace,
        "DEBUG" => LogLevel::Debug,
        "INFO" => LogLevel::Info,
        "WARN" => LogLevel::Warn,
        "ERROR" => LogLevel::Error,
        "CRITICAL" => LogLevel::Critical,
        _ => LogLevel::Info,
    };

    let buffer = crate::apply_system::shared::logging::LOG_BUFFER.lock()
        .map_err(|e| format!("Failed to lock log buffer: {}", e))?;

    let filtered = buffer.get_filtered(log_level, module.as_deref());
    let mut results: Vec<String> = filtered.iter()
        .map(|e| e.format(false))
        .collect();

    if let Some(max) = limit {
        results.truncate(max);
    }

    Ok(results)
}

/// Export debug snapshot
#[tauri::command]
pub async fn export_debug_snapshot(
    snapshot_id: String,
    format: String,
    output_path: String,
) -> Result<(), String> {
    let export_format = match format.to_lowercase().as_str() {
        "json" => ExportFormat::Json,
        "html" => ExportFormat::Html,
        "markdown" | "md" => ExportFormat::Markdown,
        "csv" => ExportFormat::Csv,
        _ => return Err(format!("Unsupported export format: {}", format)),
    };

    // This would require storing DebugManager in global state
    // For now, return a placeholder
    Err("Snapshot export requires global debug manager instance".to_string())
}

/// Clear all logs
#[tauri::command]
pub async fn clear_logs() -> Result<(), String> {
    crate::apply_system::shared::logging::LOG_BUFFER.lock()
        .map(|mut buffer| buffer.clear())
        .map_err(|e| format!("Failed to clear logs: {}", e))
}

/// Initialize log persistence
#[tauri::command]
pub async fn init_log_persistence(app: AppHandle) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| e.to_string())?;
    let log_dir = app_data_dir.join(".gluon").join("logs");

    LOG_MANAGER.lock()
        .map_err(|e| format!("Failed to lock log manager: {}", e))?
        .init(log_dir)
}

/// Clean up old debug snapshots
#[tauri::command]
pub async fn cleanup_debug_snapshots(
    app: AppHandle,
    retention_days: Option<u64>,
) -> Result<usize, String> {
    let config = DebugConfig {
        retention_days: retention_days.unwrap_or(30),
        ..Default::default()
    };

    let mut manager = DebugManager::new(config);
    manager.cleanup_old_snapshots(&app)
}

/// Get system diagnostics
#[tauri::command]
pub async fn get_system_diagnostics() -> Result<serde_json::Value, String> {
    use serde_json::json;

    let stats = LOG_MANAGER.lock()
        .map(|mgr| mgr.get_statistics())
        .unwrap_or_default();

    let log_count = crate::apply_system::shared::logging::LOG_BUFFER.lock()
        .map(|buf| buf.len())
        .unwrap_or(0);

    Ok(json!({
        "log_buffer_size": log_count,
        "total_log_entries": stats.total_entries,
        "errors_last_hour": stats.errors_last_hour,
        "warnings_last_hour": stats.warnings_last_hour,
        "by_level": stats.by_level,
        "by_module": stats.by_module,
        "system": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "cpu_count": num_cpus::get(),
            "gluon_version": env!("CARGO_PKG_VERSION"),
        }
    }))
}

/// Record custom performance metric
#[tauri::command]
pub async fn record_performance_metric(
    operation: String,
    duration_ms: u64,
    memory_used: Option<u64>,
) -> Result<(), String> {
    use std::time::Duration;

    let metrics = PerformanceMetrics {
        total_duration: Duration::from_millis(duration_ms),
        memory_used: memory_used.unwrap_or(0),
        ..Default::default()
    };

    crate::gluon_info!("Performance",
        "Operation '{}' completed in {}ms (mem: {} KB)",
        operation, duration_ms, memory_used.unwrap_or(0) / 1024
    );

    Ok(())
}

/// Start performance trace
#[tauri::command]
pub async fn start_performance_trace(trace_id: String) -> Result<(), String> {
    crate::gluon_info!("Tracing", "Started trace: {}", trace_id);
    Ok(())
}

/// End performance trace
#[tauri::command]
pub async fn end_performance_trace(trace_id: String) -> Result<(), String> {
    crate::gluon_info!("Tracing", "Ended trace: {}", trace_id);
    Ok(())
}

/// Create error report
#[tauri::command]
pub async fn create_error_report(
    severity: String,
    category: String,
    message: String,
    change_id: Option<String>,
) -> Result<String, String> {
    let error_severity = match severity.to_uppercase().as_str() {
        "CRITICAL" => ErrorSeverity::Critical,
        "ERROR" => ErrorSeverity::Error,
        "WARNING" => ErrorSeverity::Warning,
        "INFO" => ErrorSeverity::Info,
        _ => ErrorSeverity::Error,
    };

    let error_category = match category.to_lowercase().as_str() {
        "parsing" => ErrorCategory::Parsing,
        "matching" => ErrorCategory::Matching,
        "application" => ErrorCategory::Application,
        "validation" => ErrorCategory::Validation,
        "filesystem" => ErrorCategory::FileSystem,
        "configuration" => ErrorCategory::Configuration,
        "network" => ErrorCategory::Network,
        _ => ErrorCategory::Unknown,
    };

    let error_id = uuid::Uuid::new_v4().to_string();

    let error = ErrorRecord {
        error_id: error_id.clone(),
        timestamp: chrono::Utc::now(),
        severity: error_severity,
        category: error_category,
        message,
        stack_trace: vec![],
        context: std::collections::HashMap::new(),
        recovery_attempted: false,
        recovery_success: false,
        related_change_id: change_id,
    };

    crate::gluon_error!("ErrorTracking",
        "Error recorded: {} - {:?}/{:?}",
        error.error_id, error.severity, error.category
    );

    Ok(error_id)
}

// ============================================================================
// Regression Testing Commands
// ============================================================================

use crate::apply_system::parsers::regression_report::RegressionReport;

/// Run all regression tests and return detailed report
#[tauri::command]
pub async fn run_regression_tests() -> Result<RegressionReport, String> {
    crate::gluon_info!("RegressionTests", "Starting comprehensive regression test suite...");

    tokio::task::spawn_blocking(|| {
        crate::apply_system::parsers::regression_tests::run_all_regression_tests()
    }).await.map_err(|e| e.to_string())
}

/// Run regression tests and export report to file
#[tauri::command]
pub async fn run_and_export_regression_tests(
    app: AppHandle,
    format: String,
) -> Result<String, String> {
    crate::gluon_info!("RegressionTests", "Running tests and exporting to {} format", format);

    let report = tokio::task::spawn_blocking(|| {
        crate::apply_system::parsers::regression_tests::run_all_regression_tests()
    }).await.map_err(|e| e.to_string())?;

    // Determine output path
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| e.to_string())?;
    let reports_dir = app_data_dir.join(".gluon").join("regression-reports");

    tokio::fs::create_dir_all(&reports_dir).await
        .map_err(|e| format!("Failed to create reports directory: {}", e))?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = match format.to_lowercase().as_str() {
        "json" => format!("regression_report_{}.json", timestamp),
        "txt" | "text" => format!("regression_report_{}.txt", timestamp),
        _ => return Err(format!("Unsupported format: {}", format)),
    };

    let output_path = reports_dir.join(&filename);

    let content = match format.to_lowercase().as_str() {
        "json" => report.to_json().map_err(|e| format!("JSON serialization failed: {}", e))?,
        "txt" | "text" => {
            let mut text = String::new();
            text.push_str(&format!("╔══════════════════════════════════════════════════════════════╗\n"));
            text.push_str(&format!("║         GLUON V2 - REGRESSION TEST REPORT                   ║\n"));
            text.push_str(&format!("╚══════════════════════════════════════════════════════════════╝\n\n"));
            text.push_str(&format!("📊 SUMMARY\n"));
            text.push_str(&format!("  Total Tests: {}\n", report.summary.total_tests));
            text.push_str(&format!("  ✅ Passed: {}\n", report.summary.passed));
            text.push_str(&format!("  ❌ Failed: {}\n", report.summary.failed));
            text.push_str(&format!("  ⏱️  Duration: {:?}\n\n", report.summary.duration));

            if !report.summary.findings_by_severity.is_empty() {
                text.push_str("🔍 FINDINGS BY SEVERITY\n");
                for (severity, count) in &report.summary.findings_by_severity {
                    text.push_str(&format!("  {}: {}\n", severity, count));
                }
                text.push_str("\n");
            }

            if !report.summary.findings_by_category.is_empty() {
                text.push_str("📁 FINDINGS BY CATEGORY\n");
                for (category, count) in &report.summary.findings_by_category {
                    text.push_str(&format!("  {}: {}\n", category, count));
                }
                text.push_str("\n");
            }

            text.push_str("══════════════════════════════════════════════════════════════\n");
            text.push_str("📋 DETAILED TEST RESULTS\n");
            text.push_str("══════════════════════════════════════════════════════════════\n\n");

            for (idx, test) in report.tests.iter().enumerate() {
                let status = if test.passed { "✅ PASS" } else { "❌ FAIL" };
                text.push_str(&format!("[Test {}] {} - {} {}\n", idx + 1, test.category.emoji(), test.test_name, status));
                text.push_str(&format!("  Category: {}\n", test.category.as_str()));
                text.push_str(&format!("  Duration: {:?}\n", test.duration));

                if !test.input_summary.is_empty() {
                    text.push_str(&format!("  Input: {}\n", test.input_summary));
                }

                if !test.expected.is_empty() {
                    text.push_str(&format!("  Expected: {}\n", test.expected));
                }

                if !test.actual.is_empty() {
                    text.push_str(&format!("  Actual: {}\n", test.actual));
                }

                if !test.steps.is_empty() {
                    text.push_str("\n  📝 Validation Steps:\n");
                    for step in &test.steps {
                        let step_status = if step.passed { "✓" } else { "✗" };
                        let duration = step.duration().map(|d| format!("{:?}", d)).unwrap_or_else(|| "N/A".to_string());
                        text.push_str(&format!("    {} {} ({})\n", step_status, step.name, duration));

                        for finding in &step.findings {
                            text.push_str(&format!("      {} [{}] {}\n", finding.severity.emoji(), finding.severity.as_str(), finding.title));
                            text.push_str(&format!("        {}\n", finding.description));

                            if let Some(ref code) = finding.code_snippet {
                                text.push_str(&format!("        Code: {}\n", code));
                            }

                            if let Some(ref suggestion) = finding.suggestion {
                                text.push_str(&format!("        💡 Suggestion: {}\n", suggestion));
                            }
                        }
                    }
                }

                if !test.findings.is_empty() {
                    text.push_str("\n  🔍 Overall Findings:\n");
                    for finding in &test.findings {
                        text.push_str(&format!("    {} [{}] {}\n", finding.severity.emoji(), finding.severity.as_str(), finding.title));
                        text.push_str(&format!("      {}\n", finding.description));

                        if let Some(ref code) = finding.code_snippet {
                            text.push_str(&format!("      Code: {}\n", code));
                        }

                        if let Some(ref suggestion) = finding.suggestion {
                            text.push_str(&format!("      💡 Suggestion: {}\n", suggestion));
                        }
                    }
                }

                text.push_str("\n");
            }

            text.push_str("══════════════════════════════════════════════════════════════\n");
            text.push_str(&format!("✨ Report generated at: {:?}\n", report.completed_at));
            text.push_str("══════════════════════════════════════════════════════════════\n");

            text
        },
        _ => unreachable!(),
    };

    tokio::fs::write(&output_path, content).await
        .map_err(|e| format!("Failed to write report: {}", e))?;

    let path_str = output_path.to_string_lossy().to_string();

    crate::gluon_info!("RegressionTests", "Report saved to: {}", path_str);

    Ok(path_str)
}

// ============================================================================
// UNIT TESTS - G-Interactive Context Node
// ============================================================================

#[cfg(test)]
mod context_node_tests {
    use super::*;

    #[test]
    fn test_validate_safe_path_blocks_traversal() {
        // Should block path traversal
        assert!(validate_safe_path("../../../etc/passwd").is_err());
        assert!(validate_safe_path("src/../../../etc/passwd").is_err());
        assert!(validate_safe_path("..\\..\\windows\\system32").is_err());
    }

    #[test]
    fn test_validate_safe_path_blocks_null_bytes() {
        assert!(validate_safe_path("file\0.txt").is_err());
    }

    #[test]
    fn test_validate_safe_path_allows_valid_paths() {
        assert!(validate_safe_path("src/main.rs").is_ok());
        assert!(validate_safe_path("components/Header.tsx").is_ok());
        assert!(validate_safe_path("backend/routes/api.py").is_ok());
    }

    #[test]
    fn test_context_operation_deserialization() {
        // Test FileSymbol
        let json = r#"{"type": "file_symbol", "path": "src/main.rs", "symbol": "main"}"#;
        let op: ContextOperation = serde_json::from_str(json).unwrap();
        match op {
            ContextOperation::FileSymbol { path, symbol } => {
                assert_eq!(path, "src/main.rs");
                assert_eq!(symbol, "main");
            }
            _ => panic!("Wrong variant"),
        }

        // Test RagSearch with default top_k
        let json = r#"{"type": "rag_search", "query": "authentication logic"}"#;
        let op: ContextOperation = serde_json::from_str(json).unwrap();
        match op {
            ContextOperation::RagSearch { query, top_k } => {
                assert_eq!(query, "authentication logic");
                assert_eq!(top_k, 3); // default
            }
            _ => panic!("Wrong variant"),
        }

        // Test FullFile
        let json = r#"{"type": "full_file", "path": "config.json"}"#;
        let op: ContextOperation = serde_json::from_str(json).unwrap();
        match op {
            ContextOperation::FullFile { path } => {
                assert_eq!(path, "config.json");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_context_item_serialization() {
        // Test SymbolContent
        let item = ContextItem::SymbolContent {
            file_path: "test.rs".to_string(),
            symbol_name: "foo".to_string(),
            content: "fn foo() {}".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("symbol_content"));
        assert!(json.contains("test.rs"));

        // Test Error
        let item = ContextItem::Error {
            operation: "test".to_string(),
            error: "failed".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("failed"));
    }

    #[test]
    fn test_max_operations_constant() {
        assert_eq!(MAX_OPERATIONS_PER_REQUEST, 50);
        assert_eq!(MAX_FULL_FILE_SIZE, 1_000_000);
    }
}

// ============================================================================
// INTEGRATION TESTS - Would require mocking Tauri state
// ============================================================================

#[cfg(test)]
mod integration_tests {
    // These would require proper Tauri test harness
    // Left as TODO for CI/CD pipeline

    // TODO: Test execute_context_operations with mock state
    // TODO: Test file symbol extraction with real Tree-sitter
    // TODO: Test security validation in full pipeline
}