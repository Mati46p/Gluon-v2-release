//! WriteFileTool - Write content to a file on disk
//!
//! ## Example Usage
//!
//! ```json
//! {
//!   "jsonrpc": "2.0",
//!   "method": "gluon.write_file",
//!   "id": 1,
//!   "params": {
//!     "file_path": "output.txt",
//!     "content": "Hello, World!\nLine 2",
//!     "create_dirs": true
//!   }
//! }
//! ```
//!
//! ## Response
//!
//! ```json
//! {
//!   "jsonrpc": "2.0",
//!   "id": 1,
//!   "result": {
//!     "result": {
//!       "file_path": "output.txt",
//!       "bytes_written": 20,
//!       "created": true
//!     },
//!     "summary": "Successfully wrote 20 bytes to output.txt (created new file)"
//!   }
//! }
//! ```

use crate::interface::types::{GTool, ToolCategory, ToolContext, ToolError, ToolOutput, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::fs;

/// Write file tool implementation
pub struct WriteFileTool;

impl WriteFileTool {
    pub fn new() -> Self {
        Self
    }
}

/// Parameters for WriteFileTool
#[derive(Debug, Deserialize, JsonSchema)]
struct WriteFileParams {
    /// Path to the file to write (relative to working directory)
    file_path: String,

    /// Content to write to the file
    content: String,

    /// Create parent directories if they don't exist (default: false)
    #[serde(default)]
    create_dirs: bool,

    /// Append to file instead of overwriting (default: false)
    #[serde(default)]
    append: bool,
}

/// Output data from WriteFileTool
#[derive(Debug, Serialize, JsonSchema)]
struct WriteFileOutput {
    /// Path to the file that was written
    file_path: String,

    /// Number of bytes written
    bytes_written: usize,

    /// Whether a new file was created (vs overwritten)
    created: bool,
}

#[async_trait]
impl GTool for WriteFileTool {
    fn name(&self) -> &str {
        "gluon.write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file on disk. Can create new files or overwrite existing ones. \
         Optionally create parent directories and append to existing files."
    }

    fn parameters_schema(&self) -> Value {
        let schema = schemars::schema_for!(WriteFileParams);
        serde_json::to_value(schema).unwrap_or(json!({}))
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        // 1. Parse parameters
        let params: WriteFileParams = serde_json::from_value(params)
            .map_err(|e| ToolError::invalid_params(&format!("Invalid parameters: {}", e)))?;

        // 2. Resolve file path (relative to working directory)
        let file_path = context.working_dir.join(&params.file_path);

        // 3. Validate path (security check - no directory traversal)
        if !Self::is_safe_path(&file_path, &context.working_dir) {
            return Err(ToolError::permission_denied(
                "Path traversal detected - access denied",
            ));
        }

        // 4. Check if file exists (to determine if we're creating or overwriting)
        let file_existed = file_path.exists();

        // 5. Create parent directories if needed
        if params.create_dirs {
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    ToolError::execution_failed(&format!("Failed to create directories: {}", e))
                })?;
            }
        }

        // 6. Write or append to file
        let bytes_written = if params.append && file_existed {
            // Append mode
            let mut existing_content = fs::read_to_string(&file_path).await.unwrap_or_default();
            existing_content.push_str(&params.content);

            fs::write(&file_path, &existing_content)
                .await
                .map_err(|e| ToolError::execution_failed(&format!("Failed to append to file: {}", e)))?;

            existing_content.len()
        } else {
            // Write mode (overwrite)
            fs::write(&file_path, &params.content)
                .await
                .map_err(|e| ToolError::execution_failed(&format!("Failed to write file: {}", e)))?;

            params.content.len()
        };

        // 7. Format summary
        let action = if !file_existed {
            "created new file"
        } else if params.append {
            "appended to file"
        } else {
            "overwrote file"
        };

        let summary = format!(
            "Successfully wrote {} bytes to {} ({})",
            bytes_written, params.file_path, action
        );

        // 8. Return result
        let output_data = WriteFileOutput {
            file_path: params.file_path.clone(),
            bytes_written,
            created: !file_existed,
        };

        Ok(ToolOutput {
            result: serde_json::to_value(output_data)?,
            summary,
            artifacts: vec![crate::interface::types::Artifact {
                artifact_type: crate::interface::types::ArtifactType::File,
                path: params.file_path,
                mime_type: Some("text/plain".to_string()),
            }],
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileSystem
    }

    fn requires_confirmation(&self) -> bool {
        true // Writing files is potentially dangerous
    }
}

