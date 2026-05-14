// Agent Node - LLM execution node (mock implementation)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::engine::graph::{Node, NodeType};
use crate::engine::execution::{ControlFlow, ExecutionContext};
use crate::engine::EngineResult;

/// Agent Node - Executes LLM calls
///
/// Phase 1: Mock implementation that simulates LLM behavior
/// Phase 2: Real LLM integration (Claude, GPT, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNode {
    pub id: String,
    pub name: String,

    /// Prompt template (can reference blackboard variables with {key})
    #[serde(default)]
    pub prompt_template: String,

    /// Model to use (e.g., "claude-sonnet-4-5", "gpt-4")
    #[serde(default = "default_model")]
    pub model: String,

    /// Maximum output tokens
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    /// Mock response (for testing - overrides LLM call)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mock_response: Option<String>,
}

fn default_model() -> String {
    "gpt-3.5-turbo".to_string()
}

fn default_max_tokens() -> u32 {
    1000
}

impl AgentNode {
    /// Create a new agent node
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            prompt_template: String::new(),
            model: default_model(),
            max_tokens: default_max_tokens(),
            mock_response: None,
        }
    }

    /// Create with prompt template
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt_template = prompt.into();
        self
    }

    /// Create with mock response (for testing)
    pub fn with_mock_response(mut self, response: impl Into<String>) -> Self {
        self.mock_response = Some(response.into());
        self
    }

    /// Set model
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Render prompt template with blackboard variables
    fn render_prompt(&self, context: &ExecutionContext) -> String {
        let mut rendered = self.prompt_template.clone();

        // Simple template rendering: replace {key} with blackboard[key]
        let bb = context.blackboard.read().unwrap();

        for key in bb.keys() {
            let placeholder = format!("{{{}}}", key);
            if let Some(value) = bb.get_string(&key) {
                rendered = rendered.replace(&placeholder, &value);
            }
        }

        rendered
    }

    /// Mock LLM call (Phase 1 implementation)
    async fn call_llm_mock(&self, prompt: &str) -> LLMResponse {
        // Simulate API latency
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Use mock response if provided, otherwise generate generic response
        let content = if let Some(mock) = &self.mock_response {
            mock.clone()
        } else {
            format!("Mock LLM response for prompt: {}",
                if prompt.len() > 50 {
                    format!("{}...", &prompt[..50])
                } else {
                    prompt.to_string()
                })
        };

        let input_tokens = estimate_tokens(prompt);
        let output_tokens = estimate_tokens(&content);

        LLMResponse {
            content,
            model: self.model.clone(),
            input_tokens,
            output_tokens,
        }
    }
}

#[async_trait]
impl Node for AgentNode {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Agent
    }

    async fn execute(&self, context: &mut ExecutionContext) -> EngineResult<ControlFlow> {
        // 1. Render prompt from template
        let prompt = self.render_prompt(context);

        // 2. Call LLM (mock in Phase 1)
        let response = self.call_llm_mock(&prompt).await;

        // 3. Track budget
        {
            let mut budget = context.budget.lock().unwrap();
            budget.record_llm_call(&response.model, response.input_tokens, response.output_tokens)?;
        }

        // 4. Store result in blackboard
        {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert_string(
                format!("{}_response", self.id),
                response.content.clone()
            );
            bb.insert_string(
                format!("{}_model", self.id),
                response.model.clone()
            );
        }

        // 5. Continue to next node
        Ok(ControlFlow::Continue)
    }

    fn estimated_cost(&self) -> u64 {
        // Rough estimate: 1000 tokens in + 1000 tokens out
        // For GPT-3.5: ~0.15 cents
        15
    }
}

/// LLM Response
#[derive(Debug, Clone)]
struct LLMResponse {
    pub content: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Estimate token count (rough approximation: 1 token ≈ 4 chars)
fn estimate_tokens(text: &str) -> u64 {
    (text.len() / 4).max(1) as u64
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
    async fn test_agent_node_basic_execution() {
        let node = AgentNode::new("agent1", "Test Agent")
            .with_prompt("Hello, world!")
            .with_mock_response("Hi there!");

        let mut context = create_test_context();
        let result = node.execute(&mut context).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), ControlFlow::Continue));

        // Check response was stored
        let bb = context.blackboard.read().unwrap();
        assert_eq!(bb.get_string("agent1_response"), Some("Hi there!".to_string()));
    }

    #[tokio::test]
    async fn test_prompt_template_rendering() {
        let node = AgentNode::new("agent1", "Test Agent")
            .with_prompt("User said: {user_input}")
            .with_mock_response("Acknowledged");

        let mut context = create_test_context();

        // Set blackboard variable
        {
            let mut bb = context.blackboard.write().unwrap();
            bb.insert_string("user_input".to_string(), "Hello AI!".to_string());
        }

        let rendered = node.render_prompt(&context);
        assert_eq!(rendered, "User said: Hello AI!");
    }

    #[tokio::test]
    async fn test_budget_tracking() {
        let node = AgentNode::new("agent1", "Test Agent")
            .with_prompt("Test prompt")
            .with_mock_response("Test response")
            .with_model("gpt-3.5-turbo");

        let mut context = create_test_context();

        let initial_cost = context.budget.lock().unwrap().cost_cents;

        node.execute(&mut context).await.unwrap();

        let final_cost = context.budget.lock().unwrap().cost_cents;

        // Cost should have increased
        assert!(final_cost > initial_cost);
    }

    #[tokio::test]
    async fn test_budget_exceeded() {
        let node = AgentNode::new("agent1", "Expensive Agent")
            .with_prompt("Very long prompt...".repeat(1000)) // Huge prompt
            .with_mock_response("Response".repeat(1000))     // Huge response
            .with_model("claude-opus-4"); // Expensive model

        let mut context = ExecutionContext {
            blackboard: Arc::new(RwLock::new(Blackboard::new())),
            budget: Arc::new(Mutex::new(BudgetTracker::new(5))), // Very low budget (5 cents)
            history: Vec::new(),
            execution_id: "test".to_string(),
            started_at: 0,
        };

        let result = node.execute(&mut context).await;

        // Should exceed budget
        assert!(result.is_err());
    }

    #[test]
    fn test_token_estimation() {
        assert_eq!(estimate_tokens("Hello"), 1); // 5 chars / 4 = 1
        assert_eq!(estimate_tokens("Hello world"), 2); // 11 chars / 4 = 2
        assert_eq!(estimate_tokens("A".repeat(100)), 25); // 100 / 4 = 25
    }
}
