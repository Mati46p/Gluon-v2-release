//! Tauri Commands - Frontend API for Sensors
//!
//! Exposes browser instrumentation functionality to the frontend via Tauri commands.
//! Follows the same pattern as apply_system/tauri_commands.rs.

use tauri::{command, AppHandle, Emitter, State};
use crate::sensors::{
    SensorState, ScreenshotOptions, NetworkEvent, ConsoleEvent,
    NetworkFilter, ConsoleFilter, driver, Screenshot, sniffer, SnifferConfig, visual,
};

// ============================================================================
// Session Management Commands
// ============================================================================

/// Start a browser session
///
/// # Modes
/// - "parasitic": Attach to existing Chrome (must be running with --remote-debugging-port=9222)
/// - "managed": Spawn new Chrome instance (auto-detected path)
#[command]
pub async fn sensor_start_browser_session(
    mode: String,
    state: State<'_, SensorState>,
    app: AppHandle,
) -> Result<String, String> {
    let session_id = match mode.as_str() {
        "parasitic" => driver::start_parasitic_session(&state, None).await?,
        "managed" => driver::start_managed_session(&state, None).await?,
        _ => return Err("Invalid mode. Use 'parasitic' or 'managed'".to_string()),
    };

    // Emit event
    let _ = app.emit("sensor_session_started", serde_json::json!({
        "session_id": session_id,
        "mode": mode,
    }));

    Ok(session_id)
}

/// Stop a browser session
#[command]
pub async fn sensor_stop_browser_session(
    session_id: String,
    state: State<'_, SensorState>,
    app: AppHandle,
) -> Result<(), String> {
    driver::stop_session(&state, &session_id).await?;

    // Emit event
    let _ = app.emit("sensor_session_stopped", serde_json::json!({
        "session_id": session_id,
    }));

    Ok(())
}

/// Navigate to URL in a session (creates new tab)
#[command]
pub async fn sensor_navigate_to(
    session_id: String,
    url: String,
    state: State<'_, SensorState>,
    app: AppHandle,
) -> Result<String, String> {
    let tab_id = driver::navigate_to(&state, &session_id, &url).await?;

    // Emit event
    let _ = app.emit("sensor_navigation", serde_json::json!({
        "session_id": session_id,
        "tab_id": tab_id,
        "url": url,
    }));

    Ok(tab_id)
}

/// Get all active tabs in a session
#[command]
pub async fn sensor_get_active_tabs(
    session_id: String,
    state: State<'_, SensorState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    driver::get_active_tabs(&state, &session_id).await
}

/// Close a specific tab
#[command]
pub async fn sensor_close_tab(
    session_id: String,
    tab_id: String,
    state: State<'_, SensorState>,
    app: AppHandle,
) -> Result<(), String> {
    driver::close_tab(&state, &session_id, &tab_id).await?;

    // Emit event
    let _ = app.emit("sensor_tab_closed", serde_json::json!({
        "session_id": session_id,
        "tab_id": tab_id,
    }));

    Ok(())
}

// ============================================================================
// Data Query Commands
// ============================================================================

/// Get network logs with optional filters
#[command]
pub async fn sensor_get_network_logs(
    session_id: String,
    limit: Option<usize>,
    state: State<'_, SensorState>,
) -> Result<Vec<NetworkEvent>, String> {
    // Verify session exists
    if !driver::session_exists(&state, &session_id).await {
        return Err(format!("Session {} not found", session_id));
    }

    // Get logs from state
    let logs = state.network_logs.lock().await;
    let limit = limit.unwrap_or(100).min(1000); // Max 1000
    let result: Vec<NetworkEvent> = logs
        .iter()
        .rev() // Most recent first
        .take(limit)
        .cloned()
        .collect();

    Ok(result)
}

/// Get console logs with optional filters
#[command]
pub async fn sensor_get_console_logs(
    session_id: String,
    limit: Option<usize>,
    state: State<'_, SensorState>,
) -> Result<Vec<ConsoleEvent>, String> {
    // Verify session exists
    if !driver::session_exists(&state, &session_id).await {
        return Err(format!("Session {} not found", session_id));
    }

    // Get logs from state
    let logs = state.console_logs.lock().await;
    let limit = limit.unwrap_or(100).min(500); // Max 500
    let result: Vec<ConsoleEvent> = logs
        .iter()
        .rev() // Most recent first
        .take(limit)
        .cloned()
        .collect();

    Ok(result)
}

/// Get all screenshots
#[command]
pub async fn sensor_get_screenshots(
    session_id: String,
    state: State<'_, SensorState>,
) -> Result<Vec<Screenshot>, String> {
    // Verify session exists
    if !driver::session_exists(&state, &session_id).await {
        return Err(format!("Session {} not found", session_id));
    }

    // Get screenshots from cache
    let cache = state.screenshots.lock().await;
    let result: Vec<Screenshot> = cache
        .iter()
        .map(|(_, screenshot)| screenshot.clone())
        .collect();

    Ok(result)
}

