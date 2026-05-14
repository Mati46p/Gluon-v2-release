//! MCP Server Implementation
//!
//! Implements Model Context Protocol server that exposes Gluon's tools to external MCP clients.
//!
//! ## Supported Methods
//!
//! - `initialize` - Handshake with client
//! - `tools/list` - Return list of available tools
//! - `tools/call` - Execute a tool
//!
//! ## Usage
//!
//! ```bash
//! gluon-desktop --mcp
//! ```

use crate::interface::executor::ToolExecutor;
use crate::interface::mcp::mapper::{tools_to_mcp_list, McpToolDefinition};
use crate::interface::mcp::transport::StdioTransport;
use crate::interface::protocol::{error_codes, JsonRpcRequest, JsonRpcResponse};
use crate::interface::registry::ToolRegistry;
use crate::interface::types::ToolContext;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

/// MCP Server
///
/// Exposes Gluon tools via MCP protocol over stdio.
pub struct McpServer {
    registry: Arc<ToolRegistry>,
    executor: ToolExecutor,
    transport: StdioTransport,
    server_info: ServerInfo,
}

/// Server information returned in initialize response
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServerInfo {
    name: String,
    version: String,
}

/// Initialize request parameters
#[derive(Debug, Deserialize)]
struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,

    #[serde(rename = "clientInfo")]
    client_info: ClientInfo,
}

/// Client information
#[derive(Debug, Deserialize)]
struct ClientInfo {
    name: String,
    version: String,
}

/// Initialize response
#[derive(Debug, Serialize)]
struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,

    #[serde(rename = "serverInfo")]
    server_info: ServerInfo,

    capabilities: ServerCapabilities,
}

/// Server capabilities
#[derive(Debug, Serialize)]
struct ServerCapabilities {
    tools: ToolsCapability,
}

#[derive(Debug, Serialize)]
struct ToolsCapability {
    /// Whether server supports listing tools
    #[serde(rename = "listChanged")]
    list_changed: bool,
}

/// tools/call request parameters
#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,

    #[serde(default)]
    arguments: Option<Value>,
}

impl McpServer {
    /// Create a new MCP server
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        let executor = ToolExecutor::new(registry.clone());

        Self {
            registry,
            executor,
            transport: StdioTransport::new(),
            server_info: ServerInfo {
                name: "gluon-mcp-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }

    /// Run the MCP server (blocking)
    ///
    /// This is the main event loop that reads requests from stdin and writes responses to stdout.
    pub async fn run(mut self) -> Result<(), String> {
        eprintln!("[MCP Server] Starting Gluon MCP Server v{}", self.server_info.version);
        eprintln!("[MCP Server] Waiting for requests on stdin...");

        // Main event loop
        while let Some(request) = self.transport.receive().await {
            eprintln!("[MCP Server] Received request: method={}", request.method);

            let response = self.handle_request(request).await;

            if let Err(e) = self.transport.send(response).await {
                eprintln!("[MCP Server] Failed to send response: {}", e);
                return Err(e);
            }
        }

        eprintln!("[MCP Server] Stdin closed, shutting down");
        Ok(())
    }

    /// Handle a single JSON-RPC request
    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            _ => JsonRpcResponse::error(
                request.id,
                error_codes::METHOD_NOT_FOUND,
                &format!("Method not found: {}", request.method),
            ),
        }
    }

    /// Handle initialize request
    async fn handle_initialize(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        // Parse parameters (optional - client info)
        let params: Option<InitializeParams> = request
            .params
            .and_then(|p| serde_json::from_value(p).ok());

        if let Some(params) = params {
            eprintln!(
                "[MCP Server] Client: {} v{}",
                params.client_info.name, params.client_info.version
            );
        }

        // Build response
        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(), // MCP protocol version
            server_info: self.server_info.clone(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability {
                    list_changed: false, // We don't support dynamic tool updates yet
                },
            },
        };

        JsonRpcResponse::success(request.id, result)
    }

    /// Handle tools/list request
    async fn handle_tools_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        // Generate manifest from registry
        let manifest = self.registry.generate_manifest();

        eprintln!(
            "[MCP Server] Listing {} tools",
            manifest.total_count
        );

        // Convert to MCP format
        let mcp_list = tools_to_mcp_list(manifest.tools);

        JsonRpcResponse::success(request.id, mcp_list)
    }

    /// Handle tools/call request
    async fn handle_tools_call(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        // Parse tool call parameters
        let params: ToolCallParams = match request.params {
            Some(p) => match serde_json::from_value(p) {
                Ok(params) => params,
                Err(e) => {
                    return JsonRpcResponse::error(
                        request.id,
                        error_codes::INVALID_PARAMS,
                        &format!("Invalid parameters: {}", e),
                    );
                }
            },
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters",
                );
            }
        };

        eprintln!("[MCP Server] Calling tool: {}", params.name);

        // Build tool execution request
        let tool_request = JsonRpcRequest::new(
            request.id.clone(),
            params.name,
            params.arguments,
        );

        // Create tool context (default for MCP server)
        let context = ToolContext::default_for_testing();

        // Execute tool via ToolExecutor
        let response = self.executor.execute_call(tool_request, &context).await;

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::tools::ReadFileTool;

    #[test]
    fn test_server_creation() {
        let mut registry = ToolRegistry::new();
        registry.register_local(Arc::new(ReadFileTool::new()));
        let registry = Arc::new(registry);

        let _server = McpServer::new(registry);
        // Should not panic
    }

    #[test]
    fn test_server_info() {
        let registry = Arc::new(ToolRegistry::new());
        let server = McpServer::new(registry);

        assert_eq!(server.server_info.name, "gluon-mcp-server");
        assert!(!server.server_info.version.is_empty());
    }

    #[tokio::test]
    async fn test_initialize_response() {
        let registry = Arc::new(ToolRegistry::new());
        let server = McpServer::new(registry);

        let request = JsonRpcRequest::without_params(1, "initialize");
        let response = server.handle_initialize(request).await;

        assert!(response.is_success());

        let result: InitializeResult = serde_json::from_value(response.result.unwrap()).unwrap();
        assert_eq!(result.server_info.name, "gluon-mcp-server");
        assert_eq!(result.protocol_version, "2024-11-05");
    }

    #[tokio::test]
    async fn test_tools_list_response() {
        let mut registry = ToolRegistry::new();
        registry.register_local(Arc::new(ReadFileTool::new()));
        let registry = Arc::new(registry);

        let server = McpServer::new(registry);

        let request = JsonRpcRequest::without_params(1, "tools/list");
        let response = server.handle_tools_list(request).await;

        assert!(response.is_success());

        let list: crate::interface::mcp::mapper::McpToolsListResponse =
            serde_json::from_value(response.result.unwrap()).unwrap();

        assert_eq!(list.tools.len(), 1);
        assert_eq!(list.tools[0].name, "gluon.read_file");
    }

    #[tokio::test]
    async fn test_method_not_found() {
        let registry = Arc::new(ToolRegistry::new());
        let server = McpServer::new(registry);

        let request = JsonRpcRequest::without_params(1, "unknown/method");
        let response = server.handle_request(request).await;

        assert!(response.is_error());
        assert_eq!(response.error.unwrap().code, error_codes::METHOD_NOT_FOUND);
    }
}
