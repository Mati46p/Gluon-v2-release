// ============================================================================
// AI CHAT STATE MANAGEMENT
// ============================================================================

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// Global state for AI Chat system
///
/// Manages:
/// - In-memory API keys (fallback for keyring)
/// - Active streaming sessions
/// - VS Code connection state
#[derive(Clone)]
pub struct AiChatState {
    /// API Keys stored in memory (fallback if keyring fails)
    /// Key: provider_type (e.g., "gemini", "claude")
    /// Value: API key
    pub api_keys: Arc<Mutex<HashMap<String, String>>>,

    /// Active streaming sessions (for SSE)
    /// Key: session_id
    /// Value: Channel sender for streaming chunks
    pub active_streams: Arc<Mutex<HashMap<i64, tokio::sync::mpsc::Sender<String>>>>,

    /// VS Code/Claude Code connection state
    pub vscode_client: Arc<Mutex<Option<VsCodeClient>>>,
}

impl AiChatState {
    pub fn new() -> Self {
        Self {
            api_keys: Arc::new(Mutex::new(HashMap::new())),
            active_streams: Arc::new(Mutex::new(HashMap::new())),
            vscode_client: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for AiChatState {
    fn default() -> Self {
        Self::new()
    }
}

/// VS Code/Claude Code client (placeholder for Phase 2)
pub struct VsCodeClient {
    // TODO: Implement WebSocket or MCP client
    // pub connection: WebSocketConnection,
    // pub port: u16,
}
