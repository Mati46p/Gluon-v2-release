//! Event Sniffer - Network & Console Capture
//!
//! Captures network requests/responses and console logs via CDP.
//! Includes noise filtering to reduce memory usage.
//!
//! # Architecture
//!
//! The sniffer attaches to Chrome DevTools Protocol (CDP) events:
//! - **Network Domain**: requestWillBeSent, responseReceived, loadingFinished
//! - **Runtime Domain**: consoleAPICalled, exceptionThrown
//!
//! Events are filtered based on SnifferConfig and queued to ring buffers
//! in SensorState (max 1000 network, 500 console).
//!
//! # Usage
//!
//! ```rust
//! // Start browser session
//! let session_id = driver::start_parasitic_session(&state, None).await?;
//!
//! // Navigate to URL (this creates a Page)
//! let tab_id = driver::navigate_to(&state, &session_id, "https://example.com").await?;
//!
//! // Enable sniffer for this tab
//! enable_sniffer(&state, &session_id, &tab_id, SnifferConfig::default()).await?;
//! ```

use crate::sensors::{SensorState, NetworkEvent, ConsoleEvent, SnifferConfig, ConsoleLevel, add_network_event, add_console_event};
use chromiumoxide::cdp::browser_protocol::network;
use chromiumoxide::cdp::js_protocol::runtime as cdp_runtime;
use chromiumoxide::Page;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Sniffer State
// ============================================================================

/// Per-tab sniffer state
///
/// Tracks request timing and stores partial request data until response arrives
struct SnifferTabState {
    /// Request ID -> (start_time_ms, request_data)
    pending_requests: Arc<TokioMutex<HashMap<String, (u64, PartialNetworkEvent)>>>,

    /// Configuration
    config: SnifferConfig,
}

/// Partial network event (before response arrives)
#[derive(Debug, Clone)]
struct PartialNetworkEvent {
    url: String,
    method: String,
    request_headers: HashMap<String, String>,
    request_body: Option<String>,
    resource_type: String,
    timestamp: u64,
}

