//! ApplyPatchTool - Apply code changes using G-Protocol format
//!
//! This tool wraps Gluon's apply_system to enable LLMs to edit code files.
//! It supports multiple patch formats via the apply_system parsers.
//!
//! ## Supported Formats
//!
//! 1. **XML G-Protocol** (Recommended):
//! ```xml
//! <file path="src/main.rs">
//!   <search>fn old_function()</search>
//!   <replace>fn new_function()</replace>
//! </file>
//! ```
//!
//! 2. **SEARCH/REPLACE** format:
//! ```
//! // FILE: src/main.rs
//! <<<<<<< SEARCH
//! fn old_function()
//! =======
//! fn new_function()
//! >>>>>>> REPLACE
//! ```
//!
//! 3. **Unified Diff** format (git-style patches)
//!
//! ## Example Usage
//!
//! ```json
//! {
//!   "patch": "<file path='main.rs'><search>old</search><replace>new</replace></file>"
//! }
//! ```

use crate::apply_system::parsers::parse_model_response;
use crate::apply_system::matchers::match_code;
use crate::interface::types::{
    GTool, ToolCategory, ToolContext, ToolError, ToolOutput, ToolResult,
};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

/// ApplyPatch tool implementation
///
/// Applies code changes to files using Gluon's apply_system.
/// Requires confirmation since it modifies files.
pub struct ApplyPatchTool;

impl ApplyPatchTool {
    pub fn new() -> Self {
        Self
    }
}

/// Parameters for ApplyPatchTool
#[derive(Debug, Deserialize, JsonSchema)]
struct ApplyPatchParams {
    /// The patch content in any supported format (XML, SEARCH/REPLACE, diff)
    patch: String,

    /// Optional working directory (defaults to context working_dir)
    #[serde(skip_serializing_if = "Option::is_none")]
    working_dir: Option<String>,

    /// Whether to perform a dry run (validate only, don't apply)
    #[serde(default)]
    dry_run: bool,
}

/// Result of applying a patch
#[derive(Debug, Serialize)]
struct ApplyPatchResult {
    /// Number of changes successfully applied
    applied_count: usize,

    /// Number of changes that failed
    failed_count: usize,

    /// Details of each change
    changes: Vec<ChangeResult>,

    /// Summary message
    summary: String,
}

/// Result for a single change
#[derive(Debug, Serialize)]
struct ChangeResult {
    /// File path that was modified
    file_path: String,

    /// Whether this change was successfully applied
    success: bool,

    /// Error message if failed
    error: Option<String>,

    /// Lines that were changed (for dry run)
    lines_changed: Option<usize>,
}

#[async_trait]
impl GTool for ApplyPatchTool {
    fn name(&self) -> &str {
        "gluon.apply_patch"
    }

    fn description(&self) -> &str {
        "Apply code changes to files using G-Protocol patch format. \
         Supports XML G-Protocol, SEARCH/REPLACE blocks, and unified diff formats. \
         Use this to edit code files when the user requests code changes. \
         IMPORTANT: Always use specific, unique search blocks to avoid ambiguity."
    }

    fn parameters_schema(&self) -> Value {
        let schema = schemars::schema_for!(ApplyPatchParams);
        serde_json::to_value(schema).unwrap_or(json!({}))
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        eprintln!("[ApplyPatchTool] Starting patch application");

        // 1. Parse parameters
        let params: ApplyPatchParams = serde_json::from_value(params)
            .map_err(|e| ToolError::invalid_params(&format!("Invalid parameters: {}", e)))?;

        // 2. Determine working directory
        let working_dir = if let Some(ref dir) = params.working_dir {
            Path::new(dir).to_path_buf()
        } else {
            context.working_dir.clone()
        };

        eprintln!(
            "[ApplyPatchTool] Working directory: {}",
            working_dir.display()
        );
        eprintln!("[ApplyPatchTool] Dry run: {}", params.dry_run);

        // 3. Parse patch using apply_system parsers
        let changes = parse_model_response(&params.patch).map_err(|e| {
            ToolError::execution_failed(&format!("Failed to parse patch: {:?}", e))
        })?;

        if changes.is_empty() {
            return Err(ToolError::execution_failed(
                "No changes found in patch. Ensure the patch uses a supported format.",
            ));
        }

        eprintln!("[ApplyPatchTool] Parsed {} changes", changes.len());

        // 4. Apply each change
        let mut results = Vec::new();
        let mut applied_count = 0;
        let mut failed_count = 0;

        for change in changes {
            let file_path = change.file_path.clone();

            eprintln!("[ApplyPatchTool] Processing change for: {}", file_path);

            // Apply or validate the change
            let result = if params.dry_run {
                // Dry run: just validate
                Self::validate_change(&change, &working_dir).await
            } else {
                // Real apply
                Self::apply_change(&change, &working_dir).await
            };

            match result {
                Ok(lines_changed) => {
                    applied_count += 1;
                    results.push(ChangeResult {
                        file_path,
                        success: true,
                        error: None,
                        lines_changed: Some(lines_changed),
                    });
                }
                Err(e) => {
                    failed_count += 1;
                    results.push(ChangeResult {
                        file_path,
                        success: false,
                        error: Some(e),
                        lines_changed: None,
                    });
                }
            }
        }

        // 5. Generate summary
        let summary = if params.dry_run {
            format!(
                "Dry run complete: {} changes validated, {} would fail",
                applied_count, failed_count
            )
        } else {
            format!(
                "Applied {} changes successfully, {} failed",
                applied_count, failed_count
            )
        };

        let apply_result = ApplyPatchResult {
            applied_count,
            failed_count,
            changes: results,
            summary: summary.clone(),
        };

        eprintln!("[ApplyPatchTool] {}", summary);

        Ok(ToolOutput {
            result: serde_json::to_value(apply_result)?,
            summary,
            artifacts: vec![],
        })
    }

