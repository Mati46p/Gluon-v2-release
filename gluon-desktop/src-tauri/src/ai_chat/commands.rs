// ============================================================================
// AI CHAT TAURI COMMANDS
// ============================================================================

use crate::ai_chat::{
    keyring, AiChatState, AiProvider, ApiKeyStatus, ChatMessage, ChatSession,
    ChatSessionWithProvider, CreateSessionPayload, SendMessagePayload, SetApiKeyPayload,
    StreamChunk,
};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};

// ============================================================================
// PROVIDER MANAGEMENT
// ============================================================================

#[tauri::command]
pub async fn get_ai_providers(pool: State<'_, SqlitePool>) -> Result<Vec<AiProvider>, String> {
    sqlx::query_as::<_, AiProvider>("SELECT * FROM ai_providers ORDER BY name")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// SESSION MANAGEMENT
// ============================================================================

#[tauri::command]
pub async fn get_chat_sessions(
    provider_id: Option<i64>,
    pool: State<'_, SqlitePool>,
) -> Result<Vec<ChatSessionWithProvider>, String> {
    let query = r#"
        SELECT
            cs.id,
            cs.provider_id,
            ap.name as provider_name,
            ap.provider_type,
            cs.title,
            cs.created_at,
            cs.updated_at,
            cs.is_pinned,
            cs.token_usage_total,
            COUNT(cm.id) as message_count
        FROM chat_sessions cs
        JOIN ai_providers ap ON cs.provider_id = ap.id
        LEFT JOIN chat_messages cm ON cs.id = cm.session_id
        WHERE (? IS NULL OR cs.provider_id = ?)
        GROUP BY cs.id
        ORDER BY cs.is_pinned DESC, cs.updated_at DESC
    "#;

    sqlx::query_as::<_, ChatSessionWithProvider>(query)
        .bind(provider_id)
        .bind(provider_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_chat_session(
    payload: CreateSessionPayload,
    pool: State<'_, SqlitePool>,
) -> Result<ChatSession, String> {
    // Auto-generate title if not provided
    let title = payload.title.unwrap_or_else(|| {
        let now = chrono::Local::now();
        format!("Chat {}", now.format("%Y-%m-%d %H:%M"))
    });

    let now = chrono::Local::now().to_rfc3339();

    let result = sqlx::query(
        r#"INSERT INTO chat_sessions
           (provider_id, title, created_at, updated_at)
           VALUES (?, ?, ?, ?)"#,
    )
    .bind(payload.provider_id)
    .bind(&title)
    .bind(&now)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    // Fetch created session
    let session = sqlx::query_as::<_, ChatSession>("SELECT * FROM chat_sessions WHERE id = ?")
        .bind(result.last_insert_rowid())
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(session)
}

#[tauri::command]
pub async fn delete_chat_session(
    session_id: i64,
    pool: State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM chat_sessions WHERE id = ?")
        .bind(session_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn toggle_session_pin(
    session_id: i64,
    pool: State<'_, SqlitePool>,
) -> Result<bool, String> {
    // Get current pin status
    let current: (bool,) =
        sqlx::query_as("SELECT is_pinned FROM chat_sessions WHERE id = ?")
            .bind(session_id)
            .fetch_one(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let new_status = !current.0;

    sqlx::query("UPDATE chat_sessions SET is_pinned = ? WHERE id = ?")
        .bind(new_status)
        .bind(session_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(new_status)
}

#[tauri::command]
pub async fn rename_chat_session(
    session_id: i64,
    new_title: String,
    pool: State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE chat_sessions SET title = ? WHERE id = ?")
        .bind(&new_title)
        .bind(session_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ============================================================================
// MESSAGE MANAGEMENT
// ============================================================================

#[tauri::command]
pub async fn get_chat_messages(
    session_id: i64,
    pool: State<'_, SqlitePool>,
) -> Result<Vec<ChatMessage>, String> {
    sqlx::query_as::<_, ChatMessage>(
        "SELECT * FROM chat_messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_chat_message(
    payload: SendMessagePayload,
    pool: State<'_, SqlitePool>,
    state: State<'_, AiChatState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // 1. Save user message to DB
    let now = chrono::Local::now().to_rfc3339();
    let user_message_result = sqlx::query(
        r#"INSERT INTO chat_messages
           (session_id, role, content, created_at, token_count)
           VALUES (?, 'user', ?, ?, 0)"#,
    )
    .bind(payload.session_id)
    .bind(&payload.content)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let user_msg_id = user_message_result.last_insert_rowid();

    // Emit user message immediately to UI
    let _ = app_handle.emit(
        "chat_message_added",
        ChatMessage {
            id: user_msg_id,
            session_id: payload.session_id,
            role: "user".to_string(),
            content: payload.content.clone(),
            created_at: now.clone(),
            token_count: 0,
        },
    );

    // 2. Get session and provider info
    let session: ChatSession = sqlx::query_as("SELECT * FROM chat_sessions WHERE id = ?")
        .bind(payload.session_id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let provider: AiProvider = sqlx::query_as("SELECT * FROM ai_providers WHERE id = ?")
        .bind(session.provider_id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    // 3. Get conversation history
    let history: Vec<ChatMessage> = sqlx::query_as(
        "SELECT * FROM chat_messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(payload.session_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    // 4. Call appropriate provider API with streaming (spawn async task)
    let pool_clone = pool.inner().clone();
    let app_handle_clone = app_handle.clone();
    let state_clone = state.inner().clone();
    let provider_type = provider.provider_type.clone();

    tokio::spawn(async move {
        let result = match provider_type.as_str() {
            "gemini" => {
                crate::ai_chat::providers::gemini::stream_response(
                    payload.session_id,
                    history,
                    state_clone,
                    pool_clone.clone(),
                    app_handle_clone.clone(),
                )
                .await
            }
            "claude" => {
                crate::ai_chat::providers::claude::stream_response(
                    payload.session_id,
                    history,
                    state_clone,
                    pool_clone.clone(),
                    app_handle_clone.clone(),
                )
                .await
            }
            "vscode" => {
                crate::ai_chat::providers::vscode::stream_response(
                    payload.session_id,
                    history,
                    state_clone,
                    pool_clone.clone(),
                    app_handle_clone.clone(),
                )
                .await
            }
            _ => Err(format!("Unsupported provider: {}", provider_type)),
        };

        if let Err(e) = result {
            eprintln!("[AiChat] Provider error: {}", e);
            // Emit error to frontend
            let _ = app_handle_clone.emit(
                "chat_stream_chunk",
                StreamChunk {
                    session_id: payload.session_id,
                    message_id: None,
                    content: format!("Error: {}", e),
                    is_final: true,
                },
            );
        }
    });

    // Update session timestamp
    sqlx::query("UPDATE chat_sessions SET updated_at = ? WHERE id = ?")
        .bind(chrono::Local::now().to_rfc3339())
        .bind(payload.session_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ============================================================================
// API KEY MANAGEMENT
// ============================================================================

#[tauri::command]
pub async fn set_ai_api_key(
    payload: SetApiKeyPayload,
    state: State<'_, AiChatState>,
    pool: State<'_, SqlitePool>,
) -> Result<(), String> {
    // 1. Store in Keyring (persistent)
    keyring::store_api_key(&payload.provider_type, &payload.api_key)?;

    // 2. Store in RAM (fallback)
    {
        let mut keys = state.api_keys.lock().map_err(|e| e.to_string())?;
        keys.insert(payload.provider_type.clone(), payload.api_key.clone());
    } // MutexGuard dropped here

    // 3. Update DB metadata
    let provider: AiProvider =
        sqlx::query_as("SELECT * FROM ai_providers WHERE provider_type = ?")
            .bind(&payload.provider_type)
            .fetch_one(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let now = chrono::Local::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO ai_api_keys (provider_id, key_name, is_configured, last_verified)
           VALUES (?, ?, 1, ?)
           ON CONFLICT(provider_id) DO UPDATE SET
           is_configured = 1, last_verified = ?"#,
    )
    .bind(provider.id)
    .bind(format!("{}_api_key", payload.provider_type))
    .bind(&now)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_api_key_status(
    provider_type: String,
    state: State<'_, AiChatState>,
    pool: State<'_, SqlitePool>,
) -> Result<ApiKeyStatus, String> {
    // Check if key exists in keyring or RAM
    let has_key = keyring::has_api_key(&provider_type)
        || state
            .api_keys
            .lock()
            .map_err(|e| e.to_string())?
            .contains_key(&provider_type);

    // Get metadata from DB
    let provider: Option<AiProvider> =
        sqlx::query_as("SELECT * FROM ai_providers WHERE provider_type = ?")
            .bind(&provider_type)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let last_verified = if let Some(p) = provider {
        let key_info: Option<(String,)> =
            sqlx::query_as("SELECT last_verified FROM ai_api_keys WHERE provider_id = ?")
                .bind(p.id)
                .fetch_optional(pool.inner())
                .await
                .map_err(|e| e.to_string())?;

        key_info.map(|k| k.0)
    } else {
        None
    };

    Ok(ApiKeyStatus {
        provider_type,
        is_configured: has_key,
        last_verified,
    })
}

// ============================================================================
// EXPORT
// ============================================================================

#[tauri::command]
pub async fn export_chat_session(
    session_id: i64,
    pool: State<'_, SqlitePool>,
) -> Result<String, String> {
    // Get session
    let session: ChatSession = sqlx::query_as("SELECT * FROM chat_sessions WHERE id = ?")
        .bind(session_id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    // Get messages
    let messages: Vec<ChatMessage> = sqlx::query_as(
        "SELECT * FROM chat_messages WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    // Format as Markdown
    let mut markdown = format!("# {}\n\n", session.title);
    markdown.push_str(&format!("**Created:** {}\n", session.created_at));
    markdown.push_str(&format!(
        "**Total Tokens:** {}\n\n",
        session.token_usage_total
    ));
    markdown.push_str("---\n\n");

    for msg in messages {
        let role_icon = match msg.role.as_str() {
            "user" => "👤",
            "assistant" => "🤖",
            "system" => "⚙️",
            _ => "💬",
        };

        markdown.push_str(&format!(
            "### {} {}\n\n",
            role_icon,
            msg.role.to_uppercase()
        ));
        markdown.push_str(&format!("{}\n\n", msg.content));
        markdown.push_str(&format!(
            "*{} • {} tokens*\n\n",
            msg.created_at, msg.token_count
        ));
        markdown.push_str("---\n\n");
    }

    Ok(markdown)
}
