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

    /// File content AFTER this change was applied (for selective undo/redo)
    /// This allows us to recreate the exact state needed when selectively undoing changes
    pub applied_content: Option<String>,
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

    /// Breakdown of confidence components (for Weighted Anchoring)
    /// Only populated when method_used == WeightedAnchor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_breakdown: Option<ConfidenceBreakdown>,
}

/// Breakdown of confidence score components
///
/// Used by Weighted Anchoring to show transparency in matching decision.
/// Based on Document IV (Section 4 - Etap 4 consultation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceBreakdown {
    /// Similarity score from fuzzy matching (0.0-1.0)
    /// Levenshtein-based exact line matching
    pub similarity: f64,

    /// Token-level similarity (whitespace-agnostic) (0.0-1.0)
    /// Structural matching that ignores formatting
    pub token_similarity: f64,

    /// Anchor quality contribution (0.0-1.0)
    /// Combined uniqueness * structural quality weight
    pub anchor_quality: f64,
}

/// The matching methods available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchMethod {
    /// Method #0: [NEW - PRIORITY 1] Weighted Anchoring with Fuzzy Expansion
    /// Uses frequency analysis to find unique lines, anchors on them, and expands fuzzy
    /// Based on Document IV (Section 3.3: "Weighted Anchoring and Unique Line Indexing")
    #[serde(rename = "weighted_anchor")]
    WeightedAnchor,

    /// Method #1: Matched using anchor points (function names, comments, etc.)
    AnchorPoints,

    /// Method #2: Matched using fuzzy string similarity
    FuzzyMatch,

    /// Method #3: Matched using regex pattern extraction
    RegexPattern,

    /// Method #4: [System A] Surgical Block Match (AST/Indentation map)
    BlockStructure,

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

// Note: Apply errors are handled using Result<T, String> for simplicity
// This enum is kept for future structured error handling

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
            applied_content: None,
        }
    }

    /// Check if this change can be applied (not in terminal failed/skipped state)
    pub fn can_apply(&self) -> bool {
        matches!(self.status, ChangeStatus::Pending | ChangeStatus::Failed)
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
    #[allow(dead_code)]
    pub fn has_changed(&self, current_content: &str) -> bool {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(current_content.as_bytes());
        let current_hash = format!("{:x}", hasher.finalize());

        self.content_hash != current_hash
    }
}
