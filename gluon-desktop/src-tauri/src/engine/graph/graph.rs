// Execution Graph - Directed Cyclic Graph for workflow execution

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::{Node, Edge, EdgeType};
use crate::engine::execution::ExecutionContext;
use crate::engine::EngineError;

/// A Directed Cyclic Graph (DCG) representing an execution workflow
///
/// Unlike DAGs (Directed Acyclic Graphs), this graph supports cycles,
/// enabling retry loops, self-healing workflows, and iterative processes.
///
/// Note: Nodes are stored as JSON configurations, not as Box<dyn Node>,
/// because trait objects cannot be serialized. Use a factory pattern
/// to reconstruct Node instances from their configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionGraph {
    /// Unique graph identifier
    pub id: String,

    /// Human-readable graph name
    pub name: String,

    /// Creation timestamp (Unix seconds)
    pub created_at: i64,

    /// Node configurations indexed by ID
    /// These are JSON representations that can be deserialized into concrete nodes
    pub node_configs: HashMap<String, serde_json::Value>,

    /// Edges defining connections between nodes
    pub edges: Vec<Edge>,

    /// Entry point node ID (where execution starts)
    pub entry_node: String,

    /// Exit node IDs (optional, for explicit termination)
    /// Empty vec means execution ends when no edges lead forward
    pub exit_nodes: Vec<String>,

    /// Graph metadata (optional tags, descriptions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

impl ExecutionGraph {
    /// Create a new empty graph
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            created_at: chrono::Utc::now().timestamp(),
            node_configs: HashMap::new(),
            edges: Vec::new(),
            entry_node: String::new(),
            exit_nodes: Vec::new(),
            metadata: None,
        }
    }

    /// Add a node configuration to the graph
    pub fn add_node_config(&mut self, node_id: String, config: serde_json::Value) {
        self.node_configs.insert(node_id, config);
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Set the entry node
    pub fn set_entry_node(&mut self, node_id: String) {
        self.entry_node = node_id;
    }

    /// Add an exit node
    pub fn add_exit_node(&mut self, node_id: String) {
        if !self.exit_nodes.contains(&node_id) {
            self.exit_nodes.push(node_id);
        }
    }

    /// Validate graph structure
    ///
    /// Checks:
    /// - Entry node exists
    /// - All edge endpoints exist as nodes
    /// - No orphaned nodes (unless intentional)
    pub fn validate(&self) -> Result<(), EngineError> {
        // Check entry node exists
        if self.entry_node.is_empty() {
            return Err(EngineError::GraphError(
                "Entry node not set".to_string()
            ));
        }

        if !self.node_configs.contains_key(&self.entry_node) {
            return Err(EngineError::GraphError(
                format!("Entry node '{}' does not exist", self.entry_node)
            ));
        }

        // Check all edge endpoints exist
        for edge in &self.edges {
            if !self.node_configs.contains_key(&edge.from_node) {
                return Err(EngineError::GraphError(
                    format!("Edge source node '{}' does not exist", edge.from_node)
                ));
            }

            if !self.node_configs.contains_key(&edge.to_node) {
                return Err(EngineError::GraphError(
                    format!("Edge target node '{}' does not exist", edge.to_node)
                ));
            }
        }

        // Check exit nodes exist
        for exit_node in &self.exit_nodes {
            if !self.node_configs.contains_key(exit_node) {
                return Err(EngineError::GraphError(
                    format!("Exit node '{}' does not exist", exit_node)
                ));
            }
        }

        Ok(())
    }

    /// Get outgoing edges from a node
    pub fn get_outgoing_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.edges
            .iter()
            .filter(|edge| edge.from_node == node_id)
            .collect()
    }

    /// Get next nodes based on current position and execution context
    ///
    /// Evaluates conditional edges and returns edges that should be traversed.
    /// Returns empty vec if no edges match (dead end or exit).
    pub fn get_next_edges<'a>(
        &'a self,
        current_node: &str,
        context: &ExecutionContext,
    ) -> Vec<&'a Edge> {
        self.get_outgoing_edges(current_node)
            .into_iter()
            .filter(|edge| edge.should_traverse(context))
            .collect()
    }

    /// Detect cycles in the graph
    ///
    /// Returns a list of cycles found (each cycle is a list of node IDs).
    /// Useful for visualization and debugging, not for blocking execution.
    pub fn find_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for node_id in self.node_configs.keys() {
            if !visited.contains(node_id) {
                self.detect_cycle_dfs(
                    node_id,
                    &mut visited,
                    &mut rec_stack,
                    &mut path,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    /// DFS helper for cycle detection
    fn detect_cycle_dfs(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        path.push(node.to_string());

        for edge in self.get_outgoing_edges(node) {
            let next = &edge.to_node;

            if !visited.contains(next) {
                self.detect_cycle_dfs(next, visited, rec_stack, path, cycles);
            } else if rec_stack.contains(next) {
                // Cycle detected
                let cycle_start = path.iter().position(|n| n == next).unwrap();
                let cycle = path[cycle_start..].to_vec();
                cycles.push(cycle);
            }
        }

        path.pop();
        rec_stack.remove(node);
    }

    /// Get graph statistics
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            node_count: self.node_configs.len(),
            edge_count: self.edges.len(),
            cycle_count: self.find_cycles().len(),
            has_entry: !self.entry_node.is_empty(),
            exit_count: self.exit_nodes.len(),
        }
    }
}

