// Logic Nodes - Control flow nodes (If, Loop, etc.)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::engine::graph::{Node, NodeType};
use crate::engine::execution::{ControlFlow, ExecutionContext};
use crate::engine::EngineResult;

/// Loop Node - Implements retry/iteration logic
///
/// Creates cycles in the graph for self-healing workflows.
/// Example: Code -> Test -> (if failed) -> Fix -> Code (loop back)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopNode {
    pub id: String,
    pub name: String,

    /// Maximum iterations before exiting loop
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,

    /// Node to jump to when continuing loop
    /// If not specified, uses conditional edges instead
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_target: Option<String>,

    /// Condition to check before looping (optional)
    /// If specified, loop only if condition is true
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_condition: Option<String>,
}

fn default_max_iterations() -> u32 {
    10
}

impl LoopNode {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            max_iterations: default_max_iterations(),
            loop_target: None,
            loop_condition: None,
        }
    }

    /// Set maximum iterations
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set loop target node
    pub fn with_loop_target(mut self, target: impl Into<String>) -> Self {
        self.loop_target = Some(target.into());
        self
    }

    /// Set loop condition (e.g., "tests_failed == true")
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.loop_condition = Some(condition.into());
        self
    }

    /// Get current iteration count from blackboard
    fn get_iteration_count(&self, context: &ExecutionContext) -> u32 {
        let key = format!("loop_{}_iteration", self.id);
        let bb = context.blackboard.read().unwrap();
        bb.get_i64(&key).unwrap_or(0) as u32
    }

    /// Increment iteration count in blackboard
    fn increment_iteration(&self, context: &mut ExecutionContext) -> u32 {
        let key = format!("loop_{}_iteration", self.id);
        let mut bb = context.blackboard.write().unwrap();

        let current = bb.get_i64(&key).unwrap_or(0) as u32;
        let next = current + 1;

        bb.insert_i64(key, next as i64);
        next
    }

    /// Check if loop condition is satisfied
    fn check_condition(&self, context: &ExecutionContext) -> bool {
        if let Some(condition) = &self.loop_condition {
            // Reuse edge condition evaluation logic
            // For now, simple equality check
            evaluate_simple_condition(condition, context)
        } else {
            // No condition means always loop (until max iterations)
            true
        }
    }
}

#[async_trait]
impl Node for LoopNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Logic
    }

    async fn execute(&self, context: &mut ExecutionContext) -> EngineResult<ControlFlow> {
        // Get current iteration
        let current_iteration = self.get_iteration_count(context);

        // Check if we've hit the max
        if current_iteration >= self.max_iterations {
            // Exit loop - continue to next node
            return Ok(ControlFlow::Continue);
        }

        // Check loop condition (if specified)
        if !self.check_condition(context) {
            // Condition not met - exit loop
            return Ok(ControlFlow::Continue);
        }

        // Increment iteration counter
        let next_iteration = self.increment_iteration(context);

        // Store iteration info in blackboard for debugging
        {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert_bool(
                format!("loop_{}_active", self.id),
                true
            );
            bb.insert_i64(
                format!("loop_{}_remaining", self.id),
                (self.max_iterations - next_iteration) as i64
            );
        }

        // Jump back to loop target (if specified)
        if let Some(target) = &self.loop_target {
            Ok(ControlFlow::Jump(target.clone()))
        } else {
            // No explicit target - use conditional edges
            Ok(ControlFlow::Continue)
        }
    }
}

/// If Node - Simple pass-through for conditional routing
///
/// Condition logic is handled by conditional edges, not the node itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfNode {
    pub id: String,
    pub name: String,

    /// Optional: set a flag in blackboard for debugging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_flag: Option<String>,
}

impl IfNode {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            debug_flag: None,
        }
    }

    pub fn with_debug_flag(mut self, flag: impl Into<String>) -> Self {
        self.debug_flag = Some(flag.into());
        self
    }
}

#[async_trait]
impl Node for IfNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Logic
    }

    async fn execute(&self, context: &mut ExecutionContext) -> EngineResult<ControlFlow> {
        // If node is just a pass-through
        // Actual branching logic is in conditional edges

        if let Some(flag) = &self.debug_flag {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert_bool(flag.clone(), true);
        }

        Ok(ControlFlow::Continue)
    }
}

