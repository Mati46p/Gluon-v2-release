// Control Flow - Result of node execution that tells executor what to do next

use serde::{Deserialize, Serialize};

/// Result of node execution - directs the executor's next action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlFlow {
    /// Continue to next node(s) via outgoing edges
    /// The executor will find edges from current node and follow them
    Continue,

    /// Jump directly to a specific node (for loops, error recovery)
    /// Used by LoopNode to create cycles
    Jump(String),  // target node ID

    /// Suspend execution and save checkpoint
    /// Can be resumed later from this point
    Suspend {
        reason: String,
        resume_at: String,  // Node ID to resume at
    },

    /// Execution complete (success or failure)
    /// Terminates the execution loop
    Finish {
        success: bool,
        message: String,
    },

    /// Spawn parallel workers (Phase 2 - Fork-Join)
    /// Used by ForkNode to execute branches concurrently
    #[cfg(feature = "fork_join")]
    Fork {
        fork_id: String,
        branch_node_ids: Vec<String>,
    },

    /// Wait for parallel completion (Phase 2 - Fork-Join)
    /// Used by JoinNode to synchronize parallel branches
    #[cfg(feature = "fork_join")]
    Join {
        fork_id: String,
    },
}

impl ControlFlow {
    /// Create a successful finish
    pub fn finish_success(message: impl Into<String>) -> Self {
        ControlFlow::Finish {
            success: true,
            message: message.into(),
        }
    }

    /// Create a failed finish
    pub fn finish_failure(message: impl Into<String>) -> Self {
        ControlFlow::Finish {
            success: false,
            message: message.into(),
        }
    }

    /// Create a suspend
    pub fn suspend(reason: impl Into<String>, resume_at: impl Into<String>) -> Self {
        ControlFlow::Suspend {
            reason: reason.into(),
            resume_at: resume_at.into(),
        }
    }

    /// Create a jump
    pub fn jump(target: impl Into<String>) -> Self {
        ControlFlow::Jump(target.into())
    }

    /// Check if this is a terminating control flow
    pub fn is_terminal(&self) -> bool {
        matches!(self, ControlFlow::Finish { .. } | ControlFlow::Suspend { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_flow_creation() {
        let cf = ControlFlow::finish_success("All done");
        assert!(cf.is_terminal());

        let cf = ControlFlow::jump("node_123");
        assert!(!cf.is_terminal());

        let cf = ControlFlow::Continue;
        assert!(!cf.is_terminal());
    }

    #[test]
    fn test_serialization() {
        let cf = ControlFlow::finish_success("Complete");
        let json = serde_json::to_string(&cf).unwrap();
        let deserialized: ControlFlow = serde_json::from_str(&json).unwrap();

        match deserialized {
            ControlFlow::Finish { success, message } => {
                assert!(success);
                assert_eq!(message, "Complete");
            }
            _ => panic!("Wrong variant"),
        }
    }
}
