use serde::Serialize;
use tauri::{AppHandle, Emitter};
use crate::local_ai::rag_engine::VectorStore;
use regex::Regex;

// ============================================================================
// Struktury Danych
// ============================================================================

/// Update postępu dla streamingu do frontendu
#[derive(Serialize, Clone, Debug)]
pub struct AgenticStep {
    pub step_num: usize,
    pub max_steps: usize,
    pub action: String,        // "search", "include", "analyze", "done"
    pub description: String,   // "🔍 Searching for 'login'..."
    pub details: Option<String>,
}

/// Stan maszyny stanów agenta
pub struct AgentState {
    pub user_query: String,
    pub repo_map: String,          // Mapa terenu (wygenerowana raz)
    pub scratchpad: Vec<String>,    // Notatki Qwena (krótkie podsumowania)
    pub gemini_buffer: String,      // Akumulowany kontekst dla Gemini
    pub current_step: usize,
    pub max_steps: usize,
    pub conversation_history: Vec<ConversationTurn>, // Historia dla kontekstu Qwena
}

#[derive(Clone)]
struct ConversationTurn {
    role: String,      // "system", "user", "assistant"
    content: String,
}

/// Parsed command od Qwena
#[derive(Debug)]
pub enum AgentCommand {
    Search(String),
    ReadIndex(String),
    Include { file: String, reason: String },
    Done,
    Error(String),
}

// ============================================================================
// Główna Funkcja: Agent Loop
// ============================================================================

