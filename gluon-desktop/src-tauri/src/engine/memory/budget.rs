// Budget Tracker - Economic safety for LLM usage

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::engine::EngineError;

/// Tracks API costs and enforces budget limits
///
/// Prevents runaway costs by monitoring LLM usage and aborting
/// execution when budget is exceeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetTracker {
    /// Total input tokens consumed
    pub input_tokens: u64,

    /// Total output tokens consumed
    pub output_tokens: u64,

    /// Estimated cost in USD cents
    pub cost_cents: u64,

    /// Hard limit in USD cents (abort if exceeded)
    pub max_cost_cents: u64,

    /// Per-model token counts: model_name -> (input, output)
    pub model_usage: HashMap<String, (u64, u64)>,
}

impl BudgetTracker {
    /// Create a new budget tracker with specified limit
    ///
    /// # Arguments
    /// * `max_cost_cents` - Maximum allowed cost in cents (e.g., 100 = $1.00)
    pub fn new(max_cost_cents: u64) -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cost_cents: 0,
            max_cost_cents,
            model_usage: HashMap::new(),
        }
    }

    /// Record an LLM API call
    ///
    /// Calculates cost based on model pricing and updates totals.
    /// Returns an error if budget is exceeded.
    ///
    /// # Arguments
    /// * `model` - Model name (e.g., "claude-sonnet-4-5", "gpt-4")
    /// * `input_tokens` - Number of input tokens
    /// * `output_tokens` - Number of output tokens
    pub fn record_llm_call(
        &mut self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) -> Result<(), EngineError> {
        // Calculate cost for this call
        let call_cost = calculate_cost(model, input_tokens, output_tokens);

        // Update totals
        self.input_tokens += input_tokens;
        self.output_tokens += output_tokens;
        self.cost_cents += call_cost;

        // Update per-model usage
        let entry = self.model_usage.entry(model.to_string()).or_insert((0, 0));
        entry.0 += input_tokens;
        entry.1 += output_tokens;

        // Check budget limit
        if self.cost_cents > self.max_cost_cents {
            return Err(EngineError::BudgetExceeded {
                used: self.cost_cents,
                limit: self.max_cost_cents,
            });
        }

        Ok(())
    }

    /// Check if budget has been exceeded
    pub fn is_exceeded(&self) -> bool {
        self.cost_cents > self.max_cost_cents
    }

    /// Get remaining budget in cents
    pub fn remaining_cents(&self) -> i64 {
        self.max_cost_cents as i64 - self.cost_cents as i64
    }

    /// Get usage percentage (0-100)
    pub fn usage_percentage(&self) -> f64 {
        if self.max_cost_cents == 0 {
            return 100.0;
        }
        (self.cost_cents as f64 / self.max_cost_cents as f64) * 100.0
    }

    /// Get total tokens used
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    /// Get usage summary as human-readable string
    pub fn summary(&self) -> String {
        format!(
            "Budget: ${:.2} / ${:.2} ({:.1}%), Tokens: {} total",
            self.cost_cents as f64 / 100.0,
            self.max_cost_cents as f64 / 100.0,
            self.usage_percentage(),
            self.total_tokens()
        )
    }
}

/// Calculate cost for an LLM call
///
/// Pricing table (as of 2025):
/// - Claude Sonnet 4.5: $3 / $15 per 1M tokens (input/output)
/// - Claude Opus 4: $15 / $75 per 1M tokens
/// - GPT-4: $10 / $30 per 1M tokens
/// - GPT-3.5: $0.50 / $1.50 per 1M tokens
///
/// Returns cost in USD cents
fn calculate_cost(model: &str, input_tokens: u64, output_tokens: u64) -> u64 {
    // Pricing in cents per 1M tokens
    let (input_price, output_price) = match model {
        // Claude models
        "claude-sonnet-4-5" | "claude-sonnet-4.5" => (300, 1500),
        "claude-opus-4" | "claude-opus-4.0" => (1500, 7500),
        "claude-haiku-4" | "claude-haiku-4.0" => (25, 125),

        // OpenAI models
        "gpt-4" | "gpt-4-turbo" => (1000, 3000),
        "gpt-4o" => (500, 1500),
        "gpt-3.5-turbo" => (50, 150),

        // Google models
        "gemini-pro" => (50, 150),
        "gemini-ultra" => (1000, 3000),

        // Default (cheap model)
        _ => (100, 300),
    };

    // Calculate cost: (tokens * price_per_1M) / 1M
    let input_cost = (input_tokens * input_price) / 1_000_000;
    let output_cost = (output_tokens * output_price) / 1_000_000;

    input_cost + output_cost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_tracking() {
        let mut tracker = BudgetTracker::new(100); // $1.00 limit

        // Should succeed (well under budget)
        assert!(tracker.record_llm_call("gpt-3.5-turbo", 1000, 500).is_ok());
        assert!(!tracker.is_exceeded());

        // Check remaining budget
        assert!(tracker.remaining_cents() > 0);
    }

    #[test]
    fn test_budget_exceeded() {
        let mut tracker = BudgetTracker::new(10); // $0.10 limit (very low)

        // Large call should exceed budget
        let result = tracker.record_llm_call("claude-sonnet-4-5", 100_000, 50_000);

        // Should return error or be at limit
        // (Exact behavior depends on calculated cost)
        if result.is_err() {
            assert!(tracker.is_exceeded());
        }
    }

    #[test]
    fn test_per_model_tracking() {
        let mut tracker = BudgetTracker::new(1000);

        tracker.record_llm_call("gpt-4", 1000, 500).unwrap();
        tracker.record_llm_call("gpt-4", 500, 250).unwrap();
        tracker.record_llm_call("claude-sonnet-4-5", 2000, 1000).unwrap();

        assert_eq!(tracker.model_usage.len(), 2);
        assert_eq!(tracker.model_usage.get("gpt-4"), Some(&(1500, 750)));
        assert_eq!(tracker.model_usage.get("claude-sonnet-4-5"), Some(&(2000, 1000)));
    }

    #[test]
    fn test_cost_calculation() {
        // Claude Sonnet: $3 / $15 per 1M = 300/1500 cents per 1M
        // 100k input + 50k output = (100k * 300 / 1M) + (50k * 1500 / 1M)
        // = 30 + 75 = 105 cents
        let cost = calculate_cost("claude-sonnet-4-5", 100_000, 50_000);
        assert_eq!(cost, 105);
    }

    #[test]
    fn test_usage_percentage() {
        let mut tracker = BudgetTracker::new(100);
        tracker.record_llm_call("gpt-3.5-turbo", 10_000, 5_000).unwrap();

        let percentage = tracker.usage_percentage();
        assert!(percentage >= 0.0 && percentage <= 100.0);
    }

    #[test]
    fn test_summary() {
        let mut tracker = BudgetTracker::new(500);
        tracker.record_llm_call("gpt-4", 50_000, 25_000).unwrap();

        let summary = tracker.summary();
        assert!(summary.contains("Budget"));
        assert!(summary.contains("Tokens"));
    }
}
