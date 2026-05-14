//! Workflow Execution Engine
//!
//! Handles running agents with LLM inference and smart routing.

use tauri::{AppHandle, Emitter, Manager, State};
use crate::apply_system::ApplySystemState;
use crate::workflow::agent_workflow::{AgentStatus, Message};
use crate::workflow::llm_inference::{LlmClient, LlmConfig, LlmMessage, LlmProvider, ResponseParser};
use serde::{Deserialize, Serialize};

/// Configuration for LLM provider (from settings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowLlmSettings {
    pub provider: String, // "openai", "claude", "custom"
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub custom_base_url: Option<String>,
}

/// Result of agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionResult {
    pub agent_id: String,
    pub agent_name: String,
    pub status: String, // "success", "failed"
    pub response: String,
    pub routed_to: Vec<String>, // Agent IDs that received tasks
    pub required_healing: bool,
    pub error: Option<String>,
}

/// Executes an agent with a task
#[tauri::command]
pub async fn workflow_execute_agent(
    agent_id: String,
    user_message: String,
    llm_settings: WorkflowLlmSettings,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<AgentExecutionResult, String> {
    println!("[Workflow Execution] 🚀 Starting execution for agent: {}", agent_id);

    // 1. Get agent from graph
    let agent = {
        let graph = state.agent_workflow.get_graph();
        let graph_lock = graph.lock().unwrap();

        graph_lock.agents.get(&agent_id)
            .cloned()
            .ok_or_else(|| format!("Agent not found: {}", agent_id))?
    };

    println!("[Workflow Execution] 📋 Agent '{}' (Type: {:?}, Status: {:?})", agent.name, agent.agent_type, agent.status);

    // 2. Check if agent is available
    if !matches!(agent.status, AgentStatus::Idle | AgentStatus::Success) {
        return Err(format!("Agent '{}' is not available (status: {:?})", agent.name, agent.status));
    }

    // 3. Set agent to Working status
    {
        let graph = state.agent_workflow.get_graph();
        let mut graph_lock = graph.lock().unwrap();
        graph_lock.set_agent_task(&agent_id, user_message.clone())?;
    }

    // Broadcast status update
    broadcast_agent_status(&app_handle, &state, &agent_id);

    // 4. Get target agents for routing
    let target_agents = {
        let graph = state.agent_workflow.get_graph();
        let graph_lock = graph.lock().unwrap();
        let targets = graph_lock.get_targets(&agent_id);

        targets.into_iter()
            .map(|a| (a.id.clone(), a.name.clone()))
            .collect::<Vec<_>>()
    };

    println!("[Workflow Execution] 🎯 Found {} target agents for routing", target_agents.len());

    // 5. Build System Prompt with routing instructions
    let base_prompt = agent.system_prompt_template
        .clone()
        .unwrap_or_else(|| "You are a helpful AI assistant.".to_string());

    let system_prompt = LlmClient::generate_system_prompt(&base_prompt, &target_agents);

    // 6. Build message history
    let mut messages = vec![
        LlmMessage {
            role: "system".to_string(),
            content: system_prompt,
        }
    ];

    // Add agent's conversation history
    for msg in &agent.history {
        messages.push(LlmMessage {
            role: msg.role.clone(),
            content: msg.content.clone(),
        });
    }

    // Add current user message
    messages.push(LlmMessage {
        role: "user".to_string(),
        content: user_message.clone(),
    });

    // 7. Create LLM client
    let llm_config = LlmConfig {
        provider: match llm_settings.provider.as_str() {
            "openai" => LlmProvider::OpenAI,
            "claude" => LlmProvider::Claude,
            "custom" => LlmProvider::Custom {
                base_url: llm_settings.custom_base_url.unwrap_or_default(),
            },
            _ => return Err(format!("Unknown LLM provider: {}", llm_settings.provider)),
        },
        api_key: llm_settings.api_key,
        model: llm_settings.model,
        temperature: llm_settings.temperature,
        max_tokens: llm_settings.max_tokens,
    };

    let llm_client = LlmClient::new(llm_config);

    // 8. Call LLM
    println!("[Workflow Execution] 🤖 Calling LLM API...");
    let raw_response = match llm_client.complete(messages).await {
        Ok(resp) => resp,
        Err(e) => {
            println!("[Workflow Execution] ❌ LLM API call failed: {}", e);

            // Mark agent as failed
            {
                let graph = state.agent_workflow.get_graph();
                let mut graph_lock = graph.lock().unwrap();
                graph_lock.mark_agent_failed(&agent_id)?;
            }

            broadcast_agent_status(&app_handle, &state, &agent_id);

            return Err(format!("LLM API call failed: {}", e));
        }
    };

    println!("[Workflow Execution] ✅ Received response ({} chars)", raw_response.len());

    // 9. Parse response with self-healing
    println!("[Workflow Execution] 🔍 Parsing response...");
    let parsed = ResponseParser::parse_with_healing(
        raw_response.clone(),
        &target_agents,
        &llm_client,
        3, // max 3 retry attempts
    ).await?;

    println!("[Workflow Execution] ✅ Parsed successfully (healing required: {})", parsed.required_healing);
    println!("[Workflow Execution] 📤 Routing to {} agents", parsed.routing.len());

    // 10. Save response to agent history
    {
        let graph = state.agent_workflow.get_graph();
        let mut graph_lock = graph.lock().unwrap();

        // Add user message to history
        graph_lock.add_message_to_history(
            &agent_id,
            "user".to_string(),
            user_message.clone(),
        )?;

        // Add assistant response to history
        graph_lock.add_message_to_history(
            &agent_id,
            "assistant".to_string(),
            raw_response.clone(),
        )?;

        // Mark agent as success
        graph_lock.mark_agent_success(&agent_id)?;
    }

    // 11. Route to target agents
    let routed_ids: Vec<String> = parsed.routing.iter()
        .map(|r| r.target_agent_id.clone())
        .collect();

    for routing_instruction in &parsed.routing {
        println!("[Workflow Execution] 📨 Routing to '{}': {} chars",
            routing_instruction.target_agent_name,
            routing_instruction.content.len()
        );

        // Emit routing event for handling by main.rs or extension
        let _ = app_handle.emit("workflow-route-message", serde_json::json!({
            "from_agent_id": agent_id,
            "from_agent_name": agent.name,
            "to_agent_id": routing_instruction.target_agent_id,
            "to_agent_name": routing_instruction.target_agent_name,
            "content": routing_instruction.content,
        }));
    }

    // Broadcast final status
    broadcast_agent_status(&app_handle, &state, &agent_id);

    // 12. Save graph
    let storage_path = crate::workflow::workflow_commands::get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)
        .map_err(|e| format!("Failed to save workflow: {}", e))?;

    Ok(AgentExecutionResult {
        agent_id: agent_id.clone(),
        agent_name: agent.name,
        status: "success".to_string(),
        response: raw_response,
        routed_to: routed_ids,
        required_healing: parsed.required_healing,
        error: None,
    })
}