pub async fn run_agentic_context_task(
    task: String,
    max_steps: Option<usize>,
    repo_map_detail: String,
    vector_store: &VectorStore,
    app_handle: &AppHandle,
    project_roots: Vec<String>, // Do odczytywania plików
) -> Result<(String, String), String> {
    let max_steps = max_steps.unwrap_or(10);

    println!("[Agentic RAG] 🚀 Starting Agent Loop for task: '{}'", task);

    // === FAZA 1: Generate RepoMap ===
    send_progress_update(app_handle, 0, max_steps, "init", "🗺️ Building repository map...", None);

    let repo_map = generate_lightweight_repo_map(vector_store, &repo_map_detail)?;
    println!("[Agentic RAG] Generated RepoMap ({} chars)", repo_map.len());

    // === FAZA 2: Inicjalizacja Stanu Agenta ===
    let mut agent_state = AgentState {
        user_query: task.clone(),
        repo_map: repo_map.clone(),
        scratchpad: vec![],
        gemini_buffer: String::new(),
        current_step: 0,
        max_steps,
        conversation_history: vec![],
    };

    // System prompt (dodawany raz na początku)
    let system_prompt = build_agent_system_prompt(&agent_state);
    agent_state.conversation_history.push(ConversationTurn {
        role: "system".to_string(),
        content: system_prompt,
    });

    // User query jako pierwszy turn
    agent_state.conversation_history.push(ConversationTurn {
        role: "user".to_string(),
        content: format!("Task: {}\n\nWhat files should we gather? Use commands to explore.", task),
    });

    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: usize = 3;

    // === FAZA 3: Agent Loop ===
    for step in 1..=max_steps {
        agent_state.current_step = step;

        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
            println!("[Agentic RAG] 🛑 Aborting due to too many consecutive errors.");
            break;
        }

        println!("[Agentic RAG] --- Step {}/{} ---", step, max_steps);

        // Qwen generuje następny krok
        send_progress_update(app_handle, step, max_steps, "thinking", "🤔 Agent analyzing...", None);

        let qwen_response = match query_qwen_agent(&agent_state).await {
            Ok(resp) => resp,
            Err(e) => {
                println!("[Agentic RAG] ❌ Qwen query failed: {}", e);
                return Err(format!("Agent thinking failed at step {}: {}", step, e));
            }
        };

        println!("[Agentic RAG] Qwen response: {}", qwen_response);

        // Dodaj odpowiedź Qwena do historii
        agent_state.conversation_history.push(ConversationTurn {
            role: "assistant".to_string(),
            content: qwen_response.clone(),
        });

        // Parsuj komendę
        let command = parse_agent_command(&qwen_response);

        match command {
            AgentCommand::Search(query) => {
                send_progress_update(app_handle, step, max_steps, "search", &format!("🔍 Searching: {}", query), None);

                let chunks = execute_rag_search(vector_store, &query).await?;
                let file_summary = extract_file_summary(&chunks);

                let feedback = format!(
                    "Search found {} chunks in {} files: {}",
                    chunks.len(),
                    file_summary.len(),
                    file_summary.join(", ")
                );

                agent_state.scratchpad.push(feedback.clone());

                // Feedback do Qwena (jako user message)
                agent_state.conversation_history.push(ConversationTurn {
                    role: "user".to_string(),
                    content: format!("[SEARCH RESULT] {}\nWhat's next?", feedback),
                });

                println!("[Agentic RAG] Search completed: {}", feedback);
            },

            AgentCommand::ReadIndex(file_path) => {
                send_progress_update(app_handle, step, max_steps, "peek", &format!("👀 Peeking: {}", file_path), None);

                let symbols = match extract_symbols_from_chunks(vector_store, &file_path) {
                    Ok(s) => s,
                    Err(e) => format!("(No symbols found: {})", e),
                };

                let feedback = format!("File '{}' contains: {}", file_path, symbols);
                agent_state.scratchpad.push(feedback.clone());

                agent_state.conversation_history.push(ConversationTurn {
                    role: "user".to_string(),
                    content: format!("[INDEX] {}\nWhat's next?", feedback),
                });

                println!("[Agentic RAG] Index peek: {}", feedback);
            },

            AgentCommand::Include { file, reason } => {
                send_progress_update(app_handle, step, max_steps, "include", &format!("✅ Including: {}", file), Some(reason.clone()));

                // Odczytaj pełny plik z dysku
                let full_content = match read_file_from_roots(&file, &project_roots) {
                    Ok(content) => content,
                    Err(e) => {
                        let error_msg = format!("⚠️ Failed to read '{}': {}", file, e);
                        agent_state.scratchpad.push(error_msg.clone());
                        agent_state.conversation_history.push(ConversationTurn {
                            role: "user".to_string(),
                            content: format!("[ERROR] {}\nTry another file or DONE.", error_msg),
                        });
                        continue;
                    }
                };

                let line_count = full_content.lines().count();

                // Dodaj do Gemini Buffer (Markdown format)
                agent_state.gemini_buffer.push_str(&format!(
                    "\n## File: {}\n**Reason:** {}\n\n```\n{}\n```\n\n",
                    file, reason, full_content
                ));

                // Notatka w Scratchpad (krótka!)
                let note = format!("✅ INCLUDED: {} ({} lines - {})", file, line_count, reason);
                agent_state.scratchpad.push(note.clone());

                // Feedback do Qwena
                agent_state.conversation_history.push(ConversationTurn {
                    role: "user".to_string(),
                    content: format!("[CONFIRMED] {}\nWhat's next?", note),
                });

                println!("[Agentic RAG] File included: {}", file);
            },

            AgentCommand::Done => {
                send_progress_update(app_handle, step, max_steps, "done", "✨ Context building complete!", None);
                println!("[Agentic RAG] Agent called DONE. Exiting loop.");
                break;
            },

            AgentCommand::Error(err_msg) => {
                println!("[Agentic RAG] ⚠️ Parse error: {}", err_msg);
                consecutive_errors += 1;

                // Próbujemy naprawić - dajemy Qwenowi feedback
                agent_state.conversation_history.push(ConversationTurn {
                    role: "user".to_string(),
                    content: format!(
                        "[ERROR] I didn't understand your command: '{}'\nPlease use ONE of: SEARCH(\"...\"), INCLUDE(\"file\", \"reason\"), DONE",
                        err_msg
                    ),
                });

                // Nie liczymy tego jako stracony krok (chyba że przekroczymy limit)
                continue;
            }
        }
        
        // Reset licznika błędów po udanym kroku
        consecutive_errors = 0;
    }

    // === FAZA 4: Finalizacja ===
    if agent_state.gemini_buffer.is_empty() {
        return Err("Agent didn't collect any files. Try a more specific query.".to_string());
    }

    let final_context = format_final_gemini_context(&agent_state);
    let task_short_name = extract_short_task_name(&task);

    println!("[Agentic RAG] ✅ Completed! Generated {} chars for Gemini", final_context.len());

    Ok((final_context, task_short_name))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Wysyła update postępu przez Tauri event
fn send_progress_update(
    app_handle: &AppHandle,
    step: usize,
    max_steps: usize,
    action: &str,
    description: &str,
    details: Option<String>,
) {
    let update = AgenticStep {
        step_num: step,
        max_steps,
        action: action.to_string(),
        description: description.to_string(),
        details,
    };

    let _ = app_handle.emit("agentic_progress", serde_json::to_value(&update).unwrap());
}

