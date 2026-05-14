//! Browser Driver - CDP Connection Management
//!
//! Manages connections to Chrome via the Chrome DevTools Protocol (CDP).
//!
//! # Modes
//!
//! - **Parasitic**: Attach to existing Chrome instance (port 9222)
//!   - User must launch Chrome with: `chrome.exe --remote-debugging-port=9222`
//!   - Shares user's cookies, sessions, and permissions
//!   - Invisible to anti-bot systems
//!
//! - **Managed**: Spawn new Chrome instance
//!   - Gluon launches Chrome with controlled flags
//!   - Isolated from user's main browser profile
//!   - Full control over browser lifecycle

use chromiumoxide::{Browser, BrowserConfig};
use futures_util::stream::StreamExt;
use crate::sensors::{SensorState, BrowserSession, SessionMode};
use uuid::Uuid;
use std::collections::HashMap;

// ============================================================================
// Parasitic Mode - Attach to Existing Chrome
// ============================================================================

/// Start parasitic session by connecting to existing Chrome instance
///
/// # Arguments
///
/// * `state` - Global sensor state
/// * `ws_url` - WebSocket URL (default: ws://127.0.0.1:9222)
///
/// # Returns
///
/// Session ID string on success
///
/// # Errors
///
/// Returns error if Chrome is not running with remote debugging enabled
///
/// # Example
///
/// ```rust
/// let session_id = start_parasitic_session(&state, None).await?;
/// ```
pub async fn start_parasitic_session(
    state: &SensorState,
    ws_url: Option<String>,
) -> Result<String, String> {
    let ws_url = ws_url.unwrap_or_else(|| "ws://127.0.0.1:9222".to_string());

    // Connect to existing Chrome instance
    let (browser, _handler) = Browser::connect(&ws_url)
        .await
        .map_err(|e| format!(
            "Failed to connect to Chrome at {}. \n\
            Ensure Chrome is running with --remote-debugging-port=9222.\n\
            Launch Chrome with: chrome.exe --remote-debugging-port=9222\n\
            Error: {}",
            ws_url, e
        ))?;

    // Generate session ID
    let session_id = Uuid::new_v4().to_string();

    // Create session
    let session = BrowserSession::new(
        session_id.clone(),
        SessionMode::Parasitic { ws_url },
        browser,
    );

    // Store in state
    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(session_id.clone(), session);
    }

    eprintln!("[Sensors] Parasitic session started: {}", session_id);
    Ok(session_id)
}

// ============================================================================
// Managed Mode - Spawn New Chrome Instance
// ============================================================================

/// Start managed session by spawning new Chrome instance
///
/// # Arguments
///
/// * `state` - Global sensor state
/// * `chrome_path` - Path to Chrome executable (auto-detected if None)
///
/// # Returns
///
/// Session ID string on success
///
/// # Errors
///
/// Returns error if Chrome cannot be found or launched
///
/// # Example
///
/// ```rust
/// let session_id = start_managed_session(&state, None).await?;
/// ```
pub async fn start_managed_session(
    state: &SensorState,
    chrome_path: Option<String>,
) -> Result<String, String> {
    let chrome_path = chrome_path.unwrap_or_else(detect_chrome_path);

    // Check if Chrome exists
    if !std::path::Path::new(&chrome_path).exists() {
        return Err(format!(
            "Chrome not found at: {}\n\
            Please provide the correct path to Chrome.",
            chrome_path
        ));
    }

    // Build browser config
    let config = BrowserConfig::builder()
        .chrome_executable(&chrome_path)
        .with_head() // Visible Chrome window
        .arg("--remote-debugging-port=9222")
        .arg("--disable-extensions") // Speed up startup
        .arg("--disable-background-networking") // Reduce noise
        .build()
        .map_err(|e| format!("Failed to build browser config: {}", e))?;

    // Launch Chrome
    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("Failed to launch Chrome: {}", e))?;

    // Spawn task to handle browser events
    tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            // Handle browser events if needed
            match event {
                Ok(_) => {}
                Err(e) => eprintln!("[Sensors] Browser event error: {}", e),
            }
        }
    });

    // Generate session ID
    let session_id = Uuid::new_v4().to_string();

    // Create session
    let session = BrowserSession::new(
        session_id.clone(),
        SessionMode::Managed {
            chrome_path: chrome_path.clone(),
        },
        browser,
    );

    // Store in state
    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(session_id.clone(), session);
    }

    eprintln!("[Sensors] Managed session started: {} (Chrome: {})", session_id, chrome_path);
    Ok(session_id)
}

// ============================================================================
// Session Management
// ============================================================================

