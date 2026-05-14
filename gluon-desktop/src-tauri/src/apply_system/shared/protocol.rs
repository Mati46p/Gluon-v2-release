//! Message Protocol Definitions
//!
//! Defines all messages exchanged between components:
//! - Browser (V3) <-> Tauri
//!
//! All messages have unique request IDs for request-response tracking.

use crate::apply_system::shared::types::ChangeQueueItem;
use serde::{Deserialize, Serialize};

// ============================================================================
// KROK 5: Message Protocols Between Components
// ============================================================================

// ----------------------------------------------------------------------------
// Browser -> Tauri Messages
// ----------------------------------------------------------------------------
// Note: Message types are handled directly via Tauri commands
// This enum is kept for future websocket/IPC implementation

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

    /// Granular progress update for individual change application
    /// This is the core of the "Pulse" system - real-time feedback
    ApplyProgress {
        /// Request ID to track this specific apply operation
        request_id: String,
        /// Change ID being processed
        change_id: String,
        /// Current processing step
        step: ProcessingStep,
        /// Human-readable message about current activity
        message: String,
        /// Progress percentage (0-100)
        progress: u8,
        /// Optional additional details (e.g., "Confidence: 98%", "Line 45")
        details: Option<String>,
        /// File path being modified (for undo/redo operations)
        #[serde(skip_serializing_if = "Option::is_none")]
        file_path: Option<String>,
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

    /// Notification that a change was undone (from VS Code)
    ChangeUndone {
        change_id: String,
        batch_id: String,
    },

    /// Notification that a change was redone (from VS Code)
    ChangeRedone {
        change_id: String,
        batch_id: String,
    },
}

// ============================================================================
// Processing Steps for Pulse System
// ============================================================================

/// Granular steps during code change application
/// Each step represents a distinct phase with its own visual representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessingStep {
    /// Change has been queued for processing
    Queued,

    /// Validating file path and permissions
    Validating,

    /// Creating snapshot for undo/conflict detection
    Snapshotting,

    /// Matching phase - finding exact code location
    /// This is where most of the "magic" happens
    Matching,

    /// Sub-step: Analyzing code structure (BlockMatcher)
    AnalyzingStructure,

    /// Sub-step: Searching for anchor points (WeightedAnchor)
    SearchingAnchors,

    /// Sub-step: Fuzzy matching expansion
    FuzzyExpanding,

    /// Running safety checks (DestructionGuard)
    SafetyCheck,

    /// Writing changes to file
    Writing,

    /// Notifying editor with flash effect
    Notifying,

    /// Successfully completed
    Success,

    /// Failed at some stage
    Failed,
}

// ============================================================================
// Helper Structures
// ============================================================================
// Note: Request-response tracking is handled by Tauri's built-in command system
// These structures are kept for future websocket/IPC implementation
