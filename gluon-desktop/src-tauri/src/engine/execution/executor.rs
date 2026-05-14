// Graph Executor - Trampoline loop executor

use std::sync::{Arc, Mutex, RwLock};
use serde::{Deserialize, Serialize};

use crate::engine::graph::{ExecutionGraph, Node, NodeType};
use crate::engine::execution::{ControlFlow, ExecutionContext, SafetyGuard};
use crate::engine::memory::{Blackboard, BudgetTracker};
use crate::engine::{EngineError, EngineResult};

// Phase 5: UI Integration
#[cfg(feature = "ui")]
use crate::ui::events::{EventBus, UIEvent, NodeStatus};

/// Main graph executor with trampoline loop
///
/// Executes a graph iteratively (no recursion) using a trampoline pattern.
/// Supports cycles, conditional branching, and budget tracking.
pub struct GraphExecutor {
    /// The execution graph (immutable, shared)
    pub graph: Arc<ExecutionGraph>,

    /// Shared blackboard (thread-safe)
    pub blackboard: Arc<RwLock<Blackboard>>,

    /// Budget tracker (thread-safe)
    pub budget_tracker: Arc<Mutex<BudgetTracker>>,

    /// Safety guard for infinite loop detection
    safety_guard: Mutex<SafetyGuard>,

    /// Node factory for reconstructing nodes from JSON configs
    node_factory: Arc<NodeFactory>,

    /// Optional UI event bus for Command Center integration (Phase 5)
    #[cfg(feature = "ui")]
    ui_event_bus: Option<Arc<EventBus>>,
}

impl GraphExecutor {
    /// Create a new executor
    ///
    /// # Arguments
    /// * `graph` - The execution graph
    /// * `budget_limit_cents` - Maximum cost in cents (e.g., 100 = $1.00)
    pub fn new(graph: ExecutionGraph, budget_limit_cents: u64) -> Self {
        Self {
            graph: Arc::new(graph),
            blackboard: Arc::new(RwLock::new(Blackboard::new())),
            budget_tracker: Arc::new(Mutex::new(BudgetTracker::new(budget_limit_cents))),
            safety_guard: Mutex::new(SafetyGuard::new(10_000, 3600)),
            node_factory: Arc::new(NodeFactory::new()),
            #[cfg(feature = "ui")]
            ui_event_bus: None,
        }
    }

    /// Attach UI event bus for Command Center integration (Phase 5)
    #[cfg(feature = "ui")]
    pub fn with_ui_event_bus(mut self, event_bus: Arc<EventBus>) -> Self {
        self.ui_event_bus = Some(event_bus);
        self
    }

