#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{Engine as _, engine::general_purpose};
use chrono::Local;
use futures_util::{SinkExt, StreamExt};
use reqwest;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use sqlx::migrate::MigrateDatabase;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tauri::Emitter; 
use tauri::Listener;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::time::{Duration, interval};
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};
use walkdir::WalkDir;
use uuid::Uuid;
use lazy_static::lazy_static;
use gluon_desktop_lib::editor_bridge::{EditorBridge, EditorEditResponse, FileChangeNotification, ChangeRange};

// ============================================================================
// Apply System Module
// ============================================================================
use gluon_desktop_lib::apply_system::ApplySystemState;
use gluon_desktop_lib::apply_system;
// [FIX] Używamy definicji typów z biblioteki, aby uniknąć mismatchu TypeId w Tauri State
use gluon_desktop_lib::{AppState, ContextConfig, TreeNode, NodeType};
use gluon_desktop_lib::apply_system::context::ranker::rank_files;
use gluon_desktop_lib::apply_system::extraction::html_parser::HtmlParser;

// ============================================================================
// Google Integration
// ============================================================================
use gluon_desktop_lib::google::google_auth::{self, GoogleAuthState};
use gluon_desktop_lib::google::google_drive;

// ============================================================================
// Local AI Integration
// ============================================================================
use gluon_desktop_lib::local_ai::service_manager::LocalAiService;
use gluon_desktop_lib::local_ai::vector_map_manager::VectorMapManager;

// ============================================================================
// AI Chat Module
// ============================================================================
mod ai_chat;
mod vector_map_commands;

// ============================================================================
// WebSocket Protocol Structures
// ============================================================================

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct FileAsBase64Response {
    filename: String,
    mime_type: String,
    base64_content: String,
}

#[derive(serde::Deserialize, Debug)]
struct WebSocketRequest {
    id: String,
    action: String,
    #[serde(default)]
    payload: serde_json::Value,
}

#[derive(serde::Serialize, Debug)]
struct WebSocketResponse<T: serde::Serialize> {
    request_id: String,
    action: String,
    payload: T,
}

// ============================================================================
// Context Generation Progress Structures
// ============================================================================

#[derive(serde::Serialize, Clone, Debug)]
struct ContextGenerationProgress {
    step: u8,
    max_steps: u8,
    stage: String,
    message: String,
    percentage: u8,
    files_count: Option<usize>,
    total_files: Option<usize>,
    estimated_time_left: Option<u32>,
}

// Helper function to emit context generation progress
async fn emit_context_progress(
    app_handle: &AppHandle,
    step: u8,
    stage: &str,
    message: &str,
    percentage: u8,
    files_count: Option<usize>,
    total_files: Option<usize>,
) {
    let progress = ContextGenerationProgress {
        step,
        max_steps: 10,
        stage: stage.to_string(),
        message: message.to_string(),
        percentage,
        files_count,
        total_files,
        estimated_time_left: None,
    };

    // Emit przez Tauri Event System
    // Bridge listener przekieruje to automatycznie przez WebSocket
    let _ = app_handle.emit("context_generation_progress", &progress);
}

// ============================================================================
// Cancel Mechanism for Context Generation
// ============================================================================

lazy_static! {
    static ref CANCEL_TOKENS: Arc<std::sync::Mutex<HashMap<String, Arc<AtomicBool>>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
}

// Register a new cancel token for a request
fn register_cancel_token(request_id: &str) -> Arc<AtomicBool> {
    let token = Arc::new(AtomicBool::new(false));
    CANCEL_TOKENS.lock().unwrap().insert(request_id.to_string(), token.clone());
    token
}

// Check if a request has been cancelled
fn check_cancelled(request_id: &str) -> bool {
    if let Some(token) = CANCEL_TOKENS.lock().unwrap().get(request_id) {
        token.load(AtomicOrdering::Relaxed)
    } else {
        false
    }
}

// Remove a cancel token (cleanup)
fn remove_cancel_token(request_id: &str) {
    CANCEL_TOKENS.lock().unwrap().remove(request_id);
}

