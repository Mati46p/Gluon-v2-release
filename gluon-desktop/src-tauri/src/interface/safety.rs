//! Safety Middleware for Dangerous Operations
//!
//! Provides confirmation flow for tools that could cause:
//! - Data loss (file deletion, database drops)
//! - Security risks (shell command execution, network requests)
//! - System modifications (process termination, registry changes)

use super::types::{GTool, ToolContext};
use serde_json::Value;
use std::sync::Arc;

/// Safety middleware for tool execution
///
/// Responsible for:
/// - Checking if tools require confirmation
/// - Invoking confirmation callbacks
/// - Denying operations if no callback is available
pub struct SafetyMiddleware;

impl SafetyMiddleware {
    /// Check if tool requires confirmation and request it if needed
    ///
    /// # Arguments
    /// * `tool` - Tool to check
    /// * `params` - Parameters being passed to the tool
    /// * `context` - Execution context (contains confirmation callback)
    ///
    /// # Returns
    /// * `Ok(true)` - Tool is safe or user approved
    /// * `Ok(false)` - Tool requires confirmation and user denied
    /// * `Err(String)` - Confirmation check failed
    pub async fn check_and_confirm(
        tool: &Arc<dyn GTool>,
        params: &Value,
        context: &ToolContext,
    ) -> Result<bool, String> {
        // Check if tool requires confirmation
        if !tool.requires_confirmation() {
            // Tool is safe, no confirmation needed
            return Ok(true);
        }

        // Tool is dangerous - need confirmation
        if let Some(ref callback) = context.confirm_callback {
            // Callback available - ask user
            let message = Self::format_confirmation_message(tool, params);
            Ok(callback(&message).await)
        } else {
            // No callback - automatically deny dangerous operations
            // This is safe-by-default behavior
            Ok(false)
        }
    }

    /// Format a user-friendly confirmation message
    ///
    /// # Example Output
    /// ```text
    /// ⚠️  DANGEROUS OPERATION REQUIRES CONFIRMATION
    ///
    /// Tool: gluon.run_command
    /// Description: Execute shell command
    ///
    /// Parameters:
    /// {
    ///   "command": "rm -rf /"
    /// }
    ///
    /// This operation could cause data loss or security risks.
    /// Do you want to proceed?
    /// ```
    fn format_confirmation_message(tool: &Arc<dyn GTool>, params: &Value) -> String {
        let params_str = serde_json::to_string_pretty(params).unwrap_or_else(|_| "{}".to_string());

        format!(
            "⚠️  DANGEROUS OPERATION REQUIRES CONFIRMATION\n\n\
             Tool: {}\n\
             Description: {}\n\n\
             Parameters:\n\
             {}\n\n\
             This operation could cause data loss or security risks.\n\
             Do you want to proceed?",
            tool.name(),
            tool.description(),
            params_str
        )
    }

    /// Check if a tool is dangerous (requires confirmation)
    pub fn is_dangerous(tool: &Arc<dyn GTool>) -> bool {
        tool.requires_confirmation()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::types::{ToolResult, ToolOutput};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};

    // Mock safe tool
    struct SafeTool;

    #[async_trait]
    impl GTool for SafeTool {
        fn name(&self) -> &str {
            "test.safe"
        }

        fn description(&self) -> &str {
            "A safe tool"
        }

        fn parameters_schema(&self) -> Value {
            serde_json::json!({})
        }

        async fn execute(&self, _params: Value, _context: &ToolContext) -> ToolResult {
            Ok(ToolOutput {
                result: serde_json::json!({}),
                summary: "Success".to_string(),
                artifacts: vec![],
            })
        }

        fn requires_confirmation(&self) -> bool {
            false
        }
    }

    // Mock dangerous tool
    struct DangerousTool;

    #[async_trait]
    impl GTool for DangerousTool {
        fn name(&self) -> &str {
            "test.dangerous"
        }

        fn description(&self) -> &str {
            "A dangerous tool"
        }

        fn parameters_schema(&self) -> Value {
            serde_json::json!({})
        }

        async fn execute(&self, _params: Value, _context: &ToolContext) -> ToolResult {
            Ok(ToolOutput {
                result: serde_json::json!({}),
                summary: "Success".to_string(),
                artifacts: vec![],
            })
        }

        fn requires_confirmation(&self) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn test_safe_tool_no_confirmation_needed() {
        let tool = Arc::new(SafeTool);
        let context = ToolContext::default_for_testing();
        let params = serde_json::json!({});

        let result = SafetyMiddleware::check_and_confirm(&tool, &params, &context).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true); // Approved (no confirmation needed)
    }

    #[tokio::test]
    async fn test_dangerous_tool_no_callback_denied() {
        let tool = Arc::new(DangerousTool);
        let context = ToolContext::default_for_testing(); // No callback
        let params = serde_json::json!({});

        let result = SafetyMiddleware::check_and_confirm(&tool, &params, &context).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false); // Denied (no callback = auto-deny)
    }

    #[tokio::test]
    async fn test_dangerous_tool_callback_approves() {
        let tool = Arc::new(DangerousTool);

        let callback_invoked = Arc::new(AtomicBool::new(false));
        let callback_invoked_clone = callback_invoked.clone();

        let mut context = ToolContext::default_for_testing();
        context.confirm_callback = Some(Arc::new(move |_msg: &str| {
            callback_invoked_clone.store(true, Ordering::SeqCst);
            Box::pin(async { true }) // User approves
        }));

        let params = serde_json::json!({"command": "rm -rf /"});

        let result = SafetyMiddleware::check_and_confirm(&tool, &params, &context).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true); // Approved
        assert!(callback_invoked.load(Ordering::SeqCst)); // Callback was invoked
    }

    #[tokio::test]
    async fn test_dangerous_tool_callback_denies() {
        let tool = Arc::new(DangerousTool);

        let mut context = ToolContext::default_for_testing();
        context.confirm_callback = Some(Arc::new(move |_msg: &str| {
            Box::pin(async { false }) // User denies
        }));

        let params = serde_json::json!({});

        let result = SafetyMiddleware::check_and_confirm(&tool, &params, &context).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false); // Denied
    }

    #[test]
    fn test_is_dangerous() {
        let safe_tool = Arc::new(SafeTool);
        let dangerous_tool = Arc::new(DangerousTool);

        assert!(!SafetyMiddleware::is_dangerous(&safe_tool));
        assert!(SafetyMiddleware::is_dangerous(&dangerous_tool));
    }

    #[test]
    fn test_format_confirmation_message() {
        let tool = Arc::new(DangerousTool);
        let params = serde_json::json!({"command": "rm -rf /"});

        let message = SafetyMiddleware::format_confirmation_message(&tool, &params);

        assert!(message.contains("⚠️  DANGEROUS OPERATION"));
        assert!(message.contains("test.dangerous"));
        assert!(message.contains("A dangerous tool"));
        assert!(message.contains("rm -rf"));
    }
}