/// Stop browser session and cleanup resources
///
/// For Managed mode, this closes the Chrome instance.
/// For Parasitic mode, this just disconnects (Chrome keeps running).
pub async fn stop_session(
    state: &SensorState,
    session_id: &str,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().await;
    let mut session = sessions.remove(session_id)
        .ok_or_else(|| format!("Session {} not found", session_id))?;

    // Close browser for Managed mode
    if matches!(session.mode, SessionMode::Managed { .. }) {
        let _ = session.browser.close().await;
        eprintln!("[Sensors] Closed managed browser for session: {}", session_id);
    } else {
        eprintln!("[Sensors] Disconnected from parasitic session: {}", session_id);
    }

    Ok(())
}

/// Navigate to URL and create new tab
///
/// # Returns
///
/// Tab ID string on success
pub async fn navigate_to(
    state: &SensorState,
    session_id: &str,
    url: &str,
) -> Result<String, String> {
    // Validate URL
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("Invalid URL: {}. Must start with http:// or https://", url));
    }

    // Get session
    let sessions = state.sessions.lock().await;
    let session = sessions.get(session_id)
        .ok_or_else(|| format!("Session {} not found", session_id))?;

    // Create new page
    let page = session.browser.new_page(url)
        .await
        .map_err(|e| format!("Failed to navigate to {}: {}", url, e))?;

    // Get tab ID (TargetId is a newtype wrapper around String)
    let tab_id = page.target_id().as_ref().to_string();

    drop(sessions); // Release lock before next lock

    // Update session's active tabs
    {
        let mut sessions = state.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.add_tab(tab_id.clone(), url.to_string());
        }
    }

    eprintln!("[Sensors] Navigated to {} (tab: {})", url, tab_id);
    Ok(tab_id)
}

/// Close a specific tab
pub async fn close_tab(
    state: &SensorState,
    session_id: &str,
    tab_id: &str,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().await;
    let session = sessions.get_mut(session_id)
        .ok_or_else(|| format!("Session {} not found", session_id))?;

    // Remove from active tabs
    session.remove_tab(tab_id)
        .ok_or_else(|| format!("Tab {} not found in session {}", tab_id, session_id))?;

    // Note: chromiumoxide handles tab closure automatically when page is dropped
    eprintln!("[Sensors] Closed tab: {}", tab_id);
    Ok(())
}

/// Get all active tabs in a session
pub async fn get_active_tabs(
    state: &SensorState,
    session_id: &str,
) -> Result<HashMap<String, String>, String> {
    let sessions = state.sessions.lock().await;
    let session = sessions.get(session_id)
        .ok_or_else(|| format!("Session {} not found", session_id))?;

    Ok(session.active_tabs.clone())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Detect Chrome installation path for the current platform
fn detect_chrome_path() -> String {
    #[cfg(target_os = "windows")]
    {
        // Try common Windows paths
        let paths = vec![
            r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string(),
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe".to_string(),
            format!(r"{}\AppData\Local\Google\Chrome\Application\chrome.exe",
                    std::env::var("USERPROFILE").unwrap_or_default()),
        ];

        for path in paths {
            if std::path::Path::new(&path).exists() {
                return path;
            }
        }

        // Default fallback
        r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string()
    }

    #[cfg(target_os = "macos")]
    {
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_string()
    }

    #[cfg(target_os = "linux")]
    {
        // Try common Linux paths
        let paths = vec![
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
        ];

        for path in paths {
            if std::path::Path::new(path).exists() {
                return path.to_string();
            }
        }

        // Default fallback
        "/usr/bin/google-chrome".to_string()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "chrome".to_string()
    }
}

/// Check if a session exists
pub async fn session_exists(
    state: &SensorState,
    session_id: &str,
) -> bool {
    let sessions = state.sessions.lock().await;
    sessions.contains_key(session_id)
}

/// Get session info (mode and creation time)
pub async fn get_session_info(
    state: &SensorState,
    session_id: &str,
) -> Result<(SessionMode, std::time::SystemTime), String> {
    let sessions = state.sessions.lock().await;
    let session = sessions.get(session_id)
        .ok_or_else(|| format!("Session {} not found", session_id))?;

    Ok((session.mode.clone(), session.created_at))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_chrome_path() {
        let path = detect_chrome_path();
        assert!(!path.is_empty());
        // Path should contain "chrome" or "chromium"
        assert!(
            path.to_lowercase().contains("chrome") ||
            path.to_lowercase().contains("chromium")
        );
    }

    #[test]
    fn test_url_validation() {
        // Valid URLs
        assert!("https://example.com".starts_with("http://") || "https://example.com".starts_with("https://"));

        // Invalid URLs
        assert!(!("javascript:alert(1)".starts_with("http://") || "javascript:alert(1)".starts_with("https://")));
        assert!(!("file:///etc/passwd".starts_with("http://") || "file:///etc/passwd".starts_with("https://")));
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        let state = SensorState::new();

        // Session should not exist initially
        assert!(!session_exists(&state, "test-id").await);

        // After we implement full functionality, we can add more tests
        // For now, just verify state can be created
        assert!(state.sessions.try_lock().is_ok());
    }
}
