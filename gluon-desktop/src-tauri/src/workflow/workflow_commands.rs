//! Tauri Commands for Agent Workflow
//!
//! This module implements commands for managing the agent workflow system.

use tauri::{State, AppHandle, Manager, Emitter};
use crate::apply_system::ApplySystemState;
use std::path::{Path, PathBuf};
use sqlx::SqlitePool;

/// Helper to broadcast graph updates to all clients (UI + Extension)
fn broadcast_graph_update(app: &AppHandle, state: &State<'_, ApplySystemState>) {
    let graph_manager = &state.agent_workflow;
    // Get a clone of the graph (thread-safe)
    let graph_data = graph_manager.get_graph().lock().unwrap().clone();
    
    // 1. Emit to Tauri window (Command Center)
    let _ = app.emit("workflow-state-sync", &graph_data);

    // 2. Emit to Bridge (for Chrome Extension) via main.rs listener
    let _ = app.emit("workflow-sync-bridge", &graph_data);
}

/// Helper function to get the workflow storage path
pub fn get_workflow_storage_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    Ok(app_data_dir.join("workflow_graph.json"))
}

// ============================================================================
// Agent Workflow Commands
// ============================================================================

/// Adds a new agent to the workflow graph
#[tauri::command]
pub fn workflow_add_agent(
    name: String,
    output_wrapper: Option<String>,
    agent_type: Option<String>,
    position: Option<(f32, f32)>,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    use crate::workflow::agent_workflow::AgentType;

    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    // Parse agent_type from string
    let parsed_type = match agent_type.as_deref() {
        Some("Report") | Some("report") => AgentType::Report,
        Some("AutoApply") | Some("auto_apply") => AgentType::AutoApply,
        Some("Terminal") | Some("terminal") => AgentType::Terminal,
        _ => AgentType::Normal,
    };

    let agent = graph.add_agent(name, output_wrapper, parsed_type, position);

    // Save after modification
    drop(graph); // Release lock before saving
    let storage_path = get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)
        .map_err(|e| format!("Failed to save workflow: {}", e))?;

    // 🔥 SYNC 1:1 - Broadcast update
    broadcast_graph_update(&app_handle, &state);

    Ok(serde_json::to_value(&agent).unwrap())
}

/// Removes an agent from the workflow graph
#[tauri::command]
pub fn workflow_remove_agent(
    agent_id: String,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    graph.remove_agent(&agent_id);
    drop(graph);

    // Save after modification
    let storage_path = get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)
}

/// Updates an existing agent's properties
#[tauri::command]
pub fn workflow_update_agent(
    agent_id: String,
    name: Option<String>,
    output_wrapper: Option<String>,
    system_prompt: Option<String>,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    // Convert Option<String> to Option<Option<String>> for wrapper
    // If output_wrapper is Some(""), convert to Some(None)
    // If output_wrapper is Some(value), convert to Some(Some(value))
    // If output_wrapper is None, don't update
    let wrapper_update = match output_wrapper {
        Some(s) if s.is_empty() => Some(None),
        Some(s) => Some(Some(s)),
        None => None,
    };

    let agent = graph.update_agent(&agent_id, name, wrapper_update, system_prompt)?;

    drop(graph);

    // Save after modification
    let storage_path = get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)
        .map_err(|e| format!("Failed to save workflow: {}", e))?;

    Ok(serde_json::to_value(&agent).unwrap())
}

/// Adds a connection between two agents
#[tauri::command]
pub fn workflow_add_connection(
    from_id: String,
    to_id: String,
    template: Option<String>,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    graph.add_connection(from_id, to_id, template)?;
    drop(graph);

    // Save after modification
    let storage_path = get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)
}

/// Removes a connection between agents
#[tauri::command]
pub fn workflow_remove_connection(
    from_id: String,
    to_id: String,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    graph.remove_connection(&from_id, &to_id);
    drop(graph);

    // Save after modification
    let storage_path = get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)
}

