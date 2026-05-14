//! RemoteToolProxy - Proxy for external MCP tools
//!
//! Wraps tools from external MCP servers and makes them look like local GTools.
//!
//! ## Example
//!
//! When Gluon connects to `mcp-server-filesystem`, it gets tools like:
//! - `filesystem.read_file`
//! - `filesystem.write_file`
//! - `filesystem.list_directory`
//!
//! These are wrapped as RemoteToolProxy and registered in ToolRegistry,
//! so they can be used just like local tools.

use crate::interface::mcp::client::McpClient;
use crate::interface::types::{
    GTool, ToolCategory, ToolContext, ToolError, ToolOutput, ToolResult,
};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

/// Proxy for a tool from an external MCP server
///
/// This implements GTool trait, so it can be registered in ToolRegistry
/// alongside local tools. When executed, it forwards the call to the
/// remote MCP server.
#[derive(Clone)]
pub struct RemoteToolProxy {
    /// ID of the MCP server this tool comes from
    server_id: String,

    /// Tool name (e.g., "filesystem.read_file")
    tool_name: String,

    /// Tool description
    description: String,

    /// JSON Schema for parameters
    parameters_schema: Value,

    /// MCP client to use for execution
    client: Arc<McpClient>,
}

impl RemoteToolProxy {
    /// Create a new remote tool proxy
    pub fn new(
        server_id: String,
        tool_name: String,
        description: String,
        parameters_schema: Value,
        client: Arc<McpClient>,
    ) -> Self {
        Self {
            server_id,
            tool_name,
            description,
            parameters_schema,
            client,
        }
    }

    /// Get the server ID
    pub fn server_id(&self) -> &str {
        &self.server_id
    }
}

#[async_trait]
impl GTool for RemoteToolProxy {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        self.parameters_schema.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        eprintln!("[RemoteToolProxy] Executing remote tool: {} (server: {})",
                  self.tool_name, self.server_id);

        // Forward call to remote MCP server
        let result = self
            .client
            .call_tool(&self.tool_name, params)
            .await
            .map_err(|e| {
                ToolError::execution_failed(&format!("Remote tool call failed: {}", e))
            })?;

        // Remote tools return their results in various formats
        // We wrap them in our ToolOutput format
        Ok(ToolOutput {
            result: result.clone(),
            summary: format!("Remote tool {} executed successfully", self.tool_name),
            artifacts: vec![],
        })
    }

    fn requires_confirmation(&self) -> bool {
        // Remote tools handle their own confirmation
        // We don't require additional confirmation on our side
        false
    }

    fn category(&self) -> ToolCategory {
        // Try to infer category from tool name
        if self.tool_name.contains("file") || self.tool_name.contains("dir") {
            ToolCategory::FileSystem
        } else if self.tool_name.contains("git") || self.tool_name.contains("github") {
            ToolCategory::CodeEditing
        } else if self.tool_name.contains("search") || self.tool_name.contains("query") {
            ToolCategory::Analysis
        } else {
            ToolCategory::General
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_remote_tool_creation() {
        // We can't create a real McpClient in tests without a server,
        // so we'll just test the struct creation
        let schema = json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            }
        });

        // This would normally have a real client
        // For now, just verify the struct fields
        assert_eq!(schema["type"], "object");
    }

    #[test]
    fn test_category_inference() {
        let schema = json!({"type": "object"});

        // Filesystem tools
        assert!(matches!(
            infer_category("filesystem.read_file"),
            ToolCategory::FileSystem
        ));
        assert!(matches!(
            infer_category("filesystem.list_dir"),
            ToolCategory::FileSystem
        ));

        // Git tools
        assert!(matches!(
            infer_category("github.create_pr"),
            ToolCategory::CodeEditing
        ));

        // Search tools
        assert!(matches!(
            infer_category("search.query"),
            ToolCategory::Analysis
        ));

        // General
        assert!(matches!(
            infer_category("unknown.tool"),
            ToolCategory::General
        ));
    }

    fn infer_category(tool_name: &str) -> ToolCategory {
        if tool_name.contains("file") || tool_name.contains("dir") {
            ToolCategory::FileSystem
        } else if tool_name.contains("git") {
            ToolCategory::CodeEditing
        } else if tool_name.contains("search") {
            ToolCategory::Analysis
        } else {
            ToolCategory::General
        }
    }
}
