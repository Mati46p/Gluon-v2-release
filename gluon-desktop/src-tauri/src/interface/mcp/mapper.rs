//! GTool to MCP Tool Mapping
//!
//! Converts Gluon's GTool format to MCP's tool definition format.
//!
//! ## MCP Tool Format
//!
//! ```json
//! {
//!   "name": "tool_name",
//!   "description": "Human-readable description",
//!   "inputSchema": {
//!     "type": "object",
//!     "properties": { ... },
//!     "required": [ ... ]
//!   }
//! }
//! ```

use crate::interface::registry::ToolDefinition;
use crate::interface::types::GTool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// MCP Tool Definition
///
/// This is the format expected by MCP clients (Claude Desktop, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpToolDefinition {
    /// Tool name (e.g., "gluon.read_file")
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// JSON Schema for input parameters
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// MCP Tools List Response
///
/// Response to "tools/list" method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolsListResponse {
    pub tools: Vec<McpToolDefinition>,
}

/// Convert GTool to MCP tool definition
pub fn gtool_to_mcp(tool: &Arc<dyn GTool>) -> McpToolDefinition {
    McpToolDefinition {
        name: tool.name().to_string(),
        description: tool.description().to_string(),
        input_schema: tool.parameters_schema(),
    }
}

/// Convert ToolDefinition (from manifest) to MCP tool definition
pub fn tool_def_to_mcp(tool_def: &ToolDefinition) -> McpToolDefinition {
    McpToolDefinition {
        name: tool_def.name.clone(),
        description: tool_def.description.clone(),
        input_schema: tool_def.parameters.clone(),
    }
}

/// Convert multiple tool definitions to MCP list response
pub fn tools_to_mcp_list(tools: Vec<ToolDefinition>) -> McpToolsListResponse {
    McpToolsListResponse {
        tools: tools.into_iter().map(|t| tool_def_to_mcp(&t)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::tools::ReadFileTool;
    use crate::interface::types::ToolCategory;
    use crate::interface::registry::ToolSource;
    use serde_json::json;

    #[test]
    fn test_gtool_to_mcp() {
        let tool = Arc::new(ReadFileTool::new());
        let mcp_def = gtool_to_mcp(&tool);

        assert_eq!(mcp_def.name, "gluon.read_file");
        assert!(mcp_def.description.contains("Read"));
        assert!(mcp_def.input_schema.is_object());

        // Verify schema has expected structure
        let schema = mcp_def.input_schema.as_object().unwrap();
        assert!(schema.contains_key("type"));
    }

    #[test]
    fn test_tool_def_to_mcp() {
        let tool_def = ToolDefinition {
            name: "test.tool".to_string(),
            description: "Test tool description".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "param1": {"type": "string"}
                }
            }),
            category: ToolCategory::General,
            requires_confirmation: false,
            source: ToolSource::Local,
        };

        let mcp_def = tool_def_to_mcp(&tool_def);

        assert_eq!(mcp_def.name, "test.tool");
        assert_eq!(mcp_def.description, "Test tool description");
        assert!(mcp_def.input_schema.is_object());
    }

    #[test]
    fn test_tools_to_mcp_list() {
        let tools = vec![
            ToolDefinition {
                name: "tool1".to_string(),
                description: "First tool".to_string(),
                parameters: json!({"type": "object"}),
                category: ToolCategory::FileSystem,
                requires_confirmation: false,
                source: ToolSource::Local,
            },
            ToolDefinition {
                name: "tool2".to_string(),
                description: "Second tool".to_string(),
                parameters: json!({"type": "object"}),
                category: ToolCategory::Meta,
                requires_confirmation: false,
                source: ToolSource::Local,
            },
        ];

        let mcp_list = tools_to_mcp_list(tools);

        assert_eq!(mcp_list.tools.len(), 2);
        assert_eq!(mcp_list.tools[0].name, "tool1");
        assert_eq!(mcp_list.tools[1].name, "tool2");
    }

    #[test]
    fn test_mcp_serialization() {
        let mcp_def = McpToolDefinition {
            name: "test.tool".to_string(),
            description: "Test".to_string(),
            input_schema: json!({"type": "object"}),
        };

        let json = serde_json::to_string(&mcp_def).unwrap();

        // Should use camelCase for inputSchema
        assert!(json.contains("\"inputSchema\""));
        assert!(!json.contains("\"input_schema\""));
    }
}
