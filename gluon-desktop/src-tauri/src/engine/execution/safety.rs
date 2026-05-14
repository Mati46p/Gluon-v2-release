// Safety Guard - Infinite loop detection and timeout enforcement

use std::collections::HashMap;
use crate::engine::{EngineError, EngineResult};

/// Safety guard to prevent infinite loops and runaway executions
///
/// Tracks:
/// - Global iteration count (prevents deep cycles)
/// - Per-node visit count (detects tight loops)
/// - Execution duration (enforces timeout)
pub struct SafetyGuard {
    /// Maximum total iterations before aborting
    pub max_iterations: u32,

    /// Maximum execution time in seconds
    pub max_execution_time_secs: u64,

    /// Per-node visit counts (node_id -> count)
    pub visited_nodes: HashMap<String, u32>,

    /// Maximum visits per node before triggering tight loop detection
    max_visits_per_node: u32,
}

impl SafetyGuard {
    /// Create a new safety guard
    ///
    /// # Arguments
    /// * `max_iterations` - Total iteration limit (e.g., 10,000)
    /// * `max_execution_time_secs` - Timeout in seconds (e.g., 3600 = 1 hour)
    pub fn new(max_iterations: u32, max_execution_time_secs: u64) -> Self {
        Self {
            max_iterations,
            max_execution_time_secs,
            visited_nodes: HashMap::new(),
            max_visits_per_node: 100, // Detect tight loops (same node visited 100+ times)
        }
    }

    /// Check iteration count and per-node visit count
    ///
    /// Returns error if:
    /// - Total iterations exceed max_iterations
    /// - Single node visited more than max_visits_per_node times
    pub fn check_iteration(&mut self, node_id: &str, total_iterations: u32) -> EngineResult<()> {
        // Check global iteration limit
        if total_iterations > self.max_iterations {
            return Err(EngineError::InfiniteLoopDetected);
        }

        // Track per-node visits
        let visits = self.visited_nodes.entry(node_id.to_string()).or_insert(0);
        *visits += 1;

        // Check tight loop detection
        if *visits > self.max_visits_per_node {
            return Err(EngineError::ExecutionError(format!(
                "Tight loop detected: node '{}' visited {} times (limit: {})",
                node_id, visits, self.max_visits_per_node
            )));
        }

        Ok(())
    }

    /// Check execution timeout
    ///
    /// # Arguments
    /// * `start_time` - Execution start timestamp (Unix seconds)
    pub fn check_timeout(&self, start_time: i64) -> EngineResult<()> {
        let current_time = chrono::Utc::now().timestamp();
        let elapsed = (current_time - start_time) as u64;

        if elapsed > self.max_execution_time_secs {
            return Err(EngineError::TimeoutExceeded);
        }

        Ok(())
    }

    /// Reset all counters (for resuming execution)
    pub fn reset(&mut self) {
        self.visited_nodes.clear();
    }

    /// Get visit count for a specific node
    pub fn get_visit_count(&self, node_id: &str) -> u32 {
        *self.visited_nodes.get(node_id).unwrap_or(&0)
    }

    /// Get total unique nodes visited
    pub fn unique_nodes_visited(&self) -> usize {
        self.visited_nodes.len()
    }

    /// Get node with highest visit count
    pub fn most_visited_node(&self) -> Option<(String, u32)> {
        self.visited_nodes
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(id, count)| (id.clone(), *count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iteration_check_under_limit() {
        let mut guard = SafetyGuard::new(100, 3600);

        for i in 1..=50 {
            assert!(guard.check_iteration("node_a", i).is_ok());
        }
    }

    #[test]
    fn test_iteration_check_exceeds_global_limit() {
        let mut guard = SafetyGuard::new(100, 3600);

        let result = guard.check_iteration("node_a", 101);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EngineError::InfiniteLoopDetected));
    }

    #[test]
    fn test_tight_loop_detection() {
        let mut guard = SafetyGuard::new(10_000, 3600);

        // Visit same node many times
        for i in 1..=101 {
            let result = guard.check_iteration("tight_loop_node", i);
            if i <= 100 {
                assert!(result.is_ok(), "Iteration {} should succeed", i);
            } else {
                // 101st visit should trigger tight loop detection
                assert!(result.is_err(), "Iteration {} should fail", i);
            }
        }
    }

    #[test]
    fn test_multiple_nodes_tracking() {
        let mut guard = SafetyGuard::new(1000, 3600);

        guard.check_iteration("node_a", 1).unwrap();
        guard.check_iteration("node_a", 2).unwrap();
        guard.check_iteration("node_b", 3).unwrap();
        guard.check_iteration("node_c", 4).unwrap();
        guard.check_iteration("node_a", 5).unwrap();

        assert_eq!(guard.get_visit_count("node_a"), 3);
        assert_eq!(guard.get_visit_count("node_b"), 1);
        assert_eq!(guard.get_visit_count("node_c"), 1);
        assert_eq!(guard.unique_nodes_visited(), 3);
    }

    #[test]
    fn test_timeout_check() {
        let guard = SafetyGuard::new(1000, 2); // 2 second timeout

        let start_time = chrono::Utc::now().timestamp();

        // Immediately - should pass
        assert!(guard.check_timeout(start_time).is_ok());

        // Simulate 3 seconds ago - should fail
        let past_time = start_time - 3;
        let result = guard.check_timeout(past_time);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EngineError::TimeoutExceeded));
    }

    #[test]
    fn test_reset() {
        let mut guard = SafetyGuard::new(1000, 3600);

        guard.check_iteration("node_a", 1).unwrap();
        guard.check_iteration("node_b", 2).unwrap();
        assert_eq!(guard.unique_nodes_visited(), 2);

        guard.reset();
        assert_eq!(guard.unique_nodes_visited(), 0);
        assert_eq!(guard.get_visit_count("node_a"), 0);
    }

    #[test]
    fn test_most_visited_node() {
        let mut guard = SafetyGuard::new(1000, 3600);

        guard.check_iteration("node_a", 1).unwrap();
        guard.check_iteration("node_a", 2).unwrap();
        guard.check_iteration("node_a", 3).unwrap();
        guard.check_iteration("node_b", 4).unwrap();
        guard.check_iteration("node_c", 5).unwrap();
        guard.check_iteration("node_c", 6).unwrap();

        let (most_visited, count) = guard.most_visited_node().unwrap();
        assert_eq!(most_visited, "node_a");
        assert_eq!(count, 3);
    }
}