/// Generuje lekką mapę repozytorium (lista plików + opcjonalnie symbole)
fn generate_lightweight_repo_map(
    vector_store: &VectorStore,
    detail_level: &str,
) -> Result<String, String> {
    let mut file_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Zbierz unikalne pliki z VectorStore index keys
    for key in vector_store.keys() {
        // key format: "src/main.rs::10"
        if let Some(file_path) = key.split("::").next() {
            file_set.insert(file_path.to_string());
        }
    }

    let mut files: Vec<String> = file_set.into_iter().collect();
    files.sort();

    let mut output = String::from("# Repository Index\n");

    if detail_level == "detailed" {
        output.push_str("## Files with Top-Level Symbols:\n");
        for file in &files {
            output.push_str(&format!("- {}\n", file));
            // TODO: W przyszłości można dodać tree-sitter parsing symboli
            // Na razie zostawiamy tylko nazwy plików
        }
    } else {
        output.push_str("## Indexed Files:\n");
        for file in &files {
            output.push_str(&format!("- {}\n", file));
        }
    }

    output.push_str(&format!("\nTotal: {} files\n", files.len()));

    Ok(output)
}

/// Buduje system prompt dla agenta
fn build_agent_system_prompt(agent_state: &AgentState) -> String {
    format!(r#"You are a Context Gathering Agent. Your job is to find and collect relevant code files for a Large Language Model (Gemini) to solve a coding task.

## Available Commands (use ONE per response):
- SEARCH("query") - Search codebase using RAG vector search
- READ_INDEX("path/to/file.ext") - Peek at file's metadata (quick check before including)
- INCLUDE("path/to/file.ext", "reason why needed") - Add FULL file to Gemini's context
- DONE - Finish when you have enough context

## Your Memory:
### Repository Map:
{}

### Your Notes (Scratchpad):
{}

### User Task:
{}

## Rules:
1. Use SEARCH to explore, INCLUDE to collect
2. When you INCLUDE a file, you'll get confirmation but won't see the code (it goes to Gemini)
3. Keep scratchpad notes concise: "✅ auth.rs included (login logic)"
4. If you see imports/calls to other files, INCLUDE those too
5. Max {} steps - be efficient!
6. Output ONLY the command in your response, nothing else

## Examples:
SEARCH("user authentication login")
INCLUDE("src/auth.rs", "contains login function")
DONE
"#,
        agent_state.repo_map,
        if agent_state.scratchpad.is_empty() {
            "(empty - you haven't collected anything yet)".to_string()
        } else {
            agent_state.scratchpad.join("\n")
        },
        agent_state.user_query,
        agent_state.max_steps
    )
}

/// Wysyła zapytanie do Qwena (port 8082)
async fn query_qwen_agent(agent_state: &AgentState) -> Result<String, String> {
    // Budujemy prompt w formacie ChatML (Qwen 2.5 format)
    let mut full_prompt = String::new();

    for turn in &agent_state.conversation_history {
        match turn.role.as_str() {
            "system" => full_prompt.push_str(&format!("<|im_start|>system\n{}<|im_end|>\n", turn.content)),
            "user" => full_prompt.push_str(&format!("<|im_start|>user\n{}<|im_end|>\n", turn.content)),
            "assistant" => full_prompt.push_str(&format!("<|im_start|>assistant\n{}<|im_end|>\n", turn.content)),
            _ => {}
        }
    }

    full_prompt.push_str("<|im_start|>assistant\n");

    let client = reqwest::Client::new();

    // [FIX] Zmiana na API KoboldCPP
    let response = client.post("http://127.0.0.1:8082/api/v1/generate")
        .json(&serde_json::json!({
            "prompt": full_prompt,
            "max_length": 256,       // Kobold: max_length
            "temperature": 0.1,      // Precyzja
            "stop_sequence": ["<|im_end|>", "\n\n"], // Kobold: stop_sequence
            "top_p": 0.9,
            "top_k": 40,
        }))
        .send()
        .await
        .map_err(|e| format!("Qwen connection failed: {}", e))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Qwen JSON parse failed: {}", e))?;

    // [FIX] Parsowanie odpowiedzi KoboldCPP
    let content = response["results"][0]["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    if content.is_empty() {
        return Err("Qwen returned empty response".to_string());
    }

    Ok(content)
}