/// Helper to broadcast agent status updates
fn broadcast_agent_status(app: &AppHandle, state: &State<'_, ApplySystemState>, agent_id: &str) {
    let graph = state.agent_workflow.get_graph();
    let graph_data = graph.lock().unwrap().clone();

    // Emit to Tauri window (Command Center)
    let _ = app.emit("workflow-state-sync", &graph_data);

    // Emit to Bridge (for Chrome Extension)
    let _ = app.emit("workflow-sync-bridge", &graph_data);
}

/// Clears agent history
#[tauri::command]
pub fn workflow_clear_agent_history(
    agent_id: String,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let graph = state.agent_workflow.get_graph();
    let mut graph_lock = graph.lock().unwrap();

    graph_lock.clear_agent_history(&agent_id)?;

    drop(graph_lock);

    // Save
    let storage_path = crate::workflow::workflow_commands::get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)?;

    // Broadcast update
    broadcast_agent_status(&app_handle, &state, &agent_id);

    Ok(())
}

/// Gets agent history
#[tauri::command]
pub fn workflow_get_agent_history(
    agent_id: String,
    state: State<'_, ApplySystemState>,
) -> Result<Vec<Message>, String> {
    let graph = state.agent_workflow.get_graph();
    let graph_lock = graph.lock().unwrap();

    let agent = graph_lock.agents.get(&agent_id)
        .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

    Ok(agent.history.clone())
}

/// Resets agent to Idle status
#[tauri::command]
pub fn workflow_reset_agent(
    agent_id: String,
    state: State<'_, ApplySystemState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let graph = state.agent_workflow.get_graph();
    let mut graph_lock = graph.lock().unwrap();

    graph_lock.reset_agent_to_idle(&agent_id)?;

    drop(graph_lock);

    // Save
    let storage_path = crate::workflow::workflow_commands::get_workflow_storage_path(&app_handle)?;
    state.agent_workflow.save_to_storage(&storage_path)?;

    // Broadcast update
    broadcast_agent_status(&app_handle, &state, &agent_id);

    Ok(())
}
