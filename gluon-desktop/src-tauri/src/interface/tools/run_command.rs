//! RunCommandTool - Execute shell commands
//!
//! This tool allows LLMs to execute shell commands in the system.
//! It ALWAYS requires user confirmation for security.
//!
//! ## Security Considerations
//!
//! - **Requires confirmation**: User must approve each command
//! - **Working directory**: Commands run in specified directory
//! - **Timeout**: Commands have maximum execution time
//! - **Output limits**: stdout/stderr are truncated if too large
//!
//! ## Example Usage
//!
//! ```json
//! {
//!   "command": "npm install",
//!   "working_dir": "/project",
//!   "timeout_seconds": 60
//! }
//! ```

use crate::interface::types::{
    GTool, ToolCategory, ToolContext, ToolError, ToolOutput, ToolResult,
};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Maximum output size (to prevent memory issues)
const MAX_OUTPUT_SIZE: usize = 10_000_000; // 10 MB

/// Default timeout for commands
const DEFAULT_TIMEOUT_SECONDS: u64 = 120;

/// RunCommand tool implementation
///
/// Executes shell commands with safety checks and user confirmation.
pub struct RunCommandTool;

impl RunCommandTool {
    pub fn new() -> Self {
        Self
    }
}

/// Parameters for RunCommandTool
#[derive(Debug, Deserialize, JsonSchema)]
struct RunCommandParams {
    /// The shell command to execute
    command: String,

    /// Optional working directory (defaults to context working_dir)
    #[serde(skip_serializing_if = "Option::is_none")]
    working_dir: Option<String>,

    /// Timeout in seconds (default: 120)
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,

    /// Whether to capture stdout (default: true)
    #[serde(default = "default_true")]
    capture_stdout: bool,

    /// Whether to capture stderr (default: true)
    #[serde(default = "default_true")]
    capture_stderr: bool,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECONDS
}

fn default_true() -> bool {
    true
}

/// Result of running a command
#[derive(Debug, Serialize)]
struct RunCommandResult {
    /// Exit code of the command
    exit_code: i32,

    /// Whether the command succeeded (exit code 0)
    success: bool,

    /// Captured stdout (truncated if too large)
    stdout: String,

    /// Captured stderr (truncated if too large)
    stderr: String,

    /// Whether output was truncated
    truncated: bool,

    /// Execution time in milliseconds
    execution_time_ms: u64,
}

#[async_trait]
impl GTool for RunCommandTool {
    fn name(&self) -> &str {
        "gluon.run_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command in the system. \
         Use this to run build commands, tests, git operations, or other system tasks. \
         IMPORTANT: This requires user confirmation. Be clear about what the command does. \
         Examples: 'npm install', 'cargo build', 'git status'"
    }

