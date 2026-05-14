//! JSON-RPC 2.0 Protocol Types
//!
//! This module implements the JSON-RPC 2.0 specification for both:
//! - G-Protocol (Gluon's native tool calling)
//! - MCP (Model Context Protocol)
//!
//! ## JSON-RPC 2.0 Message Format
//!
//! ### Request
//! ```json
//! {
//!   "jsonrpc": "2.0",
//!   "id": 1,
//!   "method": "tool_name",
//!   "params": { "key": "value" }
//! }
//! ```
//!
//! ### Response (Success)
//! ```json
//! {
//!   "jsonrpc": "2.0",
//!   "id": 1,
//!   "result": { "data": "..." }
//! }
//! ```
//!
//! ### Response (Error)
//! ```json
//! {
//!   "jsonrpc": "2.0",
//!   "id": 1,
//!   "error": {
//!     "code": -32600,
//!     "message": "Invalid Request",
//!     "data": null
//!   }
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 Request
///
/// Represents a request to execute a tool/method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,

    /// Unique request identifier
    ///
    /// Can be a number, string, or null (for notifications).
    /// The response MUST contain the same id.
    pub id: Value,

    /// Method/tool name to execute
    ///
    /// Examples:
    /// - "gluon.read_file"
    /// - "tools/list" (MCP method)
    /// - "initialize" (MCP handshake)
    pub method: String,

    /// Parameters for the method (optional)
    ///
    /// Can be:
    /// - An object: {"file_path": "src/main.rs"}
    /// - An array: [1, 2, 3]
    /// - Omitted entirely
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request
    pub fn new(id: impl Into<Value>, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }

    /// Create a request with object params
    pub fn with_params(id: impl Into<Value>, method: impl Into<String>, params: Value) -> Self {
        Self::new(id, method, Some(params))
    }

    /// Create a request without params
    pub fn without_params(id: impl Into<Value>, method: impl Into<String>) -> Self {
        Self::new(id, method, None)
    }
}

/// JSON-RPC 2.0 Response
///
/// Represents a response to a request.
/// Either `result` OR `error` must be present, never both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,

    /// Request identifier (matches the request id)
    pub id: Value,

    /// Result data (present only on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,

    /// Error information (present only on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Create a successful response
    pub fn success(id: impl Into<Value>, result: impl Serialize) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: impl Into<Value>, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        }
    }

    /// Create an error response with additional data
    pub fn error_with_data(id: impl Into<Value>, code: i32, message: &str, data: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: Some(data),
            }),
        }
    }

    /// Check if response is successful
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    /// Check if response is an error
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// JSON-RPC 2.0 Error Object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    ///
    /// Standard codes:
    /// - -32700: Parse error (invalid JSON)
    /// - -32600: Invalid Request (missing required fields)
    /// - -32601: Method not found
    /// - -32602: Invalid params
    /// - -32603: Internal error
    /// - -32000 to -32099: Server-defined errors
    pub code: i32,

    /// Human-readable error message
    pub message: String,

    /// Additional error data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Standard JSON-RPC 2.0 error codes
pub mod error_codes {
    /// Invalid JSON was received by the server
    pub const PARSE_ERROR: i32 = -32700;

    /// The JSON sent is not a valid Request object
    pub const INVALID_REQUEST: i32 = -32600;

    /// The method does not exist / is not available
    pub const METHOD_NOT_FOUND: i32 = -32601;

    /// Invalid method parameter(s)
    pub const INVALID_PARAMS: i32 = -32602;

    /// Internal JSON-RPC error
    pub const INTERNAL_ERROR: i32 = -32603;

    /// Server error (generic)
    pub const SERVER_ERROR: i32 = -32000;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_request_serialization() {
        let request = JsonRpcRequest::new(
            1,
            "gluon.read_file",
            Some(json!({"file_path": "src/main.rs"})),
        );

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"gluon.read_file\""));
        assert!(json.contains("src/main.rs"));
    }

    #[test]
    fn test_request_deserialization() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 42,
            "method": "tools/list",
            "params": null
        }"#;

        let request: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.id, json!(42));
        assert_eq!(request.method, "tools/list");
        assert!(request.params.is_none());
    }

    #[test]
    fn test_response_success() {
        let response = JsonRpcResponse::success(
            1,
            json!({"status": "ok", "data": [1, 2, 3]}),
        );

        assert!(response.is_success());
        assert!(!response.is_error());
        assert!(response.error.is_none());

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_response_error() {
        let response = JsonRpcResponse::error(
            1,
            error_codes::METHOD_NOT_FOUND,
            "Tool not found",
        );

        assert!(!response.is_success());
        assert!(response.is_error());
        assert!(response.result.is_none());

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("-32601"));
        assert!(json.contains("Tool not found"));
    }

    #[test]
    fn test_request_without_params() {
        let request = JsonRpcRequest::without_params(1, "initialize");
        let json = serde_json::to_string(&request).unwrap();

        // params should not be in JSON when None
        assert!(!json.contains("\"params\""));
    }

    #[test]
    fn test_request_with_params() {
        let request = JsonRpcRequest::with_params(
            "req-123",
            "gluon.write_file",
            json!({"file_path": "output.txt", "content": "Hello"}),
        );

        assert_eq!(request.id, json!("req-123"));
        assert!(request.params.is_some());
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(error_codes::PARSE_ERROR, -32700);
        assert_eq!(error_codes::INVALID_REQUEST, -32600);
        assert_eq!(error_codes::METHOD_NOT_FOUND, -32601);
        assert_eq!(error_codes::INVALID_PARAMS, -32602);
        assert_eq!(error_codes::INTERNAL_ERROR, -32603);
        assert_eq!(error_codes::SERVER_ERROR, -32000);
    }

    #[test]
    fn test_roundtrip_request() {
        let original = JsonRpcRequest::with_params(
            42,
            "test_method",
            json!({"key": "value"}),
        );

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: JsonRpcRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, json!(42));
        assert_eq!(deserialized.method, "test_method");
    }

    #[test]
    fn test_roundtrip_response() {
        let original = JsonRpcResponse::success(1, json!({"result": "success"}));

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: JsonRpcResponse = serde_json::from_str(&json).unwrap();

        assert!(deserialized.is_success());
        assert_eq!(deserialized.id, json!(1));
    }
}
