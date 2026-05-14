// ============================================================================
// VS CODE / CLAUDE CODE INTEGRATION
// ============================================================================

use crate::ai_chat::{AiChatState, ChatMessage};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};

/// Stream response from VS Code/Claude Code
///
/// This is a placeholder implementation. Full implementation in Phase 5.
pub async fn stream_response(
    _session_id: i64,
    _history: Vec<ChatMessage>,
    _state: AiChatState,
    _pool: SqlitePool,
    _app_handle: AppHandle,
) -> Result<(), String> {
    // TODO: Phase 5 implementation
    // - Check if VS Code client is connected
    // - Send message through MCP or WebSocket
    // - Stream response back
    // - Emit StreamChunk events
    // - Save final response to DB

    Err("VS Code integration not implemented yet".to_string())
}

/// Connect to VS Code/Claude Code
#[tauri::command]
pub async fn connect_to_vscode(
    _port: u16,
    _state: State<'_, AiChatState>,
) -> Result<(), String> {
    // TODO: Phase 5 implementation
    // - Connect to WebSocket/MCP server
    // - Store connection in state.vscode_client

    Err("VS Code connection not implemented yet".to_string())
}
