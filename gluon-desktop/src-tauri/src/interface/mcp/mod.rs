//! Model Context Protocol (MCP) Implementation
//!
//! This module implements MCP support for Gluon:
//! - **MCP Server (Phase 4 Step 2)** - Gluon as MCP Host ✅
//! - **MCP Client (Phase 4 Step 3)** - Gluon connects to external MCP servers ✅
//!
//! ## MCP Server Usage
//!
//! Run Gluon as an MCP server:
//!
//! ```bash
//! gluon-desktop --mcp
//! ```
//!
//! This exposes all Gluon tools to MCP clients like Claude Desktop.
//!
//! ## MCP Client Usage
//!
//! Connect to external MCP servers:
//!
//! ```rust
//! use crate::interface::mcp::{McpClient, McpServerConfig};
//! use std::collections::HashMap;
//!
//! let config = McpServerConfig {
//!     id: "filesystem".to_string(),
//!     command: "mcp-server-filesystem".to_string(),
//!     args: vec!["--root".to_string(), "/project".to_string()],
//!     env: HashMap::new(),
//! };
//!
//! let client = McpClient::connect(config).await?;
//! let tools = client.list_tools().await?;
//! ```
//!
//! Remote tools are automatically wrapped as RemoteToolProxy and registered in the ToolRegistry.
//!
//! ## Supported MCP Methods
//!
//! - `initialize` - Handshake with server
//! - `tools/list` - List available tools
//! - `tools/call` - Execute a tool

pub mod transport;
pub mod mapper;
pub mod server;
pub mod client;

// Re-export commonly used types
pub use transport::StdioTransport;
pub use mapper::{McpToolDefinition, McpToolsListResponse, gtool_to_mcp, tool_def_to_mcp, tools_to_mcp_list};
pub use server::McpServer;
pub use client::{McpClient, McpServerConfig};
