// ============================================================================
// GEMINI API PROVIDER
// Google's Gemini API integration with streaming support
// ============================================================================

use crate::ai_chat::{AiChatState, ChatMessage, StreamChunk};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

// ============================================================================
// GEMINI API STRUCTURES
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageMetadata {
    prompt_tokens: i32,
    candidates_tokens: i32,
    total_tokens: i32,
}

#[derive(Debug, Deserialize)]
struct GeminiError {
    error: GeminiErrorDetails,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorDetails {
    code: i32,
    message: String,
    status: String,
}

// ============================================================================
// GEMINI CLIENT
// ============================================================================

pub struct GeminiClient {
    api_key: String,
    model: String,
    client: Client,
}

impl GeminiClient {
    /// Create a new Gemini client with API key
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "gemini-1.5-flash".to_string(), // Default model
            client: Client::new(),
        }
    }

    /// Set the model to use (e.g., "gemini-1.5-pro", "gemini-1.5-flash")
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    /// Convert chat history to Gemini format
    fn convert_messages(&self, messages: &[ChatMessage]) -> Vec<GeminiContent> {
        messages
            .iter()
            .map(|msg| {
                let role = match msg.role.as_str() {
                    "user" => "user",
                    "assistant" => "model",
                    _ => "user",
                };

                GeminiContent {
                    role: role.to_string(),
                    parts: vec![GeminiPart {
                        text: msg.content.clone(),
                    }],
                }
            })
            .collect()
    }

    /// Send a streaming request to Gemini API
    pub async fn send_message_stream(
        &self,
        messages: Vec<ChatMessage>,
        message_id: i64,
        app_handle: AppHandle,
    ) -> Result<(String, i32), String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?key={}&alt=sse",
            self.model, self.api_key
        );

        let request_body = GeminiRequest {
            contents: self.convert_messages(&messages),
            generation_config: Some(GenerationConfig {
                temperature: Some(0.7),
                top_p: Some(0.95),
                top_k: Some(40),
                max_output_tokens: Some(8192),
            }),
        };

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Failed to send streaming request: {}", e))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            // Try to parse as Gemini error
            if let Ok(gemini_error) = serde_json::from_str::<GeminiError>(&error_text) {
                return Err(format!(
                    "Gemini API error ({}): {}",
                    gemini_error.error.code, gemini_error.error.message
                ));
            }

            return Err(format!("API request failed: {}", error_text));
        }

        // Process SSE stream
        let mut stream = response.bytes_stream().eventsource();
        let mut accumulated_content = String::new();
        let mut total_tokens = 0;

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    // Parse the event data
                    if let Ok(chunk_response) =
                        serde_json::from_str::<GeminiResponse>(&event.data)
                    {
                        // Extract content from the chunk
                        if let Some(candidate) = chunk_response.candidates.first() {
                            if let Some(part) = candidate.content.parts.first() {
                                let chunk_text = &part.text;

                                if !chunk_text.is_empty() {
                                    accumulated_content.push_str(chunk_text);

                                    // Emit chunk to frontend
                                    let _ = app_handle.emit(
                                        "chat_stream_chunk",
                                        StreamChunk {
                                            session_id: 0, // Will be ignored in frontend
                                            message_id: Some(message_id),
                                            content: chunk_text.clone(),
                                            is_final: false,
                                        },
                                    );
                                }
                            }

                            // Update token count
                            if let Some(usage) = &chunk_response.usage_metadata {
                                total_tokens = usage.total_tokens;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[Gemini] Stream error: {}", e);
                    // Continue processing other events
                }
            }
        }

        // Emit final chunk
        let _ = app_handle.emit(
            "chat_stream_chunk",
            StreamChunk {
                session_id: 0, // Will be ignored in frontend
                message_id: Some(message_id),
                content: String::new(),
                is_final: true,
            },
        );

        // Estimate tokens if not provided
        let token_count = if total_tokens > 0 {
            total_tokens
        } else {
            estimate_tokens(&accumulated_content)
        };

        Ok((accumulated_content, token_count))
    }

    /// Estimate token count (approximate)
    pub fn estimate_tokens_internal(text: &str) -> i32 {
        // Rough estimation: ~4 characters per token
        // More accurate would be to use tiktoken or similar
        (text.len() / 4).max(1) as i32
    }
}

/// Estimate token count (approximate)
pub fn estimate_tokens(text: &str) -> i32 {
    GeminiClient::estimate_tokens_internal(text)
}

// ============================================================================
// PUBLIC API - USED BY COMMANDS.RS
// ============================================================================

/// Stream response from Gemini API
pub async fn stream_response(
    session_id: i64,
    history: Vec<ChatMessage>,
    state: AiChatState,
    pool: SqlitePool,
    app_handle: AppHandle,
) -> Result<(), String> {
    // Get API key from state (already loaded from keyring)
    let api_key = {
        let keys = state.api_keys.lock().map_err(|e| e.to_string())?;
        keys.get("gemini")
            .cloned()
            .ok_or_else(|| "Gemini API key not configured".to_string())?
    };

    // Create assistant message placeholder
    // FIXED: Changed from sqlx::query! to sqlx::query to avoid DATABASE_URL requirement
    let message_id = sqlx::query(
        "INSERT INTO chat_messages (session_id, role, content, token_count) VALUES (?, 'assistant', '', 0)",
    )
    .bind(session_id)
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to create message: {}", e))?
    .last_insert_rowid();

    // Emit message added event
    let message = ChatMessage {
        id: message_id,
        session_id,
        role: "assistant".to_string(),
        content: String::new(),
        token_count: 0,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let _ = app_handle.emit("chat_message_added", message);

    // Create Gemini client and stream response
    let client = GeminiClient::new(api_key);

    match client.send_message_stream(history, message_id, app_handle.clone()).await {
        Ok((content, token_count)) => {
            // Update message with final content and tokens
            // FIXED: Changed from sqlx::query! to sqlx::query
            sqlx::query(
                "UPDATE chat_messages SET content = ?, token_count = ? WHERE id = ?",
            )
            .bind(content)
            .bind(token_count)
            .bind(message_id)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to update message: {}", e))?;

            // Update session token usage
            // FIXED: Changed from sqlx::query! to sqlx::query
            sqlx::query(
                "UPDATE chat_sessions SET token_usage_total = token_usage_total + ?, updated_at = datetime('now') WHERE id = ?",
            )
            .bind(token_count)
            .bind(session_id)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to update session: {}", e))?;

            Ok(())
        }
        Err(e) => {
            // Delete the placeholder message on error
            // FIXED: Changed from sqlx::query! to sqlx::query
            let _ = sqlx::query("DELETE FROM chat_messages WHERE id = ?")
                .bind(message_id)
                .execute(&pool)
                .await;

            Err(e)
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_estimation() {
        assert_eq!(estimate_tokens("Hello, world!"), 3);
        assert_eq!(estimate_tokens(""), 1);
        assert_eq!(
            estimate_tokens("This is a longer message that should have more tokens"),
            13
        );
    }

    #[test]
    fn test_message_conversion() {
        let client = GeminiClient::new("test-key".to_string());
        let messages = vec![
            ChatMessage {
                id: 1,
                session_id: 1,
                role: "user".to_string(),
                content: "Hello".to_string(),
                token_count: 1,
                created_at: "".to_string(),
            },
            ChatMessage {
                id: 2,
                session_id: 1,
                role: "assistant".to_string(),
                content: "Hi!".to_string(),
                token_count: 1,
                created_at: "".to_string(),
            },
        ];

        let converted = client.convert_messages(&messages);
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "user");
        assert_eq!(converted[1].role, "model");
    }
}