impl WriteFileTool {
    /// Check if path is safe (no directory traversal)
    fn is_safe_path(path: &PathBuf, working_dir: &PathBuf) -> bool {
        // For write operations, we need to check the parent directory
        // because the file might not exist yet
        let check_path = if path.exists() {
            path.clone()
        } else if let Some(parent) = path.parent() {
            parent.to_path_buf()
        } else {
            return false;
        };

        // Canonicalize paths
        if let (Ok(canonical_wd), Ok(canonical_check)) = (
            working_dir.canonicalize(),
            check_path.canonicalize().or_else(|_| {
                // If canonicalization fails (directory doesn't exist),
                // check if parent is safe
                if let Some(parent) = check_path.parent() {
                    parent.canonicalize()
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Path not found",
                    ))
                }
            }),
        ) {
            canonical_check.starts_with(&canonical_wd)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_write_file_new() {
        let dir = tempdir().unwrap();

        let tool = WriteFileTool::new();
        let mut context = ToolContext::default_for_testing();
        context.working_dir = dir.path().to_path_buf();

        let params = json!({
            "file_path": "test.txt",
            "content": "Hello, World!\n"
        });

        let result = tool.execute(params, &context).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.summary.contains("created new file"));

        // Verify file was created
        let file_path = dir.path().join("test.txt");
        assert!(file_path.exists());

        let content = fs::read_to_string(file_path).unwrap();
        assert_eq!(content, "Hello, World!\n");
    }

    #[tokio::test]
    async fn test_write_file_overwrite() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("existing.txt");

        // Create existing file
        fs::write(&file_path, "Old content").unwrap();

        let tool = WriteFileTool::new();
        let mut context = ToolContext::default_for_testing();
        context.working_dir = dir.path().to_path_buf();

        let params = json!({
            "file_path": "existing.txt",
            "content": "New content"
        });

        let result = tool.execute(params, &context).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.summary.contains("overwrote file"));

        // Verify content was overwritten
        let content = fs::read_to_string(file_path).unwrap();
        assert_eq!(content, "New content");
    }

    #[tokio::test]
    async fn test_write_file_append() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("append.txt");

        // Create existing file
        fs::write(&file_path, "Line 1\n").unwrap();

        let tool = WriteFileTool::new();
        let mut context = ToolContext::default_for_testing();
        context.working_dir = dir.path().to_path_buf();

        let params = json!({
            "file_path": "append.txt",
            "content": "Line 2\n",
            "append": true
        });

        let result = tool.execute(params, &context).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.summary.contains("appended to file"));

        // Verify content was appended
        let content = fs::read_to_string(file_path).unwrap();
        assert_eq!(content, "Line 1\nLine 2\n");
    }

    #[tokio::test]
    async fn test_write_file_create_dirs() {
        let dir = tempdir().unwrap();

        let tool = WriteFileTool::new();
        let mut context = ToolContext::default_for_testing();
        context.working_dir = dir.path().to_path_buf();

        let params = json!({
            "file_path": "subdir/nested/file.txt",
            "content": "Content",
            "create_dirs": true
        });

        let result = tool.execute(params, &context).await;

        assert!(result.is_ok());

        // Verify directories and file were created
        let file_path = dir.path().join("subdir/nested/file.txt");
        assert!(file_path.exists());

        let content = fs::read_to_string(file_path).unwrap();
        assert_eq!(content, "Content");
    }

    #[tokio::test]
    async fn test_write_file_directory_traversal_blocked() {
        let dir = tempdir().unwrap();

        let tool = WriteFileTool::new();
        let mut context = ToolContext::default_for_testing();
        context.working_dir = dir.path().join("subdir");

        // Try to write file outside working directory
        let params = json!({
            "file_path": "../../../etc/passwd",
            "content": "malicious"
        });

        let result = tool.execute(params, &context).await;

        // Should be denied
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err.code,
            crate::interface::types::ToolErrorCode::PermissionDenied
        ));
    }

    #[tokio::test]
    async fn test_write_file_requires_confirmation() {
        let tool = WriteFileTool::new();
        assert!(tool.requires_confirmation());
    }
}