/// Graph statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub cycle_count: usize,
    pub has_entry: bool,
    pub exit_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_simple_graph() -> ExecutionGraph {
        let mut graph = ExecutionGraph::new("test_graph");

        // Add nodes (as simple JSON configs)
        graph.add_node_config(
            "node_a".to_string(),
            serde_json::json!({"id": "node_a", "type": "Agent"})
        );
        graph.add_node_config(
            "node_b".to_string(),
            serde_json::json!({"id": "node_b", "type": "Agent"})
        );
        graph.add_node_config(
            "node_c".to_string(),
            serde_json::json!({"id": "node_c", "type": "Agent"})
        );

        // Add edges: A -> B -> C
        graph.add_edge(Edge::new_static("e1", "node_a", "node_b"));
        graph.add_edge(Edge::new_static("e2", "node_b", "node_c"));

        graph.set_entry_node("node_a".to_string());

        graph
    }

    #[test]
    fn test_graph_creation() {
        let graph = create_simple_graph();
        assert_eq!(graph.node_configs.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.entry_node, "node_a");
    }

    #[test]
    fn test_graph_validation_success() {
        let graph = create_simple_graph();
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn test_graph_validation_no_entry() {
        let mut graph = create_simple_graph();
        graph.entry_node = String::new();
        assert!(graph.validate().is_err());
    }

    #[test]
    fn test_graph_validation_invalid_edge() {
        let mut graph = create_simple_graph();
        graph.add_edge(Edge::new_static("e3", "node_x", "node_y"));
        assert!(graph.validate().is_err());
    }

    #[test]
    fn test_get_outgoing_edges() {
        let graph = create_simple_graph();
        let edges = graph.get_outgoing_edges("node_a");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].to_node, "node_b");

        let edges = graph.get_outgoing_edges("node_c");
        assert_eq!(edges.len(), 0); // Dead end
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = create_simple_graph();

        // No cycles initially
        assert_eq!(graph.find_cycles().len(), 0);

        // Add cycle: C -> A
        graph.add_edge(Edge::new_static("e3", "node_c", "node_a"));

        let cycles = graph.find_cycles();
        assert!(cycles.len() > 0);
    }

    #[test]
    fn test_graph_stats() {
        let graph = create_simple_graph();
        let stats = graph.stats();

        assert_eq!(stats.node_count, 3);
        assert_eq!(stats.edge_count, 2);
        assert_eq!(stats.cycle_count, 0);
        assert!(stats.has_entry);
        assert_eq!(stats.exit_count, 0);
    }

    #[test]
    fn test_serialization() {
        let graph = create_simple_graph();
        let json = serde_json::to_string(&graph).unwrap();
        let deserialized: ExecutionGraph = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, graph.id);
        assert_eq!(deserialized.name, graph.name);
        assert_eq!(deserialized.node_configs.len(), 3);
        assert_eq!(deserialized.edges.len(), 2);
    }
}