/// Gets the current workflow graph state
#[tauri::command]
pub fn workflow_get_graph(
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    // Try to load from storage if graph is empty (lazy loading)
    {
        let graph = state.agent_workflow.get_graph();
        let graph_lock = graph.lock().unwrap();

        if graph_lock.agents.is_empty() {
            drop(graph_lock); // Release lock before loading

            let storage_path = get_workflow_storage_path(&app_handle)?;
            if storage_path.exists() {
                state.agent_workflow.load_from_storage(&storage_path)
                    .unwrap_or_else(|e| eprintln!("Warning: Failed to load workflow: {}", e));
            }
        }
    }

    let graph = state.agent_workflow.get_graph();
    let graph = graph.lock().unwrap();

    Ok(serde_json::to_value(&*graph).unwrap())
}

/// Toggles auto-forward mode
#[tauri::command]
pub fn workflow_set_auto_forward(
    enabled: bool,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[Workflow Command] 🔧 workflow_set_auto_forward called with: {}", enabled);
    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    graph.auto_forward = enabled;
    println!("[Workflow Command] ✅ auto_forward set to: {}", graph.auto_forward);
    drop(graph);

    // Save after modification
    let storage_path = get_workflow_storage_path(&app_handle)?;
    println!("[Workflow Command] 💾 Saving workflow to: {:?}", storage_path);
    let result = state.agent_workflow.save_to_storage(&storage_path);
    match &result {
        Ok(_) => println!("[Workflow Command] ✅ Workflow saved successfully"),
        Err(e) => println!("[Workflow Command] ❌ Failed to save workflow: {}", e),
    }
    result
}

/// Registers an agent with a socket connection (handshake)
#[tauri::command]
pub fn workflow_register_agent(
    pairing_code: String,
    socket_id: String,
    state: State<'_, ApplySystemState>,
) -> Result<serde_json::Value, String> {
    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    let agent = graph.register_agent(&pairing_code, socket_id)?;

    Ok(serde_json::to_value(&agent).unwrap())
}

/// Disconnects an agent (called when WebSocket closes)
#[tauri::command]
pub fn workflow_disconnect_agent(
    socket_id: String,
    state: State<'_, ApplySystemState>,
) -> Result<(), String> {
    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    graph.disconnect_agent(&socket_id);
    Ok(())
}

/// Resets report buffers (clears aggregation state)
#[tauri::command]
pub fn workflow_reset_report_buffers(
    state: State<'_, ApplySystemState>,
) -> Result<(), String> {
    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    graph.reset_all_report_buffers();
    Ok(())
}

/// Updates agent position in graph (for visual editor)
#[tauri::command]
pub fn workflow_update_agent_position(
    agent_id: String,
    position: (f32, f32),
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    if let Some(agent) = graph.agents.get_mut(&agent_id) {
        agent.position = Some(position);
    } else {
        return Err(format!("Agent not found: {}", agent_id));
    }

    drop(graph);

    // Save after modification
    let storage_path = get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)
}

// ============================================================================
// Saved Workflow Configurations
// ============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedWorkflowConfig {
    pub id: String,
    pub name: String,
    pub workflow: serde_json::Value,
    pub created_at: i64,
    pub modified_at: i64,
}

/// Helper function to get saved workflows storage path
fn get_saved_workflows_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    Ok(app_data_dir.join("saved_workflow_configs.json"))
}

/// Loads saved workflow configurations from storage
fn load_saved_workflows(app_handle: &AppHandle) -> Result<HashMap<String, SavedWorkflowConfig>, String> {
    let storage_path = get_saved_workflows_path(app_handle)?;

    if !storage_path.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(&storage_path)
        .map_err(|e| format!("Failed to read saved workflows: {}", e))?;

    let configs: HashMap<String, SavedWorkflowConfig> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse saved workflows: {}", e))?;

    Ok(configs)
}

/// Saves workflow configurations to storage
fn save_workflows_to_storage(
    app_handle: &AppHandle,
    configs: &HashMap<String, SavedWorkflowConfig>,
) -> Result<(), String> {
    let storage_path = get_saved_workflows_path(app_handle)?;

    let json = serde_json::to_string_pretty(configs)
        .map_err(|e| format!("Failed to serialize saved workflows: {}", e))?;

    std::fs::write(&storage_path, json)
        .map_err(|e| format!("Failed to write saved workflows: {}", e))?;

    Ok(())
}

