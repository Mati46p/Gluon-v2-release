// State Synchronization System
// Bridges the gap between the execution engine (Rust backend) and the
// Command Center UI (Tauri frontend). Broadcasts graph state at 30-60fps
// for smooth real-time visualization.

use crate::engine::{GraphExecutor, ExecutionGraph, Node, NodeType};
use crate::engine::memory::Blackboard;
use super::events::{EventBus, UIEvent, NodeStatus};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;

/// The current execution state, optimized for frontend consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionState {
    /// Graph topology (nodes and edges)
    pub graph: GraphSnapshot,

    /// Current execution status
    pub status: ExecutionStatus,

    /// Blackboard state (shared memory)
    pub blackboard: BlackboardSnapshot,

    /// Budget tracking
    pub budget: BudgetSnapshot,

    /// Active agents and their status
    pub agents: Vec<AgentSnapshot>,

    /// Timestamp of this snapshot
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub nodes: Vec<NodeSnapshot>,
    pub edges: Vec<EdgeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSnapshot {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub status: NodeStatus,
    pub progress: Option<f32>,  // 0.0 to 1.0
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSnapshot {
    pub from: String,
    pub to: String,
    pub edge_type: String,
    pub active: bool,  // Is data currently flowing?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardSnapshot {
    /// Simplified view of blackboard variables for UI
    /// Full blackboard can be GB+, so we only show keys and sizes
    pub variables: Vec<VariableInfo>,
    pub total_size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableInfo {
    pub key: String,
    pub value: serde_json::Value,  // Full value for God Mode editing
    pub value_type: String,
    pub size_bytes: usize,
    pub preview: Option<String>,  // First 100 chars for inspection
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    pub used_cents: u64,
    pub limit_cents: u64,
    pub percentage: f32,
    pub breakdown: HashMap<String, u64>,  // Per-agent costs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub id: String,
    pub name: String,
    pub status: NodeStatus,
    pub current_task: Option<String>,
    pub tokens_used: u64,
    pub cost_cents: u64,
}

/// The UI State Broadcaster
/// Runs in a separate thread, periodically snapshotting the execution state
/// and broadcasting it to the frontend via EventBus
pub struct UIStateBroadcaster {
    event_bus: Arc<EventBus>,
    state: Arc<RwLock<ExecutionState>>,
    is_running: Arc<RwLock<bool>>,
}

impl UIStateBroadcaster {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        let initial_state = ExecutionState {
            graph: GraphSnapshot {
                nodes: vec![],
                edges: vec![],
            },
            status: ExecutionStatus::Idle,
            blackboard: BlackboardSnapshot {
                variables: vec![],
                total_size_bytes: 0,
            },
            budget: BudgetSnapshot {
                used_cents: 0,
                limit_cents: 10000,  // Default $100 limit
                percentage: 0.0,
                breakdown: HashMap::new(),
            },
            agents: vec![],
            timestamp: current_timestamp(),
        };

        Self {
            event_bus,
            state: Arc::new(RwLock::new(initial_state)),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the broadcaster loop (runs in background thread)
    pub fn start(&self) {
        *self.is_running.write() = true;

        let state = Arc::clone(&self.state);
        let event_bus = Arc::clone(&self.event_bus);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_millis(33)  // ~30fps
            );

            while *is_running.read() {
                interval.tick().await;

                // Read current state and broadcast if changed
                let current_state = state.read().clone();

                // Broadcast state snapshot as a custom event
                // Frontend can listen to this for updates
                event_bus.publish(UIEvent::SystemLog {
                    level: super::events::LogLevel::Debug,
                    source: "state_sync".to_string(),
                    message: "state_snapshot".to_string(),
                    metadata: serde_json::to_value(&current_state).unwrap(),
                    timestamp: current_timestamp(),
                });
            }
        });
    }

    /// Stop the broadcaster
    pub fn stop(&self) {
        *self.is_running.write() = false;
    }

    /// Update the graph snapshot (called by GraphExecutor)
    pub fn update_graph(&self, graph: &ExecutionGraph) {
        let mut state = self.state.write();

        // Convert engine graph to UI snapshot
        state.graph = GraphSnapshot {
            nodes: graph.node_configs.iter().map(|(id, config)| {
                NodeSnapshot {
                    id: id.clone(),
                    name: config.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(id)
                        .to_string(),
                    node_type: config.get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                    status: NodeStatus::Pending,  // Will be updated by executor
                    progress: None,
                    metadata: HashMap::new(),
                }
            }).collect(),
            edges: graph.edges.iter().map(|edge| {
                EdgeSnapshot {
                    from: edge.from_node.clone(), // Changed from edge.from
                    to: edge.to_node.clone(),     // Changed from edge.to
                    edge_type: format!("{:?}", edge.edge_type),
                    active: false,
                }
            }).collect(),
        };

        state.timestamp = current_timestamp();
    }

    /// Update node status (called when node state changes)
    pub fn update_node_status(&self, node_id: &str, status: NodeStatus, progress: Option<f32>) {
        let mut state = self.state.write();

        if let Some(node) = state.graph.nodes.iter_mut().find(|n| n.id == node_id) {
            node.status = status.clone();
            node.progress = progress;
        }

        state.timestamp = current_timestamp();
    }

    /// Update blackboard snapshot
    pub fn update_blackboard(&self, blackboard: &Blackboard) {
        let mut state = self.state.write();

        // Convert blackboard to snapshot
        let variables: Vec<VariableInfo> = blackboard.iter().map(|(key, value)| {
            let value_clone = value.clone();
            let value_str = format!("{:?}", value);
            VariableInfo {
                key: key.clone(),
                value: value_clone,  // Full value for God Mode
                value_type: "Value".to_string(),  // Could be enhanced with type introspection
                size_bytes: value_str.len(),
                preview: Some(value_str.chars().take(100).collect()),
            }
        }).collect();

        let total_size: usize = variables.iter().map(|v| v.size_bytes).sum();

        state.blackboard = BlackboardSnapshot {
            variables,
            total_size_bytes: total_size,
        };

        state.timestamp = current_timestamp();
    }

    /// Update budget info
    pub fn update_budget(&self, used_cents: u64, limit_cents: u64, breakdown: HashMap<String, u64>) {
        let mut state = self.state.write();

        state.budget = BudgetSnapshot {
            used_cents,
            limit_cents,
            percentage: (used_cents as f32 / limit_cents as f32) * 100.0,
            breakdown,
        };

        state.timestamp = current_timestamp();

        // Also publish budget update event
        self.event_bus.publish(UIEvent::BudgetUpdate {
            used_cents,
            limit_cents,
            percentage: state.budget.percentage,
            timestamp: current_timestamp(),
        });
    }

    /// Inject or modify a variable in the Blackboard snapshot (God Mode)
    /// NOTE: This modifies the UI snapshot only. For full integration with
    /// the execution engine's Blackboard, this would need to call
    /// executor.blackboard.insert() - deferred to v3.1
    pub fn inject_variable(&self, key: String, value: serde_json::Value) -> Result<(), String> {
        let mut state = self.state.write();

        // Find existing variable or create new one
        let value_str = value.to_string();
        let variable_info = VariableInfo {
            key: key.clone(),
            value: value.clone(),
            value_type: match &value {
                serde_json::Value::Null => "null",
                serde_json::Value::Bool(_) => "boolean",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Array(_) => "array",
                serde_json::Value::Object(_) => "object",
            }.to_string(),
            size_bytes: value_str.len(),
            preview: Some(value_str.chars().take(100).collect()),
        };

        // Update or insert variable
        if let Some(existing) = state.blackboard.variables.iter_mut().find(|v| v.key == key) {
            *existing = variable_info;
        } else {
            state.blackboard.variables.push(variable_info);
        }

        // Recalculate total size
        state.blackboard.total_size_bytes = state.blackboard.variables
            .iter()
            .map(|v| v.size_bytes)
            .sum();

        state.timestamp = current_timestamp();

        Ok(())
    }

    /// Update execution status
    pub fn update_execution_status(&self, status: ExecutionStatus) {
        let mut state = self.state.write();
        state.status = status;
        state.timestamp = current_timestamp();
    }

    /// Get current state snapshot (for Tauri commands)
    pub fn get_state(&self) -> ExecutionState {
        self.state.read().clone()
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcaster_creation() {
        let event_bus = Arc::new(EventBus::new());
        let broadcaster = UIStateBroadcaster::new(event_bus);

        let state = broadcaster.get_state();
        assert!(matches!(state.status, ExecutionStatus::Idle));
        assert_eq!(state.graph.nodes.len(), 0);
    }

    #[test]
    fn test_budget_update() {
        let event_bus = Arc::new(EventBus::new());
        let broadcaster = UIStateBroadcaster::new(event_bus);

        let mut breakdown = HashMap::new();
        breakdown.insert("agent-1".to_string(), 500);

        broadcaster.update_budget(1000, 10000, breakdown);

        let state = broadcaster.get_state();
        assert_eq!(state.budget.used_cents, 1000);
        assert_eq!(state.budget.percentage, 10.0);
    }
}