// ============================================================================
// Database Models
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
struct Project {
    id: i64,
    path: String,
    #[sqlx(default)]
    excluded_paths: Option<String>,
    #[sqlx(default)]
    download_path: Option<String>,
    #[sqlx(default)]
    allowed_extensions: Option<String>,
    #[sqlx(default)]
    vector_map_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct Environment {
    id: i64,
    name: String,
    icon: String,
    created_at: String,
    is_default: bool,
    language: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct Prompt {
    id: i64,
    environment_id: i64,
    name: String,
    content: Option<String>,
    category: String,
    enabled_by_default: bool,
    sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct ProjectWithEnv {
    id: i64,
    path: String,
    excluded_paths: Option<String>,
    download_path: Option<String>,
    allowed_extensions: Option<String>,
    #[sqlx(default)]
    vector_map_id: Option<i64>,
    environment_id: Option<i64>,
}

// ============================================================================
// File System & Payload Models
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct Settings {
    port: String,
    auto_start: bool,
    log_level: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct ExtensionTemplate {
    id: i64,
    name: String,
    extensions: String,
}

#[derive(Serialize)]
struct FileEntry {
    path: String,
    size_bytes: u64,
    modified_at: u128,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct FileContent {
    path: String,
    content: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize, Debug)]
struct GetFileTreesPayload {
    paths: Vec<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ProjectTreeResponse {
    project_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tree: Option<Vec<TreeNode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GetFilesMultiPayload {
    projects: Vec<ProjectFilesRequest>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ProjectFilesRequest {
    root_path: String,
    relative_paths: Vec<String>,
    /// Map: filePath -> Vec<symbolName>
    /// If present, only extract these symbols instead of full file
    #[serde(default)]
    symbols: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GetFilesMultiResponse {
    project_path: String,
    files: Vec<FileContent>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DirectoryNode {
    name: String,
    path: String,
    is_excluded: bool,
    children: Vec<DirectoryNode>,
    level: u32,
}

#[tauri::command]
async fn get_directory_tree(
    root_path: String,
    excluded_paths: Vec<String>,
    max_depth: u32,
) -> Result<Vec<DirectoryNode>, String> {
    fn build_tree(
        path: &Path,
        current_level: u32,
        max_depth: u32,
        excluded: &HashSet<String>,
        root: &Path,
    ) -> Vec<DirectoryNode> {
        if current_level >= max_depth {
            return Vec::new();
        }

        let mut nodes = Vec::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let dir_name = entry.file_name().to_string_lossy().to_string();
                        let relative_path = entry
                            .path()
                            .strip_prefix(root)
                            .unwrap_or(&entry.path())
                            .to_string_lossy()
                            .replace('\\', "/");

                        let is_excluded = excluded.contains(&dir_name);
                        let children = if !is_excluded {
                            build_tree(&entry.path(), current_level + 1, max_depth, excluded, root)
                        } else {
                            Vec::new()
                        };

                        nodes.push(DirectoryNode {
                            name: dir_name,
                            path: relative_path,
                            is_excluded,
                            children,
                            level: current_level,
                        });
                    }
                }
            }
        }
        nodes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        nodes
    }

    let root = Path::new(&root_path);
    if !root.is_dir() {
        return Err(format!("Path is not a directory: {}", root_path));
    }

    let excluded_set: HashSet<String> = excluded_paths.into_iter().collect();
    Ok(build_tree(root, 0, max_depth, &excluded_set, root))
}

// ============================================================================
// Context File Generation Models
// ============================================================================

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct VirtualFilePayload {
    name: String,
    content: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GenerateContextFilePayload {
    projects: Vec<ProjectFilesRequest>,
    environment_id: i64,
    enabled_prompt_ids: Vec<i64>,
    quick_task: Option<String>,
    include_structure: bool,
    include_logs: bool,
    logs: Option<String>,
    protocol_instructions: Option<String>,
    context_architect_prompt: Option<String>,
    #[serde(default)]
    virtual_files: Vec<VirtualFilePayload>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ContextFileMetadata {
    filepath: String,
    filename: String,
    size: u64,
    file_count: usize,
    project_count: usize,
    warning: Option<String>,
    content: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ContextHistoryItem {
    filename: String,
    filepath: String,
    timestamp: String,
    config: ContextConfig,
    size: u64,
    file_count: usize,
    project_count: usize,
    #[serde(default)]
    favorite: bool,
}

// ============================================================================
// Context Favorites Management
// ============================================================================

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct ContextFavorites {
    #[serde(default)]
    favorites: HashMap<String, bool>,
}

impl ContextFavorites {
    async fn load(app_handle: &AppHandle) -> Result<Self, String> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?;
        let favorites_path = app_data_dir.join("context_favorites.json");

        if favorites_path.exists() {
            let content = fs::read_to_string(&favorites_path)
                .await
                .map_err(|e| e.to_string())?;
            serde_json::from_str(&content).map_err(|e| e.to_string())
        } else {
            Ok(Self::default())
        }
    }

    async fn save(&self, app_handle: &AppHandle) -> Result<(), String> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?;
        let favorites_path = app_data_dir.join("context_favorites.json");

        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&favorites_path, json)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn is_favorite(&self, filepath: &str) -> bool {
        self.favorites.get(filepath).copied().unwrap_or(false)
    }

    fn set_favorite(&mut self, filepath: String, is_favorite: bool) {
        if is_favorite {
            self.favorites.insert(filepath, true);
        } else {
            self.favorites.remove(&filepath);
        }
    }
}

// ============================================================================
// Other Payloads for Tauri Commands
// ============================================================================
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateEnvironmentPayload {
    name: String,
    icon: String,
    system_prompt_content: String,
    env_prompt_content: String,
    language: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateEnvironmentPayload {
    id: i64,
    name: String,
    icon: String,
    system_prompt_content: String,
    env_prompt_content: String,
    language: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePromptPayload {
    environment_id: i64,
    name: String,
    content: Option<String>,
    category: String,
    enabled_by_default: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePromptPayload {
    id: i64,
    name: String,
    content: Option<String>,
    category: String,
    sort_order: i64,
}

// ============================================================================
// License Verification Structures
// ============================================================================

#[derive(Serialize)]
struct LicenseVerificationRequest<'a> {
    license_key: &'a str,
    device_id: &'a str,
}

#[derive(Serialize)]
struct LicenseHeartbeatRequest<'a> {
    license_key: &'a str,
    device_id: &'a str,
}

#[derive(Serialize)]
struct LicenseDeactivateRequest<'a> {
    license_key: &'a str,
    device_id: &'a str,
}

#[derive(Deserialize, Debug)]
struct LicenseVerificationResponse {
    valid: bool,
    message: Option<String>,
    already_active: Option<bool>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct VerificationResult {
    success: bool,
    message: String,
    status: String,
}


// ============================================================================
// Utility Functions
// ============================================================================

fn sqlx_err(e: sqlx::Error) -> String {
    e.to_string()
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ToggleFavoritePayload {
    filepath: String,
    favorite: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RenameFilePayload {
    filepath: String,
    new_name: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GetContextHistoryPayload {
    #[serde(default)]
    selected_projects: Vec<String>,
}

// Nowa struktura dla odpowiedzi statusu
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct StatusPayload {
    status: String,
    theme_name: String,
    theme_80s_color1: String,
    theme_80s_color2: String,
}
// ============================================================================
// Tauri Commands (CRUD & Others)
// ============================================================================

#[tauri::command]
async fn get_setting(
    key: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Option<String>, String> {
    let value: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool.inner())
        .await
        .map_err(sqlx_err)?;

    Ok(value.map(|(v,)| v))
}

#[tauri::command]
async fn set_setting(
    key: String,
    value: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool.inner())
    .await
    .map_err(sqlx_err)?;
    Ok(())
}

#[tauri::command]
async fn get_license_status(_pool: tauri::State<'_, SqlitePool>) -> Result<String, String> {
    Ok("VALID".to_string())
}

// Funkcja pomocnicza do generowania lub pobierania device_id
async fn get_or_create_device_id(pool: &SqlitePool) -> Result<String, String> {
    // Sprawdź czy device_id już istnieje
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'device_id'")
            .fetch_optional(pool)
            .await
            .map_err(sqlx_err)?;

    if let Some((device_id,)) = existing {
        return Ok(device_id);
    }

    // Generuj nowy UUID
    use uuid::Uuid;
    let new_device_id = Uuid::new_v4().to_string();

    // Zapisz w bazie
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('device_id', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&new_device_id)
    .execute(pool)
    .await
    .map_err(sqlx_err)?;

    println!("[LICENSE] Generated new device_id: {}", new_device_id);
    Ok(new_device_id)
}

#[tauri::command]
async fn verify_and_save_license_key(
    key: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<VerificationResult, String> {
    println!("[LICENSE] 1/7: Rozpoczynam weryfikację klucza: {}", key);

    // Pobierz lub wygeneruj device_id
    let device_id = get_or_create_device_id(pool.inner()).await?;
    println!("[LICENSE] 2/7: Device ID: {}", device_id);

    let api_url = "https://api.ai-gluon.com/verify-license";
    let client = reqwest::Client::new();
    println!("[LICENSE] 3/7: Wysyłam zapytanie do {}", api_url);

    let response = client
        .post(api_url)
        .json(&LicenseVerificationRequest {
            license_key: &key,
            device_id: &device_id,
        })
        .send()
        .await
        .map_err(|e| {
            println!("[LICENSE] BŁĄD SIECI: {}", e);
            format!("Network request failed: {}", e)
        })?;

    println!(
        "[LICENSE] 4/7: Otrzymałem odpowiedź ze statusem: {}",
        response.status()
    );
    let response_text = response.text().await.map_err(|e| {
        println!("[LICENSE] BŁĄD ODCZYTU CIAŁA ODPOWIEDZI: {}", e);
        format!("Failed to read response body: {}", e)
    })?;
    println!("[LICENSE] 5/7: Surowa odpowiedź z API: {}", response_text);

    let api_response: LicenseVerificationResponse =
        serde_json::from_str(&response_text).map_err(|e| {
            println!("[LICENSE] BŁĄD PARSOWANIA JSON: {}", e);
            format!("Failed to parse API response: {}", e)
        })?;

    // Sprawdź czy klucz jest już aktywny na innym urządzeniu
    if api_response.already_active.unwrap_or(false) {
        println!("[LICENSE] ⚠️ Klucz jest już aktywny na innym urządzeniu!");
        return Ok(VerificationResult {
            success: false,
            message: "This license key is already active on another device. Please deactivate it there first.".to_string(),
            status: "INVALID".to_string(),
        });
    }

    let (status, message) = if api_response.valid {
        (
            "VALID",
            api_response
                .message
                .unwrap_or_else(|| "License key is valid.".to_string()),
        )
    } else {
        (
            "INVALID",
            api_response
                .message
                .unwrap_or_else(|| "Invalid license key.".to_string()),
        )
    };
    println!(
        "[LICENSE] 6/7: Ustawiam status na: {}. Wiadomość: {}",
        status, message
    );

    // Zapisz klucz i status w bazie danych (bez zmian)
    let mut tx = pool.begin().await.map_err(sqlx_err)?;
    sqlx::query("INSERT INTO settings (key, value) VALUES ('license_key', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(&key).execute(&mut *tx).await.map_err(sqlx_err)?;
    sqlx::query("INSERT INTO settings (key, value) VALUES ('license_status', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(status).execute(&mut *tx).await.map_err(sqlx_err)?;
    tx.commit().await.map_err(sqlx_err)?;

    let result = VerificationResult {
        success: api_response.valid,
        message,
        status: status.to_string(),
    };

    println!("[LICENSE] 7/7: Zwracam do frontendu: {:?}", result);
    Ok(result)
}

async fn send_license_heartbeat(_pool: &SqlitePool) -> Result<(), String> {
    Ok(())
}

// Funkcja deaktywująca licencję przy zamknięciu aplikacji
#[tauri::command]
async fn deactivate_license(pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    // Pobierz klucz licencji
    let license_key: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'license_key'")
            .fetch_optional(pool.inner())
            .await
            .map_err(sqlx_err)?;

    let license_key = match license_key {
        Some((key,)) => key,
        None => return Ok(()), // Brak klucza - nic nie rób
    };

    // Pobierz device_id
    let device_id = get_or_create_device_id(pool.inner()).await?;

    println!("[LICENSE] Deactivating license for device: {}", device_id);

    // Wyślij żądanie deaktywacji do API
    let api_url = "https://api.ai-gluon.com/license-deactivate";
    let client = reqwest::Client::new();

    match client
        .post(api_url)
        .json(&LicenseDeactivateRequest {
            license_key: &license_key,
            device_id: &device_id,
        })
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                println!("[LICENSE] ✓ License deactivated successfully");
            } else {
                println!(
                    "[LICENSE] ⚠️ Deactivation failed with status: {}",
                    response.status()
                );
            }
        }
        Err(e) => {
            println!("[LICENSE] ⚠️ Deactivation network error: {}", e);
        }
    }

    Ok(())
}

#[tauri::command]
async fn get_projects(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<ProjectWithEnv>, String> {
    let projects = sqlx::query_as::<_, ProjectWithEnv>(
        "SELECT p.id, p.path, p.excluded_paths, p.download_path, p.allowed_extensions, p.vector_map_id, pe.environment_id
         FROM projects p
         LEFT JOIN project_environment pe ON p.id = pe.project_id"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(sqlx_err)?;

    // LOG: Sprawdź, co zostało odczytane z bazy danych
    // println!("[RUST LOG] Fetched projects data: {:?}", projects);

    Ok(projects)
}

#[tauri::command]
async fn add_project(path: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    // Krok 1: Pobierz aktualną listę domyślnych wykluczeń (ta sama logika co w `get_default_exclusions`)
    let default_exclusions_json: String =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'default_exclusions'")
            .fetch_optional(pool.inner())
            .await
            .map_err(sqlx_err)?
            .unwrap_or_else(|| {
                r#"[
        "node_modules", "vendor", "bower_components",
        "build", "dist", "out", "target", "bin", "obj",
        ".next", ".nuxt", ".svelte-kit", ".vercel",
        ".venv", "venv", "env", ".env", "__pycache__",
        ".cache", ".pytest_cache", ".mypy_cache", ".gradle",
        ".git", ".svn", ".hg",
        ".idea", ".vscode",
        "*.swp", "*.swo",
        ".DS_Store", "Thumbs.db",
        "logs", "*.log", "npm-debug.log*", "yarn-error.log"
    ]"#
                .to_string()
            });

    // Krok 2: Przygotuj domyślną listę dozwolonych rozszerzeń
    let all_extensions_json =
        serde_json::to_string(ALLOWED_EXTENSIONS).map_err(|e| e.to_string())?;

    // Krok 3: Zaktualizuj zapytanie INSERT, aby uwzględniało `excluded_paths`
    sqlx::query("INSERT INTO projects (path, download_path, allowed_extensions, excluded_paths) VALUES (?, ?, ?, ?)")
        .bind(&path)
        .bind(&path) // Domyślna ścieżka pobierania to ścieżka projektu
        .bind(&all_extensions_json)
        .bind(&default_exclusions_json) // Dodaj pobrane domyślne wykluczenia
        .execute(pool.inner())
        .await
        .map_err(sqlx_err)?;
    Ok(())
}

#[tauri::command]
async fn remove_project(
    path: String,
    pool: tauri::State<'_, SqlitePool>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM projects WHERE path = ?")
        .bind(&path)
        .execute(pool.inner())
        .await
        .map_err(sqlx_err)?;

    let mut cache = state.file_tree_cache.lock().await;
    cache.remove(&path);
    println!("Removed project from cache: {}", path);

    Ok(())
}

#[tauri::command]
async fn update_project_settings(
    path: String,
    excluded_paths: Vec<String>,
    allowed_extensions: Vec<String>,
    pool: tauri::State<'_, SqlitePool>,
    state: tauri::State<'_, AppState>,
) -> Result<ProjectWithEnv, String> {
    println!(
        "[RUST LOG] Received update_project_settings for path: {}",
        path
    );
    println!("[RUST LOG] -> Exclusions: {:?}", excluded_paths);
    println!("[RUST LOG] -> Extensions: {:?}", allowed_extensions);

    let excluded_json = serde_json::to_string(&excluded_paths).map_err(|e| e.to_string())?;
    let allowed_json = serde_json::to_string(&allowed_extensions).map_err(|e| e.to_string())?;

    sqlx::query("UPDATE projects SET excluded_paths = ?, allowed_extensions = ? WHERE path = ?")
        .bind(&excluded_json)
        .bind(&allowed_json)
        .bind(&path)
        .execute(pool.inner())
        .await
        .map_err(sqlx_err)?;

    println!("[RUST LOG] Database update successful. Fetching updated project...");

    let updated_project = sqlx::query_as::<_, ProjectWithEnv>(
        "SELECT p.id, p.path, p.excluded_paths, p.download_path, p.allowed_extensions, p.vector_map_id, pe.environment_id
         FROM projects p
         LEFT JOIN project_environment pe ON p.id = pe.project_id
         WHERE p.path = ?"
    )
    .bind(&path)
    .fetch_one(pool.inner())
    .await
    .map_err(sqlx_err)?;

    println!("[RUST LOG] Returning updated project data to frontend.");

    let mut cache = state.file_tree_cache.lock().await;
    cache.remove(&path);
    println!("[RUST LOG] Invalidated cache for project: {}", path);

    Ok(updated_project)
}

#[tauri::command]
async fn set_project_download_path(
    project_id: i64,
    download_path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE projects SET download_path = ? WHERE id = ?")
        .bind(&download_path)
        .bind(project_id)
        .execute(pool.inner())
        .await
        .map_err(sqlx_err)?;
    Ok(())
}

#[tauri::command]
async fn get_environments(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Environment>, String> {
    sqlx::query_as::<_, Environment>(
        "SELECT id, name, icon, created_at, is_default, language FROM environments ORDER BY id ASC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(sqlx_err)
}

#[tauri::command]
async fn create_environment(
    payload: CreateEnvironmentPayload,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Environment, String> {
    let mut tx = pool.begin().await.map_err(sqlx_err)?;

    let result = sqlx::query("INSERT INTO environments (name, icon, language) VALUES (?, ?, ?)")
        .bind(&payload.name)
        .bind(&payload.icon)
        .bind(&payload.language)
        .execute(&mut *tx)
        .await
        .map_err(sqlx_err)?;
    let new_env_id = result.last_insert_rowid();

    sqlx::query("INSERT INTO prompts (environment_id, name, content, category, sort_order) VALUES (?, 'System Prompt', ?, 'system', 0)")
        .bind(new_env_id)
        .bind(&payload.system_prompt_content)
        .execute(&mut *tx)
        .await.map_err(sqlx_err)?;

    sqlx::query("INSERT INTO prompts (environment_id, name, content, category, sort_order) VALUES (?, 'Environment Context', ?, 'environment', 1)")
        .bind(new_env_id)
        .bind(&payload.env_prompt_content)
        .execute(&mut *tx)
        .await.map_err(sqlx_err)?;

    tx.commit().await.map_err(sqlx_err)?;

    sqlx::query_as::<_, Environment>("SELECT * FROM environments WHERE id = ?")
        .bind(new_env_id)
        .fetch_one(pool.inner())
        .await
        .map_err(sqlx_err)
}

#[tauri::command]
async fn update_environment(
    payload: UpdateEnvironmentPayload,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(sqlx_err)?;

    sqlx::query("UPDATE environments SET name = ?, icon = ?, language = ? WHERE id = ?")
        .bind(&payload.name)
        .bind(&payload.icon)
        .bind(&payload.language)
        .bind(payload.id)
        .execute(&mut *tx)
        .await
        .map_err(sqlx_err)?;

    sqlx::query("UPDATE prompts SET content = ? WHERE environment_id = ? AND category = 'system'")
        .bind(&payload.system_prompt_content)
        .bind(payload.id)
        .execute(&mut *tx)
        .await
        .map_err(sqlx_err)?;

    sqlx::query(
        "UPDATE prompts SET content = ? WHERE environment_id = ? AND category = 'environment'",
    )
    .bind(&payload.env_prompt_content)
    .bind(payload.id)
    .execute(&mut *tx)
    .await
    .map_err(sqlx_err)?;

    tx.commit().await.map_err(sqlx_err)
}

#[tauri::command]
async fn delete_environment(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    if id == 1 {
        return Err("The default environment (ID=1) cannot be deleted.".to_string());
    }
    sqlx::query("DELETE FROM environments WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(sqlx_err)?;
    Ok(())
}

#[tauri::command]
async fn get_prompts(
    env_id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Prompt>, String> {
    sqlx::query_as::<_, Prompt>(
        "SELECT * FROM prompts WHERE environment_id = ? ORDER BY sort_order ASC",
    )
    .bind(env_id)
    .fetch_all(pool.inner())
    .await
    .map_err(sqlx_err)
}

#[tauri::command]
async fn create_prompt(
    payload: CreatePromptPayload,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Prompt, String> {
    let new_prompt_id: i64 = sqlx::query(
        "INSERT INTO prompts (environment_id, name, content, category) VALUES (?, ?, ?, ?)",
    )
    .bind(payload.environment_id)
    .bind(payload.name)
    .bind(payload.content)
    .bind(payload.category)
    .bind(payload.enabled_by_default)
    .execute(pool.inner())
    .await
    .map_err(sqlx_err)?
    .last_insert_rowid();

    sqlx::query_as::<_, Prompt>("SELECT * FROM prompts WHERE id = ?")
        .bind(new_prompt_id)
        .fetch_one(pool.inner())
        .await
        .map_err(sqlx_err)
}

#[tauri::command]
async fn update_prompt(
    payload: UpdatePromptPayload,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE prompts SET name = ?, content = ?, category = ?, sort_order = ? WHERE id = ?",
    )
    .bind(payload.name)
    .bind(payload.content)
    .bind(payload.category)
    .bind(payload.sort_order)
    .bind(payload.id)
    .execute(pool.inner())
    .await
    .map_err(sqlx_err)?;
    Ok(())
}

#[tauri::command]
async fn toggle_prompt(
    id: i64,
    enabled: bool,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE prompts SET enabled_by_default = ? WHERE id = ?")
        .bind(enabled)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(sqlx_err)?;
    Ok(())
}

#[tauri::command]
async fn delete_prompt(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM prompts WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(sqlx_err)?;
    Ok(())
}

#[tauri::command]
async fn assign_project_environment(
    project_id: i64,
    env_id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO project_environment (project_id, environment_id) VALUES (?, ?)
         ON CONFLICT(project_id) DO UPDATE SET environment_id = excluded.environment_id",
    )
    .bind(project_id)
    .bind(env_id)
    .execute(pool.inner())
    .await
    .map_err(sqlx_err)?;
    Ok(())
}

#[tauri::command]
async fn get_extension_templates(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<ExtensionTemplate>, String> {
    sqlx::query_as::<_, ExtensionTemplate>(
        "SELECT id, name, extensions FROM extension_templates ORDER BY name ASC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(sqlx_err)
}

#[tauri::command]
async fn create_extension_template(
    name: String,
    extensions: Vec<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<ExtensionTemplate, String> {
    let extensions_json = serde_json::to_string(&extensions).map_err(|e| e.to_string())?;
    let new_id = sqlx::query("INSERT INTO extension_templates (name, extensions) VALUES (?, ?)")
        .bind(&name)
        .bind(&extensions_json)
        .execute(pool.inner())
        .await
        .map_err(sqlx_err)?
        .last_insert_rowid();

    sqlx::query_as::<_, ExtensionTemplate>("SELECT * FROM extension_templates WHERE id = ?")
        .bind(new_id)
        .fetch_one(pool.inner())
        .await
        .map_err(sqlx_err)
}

#[tauri::command]
async fn delete_extension_template(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM extension_templates WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(sqlx_err)?;
    Ok(())
}

#[tauri::command]
async fn run_smart_context_task(
    task: String,
    state: tauri::State<'_, AppState>
) -> Result<String, String> {
    println!("[Smart Context] 🟢 Processing Task: '{}'", task);
    
    if !state.local_ai.is_running() {
        println!("[Smart Context] ❌ Local AI not running");
        return Err("Local AI services not running. Toggle them ON in header.".to_string());
    }

    // 1. RAG Search (Pobierz 5 najbardziej relewantnych chunków - zmniejszamy z 8, by oszczędzić tokeny)
    // Note: For now, we search in the default vector map (id=1)
    // TODO: Add project_path parameter to allow searching in project-specific maps

    // First, generate embedding for the query
    use reqwest::Client;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .no_proxy()
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let embed_response = client.post("http://127.0.0.1:8081/v1/embeddings")
        .json(&serde_json::json!({
            "input": format!("search_query: {}", task),
            "model": "nomic-embed-text-v2-moe.Q8_0.gguf"
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to connect to embedding service: {}", e))?;

    let embed_json: serde_json::Value = embed_response.json().await
        .map_err(|e| format!("Failed to parse embedding response: {}", e))?;

    let query_embedding: Vec<f32> = embed_json["data"][0]["embedding"]
        .as_array()
        .ok_or("Invalid embedding format")?
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect();

    // Search in default vector map
    let search_results = state.vector_map_manager
        .search(1, query_embedding, 5)
        .await
        .map_err(|e| format!("RAG Search failed: {}", e))?;

    let chunks: Vec<String> = search_results.into_iter()
        .map(|(key, _score, content)| {
            // Format as before: "// File: ... \n Content"
            let parts: Vec<&str> = key.split("::").collect();
            let file_path = parts[0];
            format!("// File: {}\n{}", file_path, content)
        })
        .collect();

    if chunks.is_empty() {
        println!("[Smart Context] ⚠️ Index is empty or no matches! Did you run 'trigger_indexing'?");
        return Err("No relevant code found in index. Please run 'Index Project' first.".to_string());
    }

    println!("[Smart Context] ✅ RAG Search found {} relevant chunks", chunks.len());

    // [SAFETY] Limit długości kontekstu.
    // DLA GPU 8GB (R9 390): Kontekst modelu to zazwyczaj 4096 tokenów.
    // 1 token ~= 3-4 znaki. Bezpieczny limit to ok. 3000 tokenów na input (~10-11k znaków),
    // pozostawiając 1k tokenów na odpowiedź.
    let mut context_block = String::new();
    const MAX_CONTEXT_CHARS: usize = 11_000; // Zmniejszono z 20_000 aby uniknąć OOM/Crash

    for chunk in chunks {
        if context_block.len() + chunk.len() > MAX_CONTEXT_CHARS {
            println!("[Smart Context] ⚠️ Context limit reached ({} chars), skipping remaining chunks to prevent overflow.", MAX_CONTEXT_CHARS);
            break;
        }
        context_block.push_str(&chunk);
        context_block.push_str("\n\n");
    }

    println!("[Smart Context] 🧠 Sending context to Qwen (Length: {} chars)...", context_block.len());

    // 2. Qwen Refinement (System Prompt)
    let system_prompt = "You are an expert AI Developer. 
Your goal is to create a CONCISE Context File for a coding task.
Rules:
1. Analyze the provided code chunks against the User Task.
2. Select ONLY the functions, classes, or definitions strictly necessary to understand or solve the task.
3. OUTPUT COMPLETE CODE BLOCKS. Do NOT remove lines inside a function or class. Do NOT use '// ... rest of code'.
4. If a chunk is irrelevant, ignore it completely.
5. Precede each block with '// File: <path>' comment.
6. Do not add conversational text. Output code only.";

    let user_prompt = format!("User Task: {}\n\nAvailable Code Candidates:\n{}", task, context_block);

    // Wywołanie Qwena (HTTP Request do lokalnego serwera - KoboldCPP)
    // [FIX] Ustawiamy timeout na 180s, ponieważ na starszych GPU (R9 390) generowanie długiego kontekstu
    // może trwać powyżej 60s (u Ciebie ~75s). Brak timeoutu może powodować zerwanie połączenia.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    println!("[Smart Context] ⏳ Sending request to KoboldCPP (Timeout: 180s)...");

    // [FIX] Używamy natywnego endpointu KoboldCPP /api/v1/generate
    let response = client.post("http://127.0.0.1:8082/api/v1/generate")
        .json(&serde_json::json!({
            "prompt": format!("<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", system_prompt, user_prompt),
            "max_length": 2048,      // Kobold: max_length zamiast n_predict
            "temperature": 0.2,
            "stop_sequence": ["<|im_end|>"], // Kobold: stop_sequence
            "rep_pen": 1.1
        }))
        .send()
        .await
        .map_err(|e| {
            format!("Failed to connect to Chat AI (Port 8082). Error: {}", e)
        })?;

    // [FIX] Obsługa błędów HTTP i parsowania (debugging)
    let status = response.status();
    let response_text = response.text().await.map_err(|e| e.to_string())?;

    if !status.is_success() {
        println!("[Smart Context] ❌ Server returned error {}: {}", status, response_text);
        return Err(format!("AI Server Error ({}): {}", status, response_text));
    }

    // Dopiero teraz parsujemy JSON
    let response_json: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("Invalid JSON from AI: {}. Response start: {:.100}", e, response_text))?;

    // [FIX] Bezpieczne parsowanie odpowiedzi KoboldCPP: { "results": [ { "text": "..." } ] }
    // Używamy bezpiecznego dostępu, aby uniknąć paniki backendu.
    let result = if let Some(results) = response_json.get("results") {
        if let Some(first) = results.get(0) {
            first["text"].as_str().unwrap_or("// AI generation returned non-string").to_string()
        } else {
            "// AI returned empty results".to_string()
        }
    } else {
        // Fallback dla innych formatów (np. jeśli zmienisz backend w przyszłości)
        if let Some(content) = response_json.get("content") {
            content.as_str().unwrap_or("// AI content was not a string").to_string()
        } else {
            format!("// Unknown JSON format. Keys: {:?}", response_json.as_object().map(|o| o.keys().collect::<Vec<_>>()))
        }
    };

    println!("[Smart Context] ✅ Generation complete ({} chars).", result.len());
    Ok(result)
}

// ============================================================================
// Agentic RAG Command
// ============================================================================

#[tauri::command]
async fn run_agentic_context_task(
    task: String,
    max_steps: Option<usize>,
    repo_map_detail: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<serde_json::Value, String> {
    println!("[Agentic RAG] 🟢 Starting task: '{}'", task);

    if !state.local_ai.is_running() {
        println!("[Agentic RAG] ❌ Local AI not running");
        return Err("Local AI services not running. Toggle them ON in header.".to_string());
    }

    // Get project roots from database
    let projects: Vec<String> = sqlx::query_scalar("SELECT path FROM projects")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    if projects.is_empty() {
        return Err("No projects configured. Please add a project first.".to_string());
    }

    // TODO: Agentic RAG needs to be updated to work with per-project vector maps
    // For now, load the default vector map (id=1) into a temporary VectorStore
    use gluon_desktop_lib::local_ai::rag_engine::VectorStore;
    let vector_store = state.vector_map_manager.load_map(1).await
        .map_err(|e| format!("Failed to load vector map: {}", e))?;

    // Run agent loop
    let (final_context, task_name) = gluon_desktop_lib::local_ai::agentic_rag::run_agentic_context_task(
        task.clone(),
        max_steps,
        repo_map_detail,
        &vector_store,
        &app_handle,
        projects,
    )
    .await?;

    println!("[Agentic RAG] ✅ Task completed successfully");

    Ok(serde_json::json!({
        "content": final_context,
        "task_name": task_name,
    }))
}

// Struktura dla wybranych plików (używana w trigger_indexing)
#[derive(Clone, serde::Deserialize)]
struct SelectedFileInfo {
    #[serde(rename = "rootPath")]
    root_path: String,
    #[serde(rename = "relativePaths")]
    relative_paths: Vec<String>,
    /// Map: filePath -> Vec<symbolName>
    /// Optional field for selecting specific symbols from files
    #[serde(default)]
    symbols: std::collections::HashMap<String, Vec<String>>,
}

#[tauri::command]
async fn trigger_indexing(
    target_paths: Vec<String>,
    selected_files: Vec<SelectedFileInfo>,
    pool: tauri::State<'_, SqlitePool>,
    _state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle
) -> Result<String, String> {
    println!("[Indexing] 🚀 Request received. Starting background task...");

    let all_projects = get_projects(pool.clone()).await?;

    // Klonujemy dane potrzebne w wątku
    let projects_to_process: Vec<_> = if !selected_files.is_empty() {
        // NOWY TRYB: Filtruj projekty na podstawie selectedFiles
        all_projects.into_iter()
            .filter(|p| selected_files.iter().any(|sf| sf.root_path == p.path))
            .collect()
    } else if target_paths.is_empty() {
        // STARY TRYB: Wszystkie projekty
        all_projects
    } else {
        // STARY TRYB: Filtruj po target_paths
        all_projects.into_iter()
            .filter(|p| target_paths.contains(&p.path))
            .collect()
    };

    if projects_to_process.is_empty() {
        return Err("No matching projects found to index.".to_string());
    }

    // Uruchomienie w tle (Background Task)
    tokio::spawn(async move {
        println!("[Indexing Task] Started.");
        let benchmark_start = std::time::Instant::now();
        let mut total_files_processed = 0;
        let mut skipped = 0;
        let mut total_chunks_processed = 0;
        let mut total_bytes_processed = 0;
        let mut file_timings: Vec<f32> = Vec::new();
        let mut total_files_skipped_unchanged = 0;

        // Helper do sprawdzania czy RAG jest dalej włączony i nie został anulowany
        let is_rag_running = || -> bool {
            let state = app_handle.state::<AppState>();
            let cancelled = state.indexing_cancelled.load(std::sync::atomic::Ordering::Relaxed);
            state.local_ai.is_running() && !cancelled
        };

        for proj in projects_to_process {
            // Sprawdź czy RAG nie został wyłączony
            if !is_rag_running() {
                println!("[Indexing Task] ⚠️ RAG service stopped - aborting indexing.");
                return;
            }
            // Emisja eventu: Skanowanie struktury
            let _ = app_handle.emit("indexing_progress", serde_json::json!({
                "status": "scanning",
                "project": proj.path
            }));

            // Get project's vector_map_id
            let pool = app_handle.state::<SqlitePool>();
            let vector_map_id: i64 = sqlx::query_scalar(
                "SELECT COALESCE(vector_map_id, 1) FROM projects WHERE path = ?"
            )
            .bind(&proj.path)
            .fetch_one(pool.inner())
            .await
            .unwrap_or(1);

            println!("[Indexing Task] Project '{}' using vector map {}", proj.path, vector_map_id);

            // Określ listę kandydatów do indeksowania
            let candidate_files: Vec<String> = if !selected_files.is_empty() {
                // NOWY TRYB: Użyj tylko wybranych plików
                selected_files.iter()
                    .find(|sf| sf.root_path == proj.path)
                    .map(|sf| sf.relative_paths.clone())
                    .unwrap_or_default()
            } else {
                // STARY TRYB: Skanuj wszystkie pliki z wykluczeniami
                let excludes: Vec<String> = proj.excluded_paths.clone()
                    .and_then(|s| serde_json::from_str(s.as_str()).ok())
                    .unwrap_or_default();

                let extensions: Vec<String> = proj.allowed_extensions.clone()
                    .and_then(|s| serde_json::from_str(s.as_str()).ok())
                    .unwrap_or_else(|| ALLOWED_EXTENSIONS.iter().map(|s| s.to_string()).collect());

                println!("[Indexing Task] Scanning {} with {} allowed extensions...", proj.path, extensions.len());

                let files_result = scan_project_files_internal(&proj.path, &excludes, &extensions).await;

                if let Ok(files) = files_result {
                    files.iter().map(|f| f.path.clone()).collect()
                } else {
                    Vec::new()
                }
            };

            let candidate_count = candidate_files.len();
            println!("[Indexing Task] Found {} candidate files", candidate_count);

            // INCREMENTAL INDEXING: Filter to only files that changed
            let state = app_handle.state::<AppState>();
            let files_to_index = state.vector_map_manager
                .get_files_to_reindex(vector_map_id, &proj.path, candidate_files.clone())
                .await
                .unwrap_or_else(|e| {
                    println!("[Indexing Task] ⚠️ Failed to check file changes: {}. Indexing all files.", e);
                    // Fallback: index all candidates if check fails
                    candidate_files
                });

            let total_in_project = files_to_index.len();
            let total_skipped = candidate_count - files_to_index.len();
            total_files_skipped_unchanged += total_skipped;

            println!("[Indexing Task] Indexing {} files ({} unchanged, skipped)", total_in_project, total_skipped);

            for (idx, relative_path) in files_to_index.iter().enumerate() {
                // Sprawdź czy RAG nie został wyłączony przed każdym plikiem
                if !is_rag_running() {
                    println!("[Indexing Task] ⚠️ RAG service stopped during indexing - aborting.");
                    return;
                }

                let full_path = Path::new(&proj.path).join(relative_path);
                let full_path_str = full_path.to_string_lossy().to_string();

                // Log rozpoczęcia indeksowania pliku
                println!("[Indexing Task] 📄 [{}/{}] Starting: {}", idx + 1, total_in_project, relative_path);

                // Emisja eventu: Postęp pliku
                let _ = app_handle.emit("indexing_progress", serde_json::json!({
                    "status": "indexing",
                    "current_file": relative_path,
                    "current": idx + 1,
                    "total": total_in_project,
                    "project": proj.path
                }));

                if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                    let file_size = content.len();
                    let lines_count = content.lines().count();

                    println!("[Indexing Task]   📊 File stats: {} bytes, {} lines", file_size, lines_count);

                    let state = app_handle.state::<AppState>();

                    // Delete old chunks for this file before re-indexing
                    if let Err(e) = state.vector_map_manager.delete_file_chunks(vector_map_id, &full_path_str).await {
                        println!("[Indexing Task]   ⚠️ Failed to delete old chunks: {}", e);
                    }

                    // Create a temporary VectorStore for indexing this file
                    // (we need VectorStore's index_file method which calls embedding API)
                    use gluon_desktop_lib::local_ai::rag_engine::VectorStore;
                    let mut temp_store = VectorStore::new();

                    // Index file with timeout handled in rag_engine
                    let start_time = std::time::Instant::now();
                    match temp_store.index_file(full_path_str.clone(), content).await {
                        Ok(_) => {
                            let elapsed = start_time.elapsed();
                            let elapsed_secs = elapsed.as_secs_f32();

                            // Save all chunks to database via VectorMapManager
                            let all_chunks = temp_store.get_all_chunks();
                            let chunks_in_file = all_chunks.len();

                            for (chunk_key, embedding, chunk_content) in all_chunks {
                                // Parse chunk_key to get file_path and start_line
                                let parts: Vec<&str> = chunk_key.split("::").collect();
                                let chunk_file_path = parts.get(0).unwrap_or(&full_path_str.as_str()).to_string();
                                let start_line: i64 = parts.get(1)
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(0);

                                // Save chunk to database
                                if let Err(e) = state.vector_map_manager.save_chunk(
                                    vector_map_id,
                                    chunk_key,
                                    embedding,
                                    chunk_content,
                                    chunk_file_path,
                                    start_line
                                ).await {
                                    println!("[Indexing Task]   ⚠️ Failed to save chunk: {}", e);
                                }
                            }

                            total_files_processed += 1;
                            total_bytes_processed += file_size;
                            total_chunks_processed += chunks_in_file;
                            file_timings.push(elapsed_secs);

                            // Get file mtime for indexed_files table
                            let file_mtime = std::fs::metadata(&full_path)
                                .and_then(|m| m.modified())
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs() as i64)
                                .unwrap_or(0);

                            // Update indexed_files record
                            if let Err(e) = state.vector_map_manager.update_file_index_record(
                                vector_map_id,
                                &full_path_str,
                                file_mtime,
                                chunks_in_file as i64,
                                file_size as i64
                            ).await {
                                println!("[Indexing Task]   ⚠️ Failed to update file index record: {}", e);
                            }

                            println!("[Indexing Task]   ✅ Indexed successfully in {:.2}s ({} chunks)", elapsed_secs, chunks_in_file);

                            // Emit success event
                            let _ = app_handle.emit("indexing_progress", serde_json::json!({
                                "status": "file_complete",
                                "file": relative_path,
                                "current": idx + 1,
                                "total": total_in_project,
                                "elapsed_ms": elapsed.as_millis(),
                                "success": true
                            }));
                        },
                        Err(e) => {
                            println!("[Indexing Task]   ❌ Error indexing: {}", e);
                            skipped += 1;

                            // Emit error event
                            let _ = app_handle.emit("indexing_progress", serde_json::json!({
                                "status": "file_complete",
                                "file": relative_path,
                                "current": idx + 1,
                                "total": total_in_project,
                                "success": false,
                                "error": e
                            }));
                        }
                    }
                } else {
                    println!("[Indexing Task]   ⚠️  Failed to read file");
                    skipped += 1;
                }

                // Co 10 plików sprawdź anulowanie i zaktualizuj metadata mapy
                if idx > 0 && idx % 10 == 0 {
                    // Check cancellation again
                    if !is_rag_running() {
                        println!("[Indexing Task] ⚠️ RAG service stopped during indexing - aborting.");

                        // Update map metadata before exiting
                        let state = app_handle.state::<AppState>();
                        let _ = state.vector_map_manager.update_map_metadata(vector_map_id).await;
                        return;
                    }

                    // Update map metadata periodically
                    let state = app_handle.state::<AppState>();
                    let _ = state.vector_map_manager.update_map_metadata(vector_map_id).await;
                }
            }
        }

        // Final metadata update for all processed maps
        println!("[Indexing Task] Updating vector map metadata...");
        // Note: We've already updated metadata periodically during indexing

        // Calculate benchmark statistics
        let total_elapsed = benchmark_start.elapsed();
        let total_elapsed_secs = total_elapsed.as_secs_f32();
        let avg_time_per_file = if total_files_processed > 0 {
            file_timings.iter().sum::<f32>() / total_files_processed as f32
        } else {
            0.0
        };
        let avg_time_per_chunk = if total_chunks_processed > 0 {
            total_elapsed_secs / total_chunks_processed as f32
        } else {
            0.0
        };
        let chunks_per_second = if total_elapsed_secs > 0.0 {
            total_chunks_processed as f32 / total_elapsed_secs
        } else {
            0.0
        };

        // Get selected model name from settings
        let pool_for_model = app_handle.state::<SqlitePool>();
        let selected_model: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'embedding_model'")
            .fetch_optional(pool_for_model.inner())
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "nomic-embed-text-v2-moe.Q8_0.gguf".to_string());

        // Print comprehensive benchmark summary
        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║           📊 RAG INDEXING BENCHMARK SUMMARY                    ║");
        println!("╠════════════════════════════════════════════════════════════════╣");
        println!("║ Model: {:<56}║", selected_model);
        println!("╠════════════════════════════════════════════════════════════════╣");
        println!("║ Total Time:             {:<38.2} s ║", total_elapsed_secs);
        println!("║ Files Processed:         {:<38} ║", total_files_processed);
        println!("║ Files Skipped (errors):  {:<38} ║", skipped);
        println!("║ Files Skipped (unchanged): {:<36} ║", total_files_skipped_unchanged);
        println!("║ Total Chunks:            {:<38} ║", total_chunks_processed);
        println!("║ Total Data Processed:    {:<34.2} MB ║", total_bytes_processed as f32 / 1_048_576.0);
        println!("╠════════════════════════════════════════════════════════════════╣");
        println!("║ Avg Time per File:      {:<34.2} s ║", avg_time_per_file);
        println!("║ Avg Time per Chunk:     {:<34.2} s ║", avg_time_per_chunk);
        println!("║ Throughput:              {:<30.2} chunks/s ║", chunks_per_second);
        println!("╚════════════════════════════════════════════════════════════════╝\n");

        // Emisja eventu: Koniec z benchmark stats
        let _ = app_handle.emit("indexing_progress", serde_json::json!({
            "status": "complete",
            "processed": total_files_processed,
            "skipped": skipped,
            "benchmark": {
                "model": selected_model,
                "total_time_secs": total_elapsed_secs,
                "total_chunks": total_chunks_processed,
                "total_bytes": total_bytes_processed,
                "avg_time_per_file": avg_time_per_file,
                "avg_time_per_chunk": avg_time_per_chunk,
                "chunks_per_second": chunks_per_second
            }
        }));
        
        println!("[Indexing Task] Finished. Processed: {}, Skipped: {}", total_files_processed, skipped);
    });

    Ok("Indexing started in background. Check status bar.".to_string())
}

#[tauri::command]
async fn toggle_local_ai(
    enabled: bool,
    skip_auto_index: Option<bool>,
    state: tauri::State<'_, AppState>,
    pool: tauri::State<'_, SqlitePool>,
    app_handle: tauri::AppHandle
) -> Result<bool, String> {
    println!("[RUST LOG] Toggle Local AI request: {} (skip_auto_index: {:?})", enabled, skip_auto_index);

    // Save preference
    sqlx::query("INSERT INTO settings (key, value) VALUES ('local_ai_enabled', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(if enabled { "true" } else { "false" })
        .execute(pool.inner())
        .await
        .map_err(sqlx_err)?;

    if enabled {
        // Reset cancellation flag before starting
        state.indexing_cancelled.store(false, std::sync::atomic::Ordering::Relaxed);

        state.local_ai.start_services(&app_handle, pool.inner()).await?;

        let should_index = !skip_auto_index.unwrap_or(false);

        if should_index {
            println!("[RUST LOG] ✅ RAG Service enabled. Starting automatic indexing...");
            // Spawn auto-indexing task directly (avoid event system to prevent duplicate triggers)
            let app_clone = app_handle.clone();
            tokio::spawn(async move {
                // Wait for RAG services to be fully ready
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                let pool = app_clone.state::<SqlitePool>();
                let state = app_clone.state::<AppState>();
                let app_for_indexing = app_clone.clone();

                println!("[RUST LOG] 🚀 Starting auto-indexing task...");
                match trigger_indexing(
                    vec![],  // Empty = all projects
                    vec![],  // Empty = all files
                    pool,
                    state,
                    app_for_indexing
                ).await {
                    Ok(msg) => println!("[RUST LOG] ✅ Auto-indexing: {}", msg),
                    Err(e) => println!("[RUST LOG] ❌ Auto-indexing failed: {}", e),
                }
            });
        } else {
            println!("[RUST LOG] ✅ RAG Service enabled. Auto-indexing SKIPPED (waiting for specific files).");
        }
    } else {
        // Cancel any running indexing tasks
        println!("[RUST LOG] ⏹️ Cancelling any running indexing tasks...");
        state.indexing_cancelled.store(true, std::sync::atomic::Ordering::Relaxed);

        // Give indexing tasks a moment to see the cancellation flag
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        state.local_ai.stop_services();
        println!("[RUST LOG] ⏹️ RAG Service disabled.");
    }

    Ok(state.local_ai.is_running())
}

#[tauri::command]
async fn get_local_ai_status(
    state: tauri::State<'_, AppState>
) -> Result<bool, String> {
    Ok(state.local_ai.is_running())
}

#[derive(Serialize)]
struct EmbeddingModelInfo {
    filename: String,
    size_mb: f64,
    is_active: bool,
}

#[tauri::command]
async fn list_embedding_models(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>
) -> Result<Vec<EmbeddingModelInfo>, String> {
    // Find models directory
    let models_dir = app_handle.path().resource_dir()
        .ok()
        .and_then(|p| {
            let dir = p.join("models");
            if dir.exists() { Some(dir) } else { None }
        })
        .or_else(|| {
            std::env::current_dir().ok().and_then(|p| {
                let dir = p.join("gluon-desktop").join("src-tauri").join("models");
                if dir.exists() { Some(dir) } else { None }
            })
        })
        .or_else(|| {
            std::env::current_dir().ok().and_then(|p| {
                let dir = p.join("models");
                if dir.exists() { Some(dir) } else { None }
            })
        })
        .ok_or("Could not locate models directory")?;

    // Get current active model from database
    let active_model: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'embedding_model'"
    )
    .fetch_optional(pool.inner())
    .await
    .unwrap_or(None);

    let active_model = active_model.unwrap_or_else(|| "nomic-embed-text-v2-moe.Q8_0.gguf".to_string());

    // Scan for nomic-embed-text models
    let mut models = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&models_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                if filename.starts_with("nomic-embed-text") && filename.ends_with(".gguf") {
                    let size_mb = std::fs::metadata(&path)
                        .map(|m| m.len() as f64 / 1024.0 / 1024.0)
                        .unwrap_or(0.0);

                    models.push(EmbeddingModelInfo {
                        filename: filename.to_string(),
                        size_mb,
                        is_active: filename == active_model,
                    });
                }
            }
        }
    }

    // Sort by filename
    models.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(models)
}

#[tauri::command]
async fn set_embedding_model(
    model_filename: String,
    pool: tauri::State<'_, SqlitePool>,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle
) -> Result<String, String> {
    println!("[RAG Model Switch] Changing to: {}", model_filename);

    // Save to database
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('embedding_model', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value"
    )
    .bind(&model_filename)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("Failed to save model preference: {}", e))?;

    // If AI is currently running, restart it with new model
    if state.local_ai.is_running() {
        println!("[RAG Model Switch] Restarting AI service with new model...");
        state.local_ai.stop_services();
        tokio::time::sleep(Duration::from_secs(1)).await;
        state.local_ai.start_services(&app_handle, pool.inner()).await?;
        println!("[RAG Model Switch] AI service restarted successfully");
    }

    Ok(format!("Model switched to: {}", model_filename))
}


#[tauri::command]
async fn process_dom_stream(
    html: String, 
    provider: String, 
    state: tauri::State<'_, ApplySystemState>,
    editor_bridge: tauri::State<'_, EditorBridge>,
    pool: tauri::State<'_, SqlitePool>
) -> Result<serde_json::Value, String> { 

    // 2. Pobierz listę projektów z bazy danych (do Direct Disk Check)
    let project_paths: Vec<String> = sqlx::query_scalar("SELECT path FROM projects")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Failed to fetch projects: {}", e))?;

    // 1. Ekstrakcja z HTML
    let code_blocks = HtmlParser::extract(&html, &provider);
    if code_blocks.is_empty() {
        return Ok(serde_json::json!({ "status": "no_code" }));
    }

    let mut reports = Vec::new();
    let mut files_modified = Vec::new();

    // 2. Przetwarzanie bloków
    for block in code_blocks {
        let content = &block.content;
        
        // A. Zidentyfikuj plik docelowy używając RepoMap i znanych ścieżek
        let target_path = {
            let map = state.repo_map.lock().map_err(|_| "Mutex poisoned")?;
            // 3. PRZEKAZANIE project_paths
            map.find_path_for_snippet(content, &project_paths)
        };

        if let Some(path) = target_path {
            println!(">>> [Gluon Resolver] Resolved snippet to file: {:?}", path);
            
            // B. Odczytaj oryginalny plik
            if let Ok(old_content) = std::fs::read_to_string(&path) {
                
                // C. Uruchom Lazy Engine (Stitcher)
                // Używamy bloku, aby zwolnić mutex natychmiast po pobraniu silnika (choć tu akurat engine nie jest klonowalny łatwo, więc trzymamy lock na czas operacji)
                // W produkcji lepiej byłoby to zrefaktoryzować, ale na teraz:
                let apply_result = {
                    let engine = state.lazy_stitcher.lock().map_err(|_| "Mutex poisoned")?;
                    engine.apply_lazy_edit(&old_content, content, &path)
                };
                
                match apply_result {
                    Ok(result) => {
                         // D. Zapisz plik
                         // TODO: W przyszłości użyć editor_bridge.request_edit dla atomowości
                         if let Err(e) = std::fs::write(&path, &result.content) {
                             reports.push(format!("FAILED to write {:?}: {}", path, e));
                         } else {
                             reports.push(format!("SUCCESS: Stitched code into {:?}", path));
                             files_modified.push(path.to_string_lossy().to_string());

                             // --- FLASH EFFECT & OPEN IN VS CODE ---
                             // Calculate diff range manually (comparing old_content vs result.content)
                             let old_lines: Vec<&str> = old_content.lines().collect();
                             let new_lines: Vec<&str> = result.content.lines().collect();

                             let mut start_line_0based = 0;
                             // Find first mismatch (prefix)
                             while start_line_0based < old_lines.len() && start_line_0based < new_lines.len() {
                                 if old_lines[start_line_0based] != new_lines[start_line_0based] {
                                     break;
                                 }
                                 start_line_0based += 1;
                             }

                             // Find end mismatch (suffix)
                             let mut old_end = old_lines.len();
                             let mut new_end = new_lines.len();
                             while old_end > start_line_0based && new_end > start_line_0based {
                                 if old_lines[old_end - 1] != new_lines[new_end - 1] {
                                     break;
                                 }
                                 old_end -= 1;
                                 new_end -= 1;
                             }
                             
                             let end_line_0based = new_end;

                             let notification = FileChangeNotification {
                                 path: path.to_string_lossy().to_string(),
                                 ranges: vec![ChangeRange {
                                     start_line: start_line_0based,
                                     end_line: end_line_0based
                                 }]
                             };

                             // Wyślij sygnał do VS Code
                             editor_bridge.notify_changes(vec![notification]);
                         }
                    },
                    Err(e) => {
                        reports.push(format!("FAILED to stitch {:?}: {}", path, e));
                    }
                }
            } else {
                reports.push(format!("FAILED to read original file: {:?}", path));
            }
        } else {
            reports.push("IGNORED: Could not identify target file for snippet.".to_string());
        }
    }

    Ok(serde_json::json!({ 
        "status": "processed", 
        "files": files_modified,
        "details": reports 
    }))
}

const ALLOWED_EXTENSIONS: &[&str] = &[
    // Kod i tekst
    "rs", "toml", "js", "jsx", "ts", "tsx", "html", "css", "json", "md", "yml", "yaml", "txt", "py",
    "java", "kt", "kts", "go", "sql", "wxs", "xml", "xsd", "wsdl", "ini", // Dokumenty
    "pdf", "docx", "doc", "xlsx", "xls", "pptx", "ppt", "odt", "ods", "csv",
    // Pliki graficzne
    "svg", "png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff",
];

const MAX_FILE_SIZE_BYTES: u64 = 100 * 1024 * 1024; // 100 MB

const BINARY_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "doc", "xlsx", "xls", "pptx", "ppt", "odt", "ods", "csv", "svg", "png", "jpg",
    "jpeg", "gif", "webp", "bmp", "tiff",
];

async fn scan_project_files_internal(
    path: &str,
    custom_excludes: &[String],
    allowed_extensions: &[String],
) -> Result<Vec<FileEntry>, String> {
    let root_path = Path::new(path);
    if !root_path.is_dir() {
        return Err(format!("Path is not a valid directory: {}", path));
    }

    let default_exclude_dirs: HashSet<&str> = [
        "node_modules",
        ".git",
        "target",
        "dist",
        "build",
        ".next",
        "vendor",
        "__pycache__",
        ".cache",
        "out",
    ]
    .iter()
    .cloned()
    .collect();

    let custom_excludes_set: HashSet<&str> = custom_excludes.iter().map(String::as_str).collect();

    let mut files: Vec<FileEntry> = Vec::new();
    let walker = WalkDir::new(path).into_iter();

    for entry in walker.filter_entry(|e| {
        if e.file_type().is_dir() {
            if let Some(file_name) = e.file_name().to_str() {
                return !default_exclude_dirs.contains(file_name)
                    && !custom_excludes_set.contains(file_name);
            }
        }
        true
    }) {
        match entry {
            Ok(entry) => {
                if entry.file_type().is_file() {
                    let file_path = entry.path();

                    // Pomiń pliki kontekstowe Gluon, aby nie pojawiały się w drzewie
                    if let Some(file_name_os) = file_path.file_name() {
                        if let Some(file_name_str) = file_name_os.to_str() {
                            if file_name_str.ends_with(".txt")
                                && file_name_str.contains("-context-")
                            {
                                continue; // Pomiń ten plik
                            }
                        }
                    }

                    if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
                        if !allowed_extensions.contains(&ext.to_lowercase()) {
                            continue;
                        }
                    } else {
                        continue;
                    }

                    if let (Ok(relative_path), Ok(metadata)) =
                        (entry.path().strip_prefix(&root_path), entry.metadata())
                    {
                        // Sprawdzenie wielkości pliku
                        if metadata.len() > MAX_FILE_SIZE_BYTES {
                            eprintln!(
                                "Skipping oversized file: {:?} (size: {} bytes)",
                                entry.path(),
                                metadata.len()
                            );
                            continue;
                        }

                        if let Some(path_str) = relative_path.to_str() {
                            let modified_at = metadata
                                .modified()
                                .unwrap_or(SystemTime::now())
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis();
                            files.push(FileEntry {
                                path: path_str.replace('\\', "/"),
                                size_bytes: metadata.len(),
                                modified_at,
                            });
                        }
                    }
                }
            }
            Err(e) => eprintln!("Error processing entry: {}", e),
        }
    }
    Ok(files)
}

fn insert_node(
    nodes: &mut Vec<TreeNode>,
    components: &[&str],
    file: &FileEntry,
    current_path: PathBuf,
) {
    if components.is_empty() {
        return;
    }
    let head = components[0];
    let tail = &components[1..];
    let mut new_path = current_path.clone();
    new_path.push(head);

    if let Some(pos) = nodes.iter().position(|n| n.name == head) {
        if !tail.is_empty() {
            insert_node(&mut nodes[pos].children, tail, file, new_path);
        }
    } else {
        if tail.is_empty() {
            nodes.push(TreeNode {
                name: head.to_string(),
                path: file.path.clone(),
                node_type: NodeType::File,
                children: Vec::new(),
                size_bytes: Some(file.size_bytes),
                modified_at: Some(file.modified_at),
            });
        } else {
            let mut new_dir = TreeNode {
                name: head.to_string(),
                path: new_path.to_string_lossy().to_string().replace('\\', "/"),
                node_type: NodeType::Directory,
                children: Vec::new(),
                size_bytes: None,
                modified_at: None,
            };
            insert_node(&mut new_dir.children, tail, file, new_path);
            nodes.push(new_dir);
        }
    }
}

fn sort_tree_nodes(nodes: &mut [TreeNode]) {
    nodes.sort_by(|a, b| match (&a.node_type, &b.node_type) {
        (NodeType::Directory, NodeType::File) => Ordering::Less,
        (NodeType::File, NodeType::Directory) => Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });
    for node in nodes.iter_mut() {
        if !node.children.is_empty() {
            sort_tree_nodes(&mut node.children);
        }
    }
}

#[tauri::command]
async fn get_project_file_tree(
    path: String,
    state: tauri::State<'_, AppState>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(Vec<TreeNode>, usize), String> {
    {
        let cache = state.file_tree_cache.lock().await;
        if let Some((tree, count, timestamp)) = cache.get(&path) {
            if timestamp.elapsed().unwrap_or_default() < Duration::from_secs(3) {
                // println!("Cache hit for project: {}", path);
                return Ok((tree.clone(), *count));
            }
        }
    }
    // println!("Cache miss or stale for project: {}. Generating tree...", path);

    // Używamy nowej funkcji pomocniczej
    let (root, file_count) = generate_tree_for_project(&path, pool.inner(), &state).await?;

    {
        let mut cache = state.file_tree_cache.lock().await;
        cache.insert(path, (root.clone(), file_count, SystemTime::now()));
    }
    Ok((root, file_count))
}

#[tauri::command]
async fn get_initial_settings(pool: tauri::State<'_, SqlitePool>) -> Result<Settings, String> {
    let port: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'port'")
        .fetch_one(pool.inner())
        .await
        .map_err(sqlx_err)?;
    let auto_start_str: String =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'auto_start'")
            .fetch_one(pool.inner())
            .await
            .map_err(sqlx_err)?;
    let log_level: String =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'log_level'")
            .fetch_one(pool.inner())
            .await
            .map_err(sqlx_err)?;
    Ok(Settings {
        port,
        auto_start: auto_start_str == "true",
        log_level,
    })
}

#[tauri::command]
async fn get_files_content(
    root_path: String,
    relative_paths: Vec<String>,
) -> Result<Vec<FileContent>, String> {
    let mut results = Vec::new();
    let base_path = Path::new(&root_path);
    for rel_path in relative_paths {
        let full_path = base_path.join(&rel_path);
        match fs::read_to_string(full_path).await {
            Ok(content) => {
                // Dodaj ścieżkę pliku jako header (nie jest częścią treści pliku)
                let content_with_path = format!(
                    "# File path: {}\n# This line is metadata, not part of the original file content\n\n{}",
                    rel_path,
                    content
                );
                results.push(FileContent {
                    path: rel_path,
                    content: Some(content_with_path),
                    error: None,
                })
            },
            Err(e) => results.push(FileContent {
                path: rel_path,
                content: None,
                error: Some(e.to_string()),
            }),
        }
    }
    Ok(results)
}

#[tauri::command]
async fn select_download_folder(app_handle: AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    let folder = app_handle.dialog().file().blocking_pick_folder();

    match folder {
        Some(path) => {
            // Convert FilePath to PathBuf and then to String
            let path_buf = PathBuf::from(path.to_string());
            Ok(path_buf.to_string_lossy().to_string())
        }
        None => Err("No folder selected".to_string()),
    }
}

#[tauri::command]
async fn get_default_exclusions(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<String>, String> {
    let json_str: String =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'default_exclusions'")
            .fetch_optional(pool.inner())
            .await
            .map_err(sqlx_err)?
            .unwrap_or_else(|| {
                r#"[
        "node_modules", "vendor", "bower_components",
        "build", "dist", "out", "target", "bin", "obj",
        ".next", ".nuxt", ".svelte-kit", ".vercel",
        ".venv", "venv", "env", ".env", "__pycache__",
        ".cache", ".pytest_cache", ".mypy_cache", ".gradle",
        ".git", ".svn", ".hg",
        ".idea", ".vscode", ".vs",
        "*.swp", "*.swo",
        ".DS_Store", "Thumbs.db",
        "logs", "*.log", "npm-debug.log*", "yarn-error.log"
    ]"#
                .to_string()
            });

    serde_json::from_str(&json_str).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_default_exclusions(
    exclusions: Vec<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    let json_str = serde_json::to_string(&exclusions).map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('default_exclusions', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&json_str)
    .execute(pool.inner())
    .await
    .map_err(sqlx_err)?;
    Ok(())
}
// ============================================================================
// Context File Generation Logic
// ============================================================================

/// Sprawdza czy plik ma header Gluon Context
fn is_gluon_context_file(content: &str) -> bool {
    let content_without_bom = content.strip_prefix('\u{FEFF}').unwrap_or(content);

    let lines: Vec<&str> = content_without_bom.lines().take(3).collect();
    lines.len() >= 3 && lines[0] == "# GLUON CONFIG" && lines[2] == "# END CONFIG"
}

/// Wyciąga konfigurację z zawartości pliku
fn extract_config_from_content(content: &str) -> Option<ContextConfig> {
    // Usuń BOM jeśli istnieje
    let content_without_bom = content.strip_prefix('\u{FEFF}').unwrap_or(content);

    let lines: Vec<&str> = content_without_bom.lines().take(3).collect();

    if lines.len() >= 3 && lines[0] == "# GLUON CONFIG" && lines[2] == "# END CONFIG" {
        let config_line = lines[1].trim_start_matches("# ");
        serde_json::from_str::<ContextConfig>(config_line).ok()
    } else {
        None
    }
}

async fn format_prompts_section(
    pool: &SqlitePool,
    env_id: i64,
    enabled_prompt_ids: &[i64],
    quick_task: &Option<String>,
) -> Result<String, sqlx::Error> {
    let prompts = sqlx::query_as::<_, Prompt>(
        "SELECT * FROM prompts WHERE environment_id = ? ORDER BY sort_order ASC",
    )
    .bind(env_id)
    .fetch_all(pool)
    .await?;

    let mut output = String::from("=== ACTIVE PROMPTS ===\n");

    for prompt in prompts {
        if enabled_prompt_ids.contains(&prompt.id) {
            let name = &prompt.name;
            let content = prompt.content.as_deref().unwrap_or("<no content>");
            output.push_str(&format!(
                "[✓] [{}] {}\n{}\n\n",
                prompt.category, name, content
            ));
        }
    }

    if let Some(task) = quick_task {
        if !task.trim().is_empty() {
            output.push_str("=== QUICK TASK ===\n");
            output.push_str(task);
            output.push_str("\n\n");
        }
    }

    Ok(output)
}

fn format_tree_recursive(nodes: &[TreeNode], prefix: &str, graph: &gluon_desktop_lib::apply_system::context::graph::ContextGraph) -> String {
    let mut output = String::new();
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        
        output.push_str(&format!(
            "{}{}{}\n",
            prefix, connector, node.name
        ));

        // [X-RAY FEATURE] Jeśli to plik, wstrzyknij sygnatury z grafu pod nazwą pliku
        if matches!(node.node_type, NodeType::File) {
            if let Some(symbols) = graph.get_symbols(&node.path) {
                let sym_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                for (j, sym) in symbols.iter().take(5).enumerate() {
                    let is_last_sym = j == symbols.len().min(5) - 1;
                    let sym_conn = if is_last_sym { "└── " } else { "├── " };
                    output.push_str(&format!("{}  {}{}\n", sym_prefix, sym_conn, sym.name));
                }
                if symbols.len() > 5 {
                    output.push_str(&format!("{}  ... (use 'semantic_map' for full list)\n", sym_prefix));
                }
            }
        }

        let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
        if !node.children.is_empty() {
            output.push_str(&format_tree_recursive(&node.children, &new_prefix, graph));
        }
    }
    output
}

fn format_attached_files_section(attached_files: &HashMap<String, Vec<String>>) -> String {
    if attached_files.is_empty() {
        return String::new();
    }

    let mut output = String::from("=== ATTACHED FILES ===\n");
    output.push_str("# These files should be attached separately alongside this context file.\n");

    for (project_path, files) in attached_files {
        let project_name = Path::new(project_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        for file_path in files {
            output.push_str(&format!("- {}: {}\n", project_name, file_path));
        }
    }
    output.push_str("\n");
    output
}

async fn format_structure_section(
    projects: &[ProjectFilesRequest],
    state: &AppState,
    pool: &SqlitePool,
    app_handle: &tauri::AppHandle,
) -> String {
    let mut output = String::from("=== PROJECT STRUCTURE ===\n");

    // Pobierz dostęp do grafu symboli dla funkcji X-Ray
    let apply_state = app_handle.state::<gluon_desktop_lib::apply_system::ApplySystemState>();

    for project_req in projects {
        let project_name = Path::new(&project_req.root_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        output.push_str(&format!("\nPROJECT: {}\n", project_name));

        match generate_tree_for_project(&project_req.root_path, pool, state).await {
            Ok((tree, file_count)) => {
                output.push_str(&format!("(Total files: {})\n", file_count));
                // Lock the graph only when needed, after the await
                let graph = apply_state.context_graph.lock().unwrap();
                // Przekazujemy graf do funkcji rekurencyjnej
                output.push_str(&format_tree_recursive(&tree, "", &graph));
                // Lock is automatically dropped here
            }
            Err(e) => {
                output.push_str(&format!("ERROR: Failed to generate tree: {}\n", e));
            }
        }
    }

    output.push_str("\n");
    output
}

async fn generate_tree_for_project(
    path: &str,
    pool: &SqlitePool,
    _state: &AppState, // _state może być potrzebny w przyszłości
) -> Result<(Vec<TreeNode>, usize), String> {
    let project: Option<Project> = sqlx::query_as("SELECT id, path, excluded_paths, download_path, allowed_extensions, vector_map_id FROM projects WHERE path = ?")
        .bind(path)
        .fetch_optional(pool)
        .await.map_err(sqlx_err)?;

    let (custom_excludes, allowed_extensions) = if let Some(p) = project {
        let excludes: Vec<String> = p
            .excluded_paths
            .and_then(|json_str| {
                if json_str.is_empty() {
                    None
                } else {
                    serde_json::from_str(&json_str).ok()
                }
            })
            .unwrap_or_default();

        let allowed: Vec<String> = p
            .allowed_extensions
            .and_then(|json_str| {
                if json_str.is_empty() {
                    None
                } else {
                    serde_json::from_str(&json_str).ok()
                }
            })
            .unwrap_or_else(|| ALLOWED_EXTENSIONS.iter().map(|s| s.to_string()).collect());

        (excludes, allowed)
    } else {
        (
            Vec::new(),
            ALLOWED_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
        )
    };

    let files = scan_project_files_internal(path, &custom_excludes, &allowed_extensions).await?;
    let file_count = files.len();
    let mut root: Vec<TreeNode> = Vec::new();
    for file_entry in files {
        let components: Vec<&str> = file_entry.path.split('/').collect();
        insert_node(&mut root, &components, &file_entry, PathBuf::new());
    }
    sort_tree_nodes(&mut root);
    Ok((root, file_count))
}

async fn format_files_section(
    projects: &[ProjectFilesRequest],
    apply_state: &tauri::State<'_, ApplySystemState>,
    app_handle: &AppHandle,
) -> String {
    use crate::apply_system::tauri_commands::{execute_context_operations, ContextOperation, ContextItem};

    let mut output = String::from("=== PROJECT FILES ===\n");
    for project_req in projects {
        let project_name = Path::new(&project_req.root_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        output.push_str(&format!("\n--- PROJECT: {} ---\n", project_name));
        output.push_str(&format!("Path: {}\n\n", &project_req.root_path));

        // Handle files with selected symbols
        if !project_req.symbols.is_empty() {
            println!("[ContextGen] Processing {} files with selected symbols", project_req.symbols.len());

            for (file_path, symbol_names) in &project_req.symbols {
                for symbol_name in symbol_names {
                    let operation = ContextOperation::FileSymbol {
                        path: file_path.clone(),
                        symbol: symbol_name.clone(),
                    };

                    // Use execute_context_operations to get just the symbol
                    match execute_context_operations(
                        vec![operation],
                        Some(project_req.root_path.clone()),
                        apply_state.clone(),
                        app_handle.clone(),
                    ).await {
                        Ok(response) => {
                            if let Some(item) = response.items.first() {
                                match item {
                                    ContextItem::SymbolContent { content, .. } => {
                                        output.push_str(&format!("// {}::{}\n{}\n\n", file_path, symbol_name, content));
                                    }
                                    ContextItem::Error { error, .. } => {
                                        output.push_str(&format!("// ERROR extracting symbol {} from {}: {}\n\n", symbol_name, file_path, error));
                                    }
                                    _ => {
                                        output.push_str(&format!("// Unexpected response type for symbol {} from {}\n\n", symbol_name, file_path));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            output.push_str(&format!("// ERROR extracting symbol {} from {}: {}\n\n", symbol_name, file_path, e));
                        }
                    }
                }
            }
        }

        // Handle full files (no symbols selected)
        if !project_req.relative_paths.is_empty() {
            match get_files_content(
                project_req.root_path.clone(),
                project_req.relative_paths.clone(),
            )
            .await
            {
                Ok(files) => {
                    for file in files {
                        // Skip files that have selected symbols (already processed above)
                        if project_req.symbols.contains_key(&file.path) {
                            continue;
                        }

                        if let Some(content) = file.content {
                            output.push_str(&format!("// {}\n{}\n\n", file.path, content));
                        } else if let Some(error) = file.error {
                            output.push_str(&format!(
                                "// ERROR reading file: {}\n// {}\n\n",
                                file.path, error
                            ));
                        }
                    }
                }
                Err(e) => {
                    output.push_str(&format!("// ERROR reading files for project: {}\n\n", e));
                }
            }
        }
    }
    output
}

async fn generate_context_file(
    app_handle: AppHandle,
    request_id: String,
    payload: GenerateContextFilePayload,
) -> Result<ContextFileMetadata, String> {
    println!(">>> [1/10] Starting context generation...");
    emit_context_progress(&app_handle, 1, "Initializing", "Starting context generation...", 0, None, None).await;

    // Register cancel token for this request
    let cancel_token = register_cancel_token(&request_id);

    // Dostęp do stanów
    let state: tauri::State<'_, AppState> = app_handle.state();
    let apply_state: tauri::State<'_, ApplySystemState> = app_handle.state();
    let pool: tauri::State<'_, SqlitePool> = app_handle.state();

    println!(
        ">>> [2/10] Validating environment_id: {}",
        payload.environment_id
    );
    emit_context_progress(&app_handle, 2, "Validating", "Validating environment...", 10, None, None).await;
    let env_exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM environments WHERE id = ?")
        .bind(payload.environment_id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| {
            println!(">>> ERROR: Database validation failed: {}", e);
            format!("Database error: {}", e)
        })?;

    if !env_exists {
        let err_msg = format!(
            "Environment with ID {} does not exist",
            payload.environment_id
        );
        println!(">>> ERROR: {}", err_msg);
        return Err(err_msg);
    }
    println!(">>> Environment validated OK");

    // === Podział plików na tekstowe i binarne ===
    let binary_ext_set: HashSet<&str> = BINARY_EXTENSIONS.iter().cloned().collect();
    let mut text_file_projects: Vec<ProjectFilesRequest> = Vec::new();
    let mut attached_files_map: HashMap<String, Vec<String>> = HashMap::new();
    
    // Zbieramy ścieżki absolutne plików "skupionych" (wybranych przez usera) dla Repo Map
    let mut focused_files_absolute: Vec<String> = Vec::new(); 

    for proj in &payload.projects {
        let mut text_paths = Vec::new();
        let mut binary_paths = Vec::new();
        let root_path_buf = PathBuf::from(&proj.root_path);

        for path_str in &proj.relative_paths {
            let path = Path::new(path_str);
            
            // Dodaj do listy skupienia (Repo Map)
            let abs_path = root_path_buf.join(path_str).to_string_lossy().replace('\\', "/");
            focused_files_absolute.push(abs_path);

            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if binary_ext_set.contains(ext) {
                    binary_paths.push(path_str.clone());
                } else {
                    text_paths.push(path_str.clone());
                }
            } else {
                text_paths.push(path_str.clone()); // Domyślnie traktuj jako tekstowy
            }
        }

        if !text_paths.is_empty() || !proj.symbols.is_empty() {
            text_file_projects.push(ProjectFilesRequest {
                root_path: proj.root_path.clone(),
                relative_paths: text_paths,
                symbols: proj.symbols.clone(),
            });
        }
        if !binary_paths.is_empty() {
            attached_files_map.insert(proj.root_path.clone(), binary_paths);
        }
    }

    // Check if cancelled
    if cancel_token.load(AtomicOrdering::Relaxed) {
        remove_cancel_token(&request_id);
        return Err("Generation cancelled by user".to_string());
    }

    // === Przygotowanie metadanych konfiguracji ===
    println!(">>> [3/10] Preparing configuration metadata...");
    emit_context_progress(&app_handle, 3, "Preparing", "Preparing configuration metadata...", 20, None, None).await;
    let timestamp = Local::now().to_rfc3339();
    let mut selected_files_map: HashMap<String, Vec<String>> = HashMap::new();
    for proj in &payload.projects {
        selected_files_map.insert(proj.root_path.clone(), proj.relative_paths.clone());
    }

    let config = ContextConfig {
        projects: payload
            .projects
            .iter()
            .map(|p| p.root_path.clone())
            .collect(),
        selected_files: selected_files_map,
        attached_files: attached_files_map.clone(),
        environment_id: payload.environment_id,
        prompt_ids: payload.enabled_prompt_ids.clone(),
        quick_task: payload.quick_task.clone(),
        timestamp: timestamp.clone(),
        include_logs: payload.include_logs,
        logs: if payload.include_logs {
            payload.logs.clone()
        } else {
            None
        },
    };

    let config_json = serde_json::to_string(&config).map_err(|e| {
        println!(">>> ERROR: Failed to serialize config: {}", e);
        e.to_string()
    })?;
    println!(">>> Config metadata prepared");

    let mut full_content = String::new();

    // === Zapisz metadane na początku pliku ===
    full_content.push_str("# GLUON CONFIG\n");
    full_content.push_str(&format!("# {}\n", config_json));
    full_content.push_str("# END CONFIG\n\n");

    // ===  Wstaw instrukcje protokołu (jeśli przesłane) ===
    if let Some(instr) = &payload.protocol_instructions {
        println!(">>> [3.5/10] Appending G-Protocol instructions...");
        full_content.push_str(instr);
        full_content.push_str("\n\n");
    }

    // ===  Wstaw Context Architect Prompt (jeśli przesłane) ===
    if let Some(architect_prompt) = &payload.context_architect_prompt {
        println!(">>> [3.6/10] Appending Context Architect Prompt...");
        full_content.push_str(architect_prompt);
        full_content.push_str("\n\n");
    }

    println!(">>> [4/10] Generating prompts section...");
    emit_context_progress(&app_handle, 4, "Prompts", "Generating prompts section...", 30, None, None).await;
    let prompts_section = format_prompts_section(
        pool.inner(),
        payload.environment_id,
        &payload.enabled_prompt_ids,
        &payload.quick_task,
    )
    .await
    .map_err(|e| {
        println!(">>> ERROR: format_prompts_section failed: {}", e);
        sqlx_err(e)
    })?;
    full_content.push_str(&prompts_section);

    if payload.include_logs {
        if let Some(logs) = payload.logs {
            if !logs.trim().is_empty() {
                println!(">>> [5/10] Appending application logs...");
                let logs_section = format!("=== APPLICATION LOGS ===\n{}\n\n", logs);
                full_content.push_str(&logs_section);
            }
        }
    }

    // Check if cancelled
    if cancel_token.load(AtomicOrdering::Relaxed) {
        remove_cancel_token(&request_id);
        return Err("Generation cancelled by user".to_string());
    }

    // === [NEW] REPO SEMANTIC MAP GENERATION ===
    // Generujemy to TYLKO jeśli są wybrane jakieś pliki, aby dać modelowi kontekst.
    // Jeśli nie ma wybranych plików (tylko struktura), pomijamy generowanie mapy.
    let selected_files_count: usize = payload.projects.iter().map(|p| p.relative_paths.len()).sum();

    let repo_map_str = if selected_files_count > 0 {
        println!(">>> [6/10] Generating Semantic Repo Map for {} selected files...", selected_files_count);
        emit_context_progress(&app_handle, 5, "Mapping", "Preparing to analyze full codebase...", 40, None, None).await;

        let graph_arc = apply_state.context_graph.clone();
        let project_roots: Vec<String> = payload.projects.iter().map(|p| p.root_path.clone()).collect();

        // Create a channel for progress updates from the blocking task
        let (progress_tx, mut progress_rx) = mpsc::channel::<(usize, String)>(100);
        let app_handle_progress = app_handle.clone();

        // Send intermediate progress update
        emit_context_progress(&app_handle, 5, "Mapping", &format!("Starting codebase analysis ({} files selected)...", selected_files_count), 42, None, None).await;

        // Spawn blocking task for indexing
        let indexing_task = tokio::task::spawn_blocking(move || {
            let mut graph = graph_arc.lock().unwrap();

            let exclusions = vec![
                "node_modules".to_string(), ".git".to_string(), "dist".to_string(),
                "build".to_string(), "__pycache__".to_string(), ".env".to_string(),
                "migrations".to_string() // DODANO: Warto wykluczyć migracje, bo zaśmiecają mapę
            ];

            let tx_arc = Arc::new(std::sync::Mutex::new(progress_tx));

            for (idx, root) in project_roots.iter().enumerate() {
                let tx_clone = Arc::clone(&tx_arc);
                let root_name = std::path::Path::new(root)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(root)
                    .to_string();
                let total_roots = project_roots.len();

                graph.index_directory(root, &exclusions, Some(move |count: usize, path: &str| {
                    let msg = if path == "completed" {
                        format!("[{}/{}] {}: completed", idx + 1, total_roots, root_name)
                    } else {
                        let file_name = std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path);
                        format!("[{}/{}] {}: {}", idx + 1, total_roots, root_name, file_name)
                    };
                    if let Ok(tx) = tx_clone.lock() {
                        let _ = tx.blocking_send((count, msg));
                    }
                }));
            }

            // Close the channel when done
            drop(tx_arc);

            // ZMIANA: Zwiększono limit z 2000 na 16000 tokenów
            rank_files(&graph, &focused_files_absolute, 16000)
        });

        // Listen for progress updates and emit them to the UI
        let progress_listener = tokio::spawn(async move {
            let mut total_processed = 0; // Global counter across all projects
            let mut last_project_count = 0; // Counter for current project
            let mut total_estimated = 0; // Will be updated dynamically
            let mut last_percentage = 42;

            while let Some((count_in_project, path)) = progress_rx.recv().await {
                // Calculate increment from this project (how many files were processed since last update)
                let increment = count_in_project.saturating_sub(last_project_count);
                total_processed += increment;
                last_project_count = count_in_project;

                // Update estimate when project completes
                if path.contains("completed") {
                    total_estimated += count_in_project;
                    last_project_count = 0;
                }

                // Better percentage calculation - smoother progression from 42% to 48%
                // Use exponential smoothing to avoid jumps
                let target_percentage = if total_estimated > 0 {
                    42 + ((total_processed as f32 / total_estimated.max(1) as f32) * 6.0) as u8
                } else {
                    // If we don't have estimate yet, just show incremental progress
                    42 + (total_processed.min(300) / 50) as u8
                };
                let target_percentage = target_percentage.min(48).max(42);

                // Smooth transition
                last_percentage = if target_percentage > last_percentage {
                    last_percentage + 1
                } else {
                    target_percentage
                };

                // Better message formatting
                let display_msg = if path.contains("completed") {
                    path.to_string()
                } else {
                    let file_name = std::path::Path::new(&path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&path);
                    format!("Analyzing {}...", file_name)
                };

                emit_context_progress(
                    &app_handle_progress,
                    5,
                    "Repository Analysis",
                    &format!("Indexing codebase ({} files analyzed) - {}", total_processed, display_msg),
                    last_percentage,
                    None, // Don't show file count - it's confusing
                    None,
                ).await;
            }
        });

        // Wait for indexing to complete
        let map_str = indexing_task.await.map_err(|e| format!("Repo Map generation failed: {}", e))?;

        // Ensure progress listener finishes
        let _ = progress_listener.await;

        // Send progress update after blocking operation
        emit_context_progress(&app_handle, 6, "Mapping", "Finalizing semantic map...", 48, None, None).await;

        emit_context_progress(&app_handle, 6, "Mapping", "Repository analysis complete", 50, None, None).await;

        map_str
    } else {
        println!(">>> [6/10] Skipping Semantic Repo Map (no files selected)");
        emit_context_progress(&app_handle, 6, "Mapping", "Skipping semantic map (no files selected)", 50, None, None).await;
        String::new()
    };

    // Append the repo map if it was generated
    if !repo_map_str.is_empty() {
        full_content.push_str("=== REPO SEMANTIC MAP (CONTEXT) ===\n");
        full_content.push_str(&repo_map_str);
        full_content.push_str("\n\n");
    }
    // ==========================================

    // Check if cancelled
    if cancel_token.load(AtomicOrdering::Relaxed) {
        remove_cancel_token(&request_id);
        return Err("Generation cancelled by user".to_string());
    }

    if payload.include_structure {
        println!(">>> [7/10] Generating directory structure...");
        emit_context_progress(&app_handle, 7, "Structure", "Generating directory structure...", 60, None, None).await;
        let structure_section =
            format_structure_section(&payload.projects, &state, pool.inner(), &app_handle).await;
        full_content.push_str(&structure_section);
    } else {
        println!(">>> Skipping directory structure (include_structure = false)");
        emit_context_progress(&app_handle, 7, "Structure", "Skipping structure (not included)", 60, None, None).await;
    }

    // === Dodaj sekcję z załącznikami ===
    let attached_files_section = format_attached_files_section(&attached_files_map);
    if !attached_files_section.is_empty() {
        full_content.push_str(&attached_files_section);
    }

    // Check if cancelled
    if cancel_token.load(AtomicOrdering::Relaxed) {
        remove_cancel_token(&request_id);
        return Err("Generation cancelled by user".to_string());
    }

    println!(">>> [8/10] Generating files section...");
    emit_context_progress(&app_handle, 8, "Reading", &format!("Reading {} selected files...", selected_files_count), 75, None, None).await;
    let files_section = format_files_section(&text_file_projects, &apply_state, &app_handle).await;
    full_content.push_str(&files_section);

    // === Append Virtual Files (e.g. from Google Drive) ===
    if !payload.virtual_files.is_empty() {
        println!(">>> [8.5/10] Appending {} virtual files...", payload.virtual_files.len());
        full_content.push_str("\n=== ATTACHED VIRTUAL FILES ===\n");
        for vf in payload.virtual_files {
            full_content.push_str(&format!("\n// File: {}\n{}\n\n", vf.name, vf.content));
        }
    }

    // Check if cancelled
    if cancel_token.load(AtomicOrdering::Relaxed) {
        remove_cancel_token(&request_id);
        return Err("Generation cancelled by user".to_string());
    }

    println!(">>> [9/10] Writing file to disk...");
    emit_context_progress(&app_handle, 9, "Writing", "Writing file to disk...", 90, None, None).await;

    // Określ folder pobierania (bez zmian)
    let target_dir = if !payload.projects.is_empty() {
        let first_project_path = &payload.projects[0].root_path;
        let project: Option<Project> = sqlx::query_as(
            "SELECT id, path, excluded_paths, download_path, allowed_extensions, vector_map_id FROM projects WHERE path = ?"
        )
            .bind(first_project_path)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| {
                println!(">>> ERROR: Failed to fetch first project: {}", e);
                sqlx_err(e)
            })?;

        if let Some(proj) = project {
            if let Some(dl_path) = proj.download_path {
                PathBuf::from(dl_path)
            } else {
                PathBuf::from(&proj.path)
            }
        } else {
            app_handle.path().download_dir().map_err(|e| e.to_string())?
        }
    } else {
        app_handle.path().download_dir().map_err(|e| e.to_string())?
    };

    // Funkcja pomocnicza do sanityzacji nazwy projektu
    fn sanitize_project_name(path: &str) -> String {
        Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .replace(' ', "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect::<String>()
            .to_lowercase()
    }

    // Generuj nazwę pliku
    let filename = if payload.projects.len() == 1 {
        let project_name = sanitize_project_name(&payload.projects[0].root_path);
        format!("{}-context-{}.txt", project_name, Local::now().format("%Y%m%d_%H%M%S"))
    } else if payload.projects.len() <= 3 {
        let names: Vec<String> = payload.projects.iter().map(|p| sanitize_project_name(&p.root_path)).collect();
        let combined = names.join("_");
        let name_part = if combined.len() > 50 { "multi-project".to_string() } else { combined };
        format!("{}-context-{}.txt", name_part, Local::now().format("%Y%m%d_%H%M%S"))
    } else {
        format!("multi-project-context-{}.txt", Local::now().format("%Y%m%d_%H%M%S"))
    };

    let filepath = target_dir.join(&filename);
    
    let mut file = tokio::fs::File::create(&filepath).await.map_err(|e| e.to_string())?;

    file.write_all(full_content.as_bytes()).await.map_err(|e| e.to_string())?;

    println!(">>> [10/10] Preparing metadata response...");
    let metadata = fs::metadata(&filepath).await.map_err(|e| e.to_string())?;
    let size = metadata.len();
    let file_count: usize = payload.projects.iter().map(|p| p.relative_paths.len()).sum();
    emit_context_progress(&app_handle, 10, "Complete", "Context file generated successfully", 100, Some(file_count), Some(file_count)).await;
    let warning = if size > 2 * 1024 * 1024 {
        Some("File size exceeds 2MB. Gemini may reject upload.".to_string())
    } else {
        None
    };

    // Cleanup cancel token on success
    remove_cancel_token(&request_id);

    Ok(ContextFileMetadata {
        filepath: filepath.to_string_lossy().replace('\\', "/"),
        filename,
        size,
        file_count,
        project_count: payload.projects.len(),
        warning,
        content: full_content,
    })
}

// ============================================================================
// WebSocket Server Implementation
// ============================================================================

#[derive(Serialize)]
struct EnvironmentWithPrompts {
    #[serde(flatten)]
    environment: Environment,
    prompts: Vec<Prompt>,
}

async fn handle_connection(stream: TcpStream, app_handle: tauri::AppHandle) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WebSocket handshake error: {}", e);
            return;
        }
    };

    // Generate unique socket ID for this connection
    let socket_id = Uuid::new_v4().to_string();
    println!("New WebSocket connection established (socket_id: {}).", socket_id);

    // 1. Rozdzielamy strumień na zapis i odczyt
    // To kluczowe, aby móc wysyłać wiadomości do VS Code niezależnie od otrzymywanych zapytań
    let (mut ws_write, mut ws_read) = ws_stream.split();
 
    // 2. Tworzymy kanał komunikacyjny dla EditorBridge -> WebSocket
    // EditorBridge będzie wrzucał tu wiadomości JSON dla wtyczki VS Code
    let (bridge_tx, mut bridge_rx) = mpsc::unbounded_channel::<String>();
 
    // 3. Pobieramy stan mostu (EditorBridge)
    let _editor_bridge = app_handle.state::<EditorBridge>();
    // Klonujemy handle dla read_taska
    let app_handle_clone = app_handle.clone(); 
 
    // --- TASK ZAPISU (Wysyłanie do klienta) ---
    // Obsługuje wiadomości z EditorBridge oraz PINGi
    let write_task = tokio::spawn(async move {
        let mut ping_interval = interval(Duration::from_secs(30));
 
        loop {
            tokio::select! {
                // Wiadomość z mostu (np. request_edit)
                msg_opt = bridge_rx.recv() => {
                    match msg_opt {
                        Some(msg) => {
                            if ws_write.send(Message::Text(msg.into())).await.is_err() {
                                break; // Błąd zapisu = koniec połączenia
                            }
                        }
                        None => break, // Kanał zamknięty
                    }
                }
                // Ping co 30 sekund
                _ = ping_interval.tick() => {
                    if ws_write.send(Message::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    // --- PULSE SYSTEM: Setup Tauri Event Listener (Progress) ---
    let bridge_tx_for_progress = bridge_tx.clone();
    app_handle.listen("apply-system-apply-progress", move |event| {
        let payload = event.payload();
        // Parse the payload string to JSON value
        if let Ok(payload_json) = serde_json::from_str::<serde_json::Value>(payload) {
            let msg = serde_json::json!({
                "action": "apply_progress_update",
                "payload": payload_json
            });
            if let Ok(json_str) = serde_json::to_string(&msg) {
                let _ = bridge_tx_for_progress.send(json_str);
            }
        }
    });

    // --- AGENTIC RAG: Setup Tauri Event Listener (Progress) ---
    let bridge_tx_for_agentic = bridge_tx.clone();
    app_handle.listen("agentic_progress", move |event| {
        let payload = event.payload();
        if let Ok(payload_json) = serde_json::from_str::<serde_json::Value>(payload) {
            let msg = serde_json::json!({
                "action": "agentic_progress",
                "payload": payload_json
            });
            if let Ok(json_str) = serde_json::to_string(&msg) {
                let _ = bridge_tx_for_agentic.send(json_str);
            }
        }
    });

    // --- CONTEXT GENERATION: Setup Tauri Event Listener (Progress) ---
    let bridge_tx_for_context = bridge_tx.clone();
    app_handle.listen("context_generation_progress", move |event| {
        let payload = event.payload();
        if let Ok(payload_json) = serde_json::from_str::<serde_json::Value>(payload) {
            let msg = serde_json::json!({
                "action": "context_generation_progress",
                "payload": payload_json
            });
            if let Ok(json_str) = serde_json::to_string(&msg) {
                let _ = bridge_tx_for_context.send(json_str);
            }
        }
    });

    // --- STATUS UPDATES: Setup Tauri Event Listener (Undo/Redo) ---
    // To jest mechanizm "jak w Apply" - nasłuchiwanie na eventy i wysyłanie do przeglądarki
    let bridge_tx_for_status = bridge_tx.clone();
    app_handle.listen("apply-system-status-change", move |event| {
        let payload = event.payload();

        let msg = serde_json::json!({
            "action": "change_status_update",
            "payload": serde_json::from_str::<serde_json::Value>(payload).unwrap_or(serde_json::Value::Null)
        });

        if let Ok(json_str) = serde_json::to_string(&msg) {
            let _ = bridge_tx_for_status.send(json_str);
        }
    });

    // --- AI LOADING PROGRESS: Setup Tauri Event Listener ---
    let bridge_tx_for_ai_loading = bridge_tx.clone();
    app_handle.listen("ai-loading-progress", move |event| {
        let payload = event.payload();
        if let Ok(payload_json) = serde_json::from_str::<serde_json::Value>(payload) {
            let msg = serde_json::json!({
                "action": "ai_loading_progress",
                "payload": payload_json
            });
            if let Ok(json_str) = serde_json::to_string(&msg) {
                let _ = bridge_tx_for_ai_loading.send(json_str);
            }
        }
    });

    // --- INDEXING PROGRESS: Setup Tauri Event Listener ---
    let bridge_tx_for_indexing = bridge_tx.clone();
    app_handle.listen("indexing_progress", move |event| {
        let payload = event.payload();
        if let Ok(payload_json) = serde_json::from_str::<serde_json::Value>(payload) {
            let msg = serde_json::json!({
                "action": "indexing_progress",
                "payload": payload_json
            });
            if let Ok(json_str) = serde_json::to_string(&msg) {
                let _ = bridge_tx_for_indexing.send(json_str);
            }
        }
    });

    // --- WORKFLOW SYNC 1:1: Bridge Tauri -> Extension ---
    let bridge_tx_for_workflow = bridge_tx.clone();
    app_handle.listen("workflow-sync-bridge", move |event| {
        if let Ok(_payload_str) = serde_json::to_string(&event.payload()) {
            // Payload eventu jest stringiem JSON, musimy go odpakować
            // Event payload z tauri::emit jest już stringiem JSON
            // Tutaj zakładamy, że payload to serializowany ExecutionGraph
            if let Ok(graph_data) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                let msg = serde_json::json!({
                    "action": "workflow_sync",
                    "payload": graph_data
                });
                if let Ok(json_str) = serde_json::to_string(&msg) {
                    let _ = bridge_tx_for_workflow.send(json_str);
                }
            }
        }
    });

    // --- TASK ODCZYTU (Odbieranie od klienta) ---
    // Obsługuje identyfikację VS Code oraz komendy z frontendu Tauri
    let socket_id_for_read = socket_id.clone();
    let mut read_task = tokio::spawn(async move {
        let pool = app_handle_clone.state::<SqlitePool>();
        // Pobieramy ponownie stan mostu wewnątrz taska (dla wywołań metod)
        let bridge_state = app_handle_clone.state::<EditorBridge>();
 
        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Najpierw parsujemy do ogólnego JSON-a, aby sprawdzić typ klienta
                    let json_val: serde_json::Value = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("JSON parse error: {}", e);
                            continue;
                        }
                    };
 
                    // --- SCENARIUSZ 1: Identyfikacja Wtyczki Edytora (VS Code / JetBrains) ---
                    if json_val["type"] == "identify" {
                        let client = json_val["client"].as_str().unwrap_or("unknown");
                        match client {
                            "vscode" => println!("[WS] VS Code Extension identified."),
                            "jetbrains" => println!("[WS] JetBrains Plugin identified."),
                            other => println!("[WS] Unknown editor identified: {}", other),
                        }

                        // Extract roots (workspace folders) from handshake
                        let roots: Vec<String> = if let Some(arr) = json_val["roots"].as_array() {
                            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                        } else {
                            Vec::new()
                        };

                        println!("[WS] Registering connection with roots: {:?}", roots);
                        bridge_state.register_connection(bridge_tx.clone(), roots);
                        continue;
                    }

                    // --- SCENARIUSZ 1.5: Heartbeat od VS Code (window focus) ---
                    if json_val["type"] == "heartbeat" {
                        let roots: Vec<String> = if let Some(arr) = json_val["roots"].as_array() {
                            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                        } else {
                            Vec::new()
                        };

                        // println!("[WS] 💓 Heartbeat from VS Code - updating activity for roots: {:?}", roots); // Disabled - too verbose
                        bridge_state.update_activity_for_roots(roots);
                        continue;
                    }

                    // --- SCENARIUSZ 2: Odpowiedź od Wtyczki (np. po edycji) ---
                    if json_val["type"] == "edit_result" {
                        match serde_json::from_value::<EditorEditResponse>(json_val) {
                            Ok(response) => {
                                // Przekazujemy wynik do EditorBridge, który odblokuje czekający `apply_change_command`
                                bridge_state.handle_response(response);
                            }
                            Err(e) => eprintln!("[WS] Failed to parse edit_result: {}", e),
                        }
                        continue;
                    }
                    // --- Handle change_undone notification from VS Code ---
                    if json_val["type"] == "change_undone" {
                        println!("[RUST WS] Received change_undone from VS Code: {:?}", json_val);

                        // [FIX] Emituj jako Tauri Event (Broadcast) - tak jak robi to Apply
                        let payload = serde_json::json!({
                            "changeId": json_val["changeId"].as_str().unwrap_or("unknown"),
                            "batchId": json_val["batchId"].as_str().unwrap_or("unknown"),
                            "status": "undone"
                        });

                        let _ = app_handle_clone.emit("apply-system-status-change", payload);
                        continue;
                    }

                    // --- Handle change_redone notification from VS Code ---
                    if json_val["type"] == "change_redone" {
                        println!("[RUST WS] Received change_redone from VS Code: {:?}", json_val);

                        // [FIX] Emituj jako Tauri Event (Broadcast)
                        let payload = serde_json::json!({
                            "changeId": json_val["changeId"].as_str().unwrap_or("unknown"),
                            "batchId": json_val["batchId"].as_str().unwrap_or("unknown"),
                            "status": "success"
                        });

                        let _ = app_handle_clone.emit("apply-system-status-change", payload);
                        continue;
                    }
 
                    // --- SCENARIUSZ 3: Standardowe żądania z Frontendu (Przeglądarka) ---
                    // Jeśli to nie jest komunikat systemowy mostu, parsujemy jako WebSocketRequest
                    if let Ok(request) = serde_json::from_value::<WebSocketRequest>(json_val) {
                        let request_id = request.id.clone();
                        // Używamy "bridge_tx" jako zwrotki, ale musimy go zapakować w JSON
                        // Uwaga: W tym modelu Request-Response dla frontendu, frontend oczekuje odpowiedzi
                        // na tym samym sockecie. Ponieważ "write_task" ma wyłączność na "ws_write",
                        // musimy wysłać odpowiedź przez "bridge_tx", który przekaże ją do "write_task".
                        
                        let tx_clone = bridge_tx.clone();

                        // Helper do wysyłania odpowiedzi
                        let send_json = move |json: String| {
                            let _ = tx_clone.send(json);
                        };
 
                        // Log only non-routine actions (skip heartbeat, file_trees, license checks)
                        if !matches!(request.action.as_str(), "get_file_trees" | "get_license_status" | "heartbeat") {
                            // println!("[RUST WS] 📥 Received action: {}", request.action);
                        }
                        match request.action.as_str() {
                                "tauri_invoke" => {
                                    // Handle Tauri command invocations from extension
                                    println!("[RUST WS] Received tauri_invoke request");

                                    if let (Some(command), Some(args)) = (
                                        request.payload.get("command").and_then(|v| v.as_str()),
                                        request.payload.get("args")
                                    ) {
                                        println!("[RUST WS] Invoking Tauri command: {}", command);

                                        // Execute the Tauri command using the app_handle
                                        // This will call the registered Tauri command handlers
                                        let result = match command {
                                            "list_drive_files" => {
                                                let folder_id = args.get("folderId").and_then(|v| v.as_str()).map(|s| s.to_string());
                                                let search_query = args.get("searchQuery").and_then(|v| v.as_str()).map(|s| s.to_string());
                                                
                                                let google_auth_state = app_handle_clone.state::<GoogleAuthState>();
                                                match google_drive::list_drive_files(folder_id, search_query, google_auth_state).await {
                                                    Ok(files) => Ok(serde_json::to_value(files).unwrap()),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            "download_file_content" => {
                                                let file_id = args.get("fileId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let google_auth_state = app_handle_clone.state::<GoogleAuthState>();
                                                match google_drive::download_file_content(file_id, google_auth_state).await {
                                                    Ok(content) => Ok(serde_json::to_value(content).unwrap()),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            "get_drive_file_info" => {
                                                let file_id = args.get("fileId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let google_auth_state = app_handle_clone.state::<GoogleAuthState>();
                                                match google_drive::get_drive_file_info(file_id, google_auth_state).await {
                                                    Ok(file_info) => Ok(serde_json::to_value(file_info).unwrap()),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            "is_google_logged_in" => {
                                                let google_auth_state = app_handle_clone.state::<GoogleAuthState>();
                                                match google_auth::is_google_logged_in(google_auth_state).await {
                                                    Ok(logged_in) => Ok(serde_json::to_value(logged_in).unwrap()),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            "google_logout" => {
                                                let google_auth_state = app_handle_clone.state::<GoogleAuthState>();
                                                match google_auth::google_logout(google_auth_state).await {
                                                    Ok(_) => Ok(serde_json::Value::Null),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            "has_google_credentials" => {
                                                let google_auth_state = app_handle_clone.state::<GoogleAuthState>();
                                                match google_auth::has_google_credentials(google_auth_state).await {
                                                    Ok(has_creds) => Ok(serde_json::to_value(has_creds).unwrap()),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            "get_project_rag_status" => {
                                                let path = args.get("projectPath").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let pool = app_handle_clone.state::<SqlitePool>();
                                                match vector_map_commands::get_project_rag_status(path, pool).await {
                                                    Ok(status) => Ok(serde_json::to_value(status).unwrap()),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            "rag_search_manual" => {
                                                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
                                                let project_path = args.get("project_path").and_then(|v| v.as_str()).map(|s| s.to_string());
                                                let state = app_handle_clone.state::<AppState>();
                                                let pool = app_handle_clone.state::<SqlitePool>();
                                                match vector_map_commands::rag_search_manual(query, top_k, project_path, state, pool).await {
                                                    Ok(results) => Ok(serde_json::to_value(results).unwrap()),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            "get_projects" => {
                                                match get_projects(app_handle_clone.state()).await {
                                                    Ok(projects) => Ok(serde_json::to_value(projects).unwrap()),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            "get_local_ai_status" => {
                                                let state = app_handle_clone.state::<AppState>();
                                                Ok(serde_json::to_value(state.local_ai.is_running()).unwrap())
                                            },
                                            "toggle_local_ai" => {
                                                let enabled = args.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                                                let skip_auto = args.get("skip_auto_index").and_then(|v| v.as_bool());
                                                let pool = app_handle_clone.state::<SqlitePool>();
                                                let state = app_handle_clone.state::<AppState>();

                                                match toggle_local_ai(enabled, skip_auto, state, pool, app_handle_clone.clone()).await {
                                                    Ok(status) => Ok(serde_json::to_value(status).unwrap()),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            "trigger_indexing" => {
                                                // Obsługa prostego triggera z payloadem selectedFiles
                                                let selected_files_val = args.get("selectedFiles");
                                                let selected_files: Vec<SelectedFileInfo> = if let Some(val) = selected_files_val {
                                                    serde_json::from_value(val.clone()).unwrap_or_default()
                                                } else {
                                                    Vec::new()
                                                };

                                                let pool = app_handle_clone.state::<SqlitePool>();
                                                let state = app_handle_clone.state::<AppState>();

                                                match trigger_indexing(vec![], selected_files, pool, state, app_handle_clone.clone()).await {
                                                    Ok(msg) => Ok(serde_json::to_value(msg).unwrap()),
                                                    Err(e) => Err(e)
                                                }
                                            },
                                            _ => Err(format!("Unknown Tauri command: {}", command))
                                        };

                                        // Send response
                                        match result {
                                            Ok(payload) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: command.to_string(),
                                                    payload,
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: serde_json::json!({ "error": e })
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    } else {
                                        println!("[RUST WS] Invalid tauri_invoke payload");
                                        let err_resp = WebSocketResponse {
                                            request_id,
                                            action: "error".to_string(),
                                            payload: serde_json::json!({ "error": "Invalid tauri_invoke payload" })
                                        };
                                        if let Ok(json) = serde_json::to_string(&err_resp) {
                                            send_json(json);
                                        }
                                    }
                                }

                                "get_projects" => {
                                    let projects_result = get_projects(app_handle.state()).await;
                                    match projects_result {
                                        Ok(projects) => {
                                            let response = WebSocketResponse {
                                                request_id,
                                                action: "get_projects".to_string(),
                                                payload: projects,
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                send_json(json);
                                            }
                                        },
                                        Err(e) => {
                                            let err_resp = WebSocketResponse { request_id, action: "error".to_string(), payload: format!("Failed to get projects: {}", e) };
                                            if let Ok(json) = serde_json::to_string(&err_resp) { send_json(json); }
                                        }
                                    }
                                }
 
                                "get_file_as_base64" => {
                                    println!("[RUST LOG] 1/7: Entered 'get_file_as_base64' handler.");
                                    if let Some(filepath_val) = request.payload.get("filepath").and_then(|v| v.as_str()) {
                                        let filepath = PathBuf::from(filepath_val);
                                        println!("[RUST LOG] 2/7: Trying to process file: {:?}", filepath);
 
                                        let result = async {
                                            const MAX_BASE64_SIZE: u64 = 100 * 1024 * 1024; // 100MB limit
 
                                            println!("[RUST LOG] 3/7: Reading metadata...");
                                            let metadata = fs::metadata(&filepath).await.map_err(|e| format!("Metadata error: {}", e))?;
                                            println!("[RUST LOG] 4/7: Metadata OK. Size: {} bytes.", metadata.len());
 
                                            if metadata.len() > MAX_BASE64_SIZE {
                                                return Err(format!("File is too large for upload ({} > 100MB)", metadata.len()));
                                            }
 
                                            println!("[RUST LOG] 5/7: Reading file content...");
                                            let content_bytes = fs::read(&filepath).await.map_err(|e| format!("File read error: {}", e))?;
                                            let base64_content = general_purpose::STANDARD.encode(&content_bytes);
                                            println!("[RUST LOG] 6/7: Content OK. Encoded to {} Base64 chars.", base64_content.len());
 
                                            let mime_type = mime_guess::from_path(&filepath)
                                                .first_or_octet_stream()
                                                .to_string();
 
                                            println!("[RUST DIAGNOSTIC] Detected MIME type: '{}' for file: {:?}", mime_type, filepath.file_name().unwrap_or_default());
                                            println!("[RUST DIAGNOSTIC] Base64 preview: {}...", &base64_content[..std::cmp::min(base64_content.len(), 64)]);
 
                                            let filename = filepath.file_name()
                                                .unwrap_or_default()
                                                .to_string_lossy()
                                                .to_string();
 
                                            Ok::<_, String>(FileAsBase64Response {
                                                filename,
                                                mime_type,
                                                base64_content,
                                            })
                                        }.await;
 
                                        match result {
                                            Ok(payload) => {
                                                println!("[RUST LOG] 7/7: SUCCESS. Sending 'file_as_base64_loaded' response.");
                                                let response = WebSocketResponse { request_id, action: "file_as_base64_loaded".to_string(), payload };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                println!("[RUST LOG] 7/7: FAILED. Sending error response: {}", e);
                                                let err_resp = WebSocketResponse { request_id, action: "error".to_string(), payload: e };
                                                if let Ok(json) = serde_json::to_string(&err_resp) { send_json(json); }
                                            }
                                        }
                                    } else {
                                        println!("[RUST LOG] ERROR: 'filepath' not found in payload for 'get_file_as_base64'.");
                                    }
                                }
 
                                "get_file_trees" => {
                                    if !is_license_valid(pool.inner()).await {
                                        let err_resp = WebSocketResponse { request_id, action: "error".to_string(), payload: "A valid license is required for this feature.".to_string() };
                                        if let Ok(json) = serde_json::to_string(&err_resp) { send_json(json); }
                                        continue;
                                    }
                                    if let Ok(payload) = serde_json::from_value::<GetFileTreesPayload>(request.payload) {
                                        let mut handles = vec![];
                                        for path in payload.paths {
                                            let handle_clone_for_task = app_handle_clone.clone();
                                            let handle = tokio::spawn(async move {
                                                let pool_state = handle_clone_for_task.state::<SqlitePool>();
                                                match get_project_file_tree(path.clone(), handle_clone_for_task.state(), pool_state.clone()).await {
                                                    Ok((tree, file_count)) => ProjectTreeResponse { project_path: path, tree: Some(tree), file_count: Some(file_count), error: None },
                                                    Err(e) => ProjectTreeResponse { project_path: path, tree: None, file_count: None, error: Some(e) },
                                                }
                                            });
                                            handles.push(handle);
                                        }
                                        let results: Vec<_> = futures_util::future::join_all(handles).await.into_iter().filter_map(Result::ok).collect();
                                        let response = WebSocketResponse { request_id, action: "get_file_trees".to_string(), payload: results };
                                        if let Ok(json) = serde_json::to_string(&response) {
                                            send_json(json);
                                        }
                                    }
                                }
 
                                "get_files_multi" => {
                                   if !is_license_valid(pool.inner()).await {
                                        let err_resp = WebSocketResponse { request_id, action: "error".to_string(), payload: "A valid license is required for this feature.".to_string() };
                                        if let Ok(json) = serde_json::to_string(&err_resp) { send_json(json); }
                                        continue;
                                    }
                                   if let Ok(payload) = serde_json::from_value::<GetFilesMultiPayload>(request.payload) {
                                        let mut handles = vec![];
                                        for project_req in payload.projects {
                                            let handle = tokio::spawn(async move {
                                                let files = get_files_content(project_req.root_path.clone(), project_req.relative_paths).await.unwrap_or_else(|e| vec![FileContent { path: "project_level_error".to_string(), content: None, error: Some(e) }]);
                                                GetFilesMultiResponse { project_path: project_req.root_path, files }
                                            });
                                            handles.push(handle);
                                        }
                                        let results: Vec<_> = futures_util::future::join_all(handles).await.into_iter().filter_map(Result::ok).collect();
                                        let response = WebSocketResponse { request_id, action: "get_files_multi".to_string(), payload: results };
                                        if let Ok(json) = serde_json::to_string(&response) {
                                            send_json(json);
                                        }
                                    }
                                }
 
                                "get_environments" => {
                                    let result = async {
                                        let envs = get_environments(pool.clone()).await?;
                                        let mut results = vec![];
                                        for env in envs {
                                            let prompts = get_prompts(env.id, pool.clone()).await?;
                                            results.push(EnvironmentWithPrompts { environment: env, prompts });
                                        }
                                        Ok::<_, String>(results)
                                    }.await;
 
                                    if let Ok(payload) = result {
                                        let response = WebSocketResponse { request_id, action: "get_environments".to_string(), payload };
                                        if let Ok(json) = serde_json::to_string(&response) {
                                            send_json(json);
                                        }
                                    }
                                }
 
                                "get_environment_for_project" => {
                                    if let Some(path_val) = request.payload.get("path").and_then(|v| v.as_str()) {
                                        eprintln!("[Rust Backend] get_environment_for_project for path: {}", path_val);
                                        let result = async {
                                            let project_id: i64 = sqlx::query_scalar("SELECT id FROM projects WHERE path = ?")
                                                .bind(path_val).fetch_one(pool.inner()).await.map_err(sqlx_err)?;
 
                                            eprintln!("[Rust Backend] Found project_id: {}", project_id);
 
                                            let env_id = sqlx::query_scalar("SELECT environment_id FROM project_environment WHERE project_id = ?")
                                                .bind(project_id).fetch_one(pool.inner()).await.unwrap_or(1);
 
                                            eprintln!("[Rust Backend] Using environment_id: {}", env_id);
 
                                            let env = sqlx::query_as("SELECT * FROM environments WHERE id = ?").bind(env_id).fetch_one(pool.inner()).await.map_err(sqlx_err)?;
                                            let prompts = get_prompts(env_id, pool.clone()).await?;
 
                                            Ok::<_, String>(EnvironmentWithPrompts { environment: env, prompts })
                                        }.await;
 
                                        match result {
                                            Ok(payload) => {
                                                eprintln!("[Rust Backend] Sending environment: {}", payload.environment.name);
                                                let response = WebSocketResponse { request_id, action: "get_environment_for_project".to_string(), payload };
                                                if let Ok(json) = serde_json::to_string(&response) { send_json(json); }
                                            },
                                            Err(e) => {
                                                eprintln!("[Rust Backend] Error getting project environment: {} - will reset to default", e);
                                                // Send empty response to trigger default environment reset
                                                let response = WebSocketResponse { request_id, action: "get_environment_for_project".to_string(), payload: serde_json::json!({}) };
                                                if let Ok(json) = serde_json::to_string(&response) { send_json(json); }
                                            }
                                        }
                                    }
                                }
 
                                "get_context_files_history" => {
                                    // Deserializuj payload z opcjonalnymi selectedProjects
                                    let payload: GetContextHistoryPayload = serde_json::from_value(request.payload)
                                        .unwrap_or(GetContextHistoryPayload { selected_projects: Vec::new() });
 
                                    let state = app_handle_clone.state::<AppState>();
                                    let context_cache = state.context_files_cache.clone();
 
                                    let result = async {
                                        // Wczytaj ulubione
                                        let favorites = ContextFavorites::load(&app_handle_clone).await.unwrap_or_default();
 
                                        let projects: Vec<Project> = sqlx::query_as("SELECT id, path, excluded_paths, download_path, allowed_extensions, vector_map_id FROM projects")
                                            .fetch_all(pool.inner())
                                            .await
                                            .map_err(sqlx_err)?;
 
                                        // Filtruj projekty jeśli selectedProjects nie jest puste
                                        let projects_to_scan: Vec<Project> = if payload.selected_projects.is_empty() {
                                            projects
                                        } else {
                                            projects.into_iter()
                                                .filter(|p| payload.selected_projects.contains(&p.path))
                                                .collect()
                                        };
 
                                        let mut history: Vec<ContextHistoryItem> = Vec::new();
                                        let mut scanned_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();
                                        let mut found_files_on_disk: HashSet<PathBuf> = HashSet::new();
 
                                        async fn scan_directory(
                                            dir: &PathBuf,
                                            history: &mut Vec<ContextHistoryItem>,
                                            favorites: &ContextFavorites,
                                            cache: &Arc<TokioMutex<HashMap<PathBuf, (ContextConfig, SystemTime)>>>,
                                            found_files: &mut HashSet<PathBuf>
                                        ) -> Result<(), String> {
                                            if let Ok(entries) = fs::read_dir(dir).await {
                                                let mut entries_stream = entries;
                                                while let Ok(Some(entry)) = entries_stream.next_entry().await {
                                                    if let Ok(filename) = entry.file_name().into_string() {
                                                        if filename.ends_with(".txt") {
                                                            let filepath = entry.path();
 
                                                            if let Ok(metadata) = fs::metadata(&filepath).await {
                                                                let modified_time = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                                                                let mut cache_hit = false;
 
                                                                {
                                                                    let cache_lock = cache.lock().await;
                                                                    if let Some((cached_config, cached_time)) = cache_lock.get(&filepath) {
                                                                        if *cached_time == modified_time {
                                                                            let file_count: usize = cached_config.selected_files.values().map(|v| v.len()).sum();
                                                                            let project_count = cached_config.projects.len();
                                                                            let is_favorite = favorites.is_favorite(&filepath.to_string_lossy().to_string());
 
                                                                            history.push(ContextHistoryItem {
                                                                                filename: filename.clone(),
                                                                                filepath: filepath.to_string_lossy().to_string(),
                                                                                timestamp: cached_config.timestamp.clone(),
                                                                                size: metadata.len(),
                                                                                file_count,
                                                                                project_count,
                                                                                favorite: is_favorite,
                                                                                config: cached_config.clone(),
                                                                            });
                                                                            cache_hit = true;
                                                                        }
                                                                    }
                                                                }
 
                                                                if !cache_hit {
                                                                    if let Ok(content) = fs::read_to_string(&filepath).await {
                                                                        if is_gluon_context_file(&content) {
                                                                            if let Some(config) = extract_config_from_content(&content) {
                                                                                {
                                                                                    let mut cache_lock = cache.lock().await;
                                                                                    cache_lock.insert(filepath.clone(), (config.clone(), modified_time));
                                                                                }
 
                                                                                let file_count: usize = config.selected_files.values().map(|v| v.len()).sum();
                                                                                let project_count = config.projects.len();
                                                                                let is_favorite = favorites.is_favorite(&filepath.to_string_lossy().to_string());
 
                                                                                history.push(ContextHistoryItem {
                                                                                    filename: filename.clone(),
                                                                                    filepath: filepath.to_string_lossy().to_string(),
                                                                                    timestamp: config.timestamp.clone(),
                                                                                    size: metadata.len(),
                                                                                    file_count,
                                                                                    project_count,
                                                                                    favorite: is_favorite,
                                                                                    config,
                                                                                });
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                found_files.insert(filepath);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Ok(())
                                        }
 
                                        for project in &projects_to_scan {
                                            let download_dir = if let Some(ref dl_path) = project.download_path {
                                                PathBuf::from(dl_path)
                                            } else {
                                                PathBuf::from(&project.path)
                                            };
 
                                            let dir_str = download_dir.to_string_lossy().to_string();
                                            if !scanned_dirs.contains(&dir_str) {
                                                scanned_dirs.insert(dir_str);
                                                let _ = scan_directory(&download_dir, &mut history, &favorites, &context_cache, &mut found_files_on_disk).await;
                                            }
                                        }
 
                                        // Skanuj globalny folder TYLKO jeśli selectedProjects jest puste
                                        if payload.selected_projects.is_empty() {
                                            let global_dir = app_handle_clone.path().download_dir().map_err(|e| e.to_string())?;
                                            let global_dir_str = global_dir.to_string_lossy().to_string();
                                            if !scanned_dirs.contains(&global_dir_str) {
                                                let _ = scan_directory(&global_dir, &mut history, &favorites, &context_cache, &mut found_files_on_disk).await;
                                            }
                                        }
 
                                        {
                                            let mut cache_lock = context_cache.lock().await;
                                            let initial_size = cache_lock.len();
                                            cache_lock.retain(|path, _| found_files_on_disk.contains(path));
                                            let final_size = cache_lock.len();
                                            if initial_size != final_size {
                                                println!(">>> Context cache cleanup: Removed {} stale entries.", initial_size - final_size);
                                            }
                                        }
 
                                        // Sortuj: ulubione na górze, potem po timestampie
                                        history.sort_by(|a, b| {
                                            match (a.favorite, b.favorite) {
                                                (true, false) => Ordering::Less,
                                                (false, true) => Ordering::Greater,
                                                _ => b.timestamp.cmp(&a.timestamp)
                                            }
                                        });
 
                                        // println!(">>> Found {} context files total", history.len());
                                        Ok::<Vec<ContextHistoryItem>, String>(history)
                                    }.await;
 
                                    match result {
                                        Ok(history) => {
                                            // println!(">>> Found {} context files", history.len());
                                            let response = WebSocketResponse {
                                                request_id,
                                                action: "get_context_files_history".to_string(),
                                                payload: history,
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                send_json(json);
                                            }
                                        },
                                        Err(e) => {
                                            println!(">>> ERROR: {}", e);
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Failed to get history: {}", e),
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }
 
                                "get_context_file_content" => {
                                    if let Some(filepath) = request.payload.get("filepath").and_then(|v| v.as_str()) {
                                        match fs::read_to_string(filepath).await {
                                            Ok(content) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "context_file_content_loaded".to_string(),
                                                    payload: serde_json::json!({
                                                        "filepath": filepath,
                                                        "content": content
                                                    }),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to read file: {}", e),
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }
 
                                "toggle_context_favorite" => {
                                    println!(">>> Toggle favorite request");
                                    if let Ok(payload) = serde_json::from_value::<ToggleFavoritePayload>(request.payload) {
                                        let result = async {
                                            let mut favorites = ContextFavorites::load(&app_handle_clone).await.unwrap_or_default();
                                            favorites.set_favorite(payload.filepath.clone(), payload.favorite);
                                            favorites.save(&app_handle_clone).await?;
 
                                            println!(">>> Favorite status updated for: {}", payload.filepath);
                                            Ok::<(), String>(())
                                        }.await;
 
                                        match result {
                                            Ok(_) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "toggle_context_favorite".to_string(),
                                                    payload: serde_json::json!({ "success": true }),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                println!(">>> ERROR: {}", e);
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to toggle favorite: {}", e),
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }
 
                                "rename_context_file" => {
                                    println!(">>> Rename file request");
                                    if let Ok(payload) = serde_json::from_value::<RenameFilePayload>(request.payload) {
                                        let result = async {
                                            let old_path = PathBuf::from(&payload.filepath);
 
                                            // Walidacja nazwy - tylko rozszerzenie .txt
                                            if !payload.new_name.ends_with(".txt") {
                                                return Err("Filename must end with .txt".to_string());
                                            }
 
                                            // Walidacja znaków w nazwie (bez ograniczenia prefiksu)
                                            let name_without_ext = payload.new_name.trim_end_matches(".txt");
                                            if !name_without_ext.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ' ') {
                                                return Err("Filename can only contain letters, numbers, spaces, - and _".to_string());
                                            }
 
                                            // Sprawdź czy stary plik istnieje
                                            if !old_path.exists() {
                                                return Err(format!("File not found: {}", payload.filepath));
                                            }
 
                                            // Utwórz nową ścieżkę (ten sam katalog, nowa nazwa)
                                            let parent_dir = old_path.parent().ok_or("Cannot determine parent directory")?;
                                            let new_path = parent_dir.join(&payload.new_name);
 
                                            // Sprawdź czy nowa nazwa już istnieje
                                            if new_path.exists() {
                                                return Err(format!("File '{}' already exists", payload.new_name));
                                            }
 
                                            // Zmień nazwę pliku
                                            fs::rename(&old_path, &new_path).await.map_err(|e| e.to_string())?;
 
                                            // Zaktualizuj ulubione
                                            let mut favorites = ContextFavorites::load(&app_handle_clone).await.unwrap_or_default();
                                            if let Some(was_favorite) = favorites.favorites.remove(&payload.filepath) {
                                                favorites.favorites.insert(new_path.to_string_lossy().to_string(), was_favorite);
                                                favorites.save(&app_handle_clone).await?;
                                            }
 
                                            println!(">>> File renamed: {} -> {}", payload.filepath, payload.new_name);
                                            Ok::<String, String>(new_path.to_string_lossy().to_string())
                                        }.await;
 
                                        match result {
                                            Ok(new_filepath) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "rename_context_file".to_string(),
                                                    payload: serde_json::json!({
                                                        "success": true,
                                                        "newFilepath": new_filepath
                                                    }),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                println!(">>> ERROR: {}", e);
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to rename file: {}", e),
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }
 
                                "generate_context_file" => {
                                    if !is_license_valid(pool.inner()).await {
                                        let err_resp = WebSocketResponse { request_id, action: "error".to_string(), payload: "A valid license is required for this feature.".to_string() };
                                        if let Ok(json) = serde_json::to_string(&err_resp) { send_json(json); }
                                        continue;
                                    }
                                    match serde_json::from_value::<GenerateContextFilePayload>(request.payload) {
                                        Ok(payload) => {
                                            match generate_context_file(app_handle_clone.clone(), request_id.clone(), payload).await {
                                                Ok(metadata) => {
                                                    let response = WebSocketResponse {
                                                        request_id,
                                                        action: "context_file_generated".to_string(),
                                                        payload: metadata,
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&response) {
                                                        send_json(json);
                                                    }
                                                }
                                                Err(e) => {
                                                    let err_resp = WebSocketResponse {
                                                        request_id,
                                                        action: "error".to_string(),
                                                        payload: format!("Failed to generate context file: {}", e),
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&err_resp) {
                                                        send_json(json);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Invalid payload: {}", e),
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "save_semantic_map" => {
                                    #[derive(serde::Deserialize)]
                                    struct SaveSemanticMapPayload {
                                        filename: String,
                                        content: String,
                                        #[serde(rename = "projectRoot")]
                                        project_root: Option<String>,
                                    }

                                    match serde_json::from_value::<SaveSemanticMapPayload>(request.payload) {
                                        Ok(payload) => {
                                            // Determine the directory to save the file
                                            let context_dir = if let Some(root) = payload.project_root {
                                                PathBuf::from(root).join("gluon_context")
                                            } else {
                                                PathBuf::from("gluon_context")
                                            };

                                            // Create directory if it doesn't exist
                                            if let Err(e) = std::fs::create_dir_all(&context_dir) {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to create context directory: {}", e),
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                                continue;
                                            }

                                            // Write the semantic map file
                                            let filepath = context_dir.join(&payload.filename);
                                            match std::fs::write(&filepath, payload.content) {
                                                Ok(_) => {
                                                    let response = WebSocketResponse {
                                                        request_id,
                                                        action: "save_semantic_map".to_string(),
                                                        payload: serde_json::json!({
                                                            "success": true,
                                                            "filepath": filepath.to_string_lossy(),
                                                            "filename": payload.filename
                                                        }),
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&response) {
                                                        send_json(json);
                                                    }
                                                }
                                                Err(e) => {
                                                    let err_resp = WebSocketResponse {
                                                        request_id,
                                                        action: "error".to_string(),
                                                        payload: format!("Failed to write semantic map file: {}", e),
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&err_resp) {
                                                        send_json(json);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Invalid save_semantic_map payload: {}", e),
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "cancel_context_generation" => {
                                    #[derive(serde::Deserialize)]
                                    struct CancelPayload {
                                        request_id: String,
                                    }

                                    match serde_json::from_value::<CancelPayload>(request.payload) {
                                        Ok(cancel_payload) => {
                                            if let Some(token) = CANCEL_TOKENS.lock().unwrap().get(&cancel_payload.request_id) {
                                                token.store(true, AtomicOrdering::Relaxed);

                                                // Emit event that cancellation occurred
                                                let _ = app_handle_clone.emit("context_generation_cancelled", serde_json::json!({
                                                    "request_id": cancel_payload.request_id,
                                                    "message": "Generation cancelled by user"
                                                }));

                                                let response = WebSocketResponse {
                                                    request_id: request_id.clone(),
                                                    action: "context_generation_cancelled".to_string(),
                                                    payload: serde_json::json!({
                                                        "cancelled_request_id": cancel_payload.request_id,
                                                        "message": "Generation cancelled successfully"
                                                    }),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            } else {
                                                // Request not found or already completed
                                                let err_resp = WebSocketResponse {
                                                    request_id: request_id.clone(),
                                                    action: "error".to_string(),
                                                    payload: "Cannot cancel: request not found or already completed".to_string(),
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Invalid cancel payload: {}", e),
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "get_license_status" => {
                                    let result = async {
                                        let theme_name = get_setting("ui_theme".to_string(), pool.clone()).await?.unwrap_or_else(|| "gluon-v2".to_string());
                                        let color1 = get_setting("theme_80s_color1".to_string(), pool.clone()).await?.unwrap_or_else(|| "#170b01".to_string());
                                        let color2 = get_setting("theme_80s_color2".to_string(), pool.clone()).await?.unwrap_or_else(|| "#ff3300".to_string());

                                        Ok::<StatusPayload, String>(StatusPayload {
                                            status: "VALID".to_string(),
                                            theme_name,
                                            theme_80s_color1: color1,
                                            theme_80s_color2: color2,
                                        })
                                    }.await;
 
                                    match result {
                                        Ok(payload) => {
                                            let response = WebSocketResponse {
                                                request_id,
                                                action: "get_license_status".to_string(),
                                                payload,
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                send_json(json);
                                            }
                                        },
                                        Err(e) => {
                                            let err_resp = WebSocketResponse { request_id, action: "error".to_string(), payload: format!("Failed to get status: {}", e) };
                                            if let Ok(json) = serde_json::to_string(&err_resp) { send_json(json); }
                                        }
                                    }
                                }
 
                                "resolve_change_locations" => {
                                    if let Ok(payload) = serde_json::from_value::<Vec<apply_system::tauri_commands::LocationRequest>>(request.payload) {
                                        let result = apply_system::tauri_commands::resolve_change_locations(payload).await;
                                        match result {
                                            Ok(locations) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "change_locations_resolved".to_string(),
                                                    payload: locations,
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            }
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to resolve locations: {}", e),
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                "process_dom_stream" => {
                                    println!("[RUST WS] 🔍 Received 'process_dom_stream' from Extension");
                                    
                                    #[derive(serde::Deserialize)]
                                    struct DomPayload {
                                        html: String,
                                        provider: String,
                                        #[serde(default)]
                                        workspace_root: Option<String>, // Aktywny workspace z VS Code
                                    }

                                    if let Ok(payload) = serde_json::from_value::<DomPayload>(request.payload) {
                                        let editor_bridge = app_handle_clone.state::<EditorBridge>();
                                        let state = app_handle_clone.state::<ApplySystemState>();

                                        // 1. Pobierz listę projektów - PRIORYTET: Aktywne workspace z VS Code
                                        let mut project_paths: Vec<String> = Vec::new();

                                        // 1a. Jeśli VS Code podało explicit workspace_root, użyj go jako pierwszy
                                        if let Some(ref ws_root) = payload.workspace_root {
                                            println!("[RUST WS] Using explicit workspace_root from extension: {}", ws_root);
                                            project_paths.push(ws_root.clone());
                                        }

                                        // 1b. Dodaj wszystkie aktywne workspace z VS Code (z EditorBridge)
                                        let active_workspaces = editor_bridge.get_all_roots();
                                        if !active_workspaces.is_empty() {
                                            println!("[RUST WS] Found {} active VS Code workspace(s)", active_workspaces.len());

                                            // Filtruj workspace Gluon-v2 jeśli są inne dostępne (heurystyka: nie tworzymy plików w samym Gluon)
                                            let mut filtered_workspaces: Vec<String> = active_workspaces
                                                .iter()
                                                .filter(|ws| !ws.contains("Gluon-v2"))
                                                .cloned()
                                                .collect();

                                            // Jeśli wszystkie zostały odfiltrowane (tylko Gluon-v2), użyj wszystkich
                                            if filtered_workspaces.is_empty() {
                                                filtered_workspaces = active_workspaces;
                                            }

                                            if filtered_workspaces.len() > 1 {
                                                println!("[RUST WS] ⚠️ Multiple workspaces active (filtered). Priority order:");
                                                for (i, ws) in filtered_workspaces.iter().enumerate() {
                                                    println!("[RUST WS]   {}. {}", i + 1, ws);
                                                }
                                            }

                                            for ws in filtered_workspaces {
                                                if !project_paths.contains(&ws) {
                                                    project_paths.push(ws);
                                                }
                                            }
                                        }

                                        // 1c. Dodaj projekty z bazy danych jako fallback
                                        let db_projects: Vec<String> = sqlx::query_scalar("SELECT path FROM projects")
                                            .fetch_all(pool.inner())
                                            .await
                                            .unwrap_or_default();
                                        for proj in db_projects {
                                            if !project_paths.contains(&proj) {
                                                project_paths.push(proj);
                                            }
                                        }

                                        println!("[RUST WS] Loaded {} project roots for resolution (priority: active workspaces).", project_paths.len());

                                        // 2. Ekstrakcja bloków kodu
                                        let code_blocks = HtmlParser::extract(&payload.html, &payload.provider);
                                        println!("[RUST WS] Extracted {} code blocks", code_blocks.len());

                                        // Kolejka zmian do zaaplikowania
                                        let mut changes_to_apply = Vec::new();
                                        let mut files_processed = Vec::new();
                                        let mut processing_reports = Vec::new();

                                        for block in code_blocks {
                                            let content = &block.content;
                                            
                                            // A. Zidentyfikuj plik docelowy
                                            let target_path = {
                                                let map = state.repo_map.lock().unwrap();
                                                map.find_path_for_snippet(content, &project_paths)
                                            };

                                            if let Some(path) = target_path {
                                                let path_str = path.to_string_lossy().to_string();
                                                println!("[Path Resolver] Resolved snippet to: {}", path_str);

                                                // Sprawdź czy plik istnieje - jeśli nie, utwórz katalog nadrzędny
                                                if !path.exists() {
                                                    println!("[Path Resolver] 📝 Creating new file: {}", path_str);
                                                    if let Some(parent) = path.parent() {
                                                        if !parent.exists() {
                                                            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                                                                println!("[Path Resolver] ❌ Failed to create directory {:?}: {}", parent, e);
                                                                processing_reports.push(format!("Failed to create directory for {}: {}", path_str, e));
                                                                continue;
                                                            }
                                                            println!("[Path Resolver] ✅ Created directory: {:?}", parent);
                                                        }
                                                    }
                                                }

                                                // B. Użyj standardowych parserów do wydobycia zmian
                                                // To unifikuje logikę z resztą systemu (ChangeQueue)
                                                use crate::apply_system::parsers::coordinator::parse_with_fallback;
                                                
                                                match parse_with_fallback(content) {
                                                    Ok(mut parsed_changes) => {
                                                        // Przypisz ścieżkę (bo parser może jej nie mieć, jeśli snippet to tylko kod)
                                                        for change in &mut parsed_changes {
                                                            // Jeśli parser nie wykrył ścieżki (np. zwykły blok kodu), nadpisz ją wykrytą przez Resolver
                                                            if change.file_path.is_empty() || !change.file_path.contains(std::path::MAIN_SEPARATOR) {
                                                                change.file_path = path_str.clone();
                                                            }
                                                            // Ważne: zaktualizuj też metadane matchowania dla tej ścieżki
                                                            change.matching_data = crate::apply_system::matchers::anchor_extraction::extract_matching_data(&change.old_code, "");
                                                        }
                                                        changes_to_apply.extend(parsed_changes);
                                                        files_processed.push(path_str);
                                                    },
                                                    Err(e) => {
                                                        // Fallback: Jeśli parser zawiódł, sprawdź czy to może być nowy plik
                                                        println!("[RUST WS] Standard parse failed: {:?}", e);

                                                        // Jeśli plik nie istnieje, utwórz ChangeQueueItem dla nowego pliku
                                                        if !path.exists() {
                                                            println!("[RUST WS] 📝 Creating new file change item for: {}", path_str);

                                                            // Wydobądź czysty kod (usuń marker pliku i markdown fences)
                                                            let clean_content = content
                                                                .lines()
                                                                .filter(|line| {
                                                                    let trimmed = line.trim();
                                                                    // Pomiń marker pliku
                                                                    if trimmed.starts_with("# Plik:") ||
                                                                       trimmed.starts_with("# File:") ||
                                                                       trimmed.starts_with("// Plik:") ||
                                                                       trimmed.starts_with("// File:") {
                                                                        return false;
                                                                    }
                                                                    // Pomiń markdown fences
                                                                    if trimmed.starts_with("```") {
                                                                        return false;
                                                                    }
                                                                    true
                                                                })
                                                                .collect::<Vec<_>>()
                                                                .join("\n");

                                                            if !clean_content.trim().is_empty() {
                                                                use crate::apply_system::ChangeQueueItem;
                                                                let new_file_change = ChangeQueueItem::new(
                                                                    path_str.clone(),
                                                                    1,
                                                                    1,
                                                                    String::new(), // old_code pusty dla nowego pliku
                                                                    clean_content
                                                                );
                                                                changes_to_apply.push(new_file_change);
                                                                files_processed.push(path_str.clone());
                                                                println!("[RUST WS] ✅ Created new file change item");
                                                            } else {
                                                                processing_reports.push(format!("Empty content for new file: {}", path_str));
                                                            }
                                                        } else {
                                                            processing_reports.push(format!("Could not parse changes for {}: {:?}", path_str, e));
                                                        }
                                                    }
                                                }
                                            } else {
                                                println!("[Path Resolver] ⚠️ Could not identify target file for block.");
                                            }
                                        }
                                        // 3. Połącz zmiany dla nowych plików
                                        // PatternMatchingParser może podzielić kod na wiele fragmentów,
                                        // ale dla nowego pliku chcemy jeden change z całą zawartością
                                        use std::collections::HashMap;
                                        let mut file_changes_map: HashMap<String, Vec<crate::apply_system::ChangeQueueItem>> = HashMap::new();

                                        for change in changes_to_apply {
                                            file_changes_map.entry(change.file_path.clone())
                                                .or_insert_with(Vec::new)
                                                .push(change);
                                        }

                                        let mut merged_changes = Vec::new();
                                        for (file_path, mut file_changes) in file_changes_map {
                                            // Sortuj wg line_start
                                            file_changes.sort_by_key(|c| c.line_start);

                                            // Sprawdź czy plik istnieje na dysku (fizycznie, nie w pamięci)
                                            let path_for_check = std::path::Path::new(&file_path);
                                            let is_new_file = !path_for_check.exists();

                                            println!("[RUST WS] File: {}, exists: {}, fragments: {}",
                                                file_path, path_for_check.exists(), file_changes.len());

                                            if is_new_file && file_changes.len() > 1 {
                                                println!("[RUST WS] 🔗 Merging {} fragments for new file: {}", file_changes.len(), file_path);

                                                // Dla nowego pliku, połącz ALL fragmenty (zarówno old_code jak i new_code)
                                                // PatternMatchingParser może błędnie podzielić kod na pary, ale my chcemy wszystko
                                                let mut all_content_parts = Vec::new();
                                                for change in &file_changes {
                                                    // Dodaj old_code jeśli nie jest pusty (parser mógł coś tam wrzucić)
                                                    if !change.old_code.trim().is_empty() {
                                                        all_content_parts.push(change.old_code.as_str());
                                                    }
                                                    // Dodaj new_code
                                                    if !change.new_code.trim().is_empty() {
                                                        all_content_parts.push(change.new_code.as_str());
                                                    }
                                                }

                                                let merged_content = all_content_parts.join("\n");

                                                use crate::apply_system::ChangeQueueItem;
                                                let merged_change = ChangeQueueItem::new(
                                                    file_path.clone(),
                                                    1,
                                                    1,
                                                    String::new(), // old_code pusty - to nowy plik
                                                    merged_content
                                                );
                                                merged_changes.push(merged_change);
                                                println!("[RUST WS] ✅ Created merged change for new file");
                                            } else {
                                                // Dla istniejących plików lub pojedynczych zmian, zachowaj jak jest
                                                merged_changes.extend(file_changes);
                                            }
                                        }

                                        let changes_to_apply = merged_changes;

                                        // 4. Aplikuj zmiany używając standardowego apply_change_command
                                        // To zapewni obsługę Paska Postępu (Pulse), Snapshotów i Bezpieczeństwa
                                        let mut change_manifests = Vec::new();
                                        let batch_id = uuid::Uuid::new_v4().to_string();

                                        if !changes_to_apply.is_empty() {
                                            println!("[RUST WS] 🚀 Queuing {} changes for application via Transaction System...", changes_to_apply.len());

                                            // Generuj manifest dla frontendu PRZED przeniesieniem własności
                                            for change in &changes_to_apply {
                                                change_manifests.push(serde_json::json!({
                                                    "id": change.id,
                                                    "line_start": change.line_start,
                                                    "line_end": change.line_end,
                                                    "type": if change.old_code.is_empty() { "create" } else { "replace" }
                                                }));
                                            }

                                            // Dodaj do globalnej kolejki (wymagane przez apply_change_command)
                                            {
                                                let mut queue = state.change_queue.lock().unwrap();
                                                queue.append(&mut changes_to_apply.clone());
                                            }

                                            let app_handle_for_task = app_handle.clone();
                                            let parent_request_id = request.id.clone(); // PRZEKAZUJEMY ID

                                            tokio::spawn(async move {
                                                let state_for_task = app_handle_for_task.state::<ApplySystemState>();

                                                for item in changes_to_apply {
                                                    // Check if request has been cancelled
                                                    {
                                                        let cancelled = state_for_task.cancelled_requests.lock().unwrap();
                                                        if cancelled.contains(&parent_request_id) {
                                                            println!("[Auto-Apply] 🛑 Request {} cancelled, stopping processing", parent_request_id);
                                                            // Clean up the cancelled flag
                                                            drop(cancelled);
                                                            let mut cancelled_mut = state_for_task.cancelled_requests.lock().unwrap();
                                                            cancelled_mut.remove(&parent_request_id);
                                                            break;
                                                        }
                                                    }

                                                    // Odczytaj aktualną zawartość (potrzebne dla apply_change_command)
                                                    let file_content = match tokio::fs::read_to_string(&item.file_path).await {
                                                        Ok(c) => c,
                                                        Err(_) => String::new(), // Create new file handling
                                                    };

                                                    // WYWOŁAJ STANDARDOWĄ KOMENDĘ Z PRZEKAZANYM REQUEST ID
                                                    // To sprawi, że frontend otrzyma eventy postępu!
                                                    match crate::apply_system::tauri_commands::apply_change_command(
                                                        item.id.clone(),
                                                        file_content,
                                                        Some(parent_request_id.clone()), // <--- FIX: Przekazujemy ID!
                                                        state_for_task.clone(),
                                                        app_handle_for_task.clone()
                                                    ).await {
                                                        Ok(_) => println!("[Auto-Apply] ✅ Successfully applied change {}", item.id),
                                                        Err(e) => eprintln!("[Auto-Apply] ❌ Failed to apply change {}: {}", item.id, e),
                                                    }

                                                    // Krótkie opóźnienie dla płynności UI
                                                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                                                }
                                            });
                                        }

                                        let response = WebSocketResponse {
                                            request_id,
                                            action: "process_dom_stream_result".to_string(),
                                            payload: serde_json::json!({
                                                "success": true,
                                                "batch_id": batch_id,
                                                "processed_files": files_processed,
                                                "changes": change_manifests,
                                                "details": processing_reports
                                            }),
                                        };
                                        if let Ok(json) = serde_json::to_string(&response) {
                                            send_json(json);
                                        }
                                    } else {
                                        println!("[RUST WS] ❌ Failed to deserialize process_dom_stream payload");
                                    }
                                }

                                "cancel_processing" => {
                                    println!("[RUST WS] 🛑 Received 'cancel_processing' request");

                                    #[derive(serde::Deserialize)]
                                    struct CancelPayload {
                                        request_id: String,
                                    }

                                    if let Ok(payload) = serde_json::from_value::<CancelPayload>(request.payload) {
                                        println!("[RUST WS] Attempting to cancel request: {}", payload.request_id);

                                        // Add the request ID to the cancelled set
                                        let state = app_handle_clone.state::<ApplySystemState>();
                                        {
                                            let mut cancelled = state.cancelled_requests.lock().unwrap();
                                            cancelled.insert(payload.request_id.clone());
                                            println!("[RUST WS] ✅ Request {} marked as cancelled", payload.request_id);
                                        }

                                        let response = WebSocketResponse {
                                            request_id,
                                            action: "cancel_processing_result".to_string(),
                                            payload: serde_json::json!({
                                                "success": true,
                                                "message": format!("Cancellation requested for {}", payload.request_id)
                                            }),
                                        };
                                        if let Ok(json) = serde_json::to_string(&response) {
                                            send_json(json);
                                        }
                                    } else {
                                        println!("[RUST WS] ❌ Failed to deserialize cancel_processing payload");
                                    }
                                }

                                "apply_code_changes" => {
                                    println!("[RUST WS] 🔍 Received 'apply_code_changes'. Raw payload: {:?}", request.payload);
 
                                    #[derive(serde::Deserialize, Debug)]
                                    #[serde(rename_all = "camelCase")]
                                    struct IncomingChange {
                                        id: Option<String>,
                                        file_path: String,
                                        old_code: Option<String>,
                                        new_code: String,
                                        line_start: Option<usize>,
                                        line_end: Option<usize>,
                                    }
 
                                    #[derive(serde::Deserialize, Debug)]
                                    #[serde(rename_all = "camelCase")]
                                    struct ApplyCodeChangesPayload {
                                        changes: Vec<IncomingChange>,
                                        #[serde(default)]
                                        selected_projects: Vec<String>,
                                    }
 
                                    match serde_json::from_value::<ApplyCodeChangesPayload>(request.payload) {
                                        Ok(payload) => {
                                            println!("[RUST WS] ✅ Payload deserialized successfully. Changes count: {}", payload.changes.len());
                                            let apply_state = app_handle.state::<ApplySystemState>();
                                            
                                            // 1. Walidacja i przygotowanie (Security)
                                            // Klonujemy config, żeby nie trzymać blokady podczas operacji async
                                            let path_config = {
                                                let config = apply_state.config.lock().unwrap();
                                                config.path_config.clone()
                                            };
                                            
                                            let mut validated_items = Vec::new();
                                            let mut rejected_items = Vec::new();
                                            
                                            // A. Pobierz listę projektów z bazy danych, aby rozwiązać ścieżki względne
                                            let registered_projects: Vec<String> = sqlx::query_scalar("SELECT path FROM projects")
                                                .fetch_all(pool.inner())
                                                .await
                                                .unwrap_or_default();
 
                                            // Priorytetyzacja: Najpierw selected_projects, potem pozostałe
                                            let mut prioritized_projects = Vec::new();
 
                                            // 1. Dodaj wybrane projekty (jeśli są)
                                            let effective_selected_projects = if !payload.selected_projects.is_empty() {
                                                payload.selected_projects.clone()
                                            } else {
                                                // Fallback: Use active EditorBridge roots if selected_projects is empty
                                                // This ensures files are created in the project currently open in the editor
                                                let editor_bridge = app_handle_clone.state::<EditorBridge>();
                                                editor_bridge.get_all_roots()
                                            };

                                            if !effective_selected_projects.is_empty() {
                                                for selected in &effective_selected_projects {
                                                    if registered_projects.contains(selected) {
                                                        prioritized_projects.push(selected.clone());
                                                    }
                                                }
                                            }
 
                                            // 2. Dodaj pozostałe projekty
                                            for proj in &registered_projects {
                                                if !prioritized_projects.contains(proj) {
                                                    prioritized_projects.push(proj.clone());
                                                }
                                            }
 
                                            for change in payload.changes {
                                                // B. Rozwiązywanie ścieżki (Path Resolution) z Heurystyką
                                                let mut resolved_path = change.file_path.clone();
                                                let path_obj = Path::new(&change.file_path);
 
                                                // Normalizacja separatorów dla Windows (zamiana / na \)
                                                let normalized_relative_path = change.file_path.replace('/', std::path::MAIN_SEPARATOR_STR);
 
                                                // Jeśli ścieżka nie jest absolutna i nie istnieje fizycznie
                                                if !path_obj.is_absolute() && !path_obj.exists() {
                                                    let mut best_candidate = None;
                                                    let mut best_score = -1; // -1: brak, 0: root only, 1: parent exists, 2: exact file, 3+: w selected_projects
                                                    let mut is_selected_project = false;
 
                                                    for proj_root in &prioritized_projects {
                                                        let candidate = Path::new(proj_root).join(&normalized_relative_path);
                                                        let is_selected = !payload.selected_projects.is_empty() &&
                                                                        payload.selected_projects.contains(proj_root);
 
                                                        // PRIORYTET 1: Plik istnieje (Edycja)
                                                        if candidate.exists() {
                                                            // Boost score if project is in selected_projects
                                                            let score = if is_selected { 4 } else { 2 };
 
                                                            if score > best_score {
                                                                best_candidate = Some(candidate.to_string_lossy().to_string());
                                                                best_score = score;
                                                                is_selected_project = is_selected;
                                                            }
 
                                                            // Jeśli znaleziono w wybranym projekcie, to idealne dopasowanie
                                                            if is_selected {
                                                                break;
                                                            }
                                                        }
 
                                                        // PRIORYTET 2: Operacja CREATE (old_code puste)
                                                        if change.old_code.as_deref().unwrap_or("").is_empty() {
                                                            if let Some(parent) = candidate.parent() {
                                                                if parent.exists() {
                                                                    // Score: 3 dla selected, 1 dla innych
                                                                    let score = if is_selected { 3 } else { 1 };
 
                                                                    if score > best_score {
                                                                        best_score = score;
                                                                        best_candidate = Some(candidate.to_string_lossy().to_string());
                                                                        is_selected_project = is_selected;
                                                                    }
                                                                } else {
                                                                    // Score 0: Katalog nie istnieje -> Mniej prawdopodobny, ale możliwy (nowe drzewo folderów)
                                                                    if best_score < 0 {
                                                                        best_score = 0;
                                                                        best_candidate = Some(candidate.to_string_lossy().to_string());
                                                                        is_selected_project = is_selected;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
 
                                                    if let Some(best) = best_candidate {
                                                        resolved_path = best;
                                                        let selected_marker = if is_selected_project { " [SELECTED]" } else { "" };
                                                        println!("[Path Resolver] Resolved '{}' -> '{}' (Score: {}{})",
                                                            change.file_path, resolved_path, best_score, selected_marker);
                                                    }
                                                }
 
                                                // C. Walidacja i tworzenie obiektu (używając resolved_path)
                                                match crate::apply_system::shared::config::validate_file_path(&resolved_path, &path_config) {
                                                    Ok(()) => {
                                                        let old_code = change.old_code.unwrap_or_default();
                                                        let mut item = crate::apply_system::ChangeQueueItem::new(
                                                            resolved_path.clone(), // Użyj pełnej ścieżki
                                                            change.line_start.unwrap_or(0),
                                                            change.line_end.unwrap_or(0),
                                                            old_code,
                                                            change.new_code,
                                                        );
                                                        if let Some(id) = change.id {
                                                            item.id = id;
                                                        }
                                                        // Generowanie danych do matchingu
                                                        item.matching_data = crate::apply_system::matchers::anchor_extraction::extract_matching_data(
                                                            &item.old_code, 
                                                            "" 
                                                        );
                                                        validated_items.push(item);
                                                    },
                                                    Err(reason) => {
                                                        println!("[Security] Rejected change for {}: {}", resolved_path, reason);
                                                        rejected_items.push(format!("{} ({})", resolved_path, reason));
                                                    }
                                                }
                                            }
 
                                            let added_count = validated_items.len();
                                            // Klonujemy elementy do osobnego wektora, aby użyć ich w zadaniu async
                                            let items_to_process = validated_items.clone();
 
                                            if added_count > 0 {
                                                // 2. Dodanie do kolejki (krótka blokada)
                                                let mut queue = apply_state.change_queue.lock().unwrap();
                                                queue.append(&mut validated_items);
                                                println!("[RUST WS] 📥 Added {} items to queue. Total size: {}", added_count, queue.len());
                                            } else {
                                                println!("[RUST WS] ⚠️ No valid items to add (validation failed or empty list).");
                                            }
 
                                            // 3. URUCHOMIENIE AUTOMATYCZNEGO APLIKOWANIA
                                            if !items_to_process.is_empty() {
                                                println!("[RUST WS] 🚀 Triggering auto-apply for {} changes...", items_to_process.len());
                                                let app_handle_for_task = app_handle.clone();
                                                
                                                // [FIX] Przechowaj ID requestu, aby przekazać je do taska
                                                // Dzięki temu eventy postępu (Pulse) wrócą do frontendu z tym samym ID
                                                let parent_request_id = request.id.clone(); 

                                                tokio::spawn(async move {
                                                    // Pobierz stan wewnątrz wątku, aby uniknąć problemów z lifetime (E0597)
                                                    let state_for_task = app_handle_for_task.state::<ApplySystemState>();
                                                    
                                                    for item in items_to_process {
                                                        println!("[Auto-Apply] Processing change ID: {}", item.id);
                                                        
                                                        // A. Odczyt aktualnego pliku (obsługa nowych plików)
                                                        let file_content = match tokio::fs::read_to_string(&item.file_path).await {
                                                            Ok(c) => c,
                                                            Err(e) => {
                                                                if item.old_code.is_empty() {
                                                                    println!("[Auto-Apply] New file creation detected for: {}", item.file_path);
                                                                    String::new()
                                                                } else {
                                                                    eprintln!("[Auto-Apply] ❌ Failed to read file {}: {}", item.file_path, e);
                                                                    continue;
                                                                }
                                                            }
                                                        };
 
                                                        // B. Wywołanie komendy z jawnym parent_request_id
                                                        match crate::apply_system::tauri_commands::apply_change_command(
                                                            item.id.clone(),
                                                            file_content,
                                                            Some(parent_request_id.clone()), // [FIX] Przekazujemy ID!
                                                            state_for_task.clone(),
                                                            app_handle_for_task.clone()
                                                        ).await {
                                                            Ok(_) => println!("[Auto-Apply] ✅ Successfully applied change {}", item.id),
                                                            Err(e) => eprintln!("[Auto-Apply] ❌ Failed to apply change {}: {}", item.id, e),
                                                        }
 
                                                        // Krótkie opóźnienie między zmianami
                                                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                                                    }
                                                });
                                            }
 
                                            // 4. Wysłanie odpowiedzi do przeglądarki
                                            let msg = if rejected_items.is_empty() {
                                                format!("Successfully queued and applying {} changes.", added_count)
                                            } else {
                                                format!("Queued {} changes. Rejected {} unsafe paths.", added_count, rejected_items.len())
                                            };
                                            
                                            let response = WebSocketResponse {
                                                request_id,
                                                action: "apply_code_changes".to_string(),
                                                payload: serde_json::json!({
                                                    "success": true,
                                                    "message": msg,
                                                    "changesCount": added_count,
                                                    "rejectedCount": rejected_items.len()
                                                }),
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                send_json(json);
                                            }
                                        },
                                        Err(e) => {
                                            eprintln!("[Apply System] JSON Payload Error: {}", e);
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Invalid payload structure: {}", e)
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }
                                "undo_change" => {
                                    println!("[RUST WS] 🔄 Received 'undo_change' (Smart Undo). Payload: {:?}", request.payload);

                                    #[derive(serde::Deserialize, Debug)]
                                    #[serde(rename_all = "camelCase")]
                                    struct UndoChangePayload {
                                        change_id: String,
                                        file_path: String, // Keep for compatibility, though logic uses internal state
                                    }

                                    match serde_json::from_value::<UndoChangePayload>(request.payload) {
                                        Ok(payload) => {
                                            println!("[RUST WS] Triggering Smart Undo logic for change: {}", payload.change_id);

                                            // [FIX] Zamiast wysyłać do VS Code, wywołujemy logikę Backendową (Revert + Re-apply siblings)
                                            let state = app_handle_clone.state::<ApplySystemState>();

                                            // Wywołaj funkcję z tauri_commands (async)
                                            let undo_result = crate::apply_system::tauri_commands::undo_change(
                                                payload.change_id.clone(),
                                                state,
                                                app_handle_clone.clone()
                                            ).await;

                                            match undo_result {
                                                Ok(_) => {
                                                    let response = WebSocketResponse {
                                                        request_id,
                                                        action: "undo_change".to_string(),
                                                        payload: serde_json::json!({
                                                            "success": true,
                                                            "changeId": payload.change_id,
                                                            "message": "Smart Undo sequence completed"
                                                        }),
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&response) {
                                                        send_json(json);
                                                    }
                                                },
                                                Err(e) => {
                                                    eprintln!("[Undo Change] Backend Error: {}", e);
                                                    let err_resp = WebSocketResponse {
                                                        request_id,
                                                        action: "error".to_string(),
                                                        payload: format!("Smart Undo failed: {}", e)
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&err_resp) {
                                                        send_json(json);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[Undo Change] JSON Payload Error: {}", e);
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Invalid undo payload: {}", e)
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "create_debug_snapshot" => {
                                    println!("[RUST WS] 🐞 Received 'create_debug_snapshot'");
                                    if let Ok(payload) = serde_json::from_value::<apply_system::tauri_commands::CreateSnapshotPayload>(request.payload) {
                                        let app_handle_task = app_handle_clone.clone();

                                        // Async execution
                                        tokio::spawn(async move {
                                            let state_task = app_handle_task.state::<ApplySystemState>();
                                            let app_handle_for_snapshot = app_handle_task.clone();
                                            match apply_system::tauri_commands::create_debug_snapshot(payload, state_task, app_handle_for_snapshot).await {
                                                Ok(path) => println!("[Debug] Snapshot saved successfully at: {}", path),
                                                Err(e) => eprintln!("[Debug] Snapshot generation failed: {}", e),
                                            }
                                        });
                                    } else {
                                        eprintln!("[RUST WS] Failed to deserialize debug payload");
                                    }
                                }

                                "toggle_local_ai" => {
                                    if let Ok(payload) = serde_json::from_value::<serde_json::Value>(request.payload.clone()) {
                                        let enabled = payload.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                                        let skip_auto_index = payload.get("skip_auto_index").and_then(|v| v.as_bool());
                                        
                                        let pool = app_handle_clone.state::<SqlitePool>();
                                        let state = app_handle_clone.state::<AppState>();
                                        
                                        // Wrapper for async command logic
                                        let result = toggle_local_ai(enabled, skip_auto_index, state, pool, app_handle_clone.clone()).await;
                                        
                                        match result {
                                            Ok(status) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "toggle_local_ai".to_string(),
                                                    payload: status,
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse { request_id, action: "error".to_string(), payload: e };
                                                if let Ok(json) = serde_json::to_string(&err_resp) { send_json(json); }
                                            }
                                        }
                                    }
                                }


                                "get_local_ai_status" => {
                                    let state = app_handle_clone.state::<AppState>();
                                    let status = state.local_ai.is_running();
                                    let response = WebSocketResponse {
                                        request_id,
                                        action: "get_local_ai_status".to_string(),
                                        payload: status,
                                    };
                                    if let Ok(json) = serde_json::to_string(&response) {
                                        send_json(json);
                                    }
                                }

                                "list_embedding_models" => {
                                    println!("[RUST WS] 📋 Received list_embedding_models request");
                                    let pool = app_handle_clone.state::<SqlitePool>();
                                    match list_embedding_models(app_handle_clone.clone(), pool).await {
                                        Ok(models) => {
                                            println!("[RUST WS] ✅ Found {} embedding models", models.len());
                                            let response = WebSocketResponse {
                                                request_id: request_id.clone(),
                                                action: "list_embedding_models".to_string(),
                                                payload: models,
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                println!("[RUST WS] 📤 Sending list_embedding_models response (request_id: {})", request_id);
                                                send_json(json);
                                            } else {
                                                eprintln!("[RUST WS] ❌ Failed to serialize response");
                                            }
                                        },
                                        Err(e) => {
                                            eprintln!("[RUST WS] ❌ Error listing embedding models: {}", e);
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: serde_json::json!({
                                                    "error": e,
                                                    "message": "Failed to list embedding models"
                                                }),
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "set_embedding_model" => {
                                    println!("[RUST WS] 🔄 Received set_embedding_model request");

                                    match request.payload.get("model_filename").and_then(|v| v.as_str()) {
                                        Some(model_filename) => {
                                            let model_filename = model_filename.to_string();
                                            let pool = app_handle_clone.state::<SqlitePool>();
                                            let state = app_handle_clone.state::<AppState>();

                                            match set_embedding_model(
                                                model_filename.clone(),
                                                pool,
                                                state,
                                                app_handle_clone.clone()
                                            ).await {
                                                Ok(msg) => {
                                                    println!("[RUST WS] ✅ Model switched successfully: {}", model_filename);
                                                    let response = WebSocketResponse {
                                                        request_id: request_id.clone(),
                                                        action: "set_embedding_model".to_string(),
                                                        payload: serde_json::json!({
                                                            "status": "success",
                                                            "message": msg,
                                                            "model": model_filename
                                                        }),
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&response) {
                                                        send_json(json);
                                                    }
                                                },
                                                Err(e) => {
                                                    eprintln!("[RUST WS] ❌ Error switching model: {}", e);
                                                    let err_resp = WebSocketResponse {
                                                        request_id: request_id.clone(),
                                                        action: "error".to_string(),
                                                        payload: serde_json::json!({
                                                            "error": e,
                                                            "message": "Failed to switch embedding model"
                                                        }),
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&err_resp) {
                                                        send_json(json);
                                                    }
                                                }
                                            }
                                        },
                                        None => {
                                            eprintln!("[RUST WS] ❌ Missing model_filename in payload");
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: serde_json::json!({
                                                    "error": "Missing model_filename in payload",
                                                    "message": "Invalid request: model_filename is required"
                                                }),
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "agent_register" => {
                                    println!("[RUST WS] 🤝 Received 'agent_register' from socket: {}", socket_id_for_read);

                                    #[derive(serde::Deserialize, Debug)]
                                    #[serde(rename_all = "camelCase")]
                                    struct AgentRegisterPayload {
                                        pairing_code: String,
                                    }

                                    match serde_json::from_value::<AgentRegisterPayload>(request.payload) {
                                        Ok(payload) => {
                                            let state = app_handle_clone.state::<ApplySystemState>();
                                            let graph = state.agent_workflow.get_graph();
                                            let mut graph = graph.lock().unwrap();

                                            match graph.register_agent(&payload.pairing_code, socket_id_for_read.clone()) {
                                                Ok(agent) => {
                                                    println!("[Agent Workflow] ✅ Agent '{}' registered with socket {}", agent.name, socket_id_for_read);

                                                    let response = WebSocketResponse {
                                                        request_id,
                                                        action: "agent_register".to_string(),
                                                        payload: serde_json::json!({
                                                            "success": true,
                                                            "agent": agent,
                                                            "message": format!("Agent '{}' successfully registered", agent.name)
                                                        }),
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&response) {
                                                        send_json(json);
                                                    }
                                                }
                                                Err(e) => {
                                                    eprintln!("[Agent Workflow] ❌ Registration failed: {}", e);
                                                    let err_resp = WebSocketResponse {
                                                        request_id,
                                                        action: "error".to_string(),
                                                        payload: format!("Agent registration failed: {}", e)
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&err_resp) {
                                                        send_json(json);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[Agent Register] JSON Payload Error: {}", e);
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Invalid registration payload: {}", e)
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "agent_response" => {
                                    println!("[RUST WS] 🧠 Received 'agent_response' from socket: {}", socket_id_for_read);

                                    #[derive(serde::Deserialize, Debug)]
                                    #[serde(rename_all = "camelCase")]
                                    struct AgentResponsePayload {
                                        content: String,
                                        agent_id: String,
                                    }

                                    match serde_json::from_value::<AgentResponsePayload>(request.payload) {
                                        Ok(payload) => {
                                            let state = app_handle_clone.state::<ApplySystemState>();

                                            // Route message through workflow using agent_id
                                            println!("[Agent Workflow] 📨 Routing message from agent: {}", payload.agent_id);
                                            match state.agent_workflow.route_message(&payload.agent_id, &payload.content) {
                                                Ok(targets) => {
                                                    if targets.is_empty() {
                                                        println!("[Agent Workflow] ⚠️ No targets found for this agent");
                                                        let response = WebSocketResponse {
                                                            request_id,
                                                            action: "agent_response".to_string(),
                                                            payload: serde_json::json!({
                                                                "success": true,
                                                                "message": "Response received but no targets configured"
                                                            }),
                                                        };
                                                        if let Ok(json) = serde_json::to_string(&response) {
                                                            send_json(json);
                                                        }
                                                    } else {
                                                        println!("[Agent Workflow] 📤 Routing message to {} target(s)", targets.len());

                                                        // Check if auto-forward is enabled
                                                        let graph = state.agent_workflow.get_graph();
                                                        let auto_forward = graph.lock().unwrap().auto_forward;
                                                        println!("[Agent Workflow] 🔍 auto_forward setting: {}", auto_forward);

                                                        if auto_forward {
                                                            // Send to all targets
                                                            for target in &targets {
                                                                use gluon_desktop_lib::workflow::agent_workflow::AgentType;

                                                                // Tutaj używamy typu ustalonego przez heurystykę w route_message
                                                                match target.agent_type {
                                                                    AgentType::Normal => {
                                                                        // Standard WebSocket forward
                                                                        let agent_message = WebSocketResponse {
                                                                            request_id: request_id.clone(),
                                                                            action: "agent_message_received".to_string(),
                                                                            payload: serde_json::json!({
                                                                                "content": target.content,
                                                                                "from_agent": target.agent_name,
                                                                                "target_agent_id": target.agent_id,
                                                                                "auto_submit": true
                                                                            })
                                                                        };

                                                                        if let Ok(json) = serde_json::to_string(&agent_message) {
                                                                            send_json(json);
                                                                            println!("[Agent Workflow] ✉️ Sent message to agent '{}' (ID: {})", target.agent_name, target.agent_id);
                                                                        }
                                                                    },
                                                                    AgentType::Report => {
                                                                        // Agregator Logic (Wewnętrzny bufor Rust)
                                                                        println!("[Agent Workflow] 🗂️ Processing Report Aggregator: {}", target.agent_name);

                                                                        let state = app_handle_clone.state::<ApplySystemState>();
                                                                        let graph_mutex = state.agent_workflow.get_graph();
                                                                        let mut graph = graph_mutex.lock().unwrap();

                                                                        // Dodaj do bufora
                                                                        let aggregation_result = graph.add_to_report_buffer(
                                                                            &target.agent_id,
                                                                            &payload.agent_id, // ID nadawcy (źródła)
                                                                            target.content.clone()
                                                                        );

                                                                        // Pobierz nazwę nadawcy dla UI
                                                                        let sender_name = graph.agents.get(&payload.agent_id)
                                                                            .map(|a| a.name.clone())
                                                                            .unwrap_or_else(|| "Nieznany Nadawca".to_string());

                                                                        // Powiadom UI o postępie (logi w kafelku agregatora)
                                                                        let update_msg = WebSocketResponse {
                                                                            request_id: Uuid::new_v4().to_string(),
                                                                            action: "workflow_aggregator_update".to_string(),
                                                                            payload: serde_json::json!({
                                                                                "aggregator_id": target.agent_id,
                                                                                "source_agent": sender_name, // [FIX] Używamy nazwy nadawcy, nie celu
                                                                                "status": "received"
                                                                            })
                                                                        };
                                                                        if let Ok(json) = serde_json::to_string(&update_msg) { send_json(json); }

                                                                        // Jeśli agregacja zakończona -> wyślij dalej
                                                                        if let Some(final_report) = aggregation_result {
                                                                            println!("[Agent Workflow] ✅ Aggregation COMPLETE. Routing final report...");
                                                                            drop(graph); // Zwolnij mutex przed routingiem (route_message też go bierze)

                                                                            // Rekurencyjny routing (Agregator -> Jego cele)
                                                                            match state.agent_workflow.route_message(&target.agent_id, &final_report) {
                                                                                Ok(next_targets) => {
                                                                                    for next_t in next_targets {
                                                                                        // Wysyłamy wynik agregacji do kolejnych agentów (np. PM lub Zapis)
                                                                                        // Zakładamy, że kolejne to już zwykłe agenty lub kolejne raporty (rekurencja przez main loop byłaby lepsza, ale to wystarczy)
                                                                                        if next_t.agent_type == AgentType::Normal {
                                                                                            let final_msg = WebSocketResponse {
                                                                                                request_id: Uuid::new_v4().to_string(),
                                                                                                action: "agent_message_received".to_string(),
                                                                                                payload: serde_json::json!({
                                                                                                    "content": next_t.content,
                                                                                                    "from_agent": next_t.agent_name,
                                                                                                    "target_agent_id": next_t.agent_id,
                                                                                                    "auto_submit": false
                                                                                                })
                                                                                            };
                                                                                            if let Ok(json) = serde_json::to_string(&final_msg) { send_json(json); }
                                                                                        }
                                                                                    }
                                                                                },
                                                                                Err(e) => println!("[Agent Workflow] Aggregation complete but routing failed: {}", e)
                                                                            }
                                                                        } else {
                                                                            println!("[Agent Workflow] ⏳ Aggregator waiting for more inputs...");
                                                                        }
                                                                    },
                                                                    AgentType::AutoApply => {
                                                                        // [FIX] Trigger Frontend Logic for Auto-Apply
                                                                        println!("[Agent Workflow] ⚙️ Auto-Apply triggered for: {}", target.agent_name);
                                                                        
                                                                        let trigger_msg = WebSocketResponse {
                                                                            request_id: Uuid::new_v4().to_string(),
                                                                            action: "workflow_auto_apply_trigger".to_string(),
                                                                            payload: serde_json::json!({
                                                                                "agent_id": target.agent_id,
                                                                                "content": target.content
                                                                            })
                                                                        };
                                                                        if let Ok(json) = serde_json::to_string(&trigger_msg) { send_json(json); }
                                                                    },
                                                                    AgentType::Terminal => {
                                                                        // Terminal listener (no message forwarding needed)
                                                                        println!("[Agent Workflow] 💻 Terminal agent processed: {}", target.agent_name);
                                                                    }
                                                                }
                                                            }

                                                            let response = WebSocketResponse {
                                                                request_id,
                                                                action: "agent_response".to_string(),
                                                                payload: serde_json::json!({
                                                                    "success": true,
                                                                    "targetsCount": targets.len(),
                                                                    "message": "Message processed (routed/buffered)"
                                                                }),
                                                            };
                                                            if let Ok(json) = serde_json::to_string(&response) {
                                                                send_json(json);
                                                            }
                                                        } else {
                                                            // Auto-forward disabled, just acknowledge
                                                            let response = WebSocketResponse {
                                                                request_id,
                                                                action: "agent_response".to_string(),
                                                                payload: serde_json::json!({
                                                                    "success": true,
                                                                    "pending": true,
                                                                    "targetsCount": targets.len(),
                                                                    "message": "Message ready for manual forwarding"
                                                                }),
                                                            };
                                                            if let Ok(json) = serde_json::to_string(&response) {
                                                                send_json(json);
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    eprintln!("[Agent Workflow] ❌ Routing failed: {}", e);
                                                    let err_resp = WebSocketResponse {
                                                        request_id,
                                                        action: "error".to_string(),
                                                        payload: format!("Message routing failed: {}", e)
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&err_resp) {
                                                        send_json(json);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[Agent Response] JSON Payload Error: {}", e);
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Invalid response payload: {}", e)
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "workflow_get_graph" => {
                                    let state = app_handle_clone.state::<ApplySystemState>();
                                    match gluon_desktop_lib::workflow::workflow_commands::workflow_get_graph(state, app_handle_clone.clone()) {
                                        Ok(graph_data) => {
                                            let response = WebSocketResponse {
                                                request_id,
                                                action: "workflow_get_graph".to_string(),
                                                payload: graph_data,
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                send_json(json);
                                            }
                                        },
                                        Err(e) => {
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Failed to get workflow graph: {}", e)
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "workflow_add_agent" => {
                                    if let (Some(name), output_wrapper) = (
                                        request.payload.get("name").and_then(|v| v.as_str()),
                                        request.payload.get("output_wrapper").and_then(|v| v.as_str()).map(String::from)
                                    ) {
                                        let agent_type = request.payload.get("agent_type").and_then(|v| v.as_str()).map(String::from);
                                        let position = request.payload.get("position").and_then(|v| {
                                            if let (Some(x), Some(y)) = (v.get("x").and_then(|x| x.as_f64()), v.get("y").and_then(|y| y.as_f64())) {
                                                Some((x as f32, y as f32))
                                            } else {
                                                None
                                            }
                                        });
                                        let state = app_handle_clone.state::<ApplySystemState>();
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_add_agent(name.to_string(), output_wrapper, agent_type, position, state, app_handle_clone.clone()) {
                                            Ok(agent_data) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "workflow_add_agent".to_string(),
                                                    payload: agent_data,
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to add agent: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                "workflow_remove_agent" => {
                                    if let Some(agent_id) = request.payload.get("agent_id").and_then(|v| v.as_str()) {
                                        let state = app_handle_clone.state::<ApplySystemState>();
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_remove_agent(agent_id.to_string(), state, app_handle_clone.clone()) {
                                            Ok(_) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "workflow_remove_agent".to_string(),
                                                    payload: serde_json::json!({ "success": true }),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to remove agent: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                "workflow_update_agent" => {
                                    if let Some(agent_id) = request.payload.get("agent_id").and_then(|v| v.as_str()) {
                                        let name = request.payload.get("name").and_then(|v| v.as_str()).map(String::from);
                                        let output_wrapper = request.payload.get("output_wrapper").and_then(|v| v.as_str()).map(String::from);
                                        let system_prompt = request.payload.get("system_prompt").and_then(|v| v.as_str()).map(String::from);

                                        let state = app_handle_clone.state::<ApplySystemState>();
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_update_agent(
                                            agent_id.to_string(),
                                            name,
                                            output_wrapper,
                                            system_prompt,
                                            state,
                                            app_handle_clone.clone()
                                        ) {
                                            Ok(agent_data) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "workflow_update_agent".to_string(),
                                                    payload: agent_data,
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to update agent: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                "workflow_add_connection" => {
                                    if let (Some(from_id), Some(to_id), template) = (
                                        request.payload.get("from_id").and_then(|v| v.as_str()),
                                        request.payload.get("to_id").and_then(|v| v.as_str()),
                                        request.payload.get("template").and_then(|v| v.as_str()).map(String::from)
                                    ) {
                                        let state = app_handle_clone.state::<ApplySystemState>();
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_add_connection(from_id.to_string(), to_id.to_string(), template, state, app_handle_clone.clone()) {
                                            Ok(_) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "workflow_add_connection".to_string(),
                                                    payload: serde_json::json!({ "success": true }),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to add connection: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                "workflow_remove_connection" => {
                                    if let (Some(from_id), Some(to_id)) = (
                                        request.payload.get("from_id").and_then(|v| v.as_str()),
                                        request.payload.get("to_id").and_then(|v| v.as_str())
                                    ) {
                                        let state = app_handle_clone.state::<ApplySystemState>();
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_remove_connection(from_id.to_string(), to_id.to_string(), state, app_handle_clone.clone()) {
                                            Ok(_) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "workflow_remove_connection".to_string(),
                                                    payload: serde_json::json!({ "success": true }),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to remove connection: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                "workflow_set_auto_forward" => {
                                    println!("[RUST WS] 🔧 Received 'workflow_set_auto_forward' request");
                                    if let Some(enabled) = request.payload.get("enabled").and_then(|v| v.as_bool()) {
                                        println!("[Agent Workflow] 🔧 Setting auto_forward to: {}", enabled);
                                        let state = app_handle_clone.state::<ApplySystemState>();
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_set_auto_forward(enabled, state, app_handle_clone.clone()) {
                                            Ok(_) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "workflow_set_auto_forward".to_string(),
                                                    payload: serde_json::json!({ "success": true }),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to set auto-forward: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                // [V2] Workflow Preset Operations
                                "workflow_get_agent_presets" => {
                                    match gluon_desktop_lib::workflow::workflow_commands::workflow_get_agent_presets() {
                                        Ok(presets) => {
                                            let response = WebSocketResponse {
                                                request_id,
                                                action: "workflow_get_agent_presets".to_string(),
                                                payload: presets,
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                send_json(json);
                                            }
                                        },
                                        Err(e) => {
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Failed to get agent presets: {}", e)
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "workflow_get_connection_presets" => {
                                    match gluon_desktop_lib::workflow::workflow_commands::workflow_get_connection_presets() {
                                        Ok(presets) => {
                                            let response = WebSocketResponse {
                                                request_id,
                                                action: "workflow_get_connection_presets".to_string(),
                                                payload: presets,
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                send_json(json);
                                            }
                                        },
                                        Err(e) => {
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Failed to get connection presets: {}", e)
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "workflow_get_workflow_presets" => {
                                    match gluon_desktop_lib::workflow::workflow_commands::workflow_get_workflow_presets() {
                                        Ok(presets) => {
                                            let response = WebSocketResponse {
                                                request_id,
                                                action: "workflow_get_workflow_presets".to_string(),
                                                payload: presets,
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                send_json(json);
                                            }
                                        },
                                        Err(e) => {
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Failed to get workflow presets: {}", e)
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "workflow_create_agent_from_preset" => {
                                    if let Some(preset_id) = request.payload.get("preset_id").and_then(|v| v.as_str()) {
                                        let custom_name = request.payload.get("custom_name").and_then(|v| v.as_str()).map(|s| s.to_string());
                                        let position = request.payload.get("position").and_then(|v| v.as_array()).and_then(|arr| {
                                            if arr.len() == 2 {
                                                Some((arr[0].as_f64()? as f32, arr[1].as_f64()? as f32))
                                            } else {
                                                None
                                            }
                                        });

                                        let state = app_handle_clone.state::<ApplySystemState>();
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_create_agent_from_preset(
                                            preset_id.to_string(),
                                            custom_name,
                                            position,
                                            state,
                                            app_handle_clone.clone()
                                        ) {
                                            Ok(agent) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "workflow_create_agent_from_preset".to_string(),
                                                    payload: agent,
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to create agent from preset: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                // [V2] Workflow Saved Config Operations
                                "workflow_get_saved_configs" => {
                                    match gluon_desktop_lib::workflow::workflow_commands::workflow_get_saved_configs(app_handle_clone.clone()) {
                                        Ok(configs) => {
                                            let response = WebSocketResponse {
                                                request_id,
                                                action: "workflow_get_saved_configs".to_string(),
                                                payload: configs,
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                send_json(json);
                                            }
                                        },
                                        Err(e) => {
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Failed to get saved configs: {}", e)
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                "workflow_save_config" => {
                                    if let (Some(id), Some(name), Some(workflow)) = (
                                        request.payload.get("id").and_then(|v| v.as_str()),
                                        request.payload.get("name").and_then(|v| v.as_str()),
                                        request.payload.get("workflow")
                                    ) {
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_save_config(
                                            id.to_string(),
                                            name.to_string(),
                                            workflow.clone(),
                                            app_handle_clone.clone()
                                        ) {
                                            Ok(config) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "workflow_save_config".to_string(),
                                                    payload: config,
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to save config: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                "workflow_delete_saved_config" => {
                                    if let Some(id) = request.payload.get("id").and_then(|v| v.as_str()) {
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_delete_saved_config(
                                            id.to_string(),
                                            app_handle_clone.clone()
                                        ) {
                                            Ok(_) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "workflow_delete_saved_config".to_string(),
                                                    payload: serde_json::json!({ "success": true }),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to delete config: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                "run_smart_context_task" => {
                                    #[derive(serde::Deserialize)]
                                    struct TaskPayload {
                                        task: String,
                                    }
                                    if let Ok(payload) = serde_json::from_value::<TaskPayload>(request.payload) {
                                        let state = app_handle_clone.state::<AppState>();
                                        // Wywołujemy funkcję zdefiniowaną w tym pliku (na dole)
                                        match run_smart_context_task(payload.task, state).await {
                                            Ok(content) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "smart_context_generated".to_string(),
                                                    payload: content,
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Smart Context failed: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    } else {
                                        let err_resp = WebSocketResponse {
                                            request_id,
                                            action: "error".to_string(),
                                            payload: "Invalid payload for run_smart_context_task".to_string()
                                        };
                                        if let Ok(json) = serde_json::to_string(&err_resp) {
                                            send_json(json);
                                        }
                                    }
                                }

                                "execute_context_operations" => {
                                    use gluon_desktop_lib::apply_system::tauri_commands::ContextOperation;

                                    #[derive(serde::Deserialize)]
                                    struct ExecContextPayload {
                                        operations: Vec<ContextOperation>,
                                        #[serde(rename = "projectRoot")]
                                        project_root: Option<String>,
                                    }

                                    if let Ok(payload) = serde_json::from_value::<ExecContextPayload>(request.payload) {
                                        // Execute directly (await) to access send_json closure
                                        // Operations are I/O bound but generally fast enough for this loop
                                        let state = app_handle_clone.state::<ApplySystemState>();

                                        match gluon_desktop_lib::apply_system::tauri_commands::execute_context_operations(
                                            payload.operations,
                                            payload.project_root,
                                            state,
                                            app_handle_clone.clone()
                                        ).await {
                                            Ok(result) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "execute_context_operations".to_string(),
                                                    payload: serde_json::to_value(result).unwrap(),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: serde_json::json!({ "error": e, "action": "execute_context_operations" })
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    } else {
                                        let err_resp = WebSocketResponse {
                                            request_id,
                                            action: "error".to_string(),
                                            payload: serde_json::json!({ "error": "Invalid payload for execute_context_operations" })
                                        };
                                        if let Ok(json) = serde_json::to_string(&err_resp) {
                                            send_json(json);
                                        }
                                    }
                                }

                                "get_file_symbols" => {
                                    #[derive(serde::Deserialize)]
                                    struct GetSymbolsPayload {
                                        file_path: String,
                                        project_root: Option<String>,
                                    }

                                    if let Ok(payload) = serde_json::from_value::<GetSymbolsPayload>(request.payload) {
                                        match gluon_desktop_lib::apply_system::tauri_commands::get_file_symbols(
                                            payload.file_path,
                                            payload.project_root
                                        ).await {
                                            Ok(symbols) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "get_file_symbols".to_string(),
                                                    payload: serde_json::to_value(symbols).unwrap(),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: serde_json::json!({ "error": e, "action": "get_file_symbols" })
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    } else {
                                        let err_resp = WebSocketResponse {
                                            request_id,
                                            action: "error".to_string(),
                                            payload: serde_json::json!({ "error": "Invalid payload for get_file_symbols" })
                                        };
                                        if let Ok(json) = serde_json::to_string(&err_resp) {
                                            send_json(json);
                                        }
                                    }
                                }

                                "run_agentic_context_task" => {
                                    #[derive(serde::Deserialize)]
                                    struct AgenticPayload {
                                        task: String,
                                        #[serde(rename = "maxSteps")]
                                        max_steps: Option<usize>,
                                        #[serde(rename = "repoMapDetail")]
                                        repo_map_detail: String,
                                    }

                                    if let Ok(payload) = serde_json::from_value::<AgenticPayload>(request.payload) {
                                        // Clone app_handle for the spawned task
                                        let app_clone = app_handle_clone.clone();

                                        // Spawn async task (żeby nie blokować WebSocket)
                                        tokio::spawn(async move {
                                            // Get State wrappers inside the spawned task
                                            let state = app_clone.state::<AppState>();
                                            let pool = app_clone.state::<SqlitePool>();

                                            match run_agentic_context_task(
                                                payload.task,
                                                payload.max_steps,
                                                payload.repo_map_detail,
                                                state,
                                                app_clone.clone(),
                                                pool,
                                            ).await {
                                                Ok(result) => {
                                                    let response = WebSocketResponse {
                                                        request_id,
                                                        action: "agentic_context_generated".to_string(),
                                                        payload: result,
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&response) {
                                                        send_json(json);
                                                    }
                                                },
                                                Err(e) => {
                                                    let err_resp = WebSocketResponse {
                                                        request_id,
                                                        action: "error".to_string(),
                                                        payload: format!("Agentic Context failed: {}", e),
                                                    };
                                                    if let Ok(json) = serde_json::to_string(&err_resp) {
                                                        send_json(json);
                                                    }
                                                }
                                            }
                                        });
                                    } else {
                                        let err_resp = WebSocketResponse {
                                            request_id,
                                            action: "error".to_string(),
                                            payload: "Invalid payload for run_agentic_context_task".to_string()
                                        };
                                        if let Ok(json) = serde_json::to_string(&err_resp) {
                                            send_json(json);
                                        }
                                    }
                                }

                                "trigger_indexing" => {
                                    let pool = app_handle_clone.state::<SqlitePool>();
                                    let state = app_handle_clone.state::<AppState>();

                                    #[derive(serde::Deserialize)]
                                    struct IndexPayload {
                                        #[serde(default)]
                                        projects: Vec<String>,
                                        #[serde(default, rename = "selectedFiles")]
                                        selected_files: Vec<SelectedFileInfo>,
                                    }

                                    let payload_result = serde_json::from_value::<IndexPayload>(request.payload);

                                    let (targets, selected_files) = if let Ok(p) = payload_result {
                                        (p.projects, p.selected_files)
                                    } else {
                                        (Vec::new(), Vec::new())
                                    };

                                    match trigger_indexing(targets, selected_files, pool, state, app_handle_clone.clone()).await {
                                        Ok(msg) => {
                                            let response = WebSocketResponse {
                                                request_id,
                                                action: "status_update".to_string(),
                                                payload: msg,
                                            };
                                            if let Ok(json) = serde_json::to_string(&response) {
                                                send_json(json);
                                            }
                                        },
                                        Err(e) => {
                                            let err_resp = WebSocketResponse {
                                                request_id,
                                                action: "error".to_string(),
                                                payload: format!("Indexing failed: {}", e)
                                            };
                                            if let Ok(json) = serde_json::to_string(&err_resp) {
                                                send_json(json);
                                            }
                                        }
                                    }
                                }

                                // [FIX] Obsługa Auto-Apply
                                "workflow_auto_apply" => {
                                    #[derive(serde::Deserialize)]
                                    struct AutoApplyPayload {
                                        agent_id: String,
                                        content: String,
                                    }

                                    if let Ok(payload) = serde_json::from_value::<AutoApplyPayload>(request.payload) {
                                        let state = app_handle_clone.state::<ApplySystemState>();
                                        let pool = app_handle_clone.state::<SqlitePool>(); // [FIX] Get DB Pool

                                        // Wywołanie asynchroniczne
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_auto_apply(
                                            payload.agent_id, 
                                            payload.content, 
                                            state,
                                            pool, // [FIX] Pass DB Pool
                                            app_handle_clone.clone()
                                        ).await {
                                            Ok(result) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "apply_code_changes".to_string(), // Frontend oczekuje potwierdzenia pod tym kluczem lub process_dom_stream_result
                                                    payload: result,
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Auto-Apply failed: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    } else {
                                        let err_resp = WebSocketResponse {
                                            request_id,
                                            action: "error".to_string(),
                                            payload: "Invalid payload for workflow_auto_apply".to_string()
                                        };
                                        if let Ok(json) = serde_json::to_string(&err_resp) { send_json(json); }
                                    }
                                }

                                "workflow_clear_auto_apply_queue" => {
                                    if let Some(agent_id) = request.payload.get("agent_id").and_then(|v| v.as_str()) {
                                        let state = app_handle_clone.state::<ApplySystemState>();
                                        match gluon_desktop_lib::workflow::workflow_commands::workflow_clear_auto_apply_queue(agent_id.to_string(), state) {
                                            Ok(_) => {
                                                let response = WebSocketResponse {
                                                    request_id,
                                                    action: "workflow_clear_auto_apply_queue".to_string(),
                                                    payload: serde_json::json!({ "success": true }),
                                                };
                                                if let Ok(json) = serde_json::to_string(&response) {
                                                    send_json(json);
                                                }
                                            },
                                            Err(e) => {
                                                let err_resp = WebSocketResponse {
                                                    request_id,
                                                    action: "error".to_string(),
                                                    payload: format!("Failed to clear queue: {}", e)
                                                };
                                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                                    send_json(json);
                                                }
                                            }
                                        }
                                    }
                                }

                                _ => {
                                let err = WebSocketResponse { request_id, action: "error".to_string(), payload: format!("Unknown action: {}", request.action) };
                                if let Ok(json) = serde_json::to_string(&err) { send_json(json); }
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("WebSocket connection closed.");
                    break;
                }
                Err(e) => {
                    eprintln!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        // Po zakończeniu pętli odczytu (rozłączenie), informujemy bridge i odłączamy agenta
        bridge_state.disconnect();

        // Disconnect agent from workflow
        let state = app_handle_clone.state::<ApplySystemState>();
        let graph = state.agent_workflow.get_graph();
        let mut graph = graph.lock().unwrap();
        graph.disconnect_agent(&socket_id_for_read);
        println!("[Agent Workflow] 🔌 Agent disconnected (socket: {})", socket_id_for_read);
    });
 
    // Czekaj na zakończenie (jeśli write padnie, zabij read i odwrotnie)
    tokio::select! {
        _ = write_task => read_task.abort(),
        _ = &mut read_task => {}, // write_task sam się zamknie gdy bridge_rx zostanie droppowany
    }
}

async fn is_license_valid(_pool: &SqlitePool) -> bool {
    true
}

async fn start_websocket_server(app_handle: AppHandle) {
    let port = get_initial_settings(app_handle.state())
        .await
        .map_or(8743, |s| s.port.parse().unwrap_or(8743));
    let addr = format!("127.0.0.1:{}", port);
    if let Ok(listener) = TcpListener::bind(&addr).await {
        println!("WebSocket server listening on: ws://{}", addr);
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(handle_connection(stream, app_handle.clone()));
        }
    }
}

// ============================================================================
// Main Application Entry Point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    println!("[RUST LOG] Starting tauri::Builder::default()");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(GoogleAuthState::new())
        .manage(ai_chat::AiChatState::new())
        .invoke_handler(tauri::generate_handler![
            get_projects,
            add_project,
            remove_project,
            get_environments,
            create_environment,
            update_environment,
            delete_environment,
            get_prompts,
            create_prompt,
            update_prompt,
            toggle_prompt,
            delete_prompt,
            assign_project_environment,
            get_initial_settings,
            get_setting,
            set_setting,
            select_download_folder,
            get_project_file_tree,
            get_files_content,
            get_directory_tree,
            get_default_exclusions,
            set_default_exclusions,
            set_project_download_path,
            get_license_status,
            verify_and_save_license_key,
            deactivate_license,
            update_project_settings,
            get_extension_templates,
            create_extension_template,
            delete_extension_template,
            process_dom_stream,
            apply_system::tauri_commands::parse_model_response_command,
            apply_system::tauri_commands::apply_change_command,
            apply_system::tauri_commands::get_change_queue,
            apply_system::tauri_commands::apply_all_changes,
            apply_system::tauri_commands::undo_change,
            apply_system::tauri_commands::undo_all_changes,
            apply_system::tauri_commands::get_config,
            apply_system::tauri_commands::resolve_change_locations,
            apply_system::tauri_commands::refresh_context_graph,
            apply_system::tauri_commands::get_repo_map_prompt,
            apply_system::tauri_commands::get_repo_skeleton,
            apply_system::tauri_commands::get_precise_context,
            apply_system::tauri_commands::execute_context_operations,
            apply_system::tauri_commands::get_file_symbols,
            apply_system::tauri_commands::get_available_backups,
            apply_system::tauri_commands::preview_backup_content,
            apply_system::tauri_commands::restore_backup_files,
            apply_system::tauri_commands::create_debug_snapshot,
            apply_system::tauri_commands::run_integrity_audit,
            apply_system::tauri_commands::export_audit_report,
            gluon_desktop_lib::workflow::workflow_commands::workflow_add_agent,
            gluon_desktop_lib::workflow::workflow_commands::workflow_remove_agent,
            gluon_desktop_lib::workflow::workflow_commands::workflow_update_agent,
            gluon_desktop_lib::workflow::workflow_commands::workflow_add_connection,
            gluon_desktop_lib::workflow::workflow_commands::workflow_remove_connection,
            gluon_desktop_lib::workflow::workflow_commands::workflow_get_graph,
            gluon_desktop_lib::workflow::workflow_commands::workflow_set_auto_forward,
            gluon_desktop_lib::workflow::workflow_commands::workflow_register_agent,
            gluon_desktop_lib::workflow::workflow_commands::workflow_disconnect_agent,
            gluon_desktop_lib::workflow::workflow_commands::workflow_reset_report_buffers,
            gluon_desktop_lib::workflow::workflow_commands::workflow_update_agent_position,
            gluon_desktop_lib::workflow::workflow_commands::workflow_save_config,
            gluon_desktop_lib::workflow::workflow_commands::workflow_get_saved_configs,
            gluon_desktop_lib::workflow::workflow_commands::workflow_delete_saved_config,
            gluon_desktop_lib::workflow::workflow_commands::workflow_auto_apply,
            gluon_desktop_lib::workflow::workflow_commands::workflow_clear_auto_apply_queue,
            gluon_desktop_lib::workflow::workflow_commands::workflow_get_auto_apply_history,
            gluon_desktop_lib::workflow::workflow_commands::workflow_revert_batch,
            // Preset Library commands (SSOT)
            gluon_desktop_lib::workflow::workflow_commands::workflow_get_agent_presets,
            gluon_desktop_lib::workflow::workflow_commands::workflow_get_connection_presets,
            gluon_desktop_lib::workflow::workflow_commands::workflow_get_workflow_presets,
            gluon_desktop_lib::workflow::workflow_commands::workflow_get_agent_preset,
            gluon_desktop_lib::workflow::workflow_commands::workflow_create_agent_from_preset,
            gluon_desktop_lib::workflow::workflow_commands::workflow_create_from_preset,
            // Workflow Execution commands (LLM Inference + Smart Routing)
            gluon_desktop_lib::workflow::workflow_execution::workflow_execute_agent,
            gluon_desktop_lib::workflow::workflow_execution::workflow_clear_agent_history,
            gluon_desktop_lib::workflow::workflow_execution::workflow_get_agent_history,
            gluon_desktop_lib::workflow::workflow_execution::workflow_reset_agent,
            run_smart_context_task,
            run_agentic_context_task,
            trigger_indexing,
            toggle_local_ai,
            get_local_ai_status,
            list_embedding_models,
            set_embedding_model,
            // Google Drive Integration
            google_auth::set_google_credentials,
            google_auth::has_google_credentials,
            google_auth::start_google_login,
            google_auth::get_google_access_token,
            google_auth::is_google_logged_in,
            google_auth::google_logout,
            google_drive::list_drive_files,
            google_drive::download_file_content,
            google_drive::get_drive_file_info,
            // AI Chat commands
            ai_chat::commands::get_ai_providers,
            ai_chat::commands::get_chat_sessions,
            ai_chat::commands::create_chat_session,
            ai_chat::commands::delete_chat_session,
            ai_chat::commands::toggle_session_pin,
            ai_chat::commands::rename_chat_session,
            ai_chat::commands::get_chat_messages,
            ai_chat::commands::send_chat_message,
            ai_chat::commands::set_ai_api_key,
            ai_chat::commands::get_api_key_status,
            ai_chat::commands::export_chat_session,
            ai_chat::providers::vscode::connect_to_vscode,
            // Vector Map Management commands
            vector_map_commands::get_vector_maps,
            vector_map_commands::get_vector_map_stats,
            vector_map_commands::get_shared_projects,
            vector_map_commands::create_vector_map,
            vector_map_commands::update_project_vector_map,
            vector_map_commands::delete_vector_map,
            vector_map_commands::clear_vector_map,
            vector_map_commands::get_project_rag_status,
            vector_map_commands::rag_search_manual,
            ])
            .setup(|app| {
            println!("[RUST LOG] Inside .setup() closure");
            let app_handle = app.handle().clone();

            // Initialize Apply System state
            let apply_state = ApplySystemState::new();

            // [FIX] Load workflow persistence immediately to prevent overwrite on startup
            if let Ok(app_data_dir) = app.path().app_data_dir() {
                let workflow_path = app_data_dir.join("workflow_graph.json");
                if workflow_path.exists() {
                    match apply_state.agent_workflow.load_from_storage(&workflow_path) {
                        Ok(_) => println!("[RUST LOG] ✅ Workflow graph loaded from persistence."),
                        Err(e) => println!("[RUST LOG] ⚠️ Failed to load persistent workflow: {}", e),
                    }
                }
            }

            app.manage(apply_state);

            // Initialize Editor Bridge
            app.manage(EditorBridge::new());
            println!("[RUST LOG] ApplySystemState and EditorBridge initialized in setup");

            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            if !app_data_dir.exists() {
                std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");
            }

            // Note: Vector stores are now managed in SQLite database per project
            // No need to load from legacy gluon_vectors.json file
            let db_path = app_data_dir.join("gluon.db");
            let db_url = format!(
                "sqlite://{}",
                db_path.to_str().expect("DB path is not valid UTF-8")
            );

            let pool = tauri::async_runtime::block_on(async {
                // Sprawdź czy baza istnieje
                let db_exists = sqlx::Sqlite::database_exists(&db_url)
                    .await
                    .unwrap_or(false);

                if !db_exists {
                    println!("Creating new database...");
                    sqlx::Sqlite::create_database(&db_url)
                        .await
                        .expect("Failed to create DB");
                }

                // Połącz z bazą
                let pool = SqlitePool::connect(&db_url)
                    .await
                    .expect("Failed to connect to DB");

                // Spróbuj uruchomić migracje
                match sqlx::migrate!("./migrations").run(&pool).await {
                    Ok(_) => {
                        println!("Database migrations completed successfully");
                        pool
                    }
                    Err(sqlx::migrate::MigrateError::VersionMismatch(version)) => {
                        println!(
                            "⚠️ Migration version mismatch detected (version: {})",
                            version
                        );
                        println!("⚠️ This usually happens after migration restructuring.");
                        println!("🔄 Recreating database...");

                        // Zamknij połączenie
                        pool.close().await;

                        // Usuń starą bazę
                        if let Err(e) = std::fs::remove_file(&db_path) {
                            eprintln!("Failed to remove old database: {}", e);
                            panic!(
                                "Cannot recreate database. Please manually delete: {:?}",
                                db_path
                            );
                        }

                        // Utwórz nową bazę
                        sqlx::Sqlite::create_database(&db_url)
                            .await
                            .expect("Failed to recreate DB");
                        let new_pool = SqlitePool::connect(&db_url)
                            .await
                            .expect("Failed to connect to new DB");

                        // Uruchom migracje na czystej bazie
                        sqlx::migrate!("./migrations")
                            .run(&new_pool)
                            .await
                            .expect("Failed to run migrations on fresh database");

                        println!("✅ Database recreated successfully");
                        new_pool
                    }
                    Err(e) => {
                        eprintln!("Migration error: {}", e);
                        panic!("Failed to run migrations: {:?}", e);
                    }
                }
            });
            app.manage(pool.clone());

            // Initialize VectorMapManager with the database pool
            let vector_map_manager = Arc::new(VectorMapManager::new(pool.clone()));

            // Initialize AppState
            app.manage(AppState {
                file_tree_cache: Arc::new(TokioMutex::new(HashMap::new())),
                context_files_cache: Arc::new(TokioMutex::new(HashMap::new())),
                local_ai: LocalAiService::new(),
                vector_map_manager,
                indexing_cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            });

            println!("[RUST LOG] Setup complete, starting WebSocket server...");
            tauri::async_runtime::spawn(start_websocket_server(app_handle.clone()));

            // Uruchom heartbeat timer dla licencji (co 30 sekund)
            let pool_for_heartbeat = pool.clone();
            tauri::async_runtime::spawn(async move {
                use tokio::time::{Duration, interval};
                let mut heartbeat_interval = interval(Duration::from_secs(30));

                loop {
                    heartbeat_interval.tick().await;
                    if let Err(e) = send_license_heartbeat(&pool_for_heartbeat).await {
                        println!("[LICENSE HEARTBEAT] Error: {}", e);
                    }
                }
            });

            println!("[RUST LOG] License heartbeat system started");

            let show_i = MenuItem::with_id(app, "show", "Show Settings", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Gluon v2")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                // Emit backend-ready event after window is shown
                println!("[RUST LOG] Emitting backend-ready event to main window...");
                let _ = window.emit("backend-ready", ());
            }

            // Also emit to command_center window if it exists
            if let Some(window) = app.get_webview_window("command_center") {
                println!("[RUST LOG] Emitting backend-ready event to command_center window...");
                let _ = window.emit("backend-ready", ());
            }

            // --- CHECK IF RAG WAS ENABLED BEFORE (Optional Auto-Restore) ---
            // Sprawdzamy czy użytkownik miał włączony RAG w poprzedniej sesji
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let pool = app_handle_clone.state::<SqlitePool>();

                // Odczytaj ostatni stan RAG z bazy
                let was_enabled: Option<String> = sqlx::query_scalar(
                    "SELECT value FROM settings WHERE key = 'local_ai_enabled'"
                )
                .fetch_optional(pool.inner())
                .await
                .ok()
                .flatten();

                if let Some(enabled_str) = was_enabled {
                    if enabled_str == "true" {
                        println!("[RUST LOG] 🔄 RAG was enabled in last session - restoring state...");
                        let state = app_handle_clone.state::<AppState>();

                        if let Err(e) = state.local_ai.start_services(&app_handle_clone, pool.inner()).await {
                            println!("[RUST LOG] ⚠️ Failed to restore RAG Service: {}", e);
                        } else {
                            println!("[RUST LOG] ✅ RAG Service restored from previous session.");
                        }
                    } else {
                        println!("[RUST LOG] ℹ️ RAG was disabled in last session - not starting.");
                    }
                } else {
                    println!("[RUST LOG] ℹ️ No previous RAG state found - RAG disabled by default.");
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::ExitRequested { .. } => {
                    println!("[RUST LOG] 🛑 Exit requested. Force stopping Local AI services...");
                    let state = app_handle.state::<AppState>();
                    state.local_ai.stop_services();
                }
                _ => {}
            }
        });
}

fn main() {
    // Check if --mcp flag is present
    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--mcp".to_string()) {
        // Run as MCP server (stdio mode)
        run_mcp_server();
    } else {
        // Run normal Tauri GUI application
        run();
    }
}

/// Run Gluon as an MCP server
///
/// This mode exposes Gluon's tools via Model Context Protocol over stdio.
/// Used by Claude Desktop, Cursor, and other MCP clients.
#[tokio::main]
async fn run_mcp_server() {
    use gluon_desktop_lib::interface::mcp::McpServer;
    use gluon_desktop_lib::interface::registry::ToolRegistry;

    // Create tool registry with built-in tools
    let registry = ToolRegistry::new_with_builtin_tools();

    // Create and run MCP server
    let server = McpServer::new(registry);

    if let Err(e) = server.run().await {
        eprintln!("[MCP Server] Error: {}", e);
        std::process::exit(1);
    }
}