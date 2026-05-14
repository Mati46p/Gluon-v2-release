//! Message Protocol Definitions
//!
//! Defines all messages exchanged between components:
//! - Browser (V3) <-> Tauri
//!
//! All messages have unique request IDs for request-response tracking.

use crate::apply_system::types::ChangeQueueItem;
use serde::{Deserialize, Serialize};

// ============================================================================
// KROK 5: Message Protocols Between Components
// ============================================================================

// ----------------------------------------------------------------------------
// Browser -> Tauri Messages
// ----------------------------------------------------------------------------

/// Messages sent from the browser V3 extension to Tauri
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BrowserToTauriMessage {
    /// User clicked "Apply" on a single change
    ApplyChange {
        request_id: String,
        change_id: String,
    },

    /// User clicked "Apply All" - apply all pending changes
    ApplyAll { request_id: String },

    /// User clicked "Undo" on a single change
    UndoChange {
        request_id: String,
        change_id: String,
    },

    /// User clicked "Undo All" - undo all applied changes from this iteration
    UndoAll { request_id: String },

    /// User clicked "Preview" to see diff
    PreviewChange {
        request_id: String,
        change_id: String,
    },

    /// User clicked "Reject" to skip a change
    RejectChange {
        request_id: String,
        change_id: String,
    },

    /// Raw model response that needs to be parsed
    ParseModelResponse {
        request_id: String,
        /// Raw text response from the AI model
        raw_response: String,
    },

    /// Request current state of all changes
    GetChangeQueue { request_id: String },

    /// Request current settings/configuration
    GetSettings { request_id: String },

    /// Update settings
    UpdateSettings {
        request_id: String,
        settings: serde_json::Value,
    },
}

// ----------------------------------------------------------------------------
// Tauri -> Browser Messages
// ----------------------------------------------------------------------------

/// Messages sent from Tauri back to the browser
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TauriToBrowserMessage {
    /// Response to ParseModelResponse
    ParsingComplete {
        request_id: String,
        success: bool,
        /// List of parsed changes (if successful)
        changes: Option<Vec<ChangeQueueItem>>,
        /// Error message (if failed)
        error: Option<String>,
    },

    /// Response to ApplyChange
    ApplyComplete {
        request_id: String,
        change_id: String,
        success: bool,
        error: Option<String>,
    },

    /// Response to ApplyAll (sent after all changes processed)
    ApplyAllComplete {
        request_id: String,
        total: usize,
        applied: usize,
        failed: usize,
        /// IDs of changes that failed
        failed_changes: Vec<String>,
    },

    /// Response to UndoChange
    UndoComplete {
        request_id: String,
        change_id: String,
        success: bool,
        error: Option<String>,
    },

    /// Response to UndoAll
    UndoAllComplete {
        request_id: String,
        total_undone: usize,
    },

    /// Response to PreviewChange
    PreviewData {
        request_id: String,
        change_id: String,
        /// Original code (from snapshot, if exists)
        original: Option<String>,
        /// Current code (from disk)
        current: String,
        /// Proposed code (from model)
        proposed: String,
        /// Whether there's a conflict (current != original)
        has_conflict: bool,
    },

    /// Status update (sent during long operations)
    StatusUpdate {
        message: String,
        /// Type of status: "info", "success", "error", "warning"
        level: String,
    },

    /// Progress update during batch operations
    Progress {
        current: usize,
        total: usize,
        message: String,
    },

    /// Response to GetChangeQueue
    ChangeQueueState {
        request_id: String,
        changes: Vec<ChangeQueueItem>,
    },

    /// Response to GetSettings
    SettingsData {
        request_id: String,
        settings: serde_json::Value,
    },
}

// ============================================================================
// Helper Structures
// ============================================================================

/// Wrapper for all messages with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope<T> {
    /// Unique message ID
    pub id: String,

    /// Timestamp when message was created
    pub timestamp: std::time::SystemTime,

    /// The actual message payload
    pub payload: T,
}

impl<T> MessageEnvelope<T> {
    pub fn new(payload: T) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now(),
            payload,
        }
    }

    pub fn with_id(id: String, payload: T) -> Self {
        Self {
            id,
            timestamp: std::time::SystemTime::now(),
            payload,
        }
    }
}

// ============================================================================
// Request-Response Tracking
// ============================================================================

/// Track pending requests waiting for responses
#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub request_id: String,
    pub created_at: std::time::SystemTime,
    pub timeout_at: std::time::SystemTime,
}

impl PendingRequest {
    pub fn new(request_id: String, timeout_seconds: u64) -> Self {
        let now = std::time::SystemTime::now();
        let timeout_at = now + std::time::Duration::from_secs(timeout_seconds);

        Self {
            request_id,
            created_at: now,
            timeout_at,
        }
    }

    pub fn is_timed_out(&self) -> bool {
        std::time::SystemTime::now() > self.timeout_at
    }
}
