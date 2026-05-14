//! LLM Inference Engine for Workflow Agents
//!
//! Handles communication with LLM APIs (OpenAI, Claude) and smart routing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported LLM providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmProvider {
    OpenAI,
    Claude,
    Custom { base_url: String },
}

/// LLM Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: String,
    pub model: String, // e.g., "gpt-4", "claude-3-opus"
    pub temperature: f32,
    pub max_tokens: u32,
}

/// Message in LLM conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String, // "system", "user", "assistant"
    pub content: String,
}

/// Routing instruction for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingInstruction {
    pub target_agent_id: String,
    pub target_agent_name: String,
    pub content: String,
}

/// Parsed LLM response with routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedLlmResponse {
    /// Raw text response from LLM (thinking/explanation)
    pub raw_text: String,
    /// Parsed routing instructions (if any)
    pub routing: Vec<RoutingInstruction>,
    /// Whether JSON was valid on first try
    pub required_healing: bool,
}

/// LLM Client for making API calls
pub struct LlmClient {
    config: LlmConfig,
    http_client: reqwest::Client,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }

    /// Generates a System Prompt with routing instructions dynamically injected
    pub fn generate_system_prompt(
        base_template: &str,
        target_agents: &[(String, String)], // (id, name) pairs
    ) -> String {
        let mut prompt = base_template.to_string();

        if !target_agents.is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str("[ROUTING INSTRUCTION]\n");
            prompt.push_str("Jesteś połączony z następującymi węzłami:\n");

            for (id, name) in target_agents {
                prompt.push_str(&format!("- \"{}\" (ID: {})\n", name, id));
            }

            prompt.push_str("\nABY PRZEKAZAĆ IM ZADANIA, MUSISZ ZAWRZEĆ W ODPOWIEDZI BLOK KODU JSON W FORMACIE:\n");
            prompt.push_str("```json\n");
            prompt.push_str("{\n");
            prompt.push_str("  \"routing\": {\n");

            for (id, _name) in target_agents {
                prompt.push_str(&format!("    \"{}\": {{ \"content\": \"...instrukcje dla tego agenta...\" }},\n", id));
            }

            prompt.push_str("  }\n");
            prompt.push_str("}\n");
            prompt.push_str("```\n\n");
            prompt.push_str("WAŻNE:\n");
            prompt.push_str("- Możesz swobodnie myśleć tekstem PRZED blokiem JSON\n");
            prompt.push_str("- Blok JSON musi mieć poprawną składnię (strict JSON)\n");
            prompt.push_str("- Klucze w 'routing' to ID węzłów, wartości to obiekty z polem 'content'\n");
            prompt.push_str("- Jeśli nie chcesz nic przekazywać, pomiń ten węzeł lub użyj pustego obiektu {}\n");
        }

        prompt
    }

    /// Makes an LLM API call with messages
    pub async fn complete(
        &self,
        messages: Vec<LlmMessage>,
    ) -> Result<String, String> {
        match self.config.provider {
            LlmProvider::OpenAI => self.call_openai(messages).await,
            LlmProvider::Claude => self.call_claude(messages).await,
            LlmProvider::Custom { ref base_url } => self.call_custom(base_url, messages).await,
        }
    }

    /// OpenAI API call
    async fn call_openai(&self, messages: Vec<LlmMessage>) -> Result<String, String> {
        #[derive(Serialize)]
        struct OpenAIRequest {
            model: String,
            messages: Vec<LlmMessage>,
            temperature: f32,
            max_tokens: u32,
        }

        #[derive(Deserialize)]
        struct OpenAIResponse {
            choices: Vec<OpenAIChoice>,
        }

        #[derive(Deserialize)]
        struct OpenAIChoice {
            message: LlmMessage,
        }

        let request = OpenAIRequest {
            model: self.config.model.clone(),
            messages,
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
        };

        let response = self
            .http_client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("OpenAI API error {}: {}", status, body));
        }

        let data: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

        data.choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| "No response from OpenAI".to_string())
    }

    /// Claude API call
    async fn call_claude(&self, messages: Vec<LlmMessage>) -> Result<String, String> {
        #[derive(Serialize)]
        struct ClaudeRequest {
            model: String,
            messages: Vec<ClaudeMessage>,
            max_tokens: u32,
            temperature: f32,
            system: Option<String>,
        }

        #[derive(Serialize, Deserialize)]
        struct ClaudeMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct ClaudeResponse {
            content: Vec<ClaudeContent>,
        }

        #[derive(Deserialize)]
        struct ClaudeContent {
            text: String,
        }

        // Extract system message if present
        let system_msg = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        // Filter out system messages (Claude uses separate 'system' field)
        let claude_messages: Vec<ClaudeMessage> = messages
            .into_iter()
            .filter(|m| m.role != "system")
            .map(|m| ClaudeMessage {
                role: m.role,
                content: m.content,
            })
            .collect();

        let request = ClaudeRequest {
            model: self.config.model.clone(),
            messages: claude_messages,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            system: system_msg,
        };

        let response = self
            .http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Claude API error {}: {}", status, body));
        }

        let data: ClaudeResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Claude response: {}", e))?;

        data.content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| "No response from Claude".to_string())
    }

    /// Custom API call (generic OpenAI-compatible)
    async fn call_custom(&self, base_url: &str, messages: Vec<LlmMessage>) -> Result<String, String> {
        // Use OpenAI-compatible format for custom endpoints
        #[derive(Serialize)]
        struct CustomRequest {
            model: String,
            messages: Vec<LlmMessage>,
            temperature: f32,
            max_tokens: u32,
        }

        #[derive(Deserialize)]
        struct CustomResponse {
            choices: Vec<CustomChoice>,
        }

        #[derive(Deserialize)]
        struct CustomChoice {
            message: LlmMessage,
        }

        let request = CustomRequest {
            model: self.config.model.clone(),
            messages,
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
        };

        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Custom API error {}: {}", status, body));
        }

        let data: CustomResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse custom response: {}", e))?;

        data.choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| "No response from custom API".to_string())
    }
}