impl SnifferTabState {
    fn new(config: SnifferConfig) -> Self {
        Self {
            pending_requests: Arc::new(TokioMutex::new(HashMap::new())),
            config,
        }
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Enable network and console sniffer for a specific tab
///
/// # Arguments
///
/// * `state` - Global sensor state
/// * `session_id` - Browser session ID
/// * `tab_id` - Tab ID (from navigate_to)
/// * `config` - Sniffer configuration
///
/// # Returns
///
/// Ok(()) if sniffer was enabled successfully
///
/// # Errors
///
/// Returns error if session/tab not found or CDP commands fail
pub async fn enable_sniffer(
    state: &SensorState,
    session_id: &str,
    tab_id: &str,
    config: SnifferConfig,
) -> Result<(), String> {
    // Get browser session
    let sessions = state.sessions.lock().await;
    let session = sessions.get(session_id)
        .ok_or_else(|| format!("Session {} not found", session_id))?;

    // Get page for this tab
    let page = session.browser.pages().await
        .map_err(|e| format!("Failed to get pages: {}", e))?
        .into_iter()
        .find(|p| p.target_id().as_ref() == tab_id)
        .ok_or_else(|| format!("Tab {} not found in session {}", tab_id, session_id))?;

    drop(sessions); // Release lock

    // Enable CDP domains
    enable_network_domain(&page).await?;
    enable_runtime_domain(&page).await?;

    // Create per-tab state
    let tab_state = Arc::new(SnifferTabState::new(config));

    // Spawn background tasks to handle events
    spawn_network_listeners(state.clone(), page.clone(), tab_state.clone());
    spawn_console_listeners(state.clone(), page.clone(), tab_state.clone());

    eprintln!("[Sniffer] Enabled CDP domains and event listeners for tab: {}", tab_id);
    Ok(())
}

// ============================================================================
// CDP Event Listeners - Background Tasks
// ============================================================================

/// Spawn network event listeners for capturing HTTP traffic
fn spawn_network_listeners(
    state: SensorState,
    page: Page,
    tab_state: Arc<SnifferTabState>,
) {
    use futures_util::StreamExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Helper to get timestamp in milliseconds
    let get_timestamp = || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    };

    // Listen to requestWillBeSent - captures outgoing requests
    {
        let state = state.clone();
        let page = page.clone();
        let tab_state = tab_state.clone();

        tokio::spawn(async move {
            if let Ok(mut stream) = page.event_listener::<network::EventRequestWillBeSent>().await {
                while let Some(event) = stream.next().await {
                    let request_id = event.request_id.as_ref().to_string();
                    let url = event.request.url.clone();
                    let method = event.request.method.clone();
                    let resource_type = event.r#type.as_ref().map(|t| format!("{:?}", t)).unwrap_or_else(|| "Other".to_string());

                    // Check noise filter
                    if should_ignore_request(&tab_state.config, &url, &resource_type) {
                        continue;
                    }

                    let timestamp = get_timestamp();

                    // Create partial network event
                    let partial = PartialNetworkEvent {
                        url: url.clone(),
                        method: method.clone(),
                        request_headers: HashMap::new(), // TODO: Parse Headers type
                        request_body: None, // TODO: Get post data via CDP
                        resource_type: resource_type.clone(),
                        timestamp,
                    };

                    // Store pending request
                    {
                        let mut pending = tab_state.pending_requests.lock().await;
                        pending.insert(request_id.clone(), (timestamp, partial));
                    }

                    eprintln!("[Sniffer] Request: {} {}", method, url);
                }
            }
        });
    }

    // Listen to responseReceived - captures responses and creates full NetworkEvent
    {
        let state = state.clone();
        let page = page.clone();
        let tab_state = tab_state.clone();

        tokio::spawn(async move {
            if let Ok(mut stream) = page.event_listener::<network::EventResponseReceived>().await {
                while let Some(event) = stream.next().await {
                    let request_id = event.request_id.as_ref().to_string();
                    let status_code = event.response.status as u16;
                    let response_headers: HashMap<String, String> = HashMap::new(); // TODO: Parse Headers type

                    // Retrieve pending request
                    let request_data = {
                        let mut pending = tab_state.pending_requests.lock().await;
                        pending.remove(&request_id)
                    };

                    if let Some((start_time, partial)) = request_data {
                        let now = get_timestamp();
                        let duration_ms = now.saturating_sub(start_time);

                        // Create full NetworkEvent
                        let network_event = NetworkEvent {
                            request_id: request_id.clone(),
                            url: partial.url.clone(),
                            method: partial.method.clone(),
                            status_code: Some(status_code),
                            request_headers: partial.request_headers,
                            response_headers: Some(response_headers),
                            request_body: partial.request_body,
                            response_body: None, // TODO: Implement body capture with getResponseBody
                            resource_type: partial.resource_type,
                            timestamp: partial.timestamp,
                            duration_ms: Some(duration_ms),
                        };

                        // Add to ring buffer (max 1000 entries)
                        {
                            let mut logs = state.network_logs.lock().await;
                            logs.push_back(network_event.clone());
                            if logs.len() > 1000 {
                                logs.pop_front();
                            }
                        }

                        eprintln!("[Sniffer] Captured: {} {} ({}) - {}ms",
                            network_event.method, network_event.url, status_code, duration_ms);
                    }
                }
            }
        });
    }
}

/// Spawn console event listeners for capturing logs and exceptions
fn spawn_console_listeners(
    state: SensorState,
    page: Page,
    _tab_state: Arc<SnifferTabState>,
) {
    use futures_util::StreamExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Helper to get timestamp in milliseconds
    let get_timestamp = || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    };

