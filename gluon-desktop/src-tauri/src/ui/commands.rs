// Tauri Commands for Command Center UI
// These are the RPC endpoints that the frontend can call to control
// the execution engine, terminal, and get state updates.

use crate::ui::{VirtualTerminal, UIStateBroadcaster, ExecutionState, EventBus};
use crate::ui::events::UIEvent;
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Shared state for UI subsystem (managed by Tauri)
pub struct UIState {
    pub event_bus: Arc<EventBus>,
    pub terminal: Arc<VirtualTerminal>,
    pub broadcaster: Arc<UIStateBroadcaster>,
    pub paused: Arc<RwLock<bool>>,
}

// SAFETY: All fields are thread-safe (Arc<T> where T: Send + Sync)
unsafe impl Send for UIState {}
unsafe impl Sync for UIState {}

impl UIState {
    pub fn new() -> Self {
        let event_bus = Arc::new(EventBus::new());
        let terminal = Arc::new(VirtualTerminal::new(Arc::clone(&event_bus)));
        let broadcaster = Arc::new(UIStateBroadcaster::new(Arc::clone(&event_bus)));

        // Start the broadcaster
        broadcaster.start();

        Self {
            event_bus,
            terminal,
            broadcaster,
            paused: Arc::new(RwLock::new(false)),
        }
    }
}

// ============================================================================
// EXECUTION CONTROL COMMANDS
// ============================================================================

/// Pause the execution engine (God Mode)
#[tauri::command]
pub async fn ui_pause_execution(state: State<'_, UIState>) -> Result<(), String> {
    *state.paused.write() = true;

    state.event_bus.publish(UIEvent::ExecutionPaused {
        reason: "User requested pause".to_string(),
        timestamp: current_timestamp(),
    });

    state.broadcaster.update_execution_status(
        crate::ui::state_sync::ExecutionStatus::Paused
    );

    Ok(())
}

/// Resume the execution engine
#[tauri::command]
pub async fn ui_resume_execution(state: State<'_, UIState>) -> Result<(), String> {
    *state.paused.write() = false;

    state.event_bus.publish(UIEvent::ExecutionResumed {
        timestamp: current_timestamp(),
    });

    state.broadcaster.update_execution_status(
        crate::ui::state_sync::ExecutionStatus::Running
    );

    Ok(())
}

/// Check if execution is paused
#[tauri::command]
pub async fn ui_is_paused(state: State<'_, UIState>) -> Result<bool, String> {
    Ok(*state.paused.read())
}

/// Get current execution state snapshot
#[tauri::command]
pub async fn ui_get_execution_state(state: State<'_, UIState>) -> Result<ExecutionState, String> {
    Ok(state.broadcaster.get_state())
}

/// Get current graph state (for graph visualization)
#[tauri::command]
pub async fn ui_get_graph_state(state: State<'_, UIState>) -> Result<crate::ui::state_sync::GraphSnapshot, String> {
    Ok(state.broadcaster.get_state().graph)
}

