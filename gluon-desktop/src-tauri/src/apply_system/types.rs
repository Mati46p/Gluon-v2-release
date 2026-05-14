//! Core data structures for the Gluon Apply System
//!
//! This module defines all fundamental types used throughout the apply system:
//! - Change queue items
//! - Change statuses
//! - Snapshot data
//! - Matching methods and results

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

// ============================================================================
// KROK 1: Change Queue Item Structure
// ============================================================================

/// Represents a single proposed code change in the queue
///
/// This is the central data structure that flows through the entire system.
/// It contains all information needed to parse, match, apply, and track a code change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeQueueItem {
    /// Unique identifier for this change (UUID v4)
    pub id: String,

    /// Absolute path to the file where change should be applied
    pub file_path: String,

    /// Starting line number (as provided by the model)
    /// Note: This may not be accurate - matching will find the real location
    pub line_start: usize,

    /// Ending line number (as provided by the model)
    pub line_end: usize,

    /// The code fragment that should be replaced (before)
    pub old_code: String,

    /// The new code that should replace old_code (after)
    pub new_code: String,

    /// Data used for the 3 matching methods
    pub matching_data: MatchingData,

    /// Current status of this change
    pub status: ChangeStatus,

    /// Error message if status is Failed
    pub error_message: Option<String>,

    /// Timestamp when this change was applied (if Applied)
    pub applied_timestamp: Option<SystemTime>,

    /// Results from the matching process (filled after matching)
    pub match_result: Option<MatchResult>,

    /// Timestamp when this change was created/received
    pub created_at: SystemTime,
}

// ============================================================================
// KROK 2: Change Status Enum
// ============================================================================

/// All possible states a change can be in during its lifecycle
///
/// State transitions:
/// - Pending -> Matching -> Applied (success)
/// - Pending -> Matching -> Failed (matching or apply failed)
/// - Any state -> Skipped (user manually skipped)
/// - Applied -> Pending (after undo)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeStatus {
    /// Change received and queued, waiting to be processed
    Pending,

    /// Change is currently being matched/applied (transient state)
    Matching,

    /// Change is being applied to file (transient state)
    Applying,

    /// Change was successfully applied
    Applied,

    /// Change failed (parsing, matching, or application error)
    Failed,

    /// User manually skipped this change
    Skipped,
}

impl ChangeStatus {
    /// Check if this status represents a terminal state (won't change unless user action)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ChangeStatus::Applied | ChangeStatus::Failed | ChangeStatus::Skipped
        )
    }

    /// Check if this status represents an in-progress state
    pub fn is_in_progress(&self) -> bool {
        matches!(self, ChangeStatus::Matching | ChangeStatus::Applying)
    }
}

// ============================================================================
// KROK 3: Snapshot Data Structure
// ============================================================================

/// Snapshot of a file's state from the previous iteration
///
/// Used for:
/// - Conflict detection (compare with current state)
/// - Undo operations (restore from snapshot)
/// - Three-way diff view (Original panel)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotData {
    /// Full content of the file as it was
    pub content: String,

    /// SHA256 hash of content (for quick comparison without string comparison)
    pub content_hash: String,

    /// When this snapshot was created
    pub timestamp: SystemTime,

    /// File path (for debugging and verification)
    pub file_path: String,
}

// ============================================================================
// KROK 4: Matching Method Structures
// ============================================================================

/// All data needed for the 3 matching methods
///
/// This is embedded in ChangeQueueItem and filled during parsing.
/// Different methods use different fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchingData {
    /// Data for Method #1: Anchor Points
    pub anchors: AnchorPoints,

    /// SHA256 hash of old_code (for quick exact matching)
    pub code_hash: String,

    /// 3 lines of context before the changed fragment
    pub context_before: Vec<String>,

    /// 3 lines of context after the changed fragment
    pub context_after: Vec<String>,
}

/// Anchor points extracted from code for precise matching
///
/// These are unique identifiers in the code that help locate the change
/// even if line numbers are wrong.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorPoints {
    /// Name of the function containing this change (if any)
    pub function_name: Option<String>,

    /// Name of the class containing this change (if any)
    pub class_name: Option<String>,

    /// Unique comments found near this change
    /// e.g., "// TODO: refactor this"
    pub unique_comments: Vec<String>,

    /// Export or import statements near this change
    /// e.g., "export const API_URL"
    pub export_statements: Vec<String>,

    /// Any other unique identifiers found
    pub other_identifiers: Vec<String>,
}