/// Response Parser with Self-Healing
pub struct ResponseParser;

impl ResponseParser {
    /// Extracts JSON code block from markdown response
    fn extract_json_block(text: &str) -> Option<String> {
        // Look for ```json ... ``` blocks
        let json_pattern = regex::Regex::new(r"```json\s*([\s\S]*?)\s*```").unwrap();

        if let Some(captures) = json_pattern.captures(text) {
            return Some(captures[1].trim().to_string());
        }

        // Fallback: look for any ```...``` block
        let generic_pattern = regex::Regex::new(r"```\s*([\s\S]*?)\s*```").unwrap();
        if let Some(captures) = generic_pattern.captures(text) {
            let content = captures[1].trim();
            // Try to detect if it's JSON (starts with { or [)
            if content.starts_with('{') || content.starts_with('[') {
                return Some(content.to_string());
            }
        }

        None
    }

    /// Parses LLM response and extracts routing instructions with self-healing
    pub async fn parse_with_healing(
        raw_response: String,
        target_agents: &[(String, String)], // (id, name)
        llm_client: &LlmClient,
        max_retries: u32,
    ) -> Result<ParsedLlmResponse, String> {
        let mut attempts = 0;
        let mut last_error = String::new();
        let mut conversation_history = vec![
            LlmMessage {
                role: "assistant".to_string(),
                content: raw_response.clone(),
            }
        ];

        loop {
            attempts += 1;

            // Try to extract and parse JSON
            if let Some(json_str) = Self::extract_json_block(&conversation_history.last().unwrap().content) {
                match serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(json_value) => {
                        // Parse routing instructions
                        let routing = Self::extract_routing(&json_value, target_agents)?;

                        return Ok(ParsedLlmResponse {
                            raw_text: raw_response.clone(),
                            routing,
                            required_healing: attempts > 1,
                        });
                    }
                    Err(e) => {
                        last_error = format!("JSON parse error: {}", e);
                        println!("[LLM Parser] ❌ Invalid JSON (attempt {}): {}", attempts, last_error);

                        if attempts >= max_retries {
                            return Err(format!("Failed to heal JSON after {} attempts. Last error: {}", max_retries, last_error));
                        }

                        // Self-Healing: Ask LLM to fix the JSON
                        let heal_message = LlmMessage {
                            role: "user".to_string(),
                            content: format!(
                                "System Error: Invalid JSON format. Error: {}\n\nPlease fix the JSON syntax and provide a valid response. Remember: strict JSON syntax required.",
                                last_error
                            ),
                        };

                        conversation_history.push(heal_message.clone());

                        // Get fixed response from LLM
                        let healed_response = llm_client
                            .complete(conversation_history.clone())
                            .await
                            .map_err(|e| format!("Self-healing request failed: {}", e))?;

                        conversation_history.push(LlmMessage {
                            role: "assistant".to_string(),
                            content: healed_response,
                        });
                    }
                }
            } else {
                // No JSON block found - this is acceptable if no routing is needed
                println!("[LLM Parser] ℹ️ No JSON block found in response. Assuming no routing needed.");
                return Ok(ParsedLlmResponse {
                    raw_text: raw_response,
                    routing: vec![],
                    required_healing: false,
                });
            }
        }
    }

    /// Extracts routing instructions from parsed JSON
    fn extract_routing(
        json: &serde_json::Value,
        target_agents: &[(String, String)],
    ) -> Result<Vec<RoutingInstruction>, String> {
        let routing_obj = json
            .get("routing")
            .ok_or_else(|| "Missing 'routing' field in JSON".to_string())?;

        if !routing_obj.is_object() {
            return Err("'routing' field must be an object".to_string());
        }

        let mut instructions = Vec::new();

        for (agent_id, agent_name) in target_agents {
            if let Some(instruction_obj) = routing_obj.get(agent_id) {
                if let Some(content) = instruction_obj.get("content").and_then(|v| v.as_str()) {
                    instructions.push(RoutingInstruction {
                        target_agent_id: agent_id.clone(),
                        target_agent_name: agent_name.clone(),
                        content: content.to_string(),
                    });
                }
            }
        }

        Ok(instructions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_system_prompt() {
        let base = "You are a helpful assistant.";
        let targets = vec![
            ("agent1".to_string(), "Coder".to_string()),
            ("agent2".to_string(), "Tester".to_string()),
        ];

        let prompt = LlmClient::generate_system_prompt(base, &targets);

        assert!(prompt.contains("You are a helpful assistant."));
        assert!(prompt.contains("[ROUTING INSTRUCTION]"));
        assert!(prompt.contains("agent1"));
        assert!(prompt.contains("Coder"));
    }

    #[test]
    fn test_extract_json_block() {
        let response = r#"
        I will now route the tasks.

        ```json
        {
          "routing": {
            "agent1": { "content": "Write unit tests" }
          }
        }
        ```

        Done!
        "#;

        let json = ResponseParser::extract_json_block(response).unwrap();
        assert!(json.contains("routing"));
        assert!(json.contains("agent1"));
    }

    #[test]
    fn test_extract_routing() {
        let json_str = r#"
        {
          "routing": {
            "agent1": { "content": "Task 1" },
            "agent2": { "content": "Task 2" }
          }
        }
        "#;

        let json: serde_json::Value = serde_json::from_str(json_str).unwrap();
        let targets = vec![
            ("agent1".to_string(), "A".to_string()),
            ("agent2".to_string(), "B".to_string()),
        ];

        let routing = ResponseParser::extract_routing(&json, &targets).unwrap();
        assert_eq!(routing.len(), 2);
        assert_eq!(routing[0].content, "Task 1");
        assert_eq!(routing[1].content, "Task 2");
    }
}
