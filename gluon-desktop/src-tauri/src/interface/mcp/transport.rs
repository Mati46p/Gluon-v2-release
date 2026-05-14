//! MCP Transport Layer - Stdio Implementation
//!
//! Implements JSON-RPC 2.0 communication over stdin/stdout for MCP protocol.
//!
//! ## Protocol
//!
//! Messages are newline-delimited JSON (JSONL format):
//! - Each message is a complete JSON object on a single line
//! - Messages are terminated with `\n`
//! - Server reads from stdin, writes to stdout
//!
//! ## Example Flow
//!
//! ```text
//! Client -> Server (stdin):
//! {"jsonrpc":"2.0","id":1,"method":"initialize","params":{...}}
//!
//! Server -> Client (stdout):
//! {"jsonrpc":"2.0","id":1,"result":{...}}
//! ```

use crate::interface::protocol::{JsonRpcRequest, JsonRpcResponse};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

/// Stdio transport for MCP communication
///
/// Reads JSON-RPC requests from stdin, writes responses to stdout.
pub struct StdioTransport {
    receiver: mpsc::UnboundedReceiver<JsonRpcRequest>,
    _stdin_task: tokio::task::JoinHandle<()>,
}

impl StdioTransport {
    /// Create a new stdio transport
    ///
    /// Spawns a background task to read from stdin.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn stdin reader task
        let stdin_task = tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let mut reader = BufReader::new(stdin);
            let mut line = String::new();

            loop {
                line.clear();

                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        // EOF - client disconnected
                        eprintln!("[MCP Transport] Stdin closed, exiting");
                        break;
                    }
                    Ok(_) => {
                        // Try to parse JSON-RPC request
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue; // Skip empty lines
                        }

                        match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                            Ok(request) => {
                                if tx.send(request).is_err() {
                                    eprintln!("[MCP Transport] Receiver dropped, exiting");
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("[MCP Transport] Failed to parse request: {}", e);
                                eprintln!("[MCP Transport] Invalid line: {}", trimmed);
                                // Continue reading - don't crash on parse errors
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[MCP Transport] Error reading stdin: {}", e);
                        break;
                    }
                }
            }
        });

        Self {
            receiver: rx,
            _stdin_task: stdin_task,
        }
    }

    /// Receive the next JSON-RPC request from stdin
    ///
    /// Returns None when stdin is closed.
    pub async fn receive(&mut self) -> Option<JsonRpcRequest> {
        self.receiver.recv().await
    }

    /// Send a JSON-RPC response to stdout
    ///
    /// The response is serialized to JSON and written as a single line.
    pub async fn send(&self, response: JsonRpcResponse) -> Result<(), String> {
        let json = serde_json::to_string(&response)
            .map_err(|e| format!("Failed to serialize response: {}", e))?;

        let mut stdout = tokio::io::stdout();

        // Write JSON + newline
        stdout
            .write_all(json.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to stdout: {}", e))?;

        stdout
            .write_all(b"\n")
            .await
            .map_err(|e| format!("Failed to write newline: {}", e))?;

        stdout
            .flush()
            .await
            .map_err(|e| format!("Failed to flush stdout: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::protocol::JsonRpcRequest;
    use serde_json::json;

    #[test]
    fn test_transport_creation() {
        // Should not panic
        let _transport = StdioTransport::new();
    }

    #[tokio::test]
    async fn test_send_response() {
        let transport = StdioTransport::new();

        let response = JsonRpcResponse::success(1, json!({"status": "ok"}));

        // Should not panic or error
        // Note: This writes to stdout, so we can't easily verify the output in tests
        // In production, this would be captured by the MCP client
        let result = transport.send(response).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_request_serialization() {
        // Verify that requests can be serialized/deserialized
        let request = JsonRpcRequest::with_params(
            1,
            "test_method",
            json!({"key": "value"}),
        );

        let json = serde_json::to_string(&request).unwrap();
        let parsed: JsonRpcRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.method, "test_method");
        assert_eq!(parsed.id, json!(1));
    }
}