/// Parsuje komendę z odpowiedzi Qwena
fn parse_agent_command(response: &str) -> AgentCommand {
    let response = response.trim();

    // Regex dla komend
    let search_re = Regex::new(r#"SEARCH\("([^"]+)"\)"#).unwrap();
    let read_re = Regex::new(r#"READ_INDEX\("([^"]+)"\)"#).unwrap();
    let include_re = Regex::new(r#"INCLUDE\("([^"]+)",\s*"([^"]+)"\)"#).unwrap();
    let done_re = Regex::new(r"^DONE\s*$").unwrap();

    if let Some(caps) = search_re.captures(response) {
        return AgentCommand::Search(caps[1].to_string());
    }

    if let Some(caps) = read_re.captures(response) {
        return AgentCommand::ReadIndex(caps[1].to_string());
    }

    if let Some(caps) = include_re.captures(response) {
        return AgentCommand::Include {
            file: caps[1].to_string(),
            reason: caps[2].to_string(),
        };
    }

    if done_re.is_match(response) {
        return AgentCommand::Done;
    }

    // Fallback: błąd parsowania
    AgentCommand::Error(response.to_string())
}

/// Wykonuje wyszukiwanie RAG
async fn execute_rag_search(
    vector_store: &VectorStore,
    query: &str,
) -> Result<Vec<String>, String> {
    vector_store.search_by_query(query.to_string(), 5).await
}

/// Ekstraktuje nazwy plików z chunków
fn extract_file_summary(chunks: &[String]) -> Vec<String> {
    let mut files = std::collections::HashSet::new();

    for chunk in chunks {
        // Chunk format: "// File: path/to/file.rs\n..."
        if let Some(line) = chunk.lines().next() {
            if line.starts_with("// File:") {
                let file = line.replace("// File:", "").trim().to_string();
                files.insert(file);
            }
        }
    }

    files.into_iter().collect()
}

/// Ekstraktuje symbole z chunków dla danego pliku (symulacja READ_INDEX)
fn extract_symbols_from_chunks(
    vector_store: &VectorStore,
    file_path: &str,
) -> Result<String, String> {
    let mut symbols = Vec::new();

    for key in vector_store.keys() {
        if key.starts_with(file_path) {
            // key: "src/auth.rs::42"
            if let Some(line_num) = key.split("::").nth(1) {
                symbols.push(format!("L{}", line_num));
            }
        }
    }

    if symbols.is_empty() {
        return Err("File not in index".to_string());
    }

    // Limitujemy do pierwszych 10 dla zwięzłości
    symbols.truncate(10);
    Ok(symbols.join(", "))
}

/// Odczytuje plik z dysku (próbuje wszystkie root paths)
fn read_file_from_roots(file_path: &str, roots: &[String]) -> Result<String, String> {
    use std::path::Path;

    // Normalizacja ścieżki (usunięcie ./ na początku)
    let clean_path = file_path.trim_start_matches("./");

    for root in roots {
        let full_path = Path::new(root).join(clean_path);

        if full_path.exists() && full_path.is_file() {
            return std::fs::read_to_string(&full_path)
                .map_err(|e| format!("Read error: {}", e));
        }
    }

    Err(format!("File not found in any project root: {}", file_path))
}

/// Formatuje finalny kontekst dla Gemini
fn format_final_gemini_context(agent_state: &AgentState) -> String {
    let mut output = String::new();

    output.push_str("# Smart Context (Generated by Agentic RAG)\n\n");
    output.push_str(&format!("**User Task:** {}\n\n", agent_state.user_query));

    output.push_str("## Agent's Search Journey:\n");
    for (i, note) in agent_state.scratchpad.iter().enumerate() {
        output.push_str(&format!("{}. {}\n", i + 1, note));
    }

    output.push_str("\n---\n\n");
    output.push_str("## Collected Files:\n\n");
    output.push_str(&agent_state.gemini_buffer);

    output.push_str("\n---\n\n");
    output.push_str("🤖 *Generated with Gluon Agentic RAG*\n");

    output
}

/// Ekstraktuje krótką nazwę z taska (do nazwy pliku)
fn extract_short_task_name(task: &str) -> String {
    // Bierz pierwsze 3-5 słów, usuń znaki specjalne
    let words: Vec<&str> = task.split_whitespace().take(4).collect();
    let name = words.join("_");

    // Sanitize: tylko alfanumeryczne + underscore
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
        .chars()
        .take(40)
        .collect()
}