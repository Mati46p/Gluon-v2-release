// ============================================================================
// CLAUDE API CLIENT
// ============================================================================

use crate::ai_chat::{AiChatState, ChatMessage, StreamChunk};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

/// Stream response from Claude API
///
/// This is a placeholder implementation. Full implementation in Phase 3.
pub async fn stream_response(
    _session_id: i64,
    _history: Vec<ChatMessage>,
    _state: AiChatState,
    _pool: SqlitePool,
    _app_handle: AppHandle,
) -> Result<(), String> {
    // TODO: Phase 3 implementation
    // - Get API key from keyring
    // - Convert history to Claude format
    // - Make HTTP POST to Claude API with streaming
    // - Parse SSE events (content_block_delta)
    // - Emit StreamChunk events
    // - Save final response to DB

    Err("Claude provider not implemented yet".to_string())
}