    /// Execute the graph from the entry node
    ///
    /// Uses trampoline loop - iterative execution without recursion.
    /// This prevents stack overflow even with deep cycles.
    pub async fn execute(&self, start: Option<String>) -> EngineResult<ExecutionResult> {
        // Validate graph
        self.graph.validate()?;

        // Create execution context
        let mut context = ExecutionContext::new(
            self.blackboard.clone(),
            self.budget_tracker.clone(),
        );

        // Determine starting node
        let mut current_node_id = start.unwrap_or_else(|| self.graph.entry_node.clone());

        // Trampoline loop - iterative execution
        let mut iteration_count = 0;

        loop {
            iteration_count += 1;

            // Safety checks
            {
                let mut guard = self.safety_guard.lock().unwrap();
                guard.check_iteration(&current_node_id, iteration_count)?;
                guard.check_timeout(context.started_at)?;
            }

            // Check budget
            if context.is_budget_exceeded() {
                let used = context.budget.lock().unwrap().cost_cents;
                let limit = context.budget.lock().unwrap().max_cost_cents;

                // 🎯 UI Hook: Budget exceeded
                #[cfg(feature = "ui")]
                if let Some(ref bus) = self.ui_event_bus {
                    bus.publish(UIEvent::BudgetUpdate {
                        used_cents: used,
                        limit_cents: limit,
                        percentage: (used as f32 / limit as f32) * 100.0,
                        timestamp: Self::current_timestamp(),
                    });
                }

                return Err(EngineError::BudgetExceeded { used, limit });
            }

            // Get node config
            let node_config = self.graph.node_configs.get(&current_node_id)
                .ok_or_else(|| EngineError::NodeNotFound(current_node_id.clone()))?;

            // Reconstruct node from config
            let node = self.node_factory.create_node(node_config)?;

            // Record execution step
            context.record_step(&current_node_id, false);

            // 🎯 UI Hook: Node started
            #[cfg(feature = "ui")]
            if let Some(ref bus) = self.ui_event_bus {
                bus.publish(UIEvent::node_started(
                    current_node_id.clone(),
                    node.name().to_string(),
                ));
            }

            // Execute node
            let control_flow = node.execute(&mut context).await?;

            // 🎯 UI Hook: Node completed successfully
            #[cfg(feature = "ui")]
            if let Some(ref bus) = self.ui_event_bus {
                bus.publish(UIEvent::node_completed(
                    current_node_id.clone(),
                    node.name().to_string(),
                ));
            }

            // Interpret control flow
            match control_flow {
                ControlFlow::Continue => {
                    // Find next node(s) via edges
                    let next_edges = self.graph.get_next_edges(&current_node_id, &context);

                    if next_edges.is_empty() {
                        // No outgoing edges - implicit finish
                        return Ok(ExecutionResult::Success {
                            execution_id: context.execution_id.clone(),
                            final_state: self.blackboard.clone(),
                            steps: context.history.len(),
                        });
                    }

                    // Take first matching edge
                    // (If multiple conditional edges match, priority is by order in graph.edges)
                    current_node_id = next_edges[0].to_node.clone();
                }

                ControlFlow::Jump(target_node_id) => {
                    // Direct jump (for loops)
                    if !self.graph.node_configs.contains_key(&target_node_id) {
                        return Err(EngineError::NodeNotFound(target_node_id));
                    }
                    current_node_id = target_node_id;
                }

                ControlFlow::Suspend { reason, resume_at } => {
                    // 🎯 UI Hook: Execution suspended
                    #[cfg(feature = "ui")]
                    if let Some(ref bus) = self.ui_event_bus {
                        bus.publish(UIEvent::ExecutionPaused {
                            reason: reason.clone(),
                            timestamp: Self::current_timestamp(),
                        });
                    }

                    // Suspend execution - will be checkpointed
                    return Ok(ExecutionResult::Suspended {
                        execution_id: context.execution_id.clone(),
                        reason,
                        resume_at,
                        current_state: self.blackboard.clone(),
                    });
                }

                ControlFlow::Finish { success, message } => {
                    // Explicit termination
                    if success {
                        return Ok(ExecutionResult::Success {
                            execution_id: context.execution_id.clone(),
                            final_state: self.blackboard.clone(),
                            steps: context.history.len(),
                        });
                    } else {
                        return Ok(ExecutionResult::Failed {
                            execution_id: context.execution_id.clone(),
                            error: message,
                            final_state: self.blackboard.clone(),
                        });
                    }
                }

                #[cfg(feature = "fork_join")]
                ControlFlow::Fork { .. } | ControlFlow::Join { .. } => {
                    return Err(EngineError::InvalidControlFlow(
                        "Fork/Join not implemented yet (Phase 2)".to_string()
                    ));
                }
            }
        }
    }

    /// Get execution statistics
    pub fn stats(&self) -> ExecutorStats {
        ExecutorStats {
            graph_stats: self.graph.stats(),
            budget_summary: self.budget_tracker.lock().unwrap().summary(),
        }
    }

    /// Helper for UI event timestamps
    #[cfg(feature = "ui")]
    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

/// Execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ExecutionResult {
    Success {
        execution_id: String,
        #[serde(skip)]
        final_state: Arc<RwLock<Blackboard>>,
        steps: usize,
    },
    Suspended {
        execution_id: String,
        reason: String,
        resume_at: String,
        #[serde(skip)]
        current_state: Arc<RwLock<Blackboard>>,
    },
    Failed {
        execution_id: String,
        error: String,
        #[serde(skip)]
        final_state: Arc<RwLock<Blackboard>>,
    },
}

impl ExecutionResult {
    pub fn execution_id(&self) -> &str {
        match self {
            ExecutionResult::Success { execution_id, .. } => execution_id,
            ExecutionResult::Suspended { execution_id, .. } => execution_id,
            ExecutionResult::Failed { execution_id, .. } => execution_id,
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionResult::Success { .. })
    }

    pub fn get_state(&self) -> Option<Arc<RwLock<Blackboard>>> {
        match self {
            ExecutionResult::Success { final_state, .. } => Some(final_state.clone()),
            ExecutionResult::Suspended { current_state, .. } => Some(current_state.clone()),
            ExecutionResult::Failed { final_state, .. } => Some(final_state.clone()),
        }
    }
}

