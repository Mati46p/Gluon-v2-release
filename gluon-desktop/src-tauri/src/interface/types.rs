//! Core types and traits for the Interface Layer
//!
//! This module defines the fundamental abstractions for tool-calling:
//! - `GTool`: The trait that all tools must implement
//! - `ToolContext`: Execution context provided to tools
//! - `ToolResult`: Result type for tool execution
//! - `ToolOutput`: Structured output from successful tool execution
//! - `ToolError`: Error information from failed tool execution

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};

// Type alias for async confirmation callback
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Core trait that every tool must implement
///
/// Tools are the building blocks of the G-Protocol system. Each tool:
/// - Has a unique name (e.g., "gluon.read_file")
/// - Provides a human-readable description for LLMs
/// - Defines its parameter schema using JSON Schema (via schemars)
/// - Executes asynchronously with access to ToolContext
/// - Can optionally require user confirmation for dangerous operations
#[async_trait]
pub trait GTool: Send + Sync {
    /// Unique tool identifier (e.g., "gluon.read_file", "github.create_pr")
    ///
    /// Naming convention: "{namespace}.{action}"
    /// - "gluon.*" for built-in Gluon tools
    /// - "{server_id}.*" for remote MCP tools
    fn name(&self) -> &str;

    /// Human-readable description for LLM consumption
    ///
    /// Should explain:
    /// - What the tool does
    /// - When to use it
    /// - Any important constraints or limitations
    fn description(&self) -> &str;

    /// JSON Schema for tool parameters
    ///
    /// Auto-generated via `schemars::schema_for!(YourParamsStruct)`
    /// The schema is used by:
    /// - LLMs to understand how to call the tool
    /// - Runtime validation of tool calls
    /// - Documentation generation
    fn parameters_schema(&self) -> Value;

    /// Execute the tool with given parameters
    ///
    /// # Arguments
    /// * `params` - JSON object matching the parameters_schema
    /// * `context` - Execution context (access to apply_system, blackboard, etc.)
    ///
    /// # Returns
    /// * `Ok(ToolOutput)` - Successful execution with result
    /// * `Err(ToolError)` - Execution failed with error details
    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult;

    /// Whether this tool requires user confirmation before execution
    ///
    /// Default: false (most tools are safe)
    ///
    /// Set to true for:
    /// - File deletion
    /// - Shell command execution
    /// - Network requests to external services
    /// - Any operation that could cause data loss or security risk
    fn requires_confirmation(&self) -> bool {
        false
    }

    /// Tool category for organization and filtering
    ///
    /// Default: ToolCategory::General
    fn category(&self) -> ToolCategory {
        ToolCategory::General
    }
}

/// Execution context provided to tools
///
/// Contains all the resources a tool needs to execute:
/// - Access to apply_system state (for code modifications)
/// - Access to engine blackboard (for agent workflows)
/// - Confirmation callback (for dangerous operations)
/// - Request metadata (for tracing and logging)
pub struct ToolContext {
    /// Apply system state (for code modification tools)
    pub apply_state: Arc<Mutex<crate::apply_system::ApplySystemState>>,

    /// Engine blackboard (optional - only in agent workflows)
    pub blackboard: Option<Arc<RwLock<crate::engine::memory::Blackboard>>>,

    /// User confirmation callback for dangerous operations
    ///
    /// If None, dangerous operations are automatically denied.
    /// If Some, the callback is called with a confirmation message.
    pub confirm_callback: Option<Arc<dyn Fn(&str) -> BoxFuture<'static, bool> + Send + Sync>>,

    /// Unique request ID for tracing
    pub request_id: String,

    /// Working directory for file operations
    pub working_dir: PathBuf,
}

impl ToolContext {
    /// Create a default ToolContext for testing
    pub fn default_for_testing() -> Self {
        Self {
            apply_state: Arc::new(Mutex::new(crate::apply_system::ApplySystemState::new())),
            blackboard: None,
            confirm_callback: None,
            request_id: uuid::Uuid::new_v4().to_string(),
            working_dir: PathBuf::from("."),
        }
    }
}

/// Result type for tool execution
pub type ToolResult = Result<ToolOutput, ToolError>;

