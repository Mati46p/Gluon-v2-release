/**
 * KROK 42: Security - Rate Limiting for WebSocket Messages
 *
 * Prevents DoS attacks by limiting message frequency
 */
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Rate limiter for WebSocket connections
pub struct RateLimiter {
    /// Map of connection ID -> request timestamps
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    /// Maximum requests per window
    max_requests: usize,
    /// Time window for rate limiting (seconds)
    window_duration: Duration,
}

impl RateLimiter {
    /// Create new rate limiter
    ///
    /// Default: 100 requests per 10 seconds
    pub fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests: 100,
            window_duration: Duration::from_secs(10),
        }
    }

    /// Create with custom limits
    pub fn with_limits(max_requests: usize, window_seconds: u64) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window_duration: Duration::from_secs(window_seconds),
        }
    }

    /// Check if request is allowed for this connection
    ///
    /// Returns true if allowed, false if rate limit exceeded
    pub fn check_rate_limit(&self, connection_id: &str) -> bool {
        let mut requests = self.requests.lock().unwrap();

        // Get or create request list for this connection
        let request_times = requests
            .entry(connection_id.to_string())
            .or_insert_with(Vec::new);

        let now = Instant::now();

        // Remove old requests outside the window
        request_times.retain(|&time| now.duration_since(time) < self.window_duration);

        // Check if limit exceeded
        if request_times.len() >= self.max_requests {
            return false;
        }

        // Add current request
        request_times.push(now);

        true
    }

    /// Clean up old connection data
    pub fn cleanup(&self) {
        let mut requests = self.requests.lock().unwrap();

        // Remove connections with no recent requests
        let now = Instant::now();
        requests.retain(|_, times| {
            times
                .iter()
                .any(|&time| now.duration_since(time) < self.window_duration)
        });
    }

    /// Get current request count for connection
    pub fn get_request_count(&self, connection_id: &str) -> usize {
        let requests = self.requests.lock().unwrap();
        requests.get(connection_id).map(|v| v.len()).unwrap_or(0)
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_rate_limiter_basic() {
        let limiter = RateLimiter::with_limits(5, 1);

        // First 5 requests should pass
        for _ in 0..5 {
            assert!(limiter.check_rate_limit("conn1"));
        }

        // 6th request should fail
        assert!(!limiter.check_rate_limit("conn1"));
    }

    #[test]
    fn test_rate_limiter_window() {
        let limiter = RateLimiter::with_limits(3, 1);

        // Use up the limit
        for _ in 0..3 {
            assert!(limiter.check_rate_limit("conn1"));
        }

        // Should be blocked
        assert!(!limiter.check_rate_limit("conn1"));

        // Wait for window to expire
        thread::sleep(Duration::from_millis(1100));

        // Should work again
        assert!(limiter.check_rate_limit("conn1"));
    }

    #[test]
    fn test_rate_limiter_different_connections() {
        let limiter = RateLimiter::with_limits(2, 1);

        // Each connection has its own limit
        assert!(limiter.check_rate_limit("conn1"));
        assert!(limiter.check_rate_limit("conn1"));
        assert!(!limiter.check_rate_limit("conn1")); // conn1 blocked

        assert!(limiter.check_rate_limit("conn2"));
        assert!(limiter.check_rate_limit("conn2"));
        assert!(!limiter.check_rate_limit("conn2")); // conn2 blocked
    }

    #[test]
    fn test_cleanup() {
        let limiter = RateLimiter::with_limits(5, 1);

        limiter.check_rate_limit("conn1");
        limiter.check_rate_limit("conn2");

        assert_eq!(limiter.get_request_count("conn1"), 1);
        assert_eq!(limiter.get_request_count("conn2"), 1);

        // Wait for expiry
        thread::sleep(Duration::from_millis(1100));

        // Cleanup should remove old data
        limiter.cleanup();

        // Counts should be 0 after cleanup
        assert_eq!(limiter.get_request_count("conn1"), 0);
        assert_eq!(limiter.get_request_count("conn2"), 0);
    }
}