/// Executor statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorStats {
    pub graph_stats: crate::engine::graph::GraphStats,
    pub budget_summary: String,
}

/// Node factory - Reconstructs Node instances from JSON configs
///
/// Since Box<dyn Node> cannot be serialized, we store node configurations
/// as JSON and use a factory pattern to reconstruct them.
pub struct NodeFactory {
    // In Phase 2, this will contain registered node constructors
}

impl NodeFactory {
    pub fn new() -> Self {
        Self {}
    }

    /// Create a node from JSON configuration
    pub fn create_node(&self, config: &serde_json::Value) -> EngineResult<Box<dyn Node>> {
        // Extract node type
        let node_type_str = config.get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::GraphError("Missing 'type' field in node config".to_string()))?;

        let node_type: NodeType = serde_json::from_value(serde_json::json!(node_type_str))
            .map_err(|e| EngineError::GraphError(format!("Invalid node type: {}", e)))?;

        // Create node based on type
        match node_type {
            NodeType::Agent => {
                let id = config.get("id").and_then(|v| v.as_str())
                    .ok_or_else(|| EngineError::GraphError("Missing 'id' field".to_string()))?;
                let name = config.get("name").and_then(|v| v.as_str())
                    .unwrap_or("Unnamed Agent");

                Ok(Box::new(crate::engine::nodes::AgentNode::new(id, name)))
            }

            NodeType::Logic => {
                let id = config.get("id").and_then(|v| v.as_str())
                    .ok_or_else(|| EngineError::GraphError("Missing 'id' field".to_string()))?;
                let name = config.get("name").and_then(|v| v.as_str())
                    .unwrap_or("Unnamed Logic");

                // Check if it's a Loop node based on config
                if config.get("loop_target").is_some() {
                    Ok(Box::new(crate::engine::nodes::LoopNode::new(id, name)))
                } else {
                    Ok(Box::new(crate::engine::nodes::IfNode::new(id, name)))
                }
            }

            NodeType::Action => {
                let id = config.get("id").and_then(|v| v.as_str())
                    .ok_or_else(|| EngineError::GraphError("Missing 'id' field".to_string()))?;
                let name = config.get("name").and_then(|v| v.as_str())
                    .unwrap_or("Unnamed Action");

                Ok(Box::new(crate::engine::nodes::ActionNode::new(id, name)))
            }

            NodeType::Fork | NodeType::Join => {
                Err(EngineError::GraphError("Fork/Join nodes not implemented yet (Phase 2)".to_string()))
            }

            NodeType::WorkflowBridge => {
                Err(EngineError::GraphError("WorkflowBridge not implemented yet".to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::graph::Edge;

    fn create_simple_graph() -> ExecutionGraph {
        let mut graph = ExecutionGraph::new("test_graph");

        // Node A -> Node B -> Node C
        graph.add_node_config(
            "node_a".to_string(),
            serde_json::json!({"id": "node_a", "name": "Node A", "type": "Agent"})
        );
        graph.add_node_config(
            "node_b".to_string(),
            serde_json::json!({"id": "node_b", "name": "Node B", "type": "Agent"})
        );
        graph.add_node_config(
            "node_c".to_string(),
            serde_json::json!({"id": "node_c", "name": "Node C", "type": "Agent"})
        );

        graph.add_edge(Edge::new_static("e1", "node_a", "node_b"));
        graph.add_edge(Edge::new_static("e2", "node_b", "node_c"));

        graph.set_entry_node("node_a".to_string());

        graph
    }

    #[tokio::test]
    async fn test_executor_creation() {
        let graph = create_simple_graph();
        let executor = GraphExecutor::new(graph, 100);

        assert_eq!(executor.graph.node_configs.len(), 3);
        assert_eq!(executor.graph.edges.len(), 2);
    }

    #[tokio::test]
    async fn test_simple_execution() {
        let graph = create_simple_graph();
        let executor = GraphExecutor::new(graph, 100);

        let result = executor.execute(None).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.is_success());
        assert_eq!(result.execution_id().len() > 0, true);
    }

    #[tokio::test]
    async fn test_node_factory() {
        let factory = NodeFactory::new();

        let config = serde_json::json!({
            "id": "test_agent",
            "name": "Test Agent",
            "type": "Agent"
        });

        let node = factory.create_node(&config).unwrap();
        assert_eq!(node.id(), "test_agent");
        assert_eq!(node.name(), "Test Agent");
        assert_eq!(node.node_type(), NodeType::Agent);
    }
}
