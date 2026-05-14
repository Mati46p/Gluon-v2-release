//! Sensors Subsystem - Browser Instrumentation via Chrome DevTools Protocol
//!
//! This module implements a "parasitic" browser instrumentation system that
//! connects to Chrome via CDP to capture network traffic, console logs,
//! and enable deep web research capabilities.
//!
//! # Architecture
//!
//! - **Driver**: Manages CDP connections (Parasitic/Managed modes)
//! - **Sniffer**: Captures network and console events with noise filtering
//! - **Visual**: Screenshot capture and optimization for LLM tokens
//! - **State**: Thread-safe state management with ring buffers and LRU caches
//!
//! # Usage
//!
//! ```rust
//! // Start parasitic session (attach to existing Chrome)
//! let session_id = driver::start_parasitic_session(&state, None).await?;
//!
//! // Navigate to URL
//! let tab_id = driver::navigate_to(&state, &session_id, "https://example.com").await?;
//!
//! // Capture screenshot
//! let screenshot = visual::capture_screenshot(&state, &tab_id, ScreenshotOptions::default()).await?;
//! ```

use chromiumoxide::Browser;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

// Module exports
pub mod types;
pub mod driver;
pub mod sniffer;
pub mod visual;
pub mod tauri_commands;

// Re-export commonly used types
pub use types::*;

// ============================================================================
// Global Sensor State
// ============================================================================

/// Global state for the Sensors subsystem
///
/// Thread-safe state management using Arc<TokioMutex<>> pattern
/// (matches ApplySystemState from Phase 1)
///
/// # Memory Management
///
/// - Network logs: Ring buffer (VecDeque) with max 1000 entries
/// - Console logs: Ring buffer (VecDeque) with max 500 entries
/// - Screenshots: LRU cache with max 50 entries
///
/// This prevents unbounded memory growth during long-running sessions.
pub struct SensorState {
    /// Active browser sessions (session_id -> BrowserSession)
    pub sessions: Arc<TokioMutex<HashMap<String, BrowserSession>>>,

    /// Network event log (ring buffer)
    pub network_logs: Arc<TokioMutex<VecDeque<NetworkEvent>>>,

    /// Console event log (ring buffer)
    pub console_logs: Arc<TokioMutex<VecDeque<ConsoleEvent>>>,

    /// Screenshot cache (LRU eviction)
    pub screenshots: Arc<TokioMutex<LruCache<String, Screenshot>>>,

    /// Global configuration
    pub config: Arc<TokioMutex<SensorConfig>>,
}

impl SensorState {
    /// Create new SensorState with default configuration
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(TokioMutex::new(HashMap::new())),
            network_logs: Arc::new(TokioMutex::new(VecDeque::new())),
            console_logs: Arc::new(TokioMutex::new(VecDeque::new())),
            screenshots: Arc::new(TokioMutex::new(
                LruCache::new(NonZeroUsize::new(50).unwrap())
            )),
            config: Arc::new(TokioMutex::new(SensorConfig::default())),
        }
    }

    /// Clone state for use in async tasks
    pub fn clone_state(&self) -> Self {
        Self {
            sessions: Arc::clone(&self.sessions),
            network_logs: Arc::clone(&self.network_logs),
            console_logs: Arc::clone(&self.console_logs),
            screenshots: Arc::clone(&self.screenshots),
            config: Arc::clone(&self.config),
        }
    }
}

// Implement Clone manually for SensorState
impl Clone for SensorState {
    fn clone(&self) -> Self {
        self.clone_state()
    }
}

// ============================================================================
// Browser Session
// ============================================================================

/// Represents an active browser session
///
/// A session contains a Browser instance (from chromiumoxide) and
/// tracks all active tabs within that browser.
pub struct BrowserSession {
    /// Unique session identifier
    pub session_id: String,

    /// Session mode (Parasitic or Managed)
    pub mode: SessionMode,

    /// Browser handle (chromiumoxide)
    pub browser: Browser,

    /// Active tabs (tab_id -> url)
    pub active_tabs: HashMap<String, String>,

    /// Session creation timestamp
    pub created_at: std::time::SystemTime,
}

impl BrowserSession {
    /// Create new browser session
    pub fn new(session_id: String, mode: SessionMode, browser: Browser) -> Self {
        Self {
            session_id,
            mode,
            browser,
            active_tabs: HashMap::new(),
            created_at: std::time::SystemTime::now(),
        }
    }

    /// Add tab to session
    pub fn add_tab(&mut self, tab_id: String, url: String) {
        self.active_tabs.insert(tab_id, url);
    }

    /// Remove tab from session
    pub fn remove_tab(&mut self, tab_id: &str) -> Option<String> {
        self.active_tabs.remove(tab_id)
    }

