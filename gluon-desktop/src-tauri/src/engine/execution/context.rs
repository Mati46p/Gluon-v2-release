// Execution Context - Runtime state during graph execution

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock};

use crate::engine::memory::{Blackboard, BudgetTracker};

/// Execution step record (for history/debugging)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Node ID that was executed
    pub node_id: String,

    /// Timestamp (Unix milliseconds)
    pub timestamp: i64,

    /// Optional blackboard snapshot (for time-travel debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blackboard_snapshot: Option<serde_json::Value>,
}

/// Runtime context during graph execution
///
/// Contains all mutable state that nodes can access:
/// - Blackboard (shared memory)
/// - Budget tracker (cost monitoring)
/// - Execution history (for debugging)
///
/// Thread-safe: Uses Arc<RwLock> for blackboard and Arc<Mutex> for budget.
pub struct ExecutionContext {
    /// Shared memory accessible by all nodes
    /// Wrapped in RwLock for concurrent reads, exclusive writes
    pub blackboard: Arc<RwLock<Blackboard>>,

    /// Budget tracker for LLM cost monitoring
    /// Wrapped in Mutex for thread-safe mutation
    pub budget: Arc<Mutex<BudgetTracker>>,

    /// Execution history (node execution order)
    /// Useful for debugging and time-travel
    pub history: Vec<ExecutionStep>,

    /// Unique execution ID
    pub execution_id: String,

    /// Execution start timestamp (Unix seconds)
    pub started_at: i64,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(
        blackboard: Arc<RwLock<Blackboard>>,
        budget: Arc<Mutex<BudgetTracker>>,
    ) -> Self {
        Self {
            blackboard,
            budget,
            history: Vec::new(),
            execution_id: uuid::Uuid::new_v4().to_string(),
            started_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Record an execution step
    ///
    /// Adds to history for debugging and time-travel.
    /// Optionally captures full blackboard snapshot.
    pub fn record_step(&mut self, node_id: &str, capture_snapshot: bool) {
        let snapshot = if capture_snapshot {
            let bb = self.blackboard.read().unwrap();
            Some(bb.to_json())
        } else {
            None
        };

        self.history.push(ExecutionStep {
            node_id: node_id.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            blackboard_snapshot: snapshot,
        });
    }

    /// Get execution duration in seconds
    pub fn duration_secs(&self) -> i64 {
        chrono::Utc::now().timestamp() - self.started_at
    }

    /// Get number of steps executed
    pub fn step_count(&self) -> usize {
        self.history.len()
    }

    /// Get most recent node ID from history
    pub fn last_node(&self) -> Option<&str> {
        self.history.last().map(|step| step.node_id.as_str())
    }

    /// Check if budget is exceeded
    pub fn is_budget_exceeded(&self) -> bool {
        self.budget.lock().unwrap().is_exceeded()
    }

    /// Get budget summary
    pub fn budget_summary(&self) -> String {
        self.budget.lock().unwrap().summary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_context() -> ExecutionContext {
        let blackboard = Arc::new(RwLock::new(Blackboard::new()));
        let budget = Arc::new(Mutex::new(BudgetTracker::new(1000)));
        ExecutionContext::new(blackboard, budget)
    }

    #[test]
    fn test_context_creation() {
        let ctx = create_test_context();
        assert_eq!(ctx.step_count(), 0);
        assert!(ctx.execution_id.len() > 0);
        assert!(!ctx.is_budget_exceeded());
    }

    #[test]
    fn test_record_step() {
        let mut ctx = create_test_context();

        ctx.record_step("node_a", false);
        ctx.record_step("node_b", false);
        ctx.record_step("node_c", false);

        assert_eq!(ctx.step_count(), 3);
        assert_eq!(ctx.last_node(), Some("node_c"));

        assert_eq!(ctx.history[0].node_id, "node_a");
        assert_eq!(ctx.history[1].node_id, "node_b");
        assert_eq!(ctx.history[2].node_id, "node_c");
    }

    #[test]
    fn test_step_with_snapshot() {
        let mut ctx = create_test_context();

        // Add data to blackboard
        {
            let mut bb = ctx.blackboard.write().unwrap();
            bb.insert_string("test_key".to_string(), "test_value".to_string());
        }

        ctx.record_step("node_with_snapshot", true);

        let step = &ctx.history[0];
        assert!(step.blackboard_snapshot.is_some());

        let snapshot = step.blackboard_snapshot.as_ref().unwrap();
        assert_eq!(
            snapshot.get("test_key").and_then(|v| v.as_str()),
            Some("test_value")
        );
    }

    #[test]
    fn test_budget_tracking() {
        let ctx = create_test_context();

        // Record LLM call
        {
            let mut budget = ctx.budget.lock().unwrap();
            budget.record_llm_call("gpt-3.5-turbo", 1000, 500).unwrap();
        }

        assert!(!ctx.is_budget_exceeded());

        let summary = ctx.budget_summary();
        assert!(summary.contains("Budget"));
    }

    #[test]
    fn test_duration() {
        let ctx = create_test_context();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(ctx.duration_secs() >= 0);
    }
}
