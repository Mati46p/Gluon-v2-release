//! # Interface Layer - Phase 4
//!
//! The Interface Layer provides a universal tool-calling system based on JSON-RPC 2.0.
//! It enables Gluon to act as both an MCP Host (exposing tools) and MCP Client (using external tools).
//!
//! ## Architecture
//!
//! - **G-Protocol**: JSON-RPC 2.0 based tool-calling protocol (unified with MCP)
//! - **Tool Registry**: Central registry of all available tools (local and remote)
//! - **MCP Integration**: Bidirectional MCP support (Host and Client)
//! - **Safety Middleware**: Confirmation flow for dangerous operations
//!
//! ## Modules
//!
//! - `types`: Core traits (GTool) and data types
//! - `protocol`: JSON-RPC 2.0 message types
//! - `registry`: Tool registry and manifest generation
//! - `executor`: Tool execution engine with safety checks
//! - `safety`: Safety middleware for confirmations
//! - `mcp`: Model Context Protocol implementation (server & client)
//! - `tools`: Built-in tool implementations
//! - `tauri_commands`: Tauri frontend API

pub mod types;
pub mod protocol;
pub mod registry;
pub mod executor;
pub mod safety;
pub mod mcp;
pub mod tools;
pub mod tauri_commands;

// Re-export commonly used types
pub use types::{GTool, ToolContext, ToolResult, ToolOutput, ToolError, ToolCategory};
pub use protocol::{JsonRpcRequest, JsonRpcResponse, JsonRpcError};
pub use registry::ToolRegistry;
pub use executor::ToolExecutor;
pub use safety::SafetyMiddleware;