/// Result of a matching operation
///
/// Filled by matchers and attached to ChangeQueueItem after matching completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResult {
    /// The actual line where the code was found (after matching)
    pub matched_line_start: usize,

    /// The actual end line where the code was found
    pub matched_line_end: usize,

    /// Which matching method successfully found the code
    pub method_used: MatchMethod,

    /// Confidence score (0.0 - 1.0) indicating how certain we are
    /// - 1.0 = exact match, absolutely certain
    /// - 0.9-0.99 = very confident (e.g., all anchors matched)
    /// - 0.7-0.89 = confident (e.g., fuzzy match with high similarity)
    /// - 0.5-0.69 = uncertain (e.g., partial anchor match)
    /// - <0.5 = low confidence (should warn user)
    pub confidence: f32,

    /// Additional details about the match (for debugging)
    pub details: Option<String>,
}

/// The three matching methods available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchMethod {
    /// Method #1: Matched using anchor points (function names, comments, etc.)
    AnchorPoints,

    /// Method #2: Matched using fuzzy string similarity
    FuzzyMatch,

    /// Method #3: Matched using regex pattern extraction
    RegexPattern,

    /// Exact hash match (optimization - found exact code without deeper matching)
    ExactHash,
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParseError {
    /// None of the 3 parsers could understand the model response
    AllParsersFailed {
        unified_diff_error: String,
        markdown_error: String,
        pattern_error: String,
    },

    /// Parsing succeeded but validation failed
    ValidationFailed { reason: String },

    /// Model response was empty or malformed
    InvalidInput { message: String },
}

/// Errors that can occur during matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchError {
    /// All 3 matching methods failed to locate the code
    AllMatchersFailed {
        anchor_error: String,
        fuzzy_error: String,
        regex_error: String,
    },

    /// File content couldn't be read
    FileReadError { path: String, error: String },

    /// Code fragment is ambiguous (multiple possible locations)
    AmbiguousMatch { locations: Vec<usize> },
}

/// Errors that can occur during apply
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApplyError {
    /// File is locked or permission denied
    FileAccessError { path: String, error: String },

    /// Path is blacklisted for security
    SecurityViolation { path: String, reason: String },
}

// ============================================================================
// Helper Implementations
// ============================================================================

impl ChangeQueueItem {
    /// Create a new change queue item with minimal required fields
    pub fn new(
        file_path: String,
        line_start: usize,
        line_end: usize,
        old_code: String,
        new_code: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            file_path,
            line_start,
            line_end,
            old_code,
            new_code,
            matching_data: MatchingData::default(),
            status: ChangeStatus::Pending,
            error_message: None,
            applied_timestamp: None,
            match_result: None,
            created_at: SystemTime::now(),
        }
    }

    /// Check if this change can be applied (not in terminal failed/skipped state)
    pub fn can_apply(&self) -> bool {
        matches!(self.status, ChangeStatus::Pending | ChangeStatus::Failed)
    }

    /// Check if this change can be undone
    pub fn can_undo(&self) -> bool {
        self.status == ChangeStatus::Applied
    }
}

impl Default for MatchingData {
    fn default() -> Self {
        Self {
            anchors: AnchorPoints::default(),
            code_hash: String::new(),
            context_before: Vec::new(),
            context_after: Vec::new(),
        }
    }
}

impl Default for AnchorPoints {
    fn default() -> Self {
        Self {
            function_name: None,
            class_name: None,
            unique_comments: Vec::new(),
            export_statements: Vec::new(),
            other_identifiers: Vec::new(),
        }
    }
}

impl SnapshotData {
    /// Create a new snapshot from file content
    pub fn new(file_path: String, content: String) -> Self {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        Self {
            content,
            content_hash,
            timestamp: SystemTime::now(),
            file_path,
        }
    }

    /// Quick check if content has changed (by comparing hashes)
    pub fn has_changed(&self, current_content: &str) -> bool {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(current_content.as_bytes());
        let current_hash = format!("{:x}", hasher.finalize());

        self.content_hash != current_hash
    }
}
