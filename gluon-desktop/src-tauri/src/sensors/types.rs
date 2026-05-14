//! Core data types for the Sensors subsystem
//!
//! Defines structures for network events, console logs, screenshots,
//! and configuration options for the browser instrumentation system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Network Event Types
// ============================================================================

/// Represents a captured network request/response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    /// Unique identifier for this request (from CDP)
    pub request_id: String,

    /// Full URL of the request
    pub url: String,

    /// HTTP method (GET, POST, PUT, DELETE, etc.)
    pub method: String,

    /// HTTP status code (200, 404, 500, etc.)
    pub status_code: Option<u16>,

    /// Request headers
    pub request_headers: HashMap<String, String>,

    /// Response headers
    pub response_headers: Option<HashMap<String, String>>,

    /// Request body (if captured)
    pub request_body: Option<String>,

    /// Response body (if captured)
    pub response_body: Option<String>,

    /// Resource type (Document, Stylesheet, Script, XHR, Fetch, Image, etc.)
    pub resource_type: String,

    /// Unix timestamp in milliseconds
    pub timestamp: u64,

    /// Request duration in milliseconds
    pub duration_ms: Option<u64>,
}

// ============================================================================
// Console Event Types
// ============================================================================

/// Console log level (matches Chrome DevTools Protocol)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConsoleLevel {
    Verbose,
    Debug,
    Log,
    Info,
    Warning,
    Error,
}

impl ConsoleLevel {
    /// Parse from CDP console API call type
    pub fn from_cdp(level: &str) -> Self {
        match level {
            "verbose" => ConsoleLevel::Verbose,
            "debug" => ConsoleLevel::Debug,
            "log" => ConsoleLevel::Log,
            "info" => ConsoleLevel::Info,
            "warning" | "warn" => ConsoleLevel::Warning,
            "error" => ConsoleLevel::Error,
            _ => ConsoleLevel::Log,
        }
    }
}

/// Represents a captured console log or exception
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEvent {
    /// Log level
    pub level: ConsoleLevel,

    /// Log message
    pub message: String,

    /// Stack trace (for errors and warnings)
    pub stack_trace: Option<String>,

    /// Unix timestamp in milliseconds
    pub timestamp: u64,

    /// Source file URL
    pub url: Option<String>,

    /// Line number in source file
    pub line: Option<u32>,

    /// Column number in source file
    pub column: Option<u32>,
}

// ============================================================================
// Screenshot Types
// ============================================================================

/// Image format for screenshots
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageFormat {
    PNG,
    JPEG,
    WebP,
}

impl ImageFormat {
    /// Get MIME type for this format
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::PNG => "image/png",
            ImageFormat::JPEG => "image/jpeg",
            ImageFormat::WebP => "image/webp",
        }
    }

    /// Get file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::PNG => "png",
            ImageFormat::JPEG => "jpg",
            ImageFormat::WebP => "webp",
        }
    }
}

/// Represents a captured screenshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenshot {
    /// Unique identifier
    pub id: String,

    /// Tab ID this screenshot was taken from
    pub tab_id: String,

    /// URL of the page when screenshot was taken
    pub url: String,

    /// Base64-encoded image data
    pub data: String,

    /// Image format
    pub format: ImageFormat,

    /// Image width in pixels
    pub width: u32,

    /// Image height in pixels
    pub height: u32,

    /// Unix timestamp in milliseconds
    pub timestamp: u64,
}

/// Screenshot capture options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotOptions {
    /// Image format
    pub format: ImageFormat,

    /// JPEG quality (0-100, only for JPEG format)
    pub quality: u8,

    /// Maximum width (will resize if larger, preserving aspect ratio)
    pub max_width: Option<u32>,

    /// Capture mode
    pub mode: CaptureMode,

    /// CSS selector (required for Element mode)
    pub selector: Option<String>,
}

impl Default for ScreenshotOptions {
    fn default() -> Self {
        Self {
            format: ImageFormat::JPEG,
            quality: 85,
            max_width: Some(1280),
            mode: CaptureMode::Viewport,
            selector: None,
        }
    }
}

/// Screenshot capture mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CaptureMode {
    /// Capture only the visible viewport
    Viewport,

    /// Capture the entire scrollable page
    FullPage,

    /// Capture a specific element (requires selector)
    Element,
}

// ============================================================================
// Sniffer Configuration
// ============================================================================

/// Configuration for the network/console sniffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnifferConfig {
    /// Ignore tracking pixels (< 1KB images to analytics domains)
    pub ignore_tracking_pixels: bool,

    /// Ignore common CDN requests (fonts, icons, etc.)
    pub ignore_common_cdn: bool,

    /// File extensions to ignore (e.g., .woff2, .ico)
    pub ignore_extensions: Vec<String>,

    /// Capture request bodies (can be large, disable if not needed)
    pub capture_request_bodies: bool,

    /// Capture response bodies (can be large, disable if not needed)
    pub capture_response_bodies: bool,

    /// Maximum body size to capture (bytes)
    pub max_body_size: usize,

    /// Minimum console log level to capture
    pub min_console_level: ConsoleLevel,

    /// Capture JavaScript exceptions
    pub capture_exceptions: bool,
}

impl Default for SnifferConfig {
    fn default() -> Self {
        Self {
            ignore_tracking_pixels: true,
            ignore_common_cdn: true,
            ignore_extensions: vec![
                ".woff2".to_string(),
                ".woff".to_string(),
                ".ttf".to_string(),
                ".eot".to_string(),
                ".ico".to_string(),
            ],
            capture_request_bodies: true,
            capture_response_bodies: true,
            max_body_size: 5_242_880, // 5MB
            min_console_level: ConsoleLevel::Info,
            capture_exceptions: true,
        }
    }
}

// ============================================================================
// Sensor Configuration
// ============================================================================

/// Global configuration for the Sensors subsystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    /// Maximum number of network events to keep in memory
    pub max_network_events: usize,

    /// Maximum number of console events to keep in memory
    pub max_console_events: usize,

    /// Maximum number of screenshots to cache
    pub max_screenshots: usize,

    /// Default sniffer configuration
    pub default_sniffer: SnifferConfig,
}

impl Default for SensorConfig {
    fn default() -> Self {
        Self {
            max_network_events: 1000,
            max_console_events: 500,
            max_screenshots: 50,
            default_sniffer: SnifferConfig::default(),
        }
    }
}

// ============================================================================
// Session Types
// ============================================================================

/// Browser session mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionMode {
    /// Parasitic mode: attach to existing Chrome instance
    Parasitic { ws_url: String },

    /// Managed mode: spawn new Chrome instance
    Managed { chrome_path: String },
}

// ============================================================================
// Filter Types
// ============================================================================

/// Filter options for querying network logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkFilter {
    /// Filter by URL pattern (contains)
    pub url_pattern: Option<String>,

    /// Filter by HTTP method
    pub method: Option<String>,

    /// Filter by resource type
    pub resource_type: Option<String>,

    /// Filter by minimum status code
    pub min_status: Option<u16>,

    /// Filter by maximum status code
    pub max_status: Option<u16>,
}

/// Filter options for querying console logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleFilter {
    /// Filter by minimum log level
    pub min_level: Option<ConsoleLevel>,

    /// Filter by message pattern (contains)
    pub message_pattern: Option<String>,

    /// Only show exceptions
    pub only_exceptions: bool,
}
