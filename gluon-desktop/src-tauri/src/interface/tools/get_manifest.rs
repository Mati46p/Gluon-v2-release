//! GetManifestTool - Introspection meta-tool
//!
//! Returns list of all available tools with their schemas.
//! This enables LLMs to discover their own capabilities.
//!
//! ## Example Usage
//!
//! ```json
//! {
//!   "jsonrpc": "2.0",
//!   "method": "gluon.get_manifest",
//!   "id": 1,
//!   "params": {}
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
//!       "version": "1.0.0",
//!       "total_count": 3,
//!       "tools": [
//!         {
//!           "name": "gluon.read_file",
//!           "description": "Read contents of a file...",
//!           "parameters": { ... },
//!           "category": "file_system",
//!           "requires_confirmation": false,
//!           "source": "local"
//!         },
//!         ...
//!       ]
//!     },
//!     "summary": "Found 3 available tools"
//!   }
//! }
//! ```

use crate::interface::registry::ToolRegistry;
use crate::interface::types::{GTool, ToolCategory, ToolContext, ToolError, ToolOutput, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

/// GetManifest tool implementation
///
/// This is a meta-tool that provides introspection capabilities.
/// It needs access to the ToolRegistry to list all available tools.
pub struct GetManifestTool {
    /// Reference to the tool registry
    /// Note: We use Arc to share ownership with the registry itself
    registry: Arc<ToolRegistry>,
}

impl GetManifestTool {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }
}

/// Parameters for GetManifestTool (empty - no parameters needed)
#[derive(Debug, Deserialize, JsonSchema)]
struct GetManifestParams {
    /// Optional: Filter tools by category
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
}

#[async_trait]
impl GTool for GetManifestTool {
    fn name(&self) -> &str {
        "gluon.get_manifest"
    }

    fn description(&self) -> &str {
        "Returns list of all available tools with their schemas. \
         This allows you (the LLM) to discover what capabilities you have. \
         Optionally filter by category."
    }

    fn parameters_schema(&self) -> Value {
        let schema = schemars::schema_for!(GetManifestParams);
        serde_json::to_value(schema).unwrap_or(json!({}))
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        // 1. Parse parameters (optional category filter)
        let params: GetManifestParams = serde_json::from_value(params)
            .unwrap_or(GetManifestParams { category: None });

        // 2. Generate manifest from registry
        let manifest = self.registry.generate_manifest();

        // 3. Apply category filter if specified
        let filtered_manifest = if let Some(category_filter) = params.category {
            let filtered_tools: Vec<_> = manifest
                .tools
                .into_iter()
                .filter(|tool| {
                    let tool_category = format!("{:?}", tool.category).to_lowercase();
                    tool_category.contains(&category_filter.to_lowercase())
                })
                .collect();

            crate::interface::registry::ManifestOutput {
                version: manifest.version,
                total_count: filtered_tools.len(),
                tools: filtered_tools,
            }
        } else {
            manifest
        };

        // 4. Format summary
        let summary = format!("Found {} available tools", filtered_manifest.total_count);

        // 5. Return manifest
        Ok(ToolOutput {
            result: serde_json::to_value(filtered_manifest)?,
            summary,
            artifacts: vec![],
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Meta
    }

    fn requires_confirmation(&self) -> bool {
        false // Introspection is safe
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::tools::{ReadFileTool, WriteFileTool};
    use serde_json::json;

    #[tokio::test]
    async fn test_get_manifest_all_tools() {
        // Create registry with some tools
        let mut registry = ToolRegistry::new();
        registry.register_local(Arc::new(ReadFileTool::new()));
        registry.register_local(Arc::new(WriteFileTool::new()));

        let registry = Arc::new(registry);

        // Create GetManifestTool (it needs registry reference)
        let tool = GetManifestTool::new(registry.clone());
        let context = ToolContext::default_for_testing();

        // Execute with no parameters
        let params = json!({});
        let result = tool.execute(params, &context).await;

        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify summary
        assert!(output.summary.contains("Found"));
        assert!(output.summary.contains("tools"));

        // Parse manifest
        let manifest: crate::interface::registry::ManifestOutput =
            serde_json::from_value(output.result).unwrap();

        // Should have 2 tools (read + write)
        assert_eq!(manifest.total_count, 2);
        assert_eq!(manifest.tools.len(), 2);

        // Verify tools are present
        let tool_names: Vec<&str> = manifest.tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"gluon.read_file"));
        assert!(tool_names.contains(&"gluon.write_file"));
    }

    #[tokio::test]
    async fn test_get_manifest_filter_by_category() {
        let mut registry = ToolRegistry::new();
        registry.register_local(Arc::new(ReadFileTool::new()));
        registry.register_local(Arc::new(WriteFileTool::new()));

        let registry = Arc::new(registry);
        let tool = GetManifestTool::new(registry.clone());
        let context = ToolContext::default_for_testing();

        // Filter by "filesystem" category
        let params = json!({ "category": "filesystem" });
        let result = tool.execute(params, &context).await;

        assert!(result.is_ok());
        let output = result.unwrap();

        let manifest: crate::interface::registry::ManifestOutput =
            serde_json::from_value(output.result).unwrap();

        // Both read_file and write_file are filesystem tools
        assert_eq!(manifest.total_count, 2);
    }

    #[tokio::test]
    async fn test_get_manifest_no_confirmation_required() {
        let registry = Arc::new(ToolRegistry::new());
        let tool = GetManifestTool::new(registry);

        // Introspection is always safe
        assert!(!tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_get_manifest_category_is_meta() {
        let registry = Arc::new(ToolRegistry::new());
        let tool = GetManifestTool::new(registry);

        assert!(matches!(tool.category(), ToolCategory::Meta));
    }
}
