//! Tool Execution Engine
//!
//! Responsible for:
//! - Executing tool calls from JSON-RPC requests
//! - Applying safety middleware
//! - Converting results to JSON-RPC responses
//! - Error handling and logging

use super::protocol::{JsonRpcRequest, JsonRpcResponse, error_codes};
use super::registry::ToolRegistry;
use super::safety::SafetyMiddleware;
use super::types::{ToolContext, ToolErrorCode};
use serde_json::Value;
use std::sync::Arc;

/// Tool execution engine
///
/// Coordinates the execution of tool calls with safety checks.
pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
}

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    /// Execute a single JSON-RPC tool call
    ///
    /// # Arguments
    /// * `request` - JSON-RPC request containing method name and parameters
    /// * `context` - Tool execution context
    ///
    /// # Returns
    /// JSON-RPC response with either result or error
    pub async fn execute_call(
        &self,
        request: JsonRpcRequest,
        context: &ToolContext,
    ) -> JsonRpcResponse {
        // 1. Validate request
        if request.method.is_empty() {
            return JsonRpcResponse::error(
                request.id,
                error_codes::INVALID_REQUEST,
                "Method name is required",
            );
        }

        // 2. Get tool from registry
        let tool = match self.registry.get(&request.method) {
            Some(t) => t,
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    error_codes::METHOD_NOT_FOUND,
                    &format!("Tool not found: {}", request.method),
                );
            }
        };

        // 3. Safety check: Does tool require confirmation?
        if tool.requires_confirmation() {
            let params = request.params.clone().unwrap_or(Value::Null);
            match SafetyMiddleware::check_and_confirm(&tool, &params, context).await {
                Ok(false) => {
                    // User denied confirmation
                    return JsonRpcResponse::error(
                        request.id,
                        error_codes::SERVER_ERROR,
                        "User denied confirmation for dangerous operation",
                    );
                }
                Err(e) => {
                    // Confirmation check failed
                    return JsonRpcResponse::error(
                        request.id,
                        error_codes::INTERNAL_ERROR,
                        &format!("Confirmation check failed: {}", e),
                    );
                }
                Ok(true) => {
                    // User approved, continue
                }
            }
        }

        // 4. Execute tool
        let params = request.params.unwrap_or(Value::Null);
        match tool.execute(params, context).await {
            Ok(output) => {
                // Success: Return tool output
                JsonRpcResponse::success(request.id, output)
            }
            Err(err) => {
                // Failure: Convert ToolError to JSON-RPC error
                let code = match err.code {
                    ToolErrorCode::InvalidParameters => error_codes::INVALID_PARAMS,
                    ToolErrorCode::PermissionDenied => error_codes::SERVER_ERROR - 1, // -32001
                    ToolErrorCode::ResourceNotFound => error_codes::SERVER_ERROR - 2, // -32002
                    ToolErrorCode::Timeout => error_codes::SERVER_ERROR - 3,           // -32003
                    ToolErrorCode::UserCancelled => error_codes::SERVER_ERROR - 4,    // -32004
                    ToolErrorCode::ExecutionFailed => error_codes::SERVER_ERROR,      // -32000
                };

                if let Some(details) = err.details {
                    JsonRpcResponse::error_with_data(request.id, code, &err.message, details)
                } else {
                    JsonRpcResponse::error(request.id, code, &err.message)
                }
            }
        }
    }

    /// Execute multiple tool calls in sequence
    ///
    /// Returns a vector of responses matching the order of requests.
    pub async fn execute_batch(
        &self,
        requests: Vec<JsonRpcRequest>,
        context: &ToolContext,
    ) -> Vec<JsonRpcResponse> {
        let mut responses = Vec::new();

        for request in requests {
            let response = self.execute_call(request, context).await;
            responses.push(response);
        }

        responses
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::types::{GTool, ToolOutput, ToolResult, ToolError};
    use async_trait::async_trait;
    use serde_json::json;

    // Mock tool for testing
    struct MockSuccessTool;

    #[async_trait]
    impl GTool for MockSuccessTool {
        fn name(&self) -> &str {
            "test.success"
        }

        fn description(&self) -> &str {
            "Always succeeds"
        }

        fn parameters_schema(&self) -> Value {
            json!({"type": "object"})
        }

        async fn execute(&self, _params: Value, _context: &ToolContext) -> ToolResult {
            Ok(ToolOutput {
                result: json!({"status": "success"}),
                summary: "Operation successful".to_string(),
                artifacts: vec![],
            })
        }
    }

    struct MockFailureTool;

    #[async_trait]
    impl GTool for MockFailureTool {
        fn name(&self) -> &str {
            "test.failure"
        }

        fn description(&self) -> &str {
            "Always fails"
        }

        fn parameters_schema(&self) -> Value {
            json!({"type": "object"})
        }

        async fn execute(&self, _params: Value, _context: &ToolContext) -> ToolResult {
            Err(ToolError::execution_failed("Intentional failure"))
        }
    }

    fn setup_registry() -> Arc<ToolRegistry> {
        let mut registry = ToolRegistry::new();
        registry.register_local(Arc::new(MockSuccessTool));
        registry.register_local(Arc::new(MockFailureTool));
        Arc::new(registry)
    }

    #[tokio::test]
    async fn test_execute_successful_call() {
        let registry = setup_registry();
        let executor = ToolExecutor::new(registry);
        let context = ToolContext::default_for_testing();

        let request = JsonRpcRequest::without_params(1, "test.success");
        let response = executor.execute_call(request, &context).await;

        assert!(response.is_success());
        assert!(response.result.is_some());
    }

    #[tokio::test]
    async fn test_execute_failed_call() {
        let registry = setup_registry();
        let executor = ToolExecutor::new(registry);
        let context = ToolContext::default_for_testing();

        let request = JsonRpcRequest::without_params(1, "test.failure");
        let response = executor.execute_call(request, &context).await;

        assert!(response.is_error());
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, error_codes::SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_method_not_found() {
        let registry = setup_registry();
        let executor = ToolExecutor::new(registry);
        let context = ToolContext::default_for_testing();

        let request = JsonRpcRequest::without_params(1, "nonexistent.tool");
        let response = executor.execute_call(request, &context).await;

        assert!(response.is_error());
        assert_eq!(response.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn test_invalid_request_empty_method() {
        let registry = setup_registry();
        let executor = ToolExecutor::new(registry);
        let context = ToolContext::default_for_testing();

        let request = JsonRpcRequest::without_params(1, "");
        let response = executor.execute_call(request, &context).await;

        assert!(response.is_error());
        assert_eq!(response.error.unwrap().code, error_codes::INVALID_REQUEST);
    }

    #[tokio::test]
    async fn test_execute_batch() {
        let registry = setup_registry();
        let executor = ToolExecutor::new(registry);
        let context = ToolContext::default_for_testing();

        let requests = vec![
            JsonRpcRequest::without_params(1, "test.success"),
            JsonRpcRequest::without_params(2, "test.failure"),
            JsonRpcRequest::without_params(3, "test.success"),
        ];

        let responses = executor.execute_batch(requests, &context).await;

        assert_eq!(responses.len(), 3);
        assert!(responses[0].is_success());
        assert!(responses[1].is_error());
        assert!(responses[2].is_success());
    }
}
