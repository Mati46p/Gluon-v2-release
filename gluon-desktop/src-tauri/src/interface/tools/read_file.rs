//! ReadFileTool - Read file contents from disk
//!
//! ## Example Usage
//!
//! ```json
//! {
//!   "jsonrpc": "2.0",
//!   "method": "gluon.read_file",
//!   "id": 1,
//!   "params": {
//!     "file_path": "src/main.rs"
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
//!       "content": "fn main() { ... }",
//!       "size_bytes": 1234,
//!       "line_count": 42
//!     },
//!     "summary": "Successfully read 42 lines from src/main.rs (1.2 KB)"
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

/// Read file tool implementation
pub struct ReadFileTool;

impl ReadFileTool {
    pub fn new() -> Self {
        Self
    }
}

/// Parameters for ReadFileTool
#[derive(Debug, Deserialize, JsonSchema)]
struct ReadFileParams {
    /// Path to the file to read (relative to working directory)
    file_path: String,

    /// Optional: Maximum number of bytes to read (0 = unlimited)
    #[serde(default)]
    max_bytes: usize,
}

/// Output data from ReadFileTool
#[derive(Debug, Serialize, JsonSchema)]
struct ReadFileOutput {
    /// File contents
    content: String,

    /// File size in bytes
    size_bytes: u64,

    /// Number of lines
    line_count: usize,
}

#[async_trait]
impl GTool for ReadFileTool {
    fn name(&self) -> &str {
        "gluon.read_file"
    }

    fn description(&self) -> &str {
        "Read contents of a file from disk. Returns file content, size, and line count."
    }

    fn parameters_schema(&self) -> Value {
        let schema = schemars::schema_for!(ReadFileParams);
        serde_json::to_value(schema).unwrap_or(json!({}))
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        // 1. Parse parameters
        let params: ReadFileParams = serde_json::from_value(params)
            .map_err(|e| ToolError::invalid_params(&format!("Invalid parameters: {}", e)))?;

        // 2. Resolve file path (relative to working directory)
        let file_path = context.working_dir.join(&params.file_path);

        // 3. Validate path (security check - no directory traversal)
        if !Self::is_safe_path(&file_path, &context.working_dir) {
            return Err(ToolError::permission_denied(
                "Path traversal detected - access denied",
            ));
        }

        // 4. Check if file exists
        if !file_path.exists() {
            return Err(ToolError::resource_not_found(&format!(
                "File not found: {}",
                params.file_path
            )));
        }

        // 5. Check if it's a file (not a directory)
        if !file_path.is_file() {
            return Err(ToolError::invalid_params(&format!(
                "Path is not a file: {}",
                params.file_path
            )));
        }

        // 6. Read file metadata
        let metadata = fs::metadata(&file_path).await.map_err(|e| {
            ToolError::execution_failed(&format!("Failed to read file metadata: {}", e))
        })?;

        let size_bytes = metadata.len();

        // 7. Read file content
        let content = if params.max_bytes > 0 && size_bytes > params.max_bytes as u64 {
            // Read only first max_bytes
            let bytes = fs::read(&file_path).await.map_err(|e| {
                ToolError::execution_failed(&format!("Failed to read file: {}", e))
            })?;

            String::from_utf8_lossy(&bytes[..params.max_bytes.min(bytes.len())]).to_string()
                + "\n... (file truncated)"
        } else {
            // Read entire file
            fs::read_to_string(&file_path).await.map_err(|e| {
                ToolError::execution_failed(&format!("Failed to read file: {}", e))
            })?
        };

        // 8. Count lines
        let line_count = content.lines().count();

        // 9. Format summary
        let size_kb = size_bytes as f64 / 1024.0;
        let summary = format!(
            "Successfully read {} lines from {} ({:.1} KB)",
            line_count, params.file_path, size_kb
        );

        // 10. Return result
        let output_data = ReadFileOutput {
            content,
            size_bytes,
            line_count,
        };

        Ok(ToolOutput {
            result: serde_json::to_value(output_data)?,
            summary,
            artifacts: vec![],
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::FileSystem
    }
}

impl ReadFileTool {
    /// Check if path is safe (no directory traversal)
    fn is_safe_path(path: &PathBuf, working_dir: &PathBuf) -> bool {
        // Canonicalize both paths
        if let (Ok(canonical_path), Ok(canonical_wd)) = (
            path.canonicalize(),
            working_dir.canonicalize()
        ) {
            // Check if file path starts with working directory
            canonical_path.starts_with(&canonical_wd)
        } else {
            // If canonicalization fails, deny access (safe default)
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_read_file_success() {
        // Create temp directory and file
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3").unwrap();

        // Create tool and context
        let tool = ReadFileTool::new();
        let mut context = ToolContext::default_for_testing();
        context.working_dir = dir.path().to_path_buf();

        // Execute tool
        let params = json!({ "file_path": "test.txt" });
        let result = tool.execute(params, &context).await;

        // Verify success
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.summary.contains("3 lines"));
        assert!(output.summary.contains("test.txt"));

        // Parse result
        let data: ReadFileOutput = serde_json::from_value(output.result).unwrap();
        assert_eq!(data.line_count, 3);
        assert!(data.content.contains("Line 1"));
        assert!(data.content.contains("Line 2"));
        assert!(data.content.contains("Line 3"));
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let dir = tempdir().unwrap();

        let tool = ReadFileTool::new();
        let mut context = ToolContext::default_for_testing();
        context.working_dir = dir.path().to_path_buf();

        let params = json!({ "file_path": "nonexistent.txt" });
        let result = tool.execute(params, &context).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err.code, crate::interface::types::ToolErrorCode::ResourceNotFound));
        assert!(err.message.contains("not found"));
    }

    #[tokio::test]
    async fn test_read_file_directory_traversal_blocked() {
        let dir = tempdir().unwrap();

        let tool = ReadFileTool::new();
        let mut context = ToolContext::default_for_testing();
        context.working_dir = dir.path().join("subdir");

        // Try to read file outside working directory
        let params = json!({ "file_path": "../../../etc/passwd" });
        let result = tool.execute(params, &context).await;

        // Should be denied
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_file_max_bytes() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("large.txt");
        let mut file = File::create(&file_path).unwrap();

        // Write 1000 bytes
        for _ in 0..100 {
            writeln!(file, "0123456789").unwrap();
        }

        let tool = ReadFileTool::new();
        let mut context = ToolContext::default_for_testing();
        context.working_dir = dir.path().to_path_buf();

        // Read only first 50 bytes
        let params = json!({ "file_path": "large.txt", "max_bytes": 50 });
        let result = tool.execute(params, &context).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        let data: ReadFileOutput = serde_json::from_value(output.result).unwrap();

        // Content should be truncated
        assert!(data.content.len() <= 50 + 20); // +20 for truncation message
        assert!(data.content.contains("truncated"));
    }
}