/// Clear all captured data (network logs, console logs, screenshots)
#[command]
pub async fn sensor_clear_data(
    state: State<'_, SensorState>,
) -> Result<(), String> {
    // Clear network logs
    {
        let mut logs = state.network_logs.lock().await;
        logs.clear();
    }

    // Clear console logs
    {
        let mut logs = state.console_logs.lock().await;
        logs.clear();
    }

    // Clear screenshots
    {
        let mut cache = state.screenshots.lock().await;
        cache.clear();
    }

    Ok(())
}

// ============================================================================
// Configuration Commands
// ============================================================================

/// Get current sensor configuration
#[command]
pub async fn sensor_get_config(
    state: State<'_, SensorState>,
) -> Result<crate::sensors::SensorConfig, String> {
    let config = state.config.lock().await;
    Ok(config.clone())
}

/// Update sensor configuration
#[command]
pub async fn sensor_update_config(
    new_config: crate::sensors::SensorConfig,
    state: State<'_, SensorState>,
) -> Result<(), String> {
    let mut config = state.config.lock().await;
    *config = new_config;
    Ok(())
}

// ============================================================================
// Status Commands
// ============================================================================

/// Get sensor system status
#[command]
pub async fn sensor_get_status(
    state: State<'_, SensorState>,
) -> Result<SensorStatus, String> {
    let sessions = state.sessions.lock().await;
    let network_logs = state.network_logs.lock().await;
    let console_logs = state.console_logs.lock().await;
    let screenshots = state.screenshots.lock().await;

    let active_sessions: Vec<SessionInfo> = sessions.iter().map(|(id, session)| {
        SessionInfo {
            session_id: id.clone(),
            mode: match &session.mode {
                crate::sensors::SessionMode::Parasitic { ws_url } => {
                    format!("Parasitic ({})", ws_url)
                }
                crate::sensors::SessionMode::Managed { chrome_path } => {
                    format!("Managed ({})", chrome_path)
                }
            },
            active_tabs: session.active_tabs.len(),
            created_at: session.created_at
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }).collect();

    Ok(SensorStatus {
        active_sessions,
        network_events_count: network_logs.len(),
        console_events_count: console_logs.len(),
        screenshots_count: screenshots.len(),
    })
}

// ============================================================================
// Helper Types
// ============================================================================

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorStatus {
    pub active_sessions: Vec<SessionInfo>,
    pub network_events_count: usize,
    pub console_events_count: usize,
    pub screenshots_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub mode: String,
    pub active_tabs: usize,
    pub created_at: u64,
}

// ============================================================================
// Placeholder Commands (to be implemented later)
// ============================================================================

/// Enable sniffer on a tab
///
/// Attaches CDP network and console event listeners to capture:
/// - Network requests/responses
/// - Console logs and exceptions
///
/// Uses default SnifferConfig with noise filtering enabled.
#[command]
pub async fn sensor_enable_sniffer(
    session_id: String,
    tab_id: String,
    state: State<'_, SensorState>,
    app: AppHandle,
) -> Result<(), String> {
    // Enable sniffer with default configuration
    sniffer::enable_sniffer(&state, &session_id, &tab_id, SnifferConfig::default()).await?;

    // Emit event
    let _ = app.emit("sensor_sniffer_enabled", serde_json::json!({
        "session_id": session_id,
        "tab_id": tab_id,
    }));

    Ok(())
}

/// Capture screenshot from a browser tab
///
/// Captures, optimizes, and caches a screenshot from the specified tab.
///
/// # Features
/// - Multiple capture modes (viewport, full page, element)
/// - Format conversion (PNG, JPEG, WebP)
/// - Quality compression
/// - Automatic resizing for token efficiency
/// - LRU caching (max 50 screenshots)
///
/// # Returns
/// Screenshot with base64-encoded image data and token cost estimate
#[command]
pub async fn sensor_capture_screenshot(
    session_id: String,
    tab_id: String,
    options: Option<ScreenshotOptions>,
    state: State<'_, SensorState>,
    app: AppHandle,
) -> Result<Screenshot, String> {
    let options = options.unwrap_or_default();

    // Capture screenshot via Visual Cortex
    let screenshot = visual::capture_screenshot(&state, &session_id, &tab_id, options)
        .await
        .map_err(|e| e.to_string())?;

    // Emit event
    let _ = app.emit("sensor_screenshot_captured", serde_json::json!({
        "session_id": session_id,
        "tab_id": tab_id,
        "screenshot_id": screenshot.id,
        "width": screenshot.width,
        "height": screenshot.height,
        "token_cost": visual::estimate_token_cost(screenshot.width, screenshot.height),
    }));

    Ok(screenshot)
}