/// Simple condition evaluator (shared with edge.rs logic)
fn evaluate_simple_condition(condition: &str, context: &ExecutionContext) -> bool {
    let condition = condition.trim();

    // Simple equality: "key == value"
    if let Some((key, value)) = condition.split_once("==") {
        let key = key.trim();
        let value = value.trim();

        let bb = context.blackboard.read().unwrap();
        let actual = bb.get(key);

        if value == "true" || value == "false" {
            let expected = value == "true";
            return actual.and_then(|v| v.as_bool()) == Some(expected);
        } else if value.starts_with('"') && value.ends_with('"') {
            let expected = value.trim_matches('"');
            return actual.and_then(|v| v.as_str()) == Some(expected);
        } else if let Ok(expected) = value.parse::<i64>() {
            return actual.and_then(|v| v.as_i64()) == Some(expected);
        }
    }

    // Inequality: "key != value"
    if let Some((key, value)) = condition.split_once("!=") {
        return !evaluate_simple_condition(&format!("{} == {}", key, value), context);
    }

    // Greater than: "key > value"
    if let Some((key, value)) = condition.split_once('>') {
        let key = key.trim();
        let value = value.trim();

        if let Ok(expected) = value.parse::<i64>() {
            let bb = context.blackboard.read().unwrap();
            let actual = bb.get(key).and_then(|v| v.as_i64());
            return actual.map(|a| a > expected).unwrap_or(false);
        }
    }

    // Default: check if key exists and is truthy
    let bb = context.blackboard.read().unwrap();
    bb.get(condition).and_then(|v| v.as_bool()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock, Mutex};
    use crate::engine::memory::{Blackboard, BudgetTracker};

    fn create_test_context() -> ExecutionContext {
        ExecutionContext {
            blackboard: Arc::new(RwLock::new(Blackboard::new())),
            budget: Arc::new(Mutex::new(BudgetTracker::new(1000))),
            history: Vec::new(),
            execution_id: "test".to_string(),
            started_at: 0,
        }
    }

    #[tokio::test]
    async fn test_loop_node_basic() {
        let node = LoopNode::new("loop1", "Test Loop")
            .with_max_iterations(3)
            .with_loop_target("target_node");

        let mut context = create_test_context();

        // First iteration - should jump
        let result = node.execute(&mut context).await.unwrap();
        assert!(matches!(result, ControlFlow::Jump(ref target) if target == "target_node"));
        assert_eq!(node.get_iteration_count(&context), 1);

        // Second iteration
        let result = node.execute(&mut context).await.unwrap();
        assert!(matches!(result, ControlFlow::Jump(_)));
        assert_eq!(node.get_iteration_count(&context), 2);

        // Third iteration
        let result = node.execute(&mut context).await.unwrap();
        assert!(matches!(result, ControlFlow::Jump(_)));
        assert_eq!(node.get_iteration_count(&context), 3);

        // Fourth iteration - should exit (max reached)
        let result = node.execute(&mut context).await.unwrap();
        assert!(matches!(result, ControlFlow::Continue));
    }

    #[tokio::test]
    async fn test_loop_node_with_condition() {
        let node = LoopNode::new("loop1", "Conditional Loop")
            .with_max_iterations(10)
            .with_loop_target("retry")
            .with_condition("should_retry == true");

        let mut context = create_test_context();

        // Set condition to true
        {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert_bool("should_retry".to_string(), true);
        }

        // Should loop
        let result = node.execute(&mut context).await.unwrap();
        assert!(matches!(result, ControlFlow::Jump(_)));

        // Set condition to false
        {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert_bool("should_retry".to_string(), false);
        }

        // Should exit
        let result = node.execute(&mut context).await.unwrap();
        assert!(matches!(result, ControlFlow::Continue));
    }

    #[tokio::test]
    async fn test_if_node() {
        let node = IfNode::new("if1", "Test If")
            .with_debug_flag("if_executed");

        let mut context = create_test_context();
        let result = node.execute(&mut context).await.unwrap();

        assert!(matches!(result, ControlFlow::Continue));

        // Check debug flag was set
        let bb = context.blackboard.read().unwrap();
        assert_eq!(bb.get_bool("if_executed"), Some(true));
    }

    #[test]
    fn test_condition_evaluation() {
        let mut context = create_test_context();

        // Set up test data
        {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert_bool("flag".to_string(), true);
            bb.insert_i64("count".to_string(), 5);
            bb.insert_string("status".to_string(), "success".to_string());
        }

        // Boolean checks
        assert!(evaluate_simple_condition("flag == true", &context));
        assert!(!evaluate_simple_condition("flag == false", &context));

        // Numeric checks
        assert!(evaluate_simple_condition("count > 3", &context));
        assert!(!evaluate_simple_condition("count > 10", &context));

        // String checks
        assert!(evaluate_simple_condition("status == \"success\"", &context));
        assert!(!evaluate_simple_condition("status == \"failed\"", &context));
    }
}