// ============================================================================
// TERMINAL COMMANDS
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct CreateTerminalRequest {
    pub working_dir: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateTerminalResponse {
    pub session_id: String,
}

/// Create a new terminal session
#[tauri::command]
pub async fn ui_create_terminal(
    request: CreateTerminalRequest,
    state: State<'_, UIState>,
) -> Result<CreateTerminalResponse, String> {
    let session_id = state
        .terminal
        .create_session(request.working_dir)
        .map_err(|e| e.to_string())?;

    Ok(CreateTerminalResponse { session_id })
}

/// Write data to terminal (user input)
#[tauri::command]
pub async fn ui_terminal_write(
    session_id: String,
    data: Vec<u8>,
    state: State<'_, UIState>,
) -> Result<(), String> {
    state
        .terminal
        .write_to_session(&session_id, &data)
        .map_err(|e| e.to_string())
}

/// Execute a command in terminal (agent injection)
#[tauri::command]
pub async fn ui_terminal_execute(
    session_id: String,
    command: String,
    state: State<'_, UIState>,
) -> Result<(), String> {
    state
        .terminal
        .execute_in_session(&session_id, &command)
        .map_err(|e| e.to_string())
}

/// Send Ctrl+C to terminal
#[tauri::command]
pub async fn ui_terminal_interrupt(
    session_id: String,
    state: State<'_, UIState>,
) -> Result<(), String> {
    state
        .terminal
        .interrupt_session(&session_id)
        .map_err(|e| e.to_string())
}

/// Resize terminal
#[tauri::command]
pub async fn ui_terminal_resize(
    session_id: String,
    rows: u16,
    cols: u16,
    state: State<'_, UIState>,
) -> Result<(), String> {
    state
        .terminal
        .resize_session(&session_id, rows, cols)
        .map_err(|e| e.to_string())
}

/// Close terminal session
#[tauri::command]
pub async fn ui_terminal_close(
    session_id: String,
    state: State<'_, UIState>,
) -> Result<(), String> {
    state
        .terminal
        .close_session(&session_id)
        .map_err(|e| e.to_string())
}

/// List all active terminal sessions
#[tauri::command]
pub async fn ui_terminal_list(state: State<'_, UIState>) -> Result<Vec<String>, String> {
    Ok(state.terminal.list_sessions())
}

// ============================================================================
// STATE INSPECTION COMMANDS (God Mode - Blackboard Editor)
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct InjectContextRequest {
    pub key: String,
    pub value: serde_json::Value,
}

/// Hot-swap: Inject or modify a variable in the Blackboard
/// This is the "edit reality" feature - change agent's context mid-flight
#[tauri::command]
pub async fn ui_inject_context(
    request: InjectContextRequest,
    state: State<'_, UIState>,
) -> Result<(), String> {
    // Inject variable into UI snapshot (MVP implementation)
    // NOTE: This modifies the UI snapshot only. Full integration with
    // the execution engine's Blackboard will be completed in v3.1
    state.broadcaster.inject_variable(request.key.clone(), request.value.clone())?;

    // Publish success event for logging
    state.event_bus.publish(UIEvent::SystemLog {
        level: crate::ui::events::LogLevel::Info,
        source: "god_mode".to_string(),
        message: format!("✅ Injected context: {} = {:?}", request.key, request.value),
        metadata: serde_json::json!({
            "key": request.key,
            "value": request.value,
        }),
        timestamp: current_timestamp(),
    });

    Ok(())
}

// ============================================================================
// EVENT SUBSCRIPTION COMMANDS
// ============================================================================

/// Subscribe to UI events (for frontend to receive real-time updates)
/// NOTE: Tauri has built-in event system, but we're using our own EventBus
/// for more control. Frontend should use Tauri's `listen` API with our events.
#[tauri::command]
pub async fn ui_subscribe_events(state: State<'_, UIState>) -> Result<usize, String> {
    // Return subscriber count
    Ok(state.event_bus.subscriber_count())
}

// ============================================================================
// NODE CONTROL COMMANDS
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct ForceNodeStateRequest {
    pub node_id: String,
    pub status: String,  // "success", "failed", "skip"
}

/// Force a node to a specific state (override execution)
/// Useful for debugging or skipping broken nodes
#[tauri::command]
pub async fn ui_force_node_state(
    request: ForceNodeStateRequest,
    state: State<'_, UIState>,
) -> Result<(), String> {
    // Convert string to NodeStatus
    let status = match request.status.as_str() {
        "success" => crate::ui::events::NodeStatus::Success,
        "failed" => crate::ui::events::NodeStatus::Failed,
        "skip" => crate::ui::events::NodeStatus::Skipped,
        _ => return Err("Invalid status".to_string()),
    };

    state.broadcaster.update_node_status(&request.node_id, status, None);

    state.event_bus.publish(UIEvent::SystemLog {
        level: crate::ui::events::LogLevel::Warn,
        source: "god_mode".to_string(),
        message: format!("Force-set node {} to {}", request.node_id, request.status),
        metadata: serde_json::Value::Null,
        timestamp: current_timestamp(),
    });

    // TODO: Actually modify the Graph execution state
    // Will be implemented in Step 6 (Integration)

    Ok(())
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// ============================================================================
// TAURI COMMAND REGISTRATION HELPER
// ============================================================================

/// Helper macro to register all UI commands
/// Usage in lib.rs:
/// ```
/// .invoke_handler(tauri::generate_handler![
///     ui::commands::register_ui_commands!()
/// ])
/// ```
#[macro_export]
macro_rules! register_ui_commands {
    () => {
        [
            ui_pause_execution,
            ui_resume_execution,
            ui_is_paused,
            ui_get_execution_state,
            ui_get_graph_state,
            ui_create_terminal,
            ui_terminal_write,
            ui_terminal_execute,
            ui_terminal_interrupt,
            ui_terminal_resize,
            ui_terminal_close,
            ui_terminal_list,
            ui_inject_context,
            ui_subscribe_events,
            ui_force_node_state,
        ]
    };
}
