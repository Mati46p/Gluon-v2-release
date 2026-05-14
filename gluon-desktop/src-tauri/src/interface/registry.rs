//! Tool Registry - Central hub for all available tools
//!
//! The registry maintains:
//! - Local tools (built-in Gluon tools implemented in Rust)
//! - Remote tools (from external MCP servers)
//! - Tool manifest generation
//! - Tool lookup and execution

use super::types::{GTool, ToolCategory};
use super::mcp::client::{McpClient, McpServerConfig};
use super::tools::RemoteToolProxy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Central registry of all available tools
///
/// Thread-safe registry that manages both local and remote tools.
/// Tools can be registered at startup or dynamically added at runtime.
pub struct ToolRegistry {
    /// Local tools (Rust implementations)
    local_tools: Arc<RwLock<HashMap<String, Arc<dyn GTool>>>>,

    /// Connected MCP servers (server_id -> client)
    remote_servers: Arc<RwLock<HashMap<String, RemoteServerInfo>>>,
}

/// Information about a connected MCP server
struct RemoteServerInfo {
    server_id: String,
    client: Arc<McpClient>,
}

impl ToolRegistry {
    /// Create a new tool registry WITHOUT built-in tools
    ///
    /// Use `new_with_builtin_tools()` instead for production use.
    pub fn new() -> Self {
        Self {
            local_tools: Arc::new(RwLock::new(HashMap::new())),
            remote_servers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new tool registry WITH built-in tools (recommended)
    ///
    /// This registers all standard Gluon tools:
    /// - gluon.read_file
    /// - gluon.write_file
    /// - gluon.get_manifest
    /// - gluon.apply_patch
    /// - gluon.run_command
    /// - gluon.search_code
    pub fn new_with_builtin_tools() -> Arc<Self> {
        use crate::interface::tools::{
            ReadFileTool, WriteFileTool, GetManifestTool,
            ApplyPatchTool, RunCommandTool, SearchCodeTool,
        };

        // Step 1: Create empty registry
        let registry = Arc::new(Self::new());

        // Step 2: Register tools that don't need registry reference
        // Note: We use &self (interior mutability), so we don't need &mut registry
        registry.register_local(Arc::new(ReadFileTool::new()));
        registry.register_local(Arc::new(WriteFileTool::new()));
        registry.register_local(Arc::new(ApplyPatchTool::new()));
        registry.register_local(Arc::new(RunCommandTool::new()));
        registry.register_local(Arc::new(SearchCodeTool::new()));

        // Step 3: Register GetManifestTool (needs registry reference for introspection)
        // We pass a clone of the Arc to the tool, while registering it to the original Arc
        registry.register_local(Arc::new(GetManifestTool::new(registry.clone())));

        registry
    }

    /// Register all built-in Gluon tools
    ///
    /// Note: This is for future expansion. Currently all tools are registered
    /// in new_with_builtin_tools() because GetManifestTool needs special handling.
    #[allow(dead_code)]
    fn register_builtin_tools(&self) {
        // Future tools will be added here:
        // - ApplyPatchTool
        // - SearchCodeTool
        // - RunCommandTool
        // - TakeScreenshotTool
    }

    /// Register a local tool
    ///
    /// # Example
    /// ```ignore
    /// let tool = ReadFileTool::new();
    /// registry.register_local(Arc::new(tool));
    /// ```
    // CHANGED: &mut self -> &self
    pub fn register_local(&self, tool: Arc<dyn GTool>) {
        let mut tools = self.local_tools.write().unwrap();
        tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name (local only for now)
    ///
    /// Returns None if tool not found.
    pub fn get(&self, name: &str) -> Option<Arc<dyn GTool>> {
        let tools = self.local_tools.read().unwrap();
        tools.get(name).cloned()
    }

    /// Check if a tool exists
    pub fn has_tool(&self, name: &str) -> bool {
        let tools = self.local_tools.read().unwrap();
        tools.contains_key(name)
    }

    /// Get all tool names
    pub fn list_tool_names(&self) -> Vec<String> {
        let tools = self.local_tools.read().unwrap();
        tools.keys().cloned().collect()
    }

    /// Connect to an external MCP server and register its tools
    ///
    /// This will:
    /// 1. Spawn the MCP server process
    /// 2. Retrieve all available tools
    /// 3. Create RemoteToolProxy for each tool
    /// 4. Register proxies in the tool registry
    ///
    /// # Example
    /// ```ignore
    /// let config = McpServerConfig {
    ///     id: "filesystem".to_string(),
    ///     command: "mcp-server-filesystem".to_string(),
    ///     args: vec!["--root".to_string(), "/project".to_string()],
    ///     env: HashMap::new(),
    /// };
    ///
    /// registry.connect_mcp_server(config).await?;
    /// ```
    // CHANGED: &mut self -> &self
    pub async fn connect_mcp_server(&self, config: McpServerConfig) -> Result<(), String> {
        eprintln!("[ToolRegistry] Connecting to MCP server: {}", config.id);

        // 1. Connect to the MCP server
        let client = McpClient::connect(config.clone()).await?;

        // 2. List all tools from the server
        let mcp_tools = client.list_tools().await?;

        eprintln!("[ToolRegistry] Found {} tools from server {}",
                  mcp_tools.len(), config.id);

        // 3. Create RemoteToolProxy for each tool and register
        {
            let mut tools = self.local_tools.write().unwrap();

            for tool_def in mcp_tools {
                let proxy = Arc::new(RemoteToolProxy::new(
                    config.id.clone(),
                    tool_def.name.clone(),
                    tool_def.description,
                    tool_def.input_schema,
                    client.clone(),
                ));

                tools.insert(tool_def.name.clone(), proxy as Arc<dyn GTool>);

                eprintln!("[ToolRegistry] Registered remote tool: {}", tool_def.name);
            }
        }

        // 4. Store server info
        {
            let mut servers = self.remote_servers.write().unwrap();
            servers.insert(config.id.clone(), RemoteServerInfo {
                server_id: config.id.clone(),
                client,
            });
        }

        eprintln!("[ToolRegistry] Successfully connected to MCP server: {}", config.id);

        Ok(())
    }

    /// List all connected MCP server IDs
    pub fn list_connected_servers(&self) -> Vec<String> {
        let servers = self.remote_servers.read().unwrap();
        servers.keys().cloned().collect()
    }

    /// Generate tool manifest (for LLM consumption)
    ///
    /// The manifest contains all available tools with their schemas.
    /// This is used by:
    /// - LLMs to understand what tools are available
    /// - MCP servers to advertise tools
    /// - Frontend for tool browsing
    pub fn generate_manifest(&self) -> ManifestOutput {
        let tools_lock = self.local_tools.read().unwrap();

        let tools: Vec<ToolDefinition> = tools_lock
            .values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters_schema(),
                category: tool.category(),
                requires_confirmation: tool.requires_confirmation(),
                source: ToolSource::Local,
            })
            .collect();

        ManifestOutput {
            version: "1.0.0".to_string(),
            total_count: tools.len(),
            tools,
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool manifest output
///
/// Complete list of all available tools with metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ManifestOutput {
    /// Manifest format version
    pub version: String,

    /// Total number of tools
    pub total_count: usize,

    /// List of tool definitions
    pub tools: Vec<ToolDefinition>,
}

/// Tool definition in manifest
///
/// Contains all metadata needed for LLM to understand and use the tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolDefinition {
    /// Unique tool name (e.g., "gluon.read_file")
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// JSON Schema for parameters
    pub parameters: Value,

    /// Tool category
    pub category: ToolCategory,

    /// Whether confirmation is required
    pub requires_confirmation: bool,

    /// Tool source (local or remote)
    pub source: ToolSource,
}

/// Tool source indicator
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ToolSource {
    /// Built-in Gluon tool
    Local,

    /// Remote tool from MCP server
    Remote {
        /// ID of the MCP server
        server_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::types::{ToolContext, ToolError, ToolOutput, ToolResult};
    use async_trait::async_trait;
    use serde_json::json;

    // Mock tool for testing
    struct MockTool {
        name: String,
    }

    #[async_trait]
    impl GTool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A mock tool for testing"
        }

        fn parameters_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "test": {"type": "string"}
                }
            })
        }

        fn category(&self) -> ToolCategory {
            ToolCategory::System
        }

        fn requires_confirmation(&self) -> bool {
            false
        }

        async fn execute(&self, _params: Value, _context: &ToolContext) -> ToolResult {
            Ok(ToolOutput {
                result: json!({"status": "ok"}),
                summary: "Mock execution".to_string(),
                artifacts: vec![],
            })
        }
    }

    #[test]
    fn test_registry_creation() {
        let registry = ToolRegistry::new();
        // Should be empty initially (no built-in tools yet)
        assert_eq!(registry.list_tool_names().len(), 0);
    }

    #[test]
    fn test_register_and_get_tool() {
        // No mut needed anymore
        let registry = ToolRegistry::new();

        let tool = Arc::new(MockTool {
            name: "test.mock_tool".to_string(),
        });

        registry.register_local(tool.clone());

        // Should be able to retrieve it
        assert!(registry.has_tool("test.mock_tool"));
        assert!(registry.get("test.mock_tool").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_list_tool_names() {
        let registry = ToolRegistry::new();

        registry.register_local(Arc::new(MockTool {
            name: "tool1".to_string(),
        }));
        registry.register_local(Arc::new(MockTool {
            name: "tool2".to_string(),
        }));

        let names = registry.list_tool_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"tool1".to_string()));
        assert!(names.contains(&"tool2".to_string()));
    }

    #[test]
    fn test_manifest_generation() {
        let registry = ToolRegistry::new();

        registry.register_local(Arc::new(MockTool {
            name: "test.tool1".to_string(),
        }));
        registry.register_local(Arc::new(MockTool {
            name: "test.tool2".to_string(),
        }));

        let manifest = registry.generate_manifest();

        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.total_count, 2);
        assert_eq!(manifest.tools.len(), 2);

        // Check first tool
        let tool_def = &manifest.tools[0];
        // Note: Hashmap order is not guaranteed, so we can't hard check index 0 is tool1
        assert!(tool_def.name.starts_with("test."));
        assert_eq!(tool_def.description, "A mock tool for testing");
        assert!(!tool_def.requires_confirmation);
        assert!(matches!(tool_def.source, ToolSource::Local));
    }

    #[test]
    fn test_manifest_serialization() {
        let registry = ToolRegistry::new();

        registry.register_local(Arc::new(MockTool {
            name: "test.tool".to_string(),
        }));

        let manifest = registry.generate_manifest();
        let json = serde_json::to_string_pretty(&manifest).unwrap();

        // Should be valid JSON
        assert!(json.contains("\"version\""));
        assert!(json.contains("\"total_count\""));
        assert!(json.contains("\"tools\""));
        assert!(json.contains("test.tool"));
    }

    #[test]
    fn test_default_registry() {
        let registry = ToolRegistry::default();
        // Should be identical to new()
        assert_eq!(registry.list_tool_names().len(), 0);
    }
}