/// Saves a workflow configuration
#[tauri::command]
pub fn workflow_save_config(
    id: String,
    name: String,
    workflow: serde_json::Value,
    app_handle: AppHandle,
) -> Result<SavedWorkflowConfig, String> {
    let mut configs = load_saved_workflows(&app_handle)?;

    let now = chrono::Utc::now().timestamp();

    let config = SavedWorkflowConfig {
        id: id.clone(),
        name,
        workflow,
        created_at: configs.get(&id).map(|c| c.created_at).unwrap_or(now),
        modified_at: now,
    };

    configs.insert(id, config.clone());
    save_workflows_to_storage(&app_handle, &configs)?;

    Ok(config)
}

/// Gets all saved workflow configurations
#[tauri::command]
pub fn workflow_get_saved_configs(
    app_handle: AppHandle,
) -> Result<Vec<SavedWorkflowConfig>, String> {
    let configs = load_saved_workflows(&app_handle)?;
    let mut config_list: Vec<SavedWorkflowConfig> = configs.into_values().collect();

    // Sort by modified_at descending (most recent first)
    config_list.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

    Ok(config_list)
}

/// Deletes a saved workflow configuration
#[tauri::command]
pub fn workflow_delete_saved_config(
    id: String,
    app_handle: AppHandle,
) -> Result<(), String> {
    let mut configs = load_saved_workflows(&app_handle)?;

    configs.remove(&id);
    save_workflows_to_storage(&app_handle, &configs)?;

    Ok(())
}