    /// Get tab URL
    pub fn get_tab_url(&self, tab_id: &str) -> Option<&String> {
        self.active_tabs.get(tab_id)
    }
}

// Implement Drop to ensure cleanup
impl Drop for BrowserSession {
    fn drop(&mut self) {
        // For Managed mode, we should close the browser
        // This will be handled by chromiumoxide's Drop implementation
        if matches!(self.mode, SessionMode::Managed { .. }) {
            eprintln!("[Sensors] Dropping managed browser session: {}", self.session_id);
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Add network event to state with ring buffer eviction
pub async fn add_network_event(state: &SensorState, event: NetworkEvent) {
    // Get max events from config
    let config = state.config.lock().await;
    let max_events = config.max_network_events;
    drop(config); // Release lock ASAP

    // Add to ring buffer
    let mut logs = state.network_logs.lock().await;
    if logs.len() >= max_events {
        logs.pop_front(); // Evict oldest
    }
    logs.push_back(event);
}

/// Add console event to state with ring buffer eviction
pub async fn add_console_event(state: &SensorState, event: ConsoleEvent) {
    // Get max events from config
    let config = state.config.lock().await;
    let max_events = config.max_console_events;
    drop(config); // Release lock ASAP

    // Add to ring buffer
    let mut logs = state.console_logs.lock().await;
    if logs.len() >= max_events {
        logs.pop_front(); // Evict oldest
    }
    logs.push_back(event);
}

/// Add screenshot to cache (LRU eviction happens automatically)
pub async fn add_screenshot(state: &SensorState, screenshot: Screenshot) {
    let mut cache = state.screenshots.lock().await;
    cache.put(screenshot.id.clone(), screenshot);
}

/// Get session by ID
pub async fn get_session(state: &SensorState, session_id: &str) -> Option<String> {
    let sessions = state.sessions.lock().await;
    sessions.get(session_id).map(|s| s.session_id.clone())
}

/// Check if session exists
pub async fn session_exists(state: &SensorState, session_id: &str) -> bool {
    let sessions = state.sessions.lock().await;
    sessions.contains_key(session_id)
}

/// Get all active session IDs
pub async fn get_all_sessions(state: &SensorState) -> Vec<String> {
    let sessions = state.sessions.lock().await;
    sessions.keys().cloned().collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensor_state_creation() {
        let state = SensorState::new();
        assert!(state.sessions.try_lock().is_ok());
        assert!(state.network_logs.try_lock().is_ok());
        assert!(state.console_logs.try_lock().is_ok());
    }

    #[test]
    fn test_image_format_mime_types() {
        assert_eq!(ImageFormat::PNG.mime_type(), "image/png");
        assert_eq!(ImageFormat::JPEG.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::WebP.mime_type(), "image/webp");
    }

    #[test]
    fn test_console_level_ordering() {
        assert!(ConsoleLevel::Verbose < ConsoleLevel::Debug);
        assert!(ConsoleLevel::Debug < ConsoleLevel::Log);
        assert!(ConsoleLevel::Log < ConsoleLevel::Info);
        assert!(ConsoleLevel::Info < ConsoleLevel::Warning);
        assert!(ConsoleLevel::Warning < ConsoleLevel::Error);
    }

    #[test]
    fn test_console_level_from_cdp() {
        assert_eq!(ConsoleLevel::from_cdp("error"), ConsoleLevel::Error);
        assert_eq!(ConsoleLevel::from_cdp("warning"), ConsoleLevel::Warning);
        assert_eq!(ConsoleLevel::from_cdp("info"), ConsoleLevel::Info);
        assert_eq!(ConsoleLevel::from_cdp("log"), ConsoleLevel::Log);
        assert_eq!(ConsoleLevel::from_cdp("unknown"), ConsoleLevel::Log);
    }

    #[tokio::test]
    async fn test_ring_buffer_eviction() {
        let state = SensorState::new();

        // Set max to 3 for testing
        {
            let mut config = state.config.lock().await;
            config.max_network_events = 3;
        }

        // Add 5 events
        for i in 0..5 {
            let event = NetworkEvent {
                request_id: format!("req-{}", i),
                url: format!("https://example.com/{}", i),
                method: "GET".to_string(),
                status_code: Some(200),
                request_headers: HashMap::new(),
                response_headers: None,
                request_body: None,
                response_body: None,
                resource_type: "Document".to_string(),
                timestamp: i as u64,
                duration_ms: Some(100),
            };
            add_network_event(&state, event).await;
        }

        // Should only have last 3 events (oldest evicted)
        let logs = state.network_logs.lock().await;
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].timestamp, 2); // Event 0 and 1 were evicted
        assert_eq!(logs[1].timestamp, 3);
        assert_eq!(logs[2].timestamp, 4);
    }
}
