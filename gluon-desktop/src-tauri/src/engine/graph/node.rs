// Node - Core abstraction for executable graph nodes

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::engine::execution::{ControlFlow, ExecutionContext};
use crate::engine::EngineResult;

/// Node type discriminator for serialization and type checking
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NodeType {
    /// LLM agent call (Claude, GPT, etc.)
    Agent,

    /// Logic nodes (If/Switch/Loop)
    Logic,

    /// Action nodes (shell commands, file I/O)
    Action,

    /// Fork node - spawns parallel branches (Phase 2)
    Fork,

    /// Join node - merges parallel results (Phase 2)
    Join,

    /// Bridge to old workflow system
    WorkflowBridge,
}

impl NodeType {
    /// Check if this node type requires LLM calls
    pub fn requires_llm(&self) -> bool {
        matches!(self, NodeType::Agent)
    }

    /// Check if this node type is a control flow node
    pub fn is_control_flow(&self) -> bool {
        matches!(self, NodeType::Logic | NodeType::Fork | NodeType::Join)
    }
}

/// Core trait for all executable nodes in the graph
///
/// Nodes are the fundamental building blocks of execution graphs.
/// Each node implements custom logic and returns a ControlFlow to
/// direct the executor's next action.
#[async_trait]
pub trait Node: Send + Sync {
    /// Unique identifier for this node instance
    fn id(&self) -> &str;

    /// Human-readable name for this node
    fn name(&self) -> &str;

    /// Node type discriminator
    fn node_type(&self) -> NodeType;

    /// Execute the node's logic
    ///
    /// This is the main entry point for node execution.
    /// The node can:
    /// - Read from the blackboard (via context.blackboard)
    /// - Write results to the blackboard
    /// - Track budget usage (for LLM calls)
    /// - Return ControlFlow to direct execution
    ///
    /// # Arguments
    /// * `context` - Mutable execution context with blackboard, budget, etc.
    ///
    /// # Returns
    /// * `ControlFlow` - What the executor should do next
    async fn execute(&self, context: &mut ExecutionContext) -> EngineResult<ControlFlow>;

    /// Optional cleanup on node exit
    ///
    /// Called when execution leaves this node (whether successful or not).
    /// Useful for releasing resources, logging, etc.
    async fn cleanup(&self, _context: &mut ExecutionContext) -> EngineResult<()> {
        Ok(()) // Default: no cleanup
    }

    /// Serialize node configuration for checkpointing
    ///
    /// This should return a JSON representation of the node's configuration,
    /// NOT its runtime state (which is stored in the blackboard).
    fn serialize_config(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id(),
            "name": self.name(),
            "type": self.node_type(),
        })
    }

    /// Estimate cost (in cents) for executing this node
    ///
    /// Used for budget planning. Returns 0 for non-LLM nodes.
    fn estimated_cost(&self) -> u64 {
        0
    }
}

// Note: We cannot make Node object-safe and Serialize at the same time,
// because Box<dyn Node> cannot implement Serialize.
//
// Solution: Serialize only the node configuration (as JSON), then use a
// factory pattern to reconstruct nodes from their type + config.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type_classification() {
        assert!(NodeType::Agent.requires_llm());
        assert!(!NodeType::Logic.requires_llm());

        assert!(NodeType::Logic.is_control_flow());
        assert!(!NodeType::Agent.is_control_flow());
    }

    #[test]
    fn test_node_type_serialization() {
        let node_type = NodeType::Agent;
        let json = serde_json::to_string(&node_type).unwrap();
        assert_eq!(json, "\"Agent\"");

        let deserialized: NodeType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, NodeType::Agent);
    }
}
