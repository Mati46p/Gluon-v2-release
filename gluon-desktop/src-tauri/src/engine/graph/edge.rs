// Edge - Directed connections between nodes with conditional routing

use serde::{Deserialize, Serialize};
use crate::engine::execution::ExecutionContext;

/// Represents a directed edge in the execution graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier for this edge
    pub id: String,

    /// Source node ID
    pub from_node: String,

    /// Target node ID
    pub to_node: String,

    /// Edge type (static, conditional, etc.)
    pub edge_type: EdgeType,

    /// Optional label for visualization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Type of edge connection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EdgeType {
    /// Always traverse this edge (unconditional)
    Static,

    /// Traverse only if condition evaluates to true
    Conditional {
        /// Boolean expression evaluated against blackboard
        /// Examples:
        /// - "tests_passed == true"
        /// - "error_count > 0"
        /// - "status == 'success'"
        condition: String,
    },

    /// Edge from Fork node to parallel branch (Phase 2)
    #[cfg(feature = "fork_join")]
    ForkBranch {
        /// Fork group ID (all branches in same fork share this)
        fork_id: String,
    },

    /// Edge from parallel branch to Join node (Phase 2)
    #[cfg(feature = "fork_join")]
    JoinBranch {
        /// Fork group ID
        fork_id: String,
    },
}

impl Edge {
    /// Create a new static edge
    pub fn new_static(
        id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            from_node: from.into(),
            to_node: to.into(),
            edge_type: EdgeType::Static,
            label: None,
        }
    }

    /// Create a new conditional edge
    pub fn new_conditional(
        id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        condition: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            from_node: from.into(),
            to_node: to.into(),
            edge_type: EdgeType::Conditional {
                condition: condition.into(),
            },
            label: None,
        }
    }

    /// Set the edge label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Evaluate if this edge should be traversed
    ///
    /// For static edges, always returns true.
    /// For conditional edges, evaluates the condition against the blackboard.
    pub fn should_traverse(&self, context: &ExecutionContext) -> bool {
        match &self.edge_type {
            EdgeType::Static => true,

            EdgeType::Conditional { condition } => {
                evaluate_condition(condition, context)
            }

            #[cfg(feature = "fork_join")]
            EdgeType::ForkBranch { .. } | EdgeType::JoinBranch { .. } => {
                true // Fork/Join edges are always traversed
            }
        }
    }
}

/// Evaluate a boolean condition against the execution context
///
/// Simple expression evaluator supporting:
/// - Equality: "key == value"
/// - Inequality: "key != value"
/// - Comparison: "key > value", "key < value"
/// - Boolean: "key == true", "key == false"
///
/// For Phase 1, we use a simple parser. In Phase 2, we can integrate
/// the `evalexpr` crate for more complex expressions.
fn evaluate_condition(condition: &str, context: &ExecutionContext) -> bool {
    let condition = condition.trim();

    // Simple equality check: "key == value"
    if let Some((key, value)) = condition.split_once("==") {
        let key = key.trim();
        let value = value.trim();

        let blackboard = context.blackboard.read().unwrap();

        // Get value from blackboard
        let actual = blackboard.get(key);

        // Handle different value types
        if value == "true" || value == "false" {
            // Boolean comparison
            let expected = value == "true";
            return actual.and_then(|v| v.as_bool()) == Some(expected);
        } else if value.starts_with('"') && value.ends_with('"') {
            // String comparison
            let expected = value.trim_matches('"');
            return actual.and_then(|v| v.as_str()) == Some(expected);
        } else if let Ok(expected) = value.parse::<i64>() {
            // Integer comparison
            return actual.and_then(|v| v.as_i64()) == Some(expected);
        }

        return false;
    }

    // Inequality check: "key != value"
    if let Some((key, value)) = condition.split_once("!=") {
        let key = key.trim();
        let value = value.trim();

        let blackboard = context.blackboard.read().unwrap();
        let actual = blackboard.get(key);

        if value == "true" || value == "false" {
            let expected = value == "true";
            return actual.and_then(|v| v.as_bool()) != Some(expected);
        } else if value.starts_with('"') && value.ends_with('"') {
            let expected = value.trim_matches('"');
            return actual.and_then(|v| v.as_str()) != Some(expected);
        }

        return false;
    }

    // Greater than: "key > value"
    if let Some((key, value)) = condition.split_once('>') {
        let key = key.trim();
        let value = value.trim();

        if let Ok(expected) = value.parse::<i64>() {
            let blackboard = context.blackboard.read().unwrap();
            let actual = blackboard.get(key).and_then(|v| v.as_i64());
            return actual.map(|a| a > expected).unwrap_or(false);
        }

        return false;
    }

    // Less than: "key < value"
    if let Some((key, value)) = condition.split_once('<') {
        let key = key.trim();
        let value = value.trim();

        if let Ok(expected) = value.parse::<i64>() {
            let blackboard = context.blackboard.read().unwrap();
            let actual = blackboard.get(key).and_then(|v| v.as_i64());
            return actual.map(|a| a < expected).unwrap_or(false);
        }

        return false;
    }

    // If no operator found, treat as boolean key existence check
    let blackboard = context.blackboard.read().unwrap();
    blackboard.get(condition).and_then(|v| v.as_bool()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};
    use crate::engine::memory::Blackboard;

    fn create_test_context() -> ExecutionContext {
        ExecutionContext {
            blackboard: Arc::new(RwLock::new(Blackboard::new())),
            budget: Arc::new(std::sync::Mutex::new(
                crate::engine::memory::BudgetTracker::new(1000)
            )),
            history: Vec::new(),
            execution_id: "test".to_string(),
            started_at: 0,
        }
    }

    #[test]
    fn test_static_edge_always_traverses() {
        let edge = Edge::new_static("e1", "a", "b");
        let context = create_test_context();

        assert!(edge.should_traverse(&context));
    }

    #[test]
    fn test_conditional_edge_boolean() {
        let edge = Edge::new_conditional("e1", "a", "b", "tests_passed == true");
        let mut context = create_test_context();

        // Initially false (key doesn't exist)
        assert!(!edge.should_traverse(&context));

        // Set to true
        {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert("tests_passed".to_string(), serde_json::json!(true));
        }
        assert!(edge.should_traverse(&context));

        // Set to false
        {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert("tests_passed".to_string(), serde_json::json!(false));
        }
        assert!(!edge.should_traverse(&context));
    }

    #[test]
    fn test_conditional_edge_string() {
        let edge = Edge::new_conditional("e1", "a", "b", "status == \"success\"");
        let mut context = create_test_context();

        assert!(!edge.should_traverse(&context));

        {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert("status".to_string(), serde_json::json!("success"));
        }
        assert!(edge.should_traverse(&context));
    }

    #[test]
    fn test_conditional_edge_number() {
        let edge = Edge::new_conditional("e1", "a", "b", "error_count > 0");
        let mut context = create_test_context();

        assert!(!edge.should_traverse(&context));

        {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert("error_count".to_string(), serde_json::json!(5));
        }
        assert!(edge.should_traverse(&context));
    }

    #[test]
    fn test_edge_serialization() {
        let edge = Edge::new_conditional("e1", "a", "b", "x == true")
            .with_label("if success");

        let json = serde_json::to_string(&edge).unwrap();
        let deserialized: Edge = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "e1");
        assert_eq!(deserialized.from_node, "a");
        assert_eq!(deserialized.to_node, "b");
        assert_eq!(deserialized.label, Some("if success".to_string()));
    }
}