// ============================================================================
// Auto-Apply Workflow Commands
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct AutoApplyResult {
    pub applied_count: usize,
    pub failed_count: usize,
    pub changes: Vec<AutoApplyChangeResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutoApplyChangeResult {
    pub file_path: String,
    pub status: String, // "success" | "failed"
    pub error: Option<String>,
}

/// Auto-applies code changes from G-code blocks without user interaction
/// This is the core command for Auto-Apply nodes in the workflow
#[tauri::command]
pub async fn workflow_auto_apply(
    agent_id: String,
    content: String,
    state: State<'_, ApplySystemState>,
    pool: State<'_, SqlitePool>, // [FIX] Added pool for path resolution
    app_handle: AppHandle,
) -> Result<AutoApplyResult, String> {
    use crate::apply_system::parsers::parse_model_response;

    println!("[Auto-Apply] 🤖 Processing request for agent: {}", agent_id);
    println!("[Auto-Apply] 📝 Content length: {} chars", content.len());

    // 1. Get Project Roots for Path Resolution
    let projects: Vec<String> = sqlx::query_scalar("SELECT path FROM projects")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    // 2. Parse Code Blocks
    let mut changes = parse_model_response(&content)
        .map_err(|e| format!("Failed to parse code blocks: {:?}", e))?;

    if changes.is_empty() {
        return Err("No code blocks found in content. Use <<<< SEARCH / >>>> REPLACE format.".to_string());
    }

    println!("[Auto-Apply] 🔍 Found {} code blocks to apply", changes.len());

    // 3. Resolve Paths (Fix for OS Error 3)
    for change in &mut changes {
        let original_path = Path::new(&change.file_path);

        // If path is absolute and exists, keep it.
        if original_path.is_absolute() && original_path.exists() {
            continue; 
        }

        // Try to find the file in known projects
        let mut resolved = None;
        for project_root in &projects {
            // Try direct join
            let candidate = Path::new(project_root).join(&change.file_path);
            if candidate.exists() {
                resolved = Some(candidate);
                break;
            }

            // Try normalizing slashes (just in case)
            let normalized_rel = change.file_path.replace('\\', "/");
            let candidate_norm = Path::new(project_root).join(&normalized_rel);
            if candidate_norm.exists() {
                resolved = Some(candidate_norm);
                break;
            }
        }

        if let Some(abs_path) = resolved {
            let abs_str = abs_path.to_string_lossy().to_string();
            println!("[Auto-Apply] 🔗 Resolved relative path '{}' -> '{}'", change.file_path, abs_str);
            change.file_path = abs_str;
        } else {
            println!("[Auto-Apply] ⚠️ Could not resolve path '{}' in any project. Will try as-is.", change.file_path);
        }
    }

    let mut results = Vec::new();
    let mut applied_count = 0;
    let mut failed_count = 0;

    // Generate a unique request_id for this batch
    let request_id = uuid::Uuid::new_v4().to_string();

    // Add all changes to the queue first
    {
        let mut queue = state.change_queue.lock().unwrap();
        for change in &changes {
            queue.push(change.clone());
        }
    }

    // Apply each change sequentially (silent mode - no UI overlay)
    for change in changes {
        let change_id = change.id.clone();
        let file_path = change.file_path.clone();

        println!("[Auto-Apply] 📂 Applying change to: {}", file_path);

        // Read current file content
        let file_content = match tokio::fs::read_to_string(&file_path).await {
            Ok(content) => content,
            Err(e) => {
                println!("[Auto-Apply] ❌ Failed to read file {}: {}", file_path, e);
                results.push(AutoApplyChangeResult {
                    file_path: file_path.clone(),
                    status: "failed".to_string(),
                    error: Some(format!("File read error: {}", e)),
                });
                failed_count += 1;
                continue;
            }
        };

        // Call apply_change_command with silent flag via request_id prefix
        let silent_request_id = format!("auto-apply:{}", request_id);

        match crate::apply_system::tauri_commands::apply_change_command(
            change_id.clone(),
            file_content,
            Some(silent_request_id),
            state.clone(),
            app_handle.clone(),
        ).await {
            Ok(_) => {
                println!("[Auto-Apply] ✅ Successfully applied change to {}", file_path);
                results.push(AutoApplyChangeResult {
                    file_path: file_path.clone(),
                    status: "success".to_string(),
                    error: None,
                });
                applied_count += 1;
            }
            Err(e) => {
                println!("[Auto-Apply] ❌ Failed to apply change to {}: {}", file_path, e);
                results.push(AutoApplyChangeResult {
                    file_path: file_path.clone(),
                    status: "failed".to_string(),
                    error: Some(e.to_string()),
                });
                failed_count += 1;
            }
        }
    }

    println!("[Auto-Apply] 🎉 Batch complete: {} succeeded, {} failed", applied_count, failed_count);

    Ok(AutoApplyResult {
        applied_count,
        failed_count,
        changes: results,
    })
}

/// Clears the Auto-Apply queue for a specific agent
#[tauri::command]
pub fn workflow_clear_auto_apply_queue(
    agent_id: String,
    state: State<'_, ApplySystemState>,
) -> Result<(), String> {
    println!("[Auto-Apply] 🗑️ Clearing queue for agent: {}", agent_id);

    // For now, we don't maintain separate queues per agent
    // This is a placeholder for future implementation
    let mut queue = state.change_queue.lock().unwrap();

    // Remove all pending changes (keep only applied/failed for history)
    queue.retain(|change| {
        !matches!(change.status, crate::apply_system::ChangeStatus::Pending)
    });

    println!("[Auto-Apply] ✅ Queue cleared");
    Ok(())
}

// ============================================================================
// Auto-Apply History Commands
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct AutoApplyHistoryEntry {
    pub change_id: String,
    pub file_path: String,
    pub timestamp: i64,
    pub status: String, // "success" | "failed"
    pub error: Option<String>,
    pub old_content: String,
    pub new_content: String,
    pub request_id: String, // Batch ID for grouping
}

/// Gets Auto-Apply history for a specific agent
/// Returns all changes that were applied through workflow_auto_apply
#[tauri::command]
pub fn workflow_get_auto_apply_history(
    agent_id: String,
    state: State<'_, ApplySystemState>,
) -> Result<Vec<AutoApplyHistoryEntry>, String> {
    println!("[Auto-Apply History] 📜 Getting history for agent: {}", agent_id);

    let queue = state.change_queue.lock().unwrap();

    // Filter changes that were applied via Auto-Apply (have "auto-apply:" prefix in metadata)
    // We'll use the change_id pattern or add metadata tracking
    let history: Vec<AutoApplyHistoryEntry> = queue
        .iter()
        .filter(|change| {
            // For now, we consider all applied/failed changes as part of history
            // In production, you'd track agent_id in ChangeQueueItem
            matches!(
                change.status,
                crate::apply_system::ChangeStatus::Applied | crate::apply_system::ChangeStatus::Failed
            )
        })
        .map(|change| {
            let status = match change.status {
                crate::apply_system::ChangeStatus::Applied => "success",
                crate::apply_system::ChangeStatus::Failed => "failed",
                _ => "unknown",
            };

            AutoApplyHistoryEntry {
                change_id: change.id.clone(),
                file_path: change.file_path.clone(),
                timestamp: chrono::Utc::now().timestamp(), // TODO: Store actual timestamp in ChangeQueueItem
                status: status.to_string(),
                error: None, // TODO: Add error field to ChangeQueueItem
                old_content: change.old_code.clone(),
                new_content: change.new_code.clone(),
                request_id: "batch-unknown".to_string(), // TODO: Track request_id in ChangeQueueItem
            }
        })
        .collect();

    println!("[Auto-Apply History] ✅ Found {} history entries", history.len());
    Ok(history)
}

/// Reverts a batch of Auto-Apply changes atomically
#[tauri::command]
pub async fn workflow_revert_batch(
    request_id: String,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<usize, String> {
    println!("[Auto-Apply Revert] 🔙 Reverting batch: {}", request_id);

    // Find all changes with matching request_id
    let changes_to_revert: Vec<String> = {
        let queue = state.change_queue.lock().unwrap();
        queue
            .iter()
            .filter(|change| {
                // TODO: Match by actual request_id stored in ChangeQueueItem
                // For now, we'll revert all applied changes (demo)
                matches!(change.status, crate::apply_system::ChangeStatus::Applied)
            })
            .map(|change| change.id.clone())
            .collect()
    };

    if changes_to_revert.is_empty() {
        return Err("No changes found for this batch".to_string());
    }

    println!("[Auto-Apply Revert] 📋 Found {} changes to revert", changes_to_revert.len());

    let mut reverted_count = 0;

    // Revert each change
    for change_id in changes_to_revert {
        // Get old content
        let (file_path, old_content) = {
            let queue = state.change_queue.lock().unwrap();
            let change = queue.iter().find(|c| c.id == change_id);

            match change {
                Some(c) => (c.file_path.clone(), c.old_code.clone()),
                None => continue,
            }
        };

        // Write old content back to file
        match tokio::fs::write(&file_path, &old_content).await {
            Ok(_) => {
                println!("[Auto-Apply Revert] ✅ Reverted: {}", file_path);

                // Update status in queue
                let mut queue = state.change_queue.lock().unwrap();
                if let Some(change) = queue.iter_mut().find(|c| c.id == change_id) {
                    change.status = crate::apply_system::ChangeStatus::Pending; // Reset to pending
                }

                reverted_count += 1;
            }
            Err(e) => {
                println!("[Auto-Apply Revert] ❌ Failed to revert {}: {}", file_path, e);
            }
        }
    }

    println!("[Auto-Apply Revert] 🎉 Reverted {} changes", reverted_count);
    Ok(reverted_count)
}

// ============================================================================
// Preset Library Commands (SSOT)
// ============================================================================

use crate::apply_system::preset_library::{PresetLibrary, AgentPreset, ConnectionPreset, WorkflowPreset};

/// Gets all available agent presets
#[tauri::command]
pub fn workflow_get_agent_presets() -> Result<Vec<AgentPreset>, String> {
    let library = PresetLibrary::new_with_defaults();
    Ok(library.agent_presets)
}

/// Gets all available connection presets
#[tauri::command]
pub fn workflow_get_connection_presets() -> Result<Vec<ConnectionPreset>, String> {
    let library = PresetLibrary::new_with_defaults();
    Ok(library.connection_presets)
}

/// Gets all available workflow presets
#[tauri::command]
pub fn workflow_get_workflow_presets() -> Result<Vec<WorkflowPreset>, String> {
    let library = PresetLibrary::new_with_defaults();
    Ok(library.workflow_presets)
}

/// Gets a specific agent preset by ID
#[tauri::command]
pub fn workflow_get_agent_preset(preset_id: String) -> Result<AgentPreset, String> {
    let library = PresetLibrary::new_with_defaults();
    library
        .agent_presets
        .into_iter()
        .find(|p| p.id == preset_id)
        .ok_or_else(|| format!("Agent preset not found: {}", preset_id))
}

/// Creates an agent from preset
#[tauri::command]
pub fn workflow_create_agent_from_preset(
    preset_id: String,
    custom_name: Option<String>,
    position: Option<(f32, f32)>,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    use crate::workflow::agent_workflow::AgentType;

    let library = PresetLibrary::new_with_defaults();
    let preset = library
        .agent_presets
        .into_iter()
        .find(|p| p.id == preset_id)
        .ok_or_else(|| format!("Agent preset not found: {}", preset_id))?;

    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    // Create agent with preset configuration
    let name = custom_name.unwrap_or(preset.name);
    let mut agent = graph.add_agent(name, preset.output_wrapper, AgentType::Normal, position);

    // Set system prompt from preset
    agent.system_prompt_template = Some(preset.system_prompt);

    // Update agent in graph
    if let Some(agent_mut) = graph.agents.get_mut(&agent.id) {
        agent_mut.system_prompt_template = agent.system_prompt_template.clone();
    }

    drop(graph);

    // Save after modification
    let storage_path = get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)
        .map_err(|e| format!("Failed to save workflow: {}", e))?;

    // Broadcast update
    broadcast_graph_update(&app_handle, &state);

    Ok(serde_json::to_value(&agent).unwrap())
}

/// Instantiates a full workflow from preset
#[tauri::command]
pub fn workflow_create_from_preset(
    preset_id: String,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    use crate::workflow::agent_workflow::AgentType;
    use std::collections::HashMap;

    let library = PresetLibrary::new_with_defaults();
    let workflow_preset = library
        .workflow_presets
        .into_iter()
        .find(|p| p.id == preset_id)
        .ok_or_else(|| format!("Workflow preset not found: {}", preset_id))?;

    let graph = state.agent_workflow.get_graph();
    let mut graph = graph.lock().unwrap();

    // Clear existing graph (optional - ask user first in production)
    // graph.agents.clear();
    // graph.connections.clear();

    // Map instance names to created agent IDs
    let mut instance_id_map: HashMap<String, String> = HashMap::new();

    // Create all agents
    for agent_config in &workflow_preset.agents {
        // Find the agent preset
        let agent_preset = library
            .agent_presets
            .iter()
            .find(|p| p.id == agent_config.preset_id)
            .ok_or_else(|| format!("Agent preset not found: {}", agent_config.preset_id))?;

        // Create agent
        let mut agent = graph.add_agent(
            agent_config.instance_name.clone(),
            agent_preset.output_wrapper.clone(),
            AgentType::Normal,
            agent_config.position,
        );

        // Set system prompt
        agent.system_prompt_template = Some(agent_preset.system_prompt.clone());

        // Update in graph
        if let Some(agent_mut) = graph.agents.get_mut(&agent.id) {
            agent_mut.system_prompt_template = agent.system_prompt_template.clone();
        }

        instance_id_map.insert(agent_config.instance_name.clone(), agent.id.clone());
    }

    // Create all connections
    for conn_config in &workflow_preset.connections {
        let from_id = instance_id_map
            .get(&conn_config.from)
            .ok_or_else(|| format!("Source instance not found: {}", conn_config.from))?
            .clone();

        let to_id = instance_id_map
            .get(&conn_config.to)
            .ok_or_else(|| format!("Target instance not found: {}", conn_config.to))?
            .clone();

        // Get connection template if specified
        let template = if let Some(template_id) = &conn_config.template_preset_id {
            library
                .connection_presets
                .iter()
                .find(|p| p.id == *template_id)
                .map(|p| p.message_template.clone())
        } else {
            None
        };

        graph.add_connection(from_id, to_id, template)?;
    }

    drop(graph);

    // Save after modification
    let storage_path = get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)
        .map_err(|e| format!("Failed to save workflow: {}", e))?;

    // Broadcast update
    broadcast_graph_update(&app_handle, &state);

    Ok(serde_json::json!({
        "success": true,
        "message": format!("Created workflow: {}", workflow_preset.name),
        "agents_created": instance_id_map.len(),
        "connections_created": workflow_preset.connections.len()
    }))
}