//! MCP Client Implementation
//!
//! Connects to external MCP servers and uses their tools.
//!
//! ## Usage
//!
//! ```rust
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

use crate::interface::mcp::mapper::McpToolDefinition;
use crate::interface::protocol::{JsonRpcRequest, JsonRpcResponse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};

/// Configuration for connecting to an external MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Unique ID for this server
    pub id: String,

    /// Command to execute (e.g., "mcp-server-filesystem")
    pub command: String,

    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// MCP Client - connects to external MCP servers
pub struct McpClient {
    server_id: String,
    request_counter: Arc<AtomicU64>,
    sender: Arc<Mutex<mpsc::UnboundedSender<JsonRpcRequest>>>,
    receiver: Arc<Mutex<mpsc::UnboundedReceiver<JsonRpcResponse>>>,
    _child_process: Arc<Mutex<Child>>,
}

impl McpClient {
    /// Connect to an external MCP server
    ///
    /// Spawns the server process and establishes stdio communication.
    pub async fn connect(config: McpServerConfig) -> Result<Arc<Self>, String> {
        eprintln!("[MCP Client] Connecting to server: {}", config.id);

        // Spawn the MCP server process
        let mut child = Command::new(&config.command)
            .args(&config.args)
            .envs(&config.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn MCP server '{}': {}", config.command, e))?;

        // Get stdin/stdout handles
        let mut stdin = child
            .stdin
            .take()
            .ok_or("Failed to get stdin handle")?;

        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to get stdout handle")?;

        // Create channels for request/response
        let (req_tx, mut req_rx) = mpsc::unbounded_channel::<JsonRpcRequest>();
        let (resp_tx, resp_rx) = mpsc::unbounded_channel::<JsonRpcResponse>();

        // Spawn task to write requests to server stdin
        let server_id_clone = config.id.clone();
        tokio::spawn(async move {
            while let Some(request) = req_rx.recv().await {
                let json = serde_json::to_string(&request).unwrap();

                if let Err(e) = stdin.write_all(json.as_bytes()).await {
                    eprintln!("[MCP Client] Failed to write request: {}", e);
                    break;
                }
                if let Err(e) = stdin.write_all(b"\n").await {
                    eprintln!("[MCP Client] Failed to write newline: {}", e);
                    break;
                }
                if let Err(e) = stdin.flush().await {
                    eprintln!("[MCP Client] Failed to flush: {}", e);
                    break;
                }
            }
            eprintln!("[MCP Client] Request writer task exiting for {}", server_id_clone);
        });

        // Spawn task to read responses from server stdout
        let server_id_clone2 = config.id.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        eprintln!("[MCP Client] Server stdout closed for {}", server_id_clone2);
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        match serde_json::from_str::<JsonRpcResponse>(trimmed) {
                            Ok(response) => {
                                if resp_tx.send(response).is_err() {
                                    eprintln!("[MCP Client] Receiver dropped");
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("[MCP Client] Failed to parse response: {}", e);
                                eprintln!("[MCP Client] Invalid line: {}", trimmed);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[MCP Client] Error reading stdout: {}", e);
                        break;
                    }
                }
            }
        });

        let client = Arc::new(Self {
            server_id: config.id.clone(),
            request_counter: Arc::new(AtomicU64::new(1)),
            sender: Arc::new(Mutex::new(req_tx)),
            receiver: Arc::new(Mutex::new(resp_rx)),
            _child_process: Arc::new(Mutex::new(child)),
        });

        // Send initialize request
        client.initialize().await?;

        eprintln!("[MCP Client] Successfully connected to {}", config.id);

        Ok(client)
    }

    /// Send initialize request to server
    async fn initialize(&self) -> Result<(), String> {
        let request = JsonRpcRequest::with_params(
            self.next_id(),
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "clientInfo": {
                    "name": "gluon-mcp-client",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        );

        let response = self.call(request).await?;

        if response.is_error() {
            return Err(format!(
                "Initialize failed: {}",
                response.error.unwrap().message
            ));
        }

        Ok(())
    }

    /// List all tools from the server
    pub async fn list_tools(&self) -> Result<Vec<McpToolDefinition>, String> {
        let request = JsonRpcRequest::without_params(self.next_id(), "tools/list");

        let response = self.call(request).await?;

        if response.is_error() {
            return Err(format!(
                "tools/list failed: {}",
                response.error.unwrap().message
            ));
        }

        let result = response.result.ok_or("No result in response")?;

        #[derive(Deserialize)]
        struct ToolsListResult {
            tools: Vec<McpToolDefinition>,
        }

        let tools_result: ToolsListResult = serde_json::from_value(result)
            .map_err(|e| format!("Failed to parse tools list: {}", e))?;

        Ok(tools_result.tools)
    }

    /// Call a tool on the remote server
    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value, String> {
        let request = JsonRpcRequest::with_params(
            self.next_id(),
            "tools/call",
            json!({
                "name": tool_name,
                "arguments": arguments
            }),
        );

        let response = self.call(request).await?;

        if response.is_error() {
            return Err(format!(
                "Tool call failed: {}",
                response.error.unwrap().message
            ));
        }

        response.result.ok_or("No result in response".to_string())
    }

    /// Send request and wait for response
    async fn call(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, String> {
        // Send request
        {
            let sender = self.sender.lock().await;
            sender
                .send(request.clone())
                .map_err(|_| "Failed to send request")?;
        }

        // Wait for response with matching ID
        let request_id = request.id.clone();

        // Simple approach: wait for next response (assumes responses come in order)
        // For production, use a proper request/response matching system
        let mut receiver = self.receiver.lock().await;

        tokio::time::timeout(std::time::Duration::from_secs(30), receiver.recv())
            .await
            .map_err(|_| "Request timeout".to_string())?
            .ok_or_else(|| "Response channel closed".to_string())
    }

    /// Get next request ID
    fn next_id(&self) -> u64 {
        self.request_counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Get server ID
    pub fn server_id(&self) -> &str {
        &self.server_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = McpServerConfig {
            id: "test".to_string(),
            command: "test-command".to_string(),
            args: vec!["--arg1".to_string(), "value".to_string()],
            env: {
                let mut map = HashMap::new();
                map.insert("KEY".to_string(), "VALUE".to_string());
                map
            },
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"id\":\"test\""));
        assert!(json.contains("test-command"));
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{
            "id": "filesystem",
            "command": "mcp-server-filesystem",
            "args": ["--root", "/project"]
        }"#;

        let config: McpServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.id, "filesystem");
        assert_eq!(config.command, "mcp-server-filesystem");
        assert_eq!(config.args.len(), 2);
    }

    // Note: Integration tests with real MCP server would go in tests/ directory
}
