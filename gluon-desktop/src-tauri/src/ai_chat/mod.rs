// ============================================================================
// AI CHAT MODULE
// ============================================================================

pub mod state;
pub mod commands;
pub mod keyring;
pub mod providers;

// Re-exports for convenience
pub use state::AiChatState;
pub use commands::*;

// ============================================================================
// Data Models
// ============================================================================

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub api_endpoint: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    pub id: i64,
    pub provider_id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_pinned: bool,
    pub token_usage_total: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: i64,
    pub session_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub token_count: i64,
}

// Extended session with provider info (for JOIN queries)
#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionWithProvider {
    pub id: i64,
    pub provider_id: i64,
    pub provider_name: String,
    pub provider_type: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_pinned: bool,
    pub token_usage_total: i64,
    pub message_count: i64,
}

// ============================================================================
// Request/Response Payloads
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionPayload {
    pub provider_id: i64,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessagePayload {
    pub session_id: i64,
    pub content: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunk {
    pub session_id: i64,
    pub message_id: Option<i64>,
    pub content: String,
    pub is_final: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetApiKeyPayload {
    pub provider_type: String,
    pub api_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyStatus {
    pub provider_type: String,
    pub is_configured: bool,
    pub last_verified: Option<String>,
}