/// Successful tool execution output
///
/// Contains both structured data (for programmatic use) and
/// human-readable summary (for LLM consumption).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolOutput {
    /// Structured result data (JSON value)
    ///
    /// This is the primary output that can be:
    /// - Parsed by other tools
    /// - Used in agent reasoning
    /// - Returned to external MCP clients
    pub result: Value,

    /// Human-readable summary of the operation
    ///
    /// Examples:
    /// - "Successfully read 150 lines from src/main.rs"
    /// - "Applied 3 code changes to 2 files"
    /// - "Screenshot captured: 1920x1080, 45KB"
    pub summary: String,

    /// Optional artifacts produced by the tool
    ///
    /// Examples:
    /// - File paths of modified files
    /// - Screenshot image data
    /// - Log file locations
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]  // Don't include in schema (runtime only)
    pub artifacts: Vec<Artifact>,
}

/// Artifact produced by tool execution
///
/// Represents a file, image, or other resource created/modified by the tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Type of artifact
    pub artifact_type: ArtifactType,

    /// Path or identifier of the artifact
    pub path: String,

    /// MIME type (e.g., "image/png", "text/plain")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Type of artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    /// File on disk
    File,

    /// Image (screenshot, diagram, etc.)
    Image,

    /// Log file
    Log,
}

/// Error information from failed tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolError {
    /// Error code (standardized across tools)
    pub code: ToolErrorCode,

    /// Human-readable error message
    pub message: String,

    /// Optional additional details (stack trace, context, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl ToolError {
    /// Create an InvalidParameters error
    pub fn invalid_params(message: &str) -> Self {
        Self {
            code: ToolErrorCode::InvalidParameters,
            message: message.to_string(),
            details: None,
        }
    }

    /// Create an ExecutionFailed error
    pub fn execution_failed(message: &str) -> Self {
        Self {
            code: ToolErrorCode::ExecutionFailed,
            message: message.to_string(),
            details: None,
        }
    }

    /// Create a PermissionDenied error
    pub fn permission_denied(message: &str) -> Self {
        Self {
            code: ToolErrorCode::PermissionDenied,
            message: message.to_string(),
            details: None,
        }
    }

    /// Create a ResourceNotFound error
    pub fn resource_not_found(message: &str) -> Self {
        Self {
            code: ToolErrorCode::ResourceNotFound,
            message: message.to_string(),
            details: None,
        }
    }
}

impl From<serde_json::Error> for ToolError {
    fn from(err: serde_json::Error) -> Self {
        Self {
            code: ToolErrorCode::ExecutionFailed,
            message: format!("JSON serialization error: {}", err),
            details: None,
        }
    }
}

/// Standardized error codes for tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolErrorCode {
    /// Invalid parameters provided to tool
    InvalidParameters,

    /// Tool execution failed (internal error)
    ExecutionFailed,

    /// User denied confirmation for dangerous operation
    PermissionDenied,

    /// Requested resource not found (file, endpoint, etc.)
    ResourceNotFound,

    /// Operation timed out
    Timeout,

    /// User manually cancelled the operation
    UserCancelled,
}

/// Tool category for organization
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    /// General-purpose tools
    General,

    /// File system operations (read, write, list)
    FileSystem,

    /// Code editing and transformation
    CodeEditing,

    /// System operations (shell commands, process management)
    System,

    /// Browser automation and instrumentation
    Browser,

    /// Code analysis and search
    Analysis,

    /// Meta-tools (introspection, manifest, etc.)
    Meta,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_error_constructors() {
        let err = ToolError::invalid_params("Missing file_path");
        assert!(matches!(err.code, ToolErrorCode::InvalidParameters));
        assert_eq!(err.message, "Missing file_path");

        let err = ToolError::execution_failed("File not accessible");
        assert!(matches!(err.code, ToolErrorCode::ExecutionFailed));

        let err = ToolError::permission_denied("User denied confirmation");
        assert!(matches!(err.code, ToolErrorCode::PermissionDenied));

        let err = ToolError::resource_not_found("File does not exist");
        assert!(matches!(err.code, ToolErrorCode::ResourceNotFound));
    }

    #[test]
    fn test_tool_output_serialization() {
        let output = ToolOutput {
            result: serde_json::json!({"status": "success", "count": 42}),
            summary: "Operation completed successfully".to_string(),
            artifacts: vec![
                Artifact {
                    artifact_type: ArtifactType::File,
                    path: "output.txt".to_string(),
                    mime_type: Some("text/plain".to_string()),
                },
            ],
        };

        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("Operation completed successfully"));
    }

    #[test]
    fn test_tool_category_serialization() {
        let category = ToolCategory::FileSystem;
        let json = serde_json::to_string(&category).unwrap();
        assert_eq!(json, "\"file_system\"");

        let category = ToolCategory::CodeEditing;
        let json = serde_json::to_string(&category).unwrap();
        assert_eq!(json, "\"code_editing\"");
    }
}