    fn parameters_schema(&self) -> Value {
        let schema = schemars::schema_for!(RunCommandParams);
        serde_json::to_value(schema).unwrap_or(json!({}))
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        eprintln!("[RunCommandTool] Executing command");

        // 1. Parse parameters
        let params: RunCommandParams = serde_json::from_value(params)
            .map_err(|e| ToolError::invalid_params(&format!("Invalid parameters: {}", e)))?;

        // 2. Determine working directory
        let working_dir = if let Some(ref dir) = params.working_dir {
            PathBuf::from(dir)
        } else {
            context.working_dir.clone()
        };

        eprintln!("[RunCommandTool] Command: {}", params.command);
        eprintln!(
            "[RunCommandTool] Working directory: {}",
            working_dir.display()
        );

        // 3. Validate working directory exists
        if !working_dir.exists() {
            return Err(ToolError::execution_failed(&format!(
                "Working directory does not exist: {}",
                working_dir.display()
            )));
        }

        // 4. Parse command (handle both Unix and Windows)
        let (shell, shell_arg) = if cfg!(target_os = "windows") {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };

        // 5. Execute command with timeout
        let start_time = std::time::Instant::now();

        let mut child = Command::new(shell)
            .arg(shell_arg)
            .arg(&params.command)
            .current_dir(&working_dir)
            .stdout(if params.capture_stdout {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(if params.capture_stderr {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .spawn()
            .map_err(|e| ToolError::execution_failed(&format!("Failed to spawn command: {}", e)))?;

        // 6. Read stdout and stderr with size limits
        let stdout_task = if params.capture_stdout {
            let stdout = child.stdout.take().unwrap();
            Some(tokio::spawn(async move {
                Self::read_stream(stdout, MAX_OUTPUT_SIZE).await
            }))
        } else {
            None
        };

        let stderr_task = if params.capture_stderr {
            let stderr = child.stderr.take().unwrap();
            Some(tokio::spawn(async move {
                Self::read_stream(stderr, MAX_OUTPUT_SIZE).await
            }))
        } else {
            None
        };

        // 7. Wait for command with timeout
        let timeout = Duration::from_secs(params.timeout_seconds);
        let exit_status = tokio::time::timeout(timeout, child.wait())
            .await
            .map_err(|_| {
                // Timeout occurred - try to kill the process
                let _ = child.start_kill();
                ToolError::execution_failed(&format!(
                    "Command timed out after {} seconds",
                    params.timeout_seconds
                ))
            })?
            .map_err(|e| ToolError::execution_failed(&format!("Command failed: {}", e)))?;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        // 8. Collect outputs
        let (stdout, stdout_truncated) = if let Some(task) = stdout_task {
            task.await.unwrap_or_else(|_| (String::new(), false))
        } else {
            (String::new(), false)
        };

        let (stderr, stderr_truncated) = if let Some(task) = stderr_task {
            task.await.unwrap_or_else(|_| (String::new(), false))
        } else {
            (String::new(), false)
        };

        let truncated = stdout_truncated || stderr_truncated;

        // 9. Build result
        let exit_code = exit_status.code().unwrap_or(-1);
        let success = exit_code == 0;

        let result = RunCommandResult {
            exit_code,
            success,
            stdout,
            stderr,
            truncated,
            execution_time_ms,
        };

        let summary = if success {
            format!(
                "Command completed successfully in {}ms",
                execution_time_ms
            )
        } else {
            format!(
                "Command failed with exit code {} after {}ms",
                exit_code, execution_time_ms
            )
        };

        eprintln!("[RunCommandTool] {}", summary);

        Ok(ToolOutput {
            result: serde_json::to_value(result)?,
            summary,
            artifacts: vec![],
        })
    }

    fn requires_confirmation(&self) -> bool {
        true // ALWAYS require confirmation for security
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
}

impl RunCommandTool {
    /// Read from a stream with size limits
    async fn read_stream<R: tokio::io::AsyncRead + Unpin>(
        stream: R,
        max_size: usize,
    ) -> (String, bool) {
        let mut reader = BufReader::new(stream);
        let mut output = String::new();
        let mut line = String::new();
        let mut truncated = false;

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    if output.len() + line.len() > max_size {
                        truncated = true;
                        output.push_str(&line[..(max_size - output.len())]);
                        output.push_str("\n... (output truncated)");
                        break;
                    }
                    output.push_str(&line);
                }
                Err(_) => break,
            }
        }

        (output, truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_metadata() {
        let tool = RunCommandTool::new();

        assert_eq!(tool.name(), "gluon.run_command");
        assert!(tool.requires_confirmation());
        assert!(matches!(tool.category(), ToolCategory::System));
    }

    #[test]
    fn test_parameters_schema() {
        let tool = RunCommandTool::new();
        let schema = tool.parameters_schema();

        // Should have type = object
        assert_eq!(schema["type"], "object");

        // Should have 'command' property
        let properties = &schema["properties"];
        assert!(properties.is_object());
        assert!(properties["command"].is_object());
    }

    #[tokio::test]
    async fn test_simple_command() {
        let tool = RunCommandTool::new();
        let context = ToolContext::default_for_testing();

        // Simple echo command (cross-platform)
        let params = if cfg!(target_os = "windows") {
            json!({
                "command": "echo test",
                "timeout_seconds": 5
            })
        } else {
            json!({
                "command": "echo test",
                "timeout_seconds": 5
            })
        };

        let result = tool.execute(params, &context).await;

        // Should succeed
        assert!(result.is_ok());

        let output = result.unwrap();
        let result_data: RunCommandResult =
            serde_json::from_value(output.result).unwrap();

        assert!(result_data.success);
        assert_eq!(result_data.exit_code, 0);
        assert!(result_data.stdout.contains("test"));
    }

    #[tokio::test]
    async fn test_failing_command() {
        let tool = RunCommandTool::new();
        let context = ToolContext::default_for_testing();

        // Command that should fail
        let params = json!({
            "command": "exit 1",
            "timeout_seconds": 5
        });

        let result = tool.execute(params, &context).await;

        // Should execute but report failure
        assert!(result.is_ok());

        let output = result.unwrap();
        let result_data: RunCommandResult =
            serde_json::from_value(output.result).unwrap();

        assert!(!result_data.success);
        assert_eq!(result_data.exit_code, 1);
    }
}