    fn requires_confirmation(&self) -> bool {
        true // File modifications are dangerous
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::CodeEditing
    }
}

impl ApplyPatchTool {
    /// Apply a single change to a file
    async fn apply_change(
        change: &crate::apply_system::ChangeQueueItem,
        working_dir: &Path,
    ) -> Result<usize, String> {
        let file_path = working_dir.join(&change.file_path);

        // 1. Read current file content
        let current_content = tokio::fs::read_to_string(&file_path)
            .await
            .map_err(|e| format!("Failed to read {}: {}", change.file_path, e))?;

        // 2. Match the search block
        let match_result = match_code(&current_content, &change.old_code, change.line_start, Some(&change.file_path))
            .map_err(|e| format!("Failed to match code: {:?}", e))?;

        if match_result.matched_line_start == 0 {
            return Err(format!(
                "Search block not found in {}. The code may have changed.",
                change.file_path
            ));
        }

        // 3. Apply replacement
        let new_content = current_content.replace(&change.old_code, &change.new_code);

        // 4. Write updated content
        tokio::fs::write(&file_path, &new_content)
            .await
            .map_err(|e| format!("Failed to write {}: {}", change.file_path, e))?;

        // Calculate lines changed
        let lines_changed = change.new_code.lines().count();

        Ok(lines_changed)
    }

    /// Validate a change without applying it
    async fn validate_change(
        change: &crate::apply_system::ChangeQueueItem,
        working_dir: &Path,
    ) -> Result<usize, String> {
        let file_path = working_dir.join(&change.file_path);

        // 1. Check if file exists
        if !file_path.exists() {
            return Err(format!("File not found: {}", change.file_path));
        }

        // 2. Read current file content
        let current_content = tokio::fs::read_to_string(&file_path)
            .await
            .map_err(|e| format!("Failed to read {}: {}", change.file_path, e))?;

        // 3. Validate that search block matches
        let match_result = match_code(&current_content, &change.old_code, change.line_start, Some(&change.file_path))
            .map_err(|e| format!("Failed to match code: {:?}", e))?;

        if match_result.matched_line_start == 0 {
            return Err(format!(
                "Search block not found in {}",
                change.file_path
            ));
        }

        // Return lines that would be changed
        let lines_changed = change.new_code.lines().count();
        Ok(lines_changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_metadata() {
        let tool = ApplyPatchTool::new();

        assert_eq!(tool.name(), "gluon.apply_patch");
        assert!(tool.requires_confirmation());
        assert!(matches!(tool.category(), ToolCategory::CodeEditing));
    }

    #[test]
    fn test_parameters_schema() {
        let tool = ApplyPatchTool::new();
        let schema = tool.parameters_schema();

        // Should have type = object
        assert_eq!(schema["type"], "object");

        // Should have 'patch' property
        let properties = &schema["properties"];
        assert!(properties.is_object());
        assert!(properties["patch"].is_object());
    }

    #[tokio::test]
    async fn test_invalid_params() {
        let tool = ApplyPatchTool::new();
        let context = ToolContext::default_for_testing();

        let result = tool
            .execute(json!({"invalid": "params"}), &context)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.code == "INVALID_PARAMS");
    }

    // Integration tests would require temporary files
    // Those are better placed in tests/ directory
}