    // Listen to consoleAPICalled - captures console.log/warn/error
    {
        let state = state.clone();
        let page = page.clone();

        tokio::spawn(async move {
            if let Ok(mut stream) = page.event_listener::<cdp_runtime::EventConsoleApiCalled>().await {
                while let Some(event) = stream.next().await {
                    let level = ConsoleLevel::from_cdp(&format!("{:?}", event.r#type).to_lowercase());
                    let message = event.args.iter()
                        .map(|arg| {
                            arg.value.as_ref()
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string()
                        })
                        .collect::<Vec<_>>()
                        .join(" ");

                    let console_event = ConsoleEvent {
                        level: level.clone(),
                        message: message.clone(),
                        stack_trace: event.stack_trace.as_ref().map(|st| {
                            st.call_frames.iter()
                                .map(|cf| format!("at {} ({}:{}:{})",
                                    cf.function_name, cf.url, cf.line_number, cf.column_number))
                                .collect::<Vec<_>>()
                                .join("\n")
                        }),
                        timestamp: get_timestamp(),
                        url: event.stack_trace.as_ref()
                            .and_then(|st| st.call_frames.first())
                            .map(|cf| cf.url.clone()),
                        line: event.stack_trace.as_ref()
                            .and_then(|st| st.call_frames.first())
                            .map(|cf| cf.line_number as u32),
                        column: event.stack_trace.as_ref()
                            .and_then(|st| st.call_frames.first())
                            .map(|cf| cf.column_number as u32),
                    };

                    // Add to ring buffer (max 500 entries)
                    {
                        let mut logs = state.console_logs.lock().await;
                        logs.push_back(console_event.clone());
                        if logs.len() > 500 {
                            logs.pop_front();
                        }
                    }

                    eprintln!("[Sniffer] Console {:?}: {}", level, message);
                }
            }
        });
    }

    // Listen to exceptionThrown - captures runtime exceptions
    {
        let state = state.clone();
        let page = page.clone();

        tokio::spawn(async move {
            if let Ok(mut stream) = page.event_listener::<cdp_runtime::EventExceptionThrown>().await {
                while let Some(event) = stream.next().await {
                    let exception = &event.exception_details;
                    let message = exception.text.clone();

                    let console_event = ConsoleEvent {
                        level: ConsoleLevel::Error,
                        message: format!("Exception: {}", message),
                        stack_trace: exception.stack_trace.as_ref().map(|st| {
                            st.call_frames.iter()
                                .map(|cf| format!("at {} ({}:{}:{})",
                                    cf.function_name, cf.url, cf.line_number, cf.column_number))
                                .collect::<Vec<_>>()
                                .join("\n")
                        }),
                        timestamp: get_timestamp(),
                        url: exception.url.clone(),
                        line: Some(exception.line_number as u32),
                        column: Some(exception.column_number as u32),
                    };

                    // Add to ring buffer (max 500 entries)
                    {
                        let mut logs = state.console_logs.lock().await;
                        logs.push_back(console_event.clone());
                        if logs.len() > 500 {
                            logs.pop_front();
                        }
                    }

                    eprintln!("[Sniffer] Exception: {}", message);
                }
            }
        });
    }
}

// ============================================================================
// CDP Domain Enablement
// ============================================================================

/// Enable Chrome DevTools Protocol Network domain
async fn enable_network_domain(page: &Page) -> Result<(), String> {
    page.execute(network::EnableParams::default()).await
        .map_err(|e| format!("Failed to enable Network domain: {}", e))?;
    Ok(())
}

/// Enable Chrome DevTools Protocol Runtime domain
async fn enable_runtime_domain(page: &Page) -> Result<(), String> {
    page.execute(cdp_runtime::EnableParams::default()).await
        .map_err(|e| format!("Failed to enable Runtime domain: {}", e))?;
    Ok(())
}

// ============================================================================
// Noise Filtering
// ============================================================================

/// Check if a request should be ignored based on noise filtering rules
fn should_ignore_request(config: &SnifferConfig, url: &str, resource_type: &str) -> bool {
    // Ignore tracking pixels (tiny images to analytics domains)
    if config.ignore_tracking_pixels && is_tracking_pixel(url, resource_type) {
        return true;
    }

    // Ignore common CDN requests
    if config.ignore_common_cdn && is_cdn_request(url) {
        return true;
    }

    // Ignore by file extension
    for ext in &config.ignore_extensions {
        if url.ends_with(ext) {
            return true;
        }
    }

    false
}

/// Detect tracking pixels (1x1 images to analytics domains)
fn is_tracking_pixel(url: &str, resource_type: &str) -> bool {
    if resource_type != "Image" {
        return false;
    }

    // Common tracking pixel patterns
    let tracking_domains = [
        "google-analytics.com",
        "googletagmanager.com",
        "facebook.com/tr",
        "doubleclick.net",
        "analytics.google.com",
        "pixel.facebook.com",
        "connect.facebook.net",
    ];

    tracking_domains.iter().any(|domain| url.contains(domain))
}

/// Detect common CDN requests
fn is_cdn_request(url: &str) -> bool {
    let cdn_domains = [
        "cloudflare.com",
        "cloudfront.net",
        "akamaihd.net",
        "fastly.net",
        "jsdelivr.net",
        "unpkg.com",
        "cdnjs.cloudflare.com",
        "fonts.googleapis.com",
        "fonts.gstatic.com",
    ];

    cdn_domains.iter().any(|domain| url.contains(domain))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracking_pixel_detection() {
        assert!(is_tracking_pixel("https://google-analytics.com/collect?v=1", "Image"));
        assert!(is_tracking_pixel("https://www.facebook.com/tr/?id=123", "Image"));
        assert!(!is_tracking_pixel("https://example.com/logo.png", "Image"));
        assert!(!is_tracking_pixel("https://google-analytics.com/script.js", "Script"));
    }

    #[test]
    fn test_cdn_detection() {
        assert!(is_cdn_request("https://cdnjs.cloudflare.com/ajax/libs/jquery/3.6.0/jquery.min.js"));
        assert!(is_cdn_request("https://fonts.googleapis.com/css?family=Roboto"));
        assert!(!is_cdn_request("https://example.com/api/data"));
    }

    #[test]
    fn test_should_ignore_request() {
        let config = SnifferConfig::default();

        // Should ignore tracking pixels
        assert!(should_ignore_request(&config, "https://google-analytics.com/pixel.gif", "Image"));

        // Should ignore CDN
        assert!(should_ignore_request(&config, "https://cdnjs.cloudflare.com/jquery.js", "Script"));

        // Should ignore .woff2 extension
        assert!(should_ignore_request(&config, "https://example.com/font.woff2", "Font"));

        // Should NOT ignore normal requests
        assert!(!should_ignore_request(&config, "https://example.com/api/data", "XHR"));
    }

    #[test]
    fn test_console_level_ordering() {
        assert!(ConsoleLevel::Info < ConsoleLevel::Warning);
        assert!(ConsoleLevel::Warning < ConsoleLevel::Error);
    }

    #[test]
    fn test_console_level_from_cdp() {
        assert_eq!(ConsoleLevel::from_cdp("log"), ConsoleLevel::Log);
        assert_eq!(ConsoleLevel::from_cdp("error"), ConsoleLevel::Error);
        assert_eq!(ConsoleLevel::from_cdp("warn"), ConsoleLevel::Warning);
    }
}

/*
 * NOTE: Event listener implementation pending chromiumoxide API documentation
 *
 * The full network and console event capture requires understanding the exact
 * event listener types in chromiumoxide 0.7. The implementation should follow
 * this pattern once type names are determined:
 *
 * ```rust
 * async fn spawn_network_listeners(state: SensorState, page: Page, tab_state: Arc<SnifferTabState>) {
 *     // Listen to requestWillBeSent
 *     tokio::spawn(async move {
 *         let mut stream = page.event_listener::<CORRECT_TYPE>().await.unwrap();
 *         while let Some(event) = stream.next().await {
 *             // Extract URL, method, headers from event
 *             // Store in pending_requests with timestamp
 *         }
 *     });
 *
 *     // Listen to responseReceived
 *     // ...similar pattern
 *
 *     // Listen to loadingFinished
 *     // ...capture body, create NetworkEvent, add_network_event()
 * }
 *
 * async fn spawn_console_listeners(state: SensorState, page: Page, tab_state: Arc<SnifferTabState>) {
 *     // Listen to consoleAPICalled
 *     // Listen to exceptionThrown
 *     // ...create ConsoleEvent, add_console_event()
 * }
 * ```
 *
 * The filtering logic (should_ignore_request) and helper functions are fully implemented
 * and ready to use once event capture is working.
 */
