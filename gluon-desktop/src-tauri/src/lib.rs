// Gluon Apply System - AI code change application system
pub mod apply_system;
pub mod editor_bridge;
pub mod google;
pub mod workflow;
pub mod engine;    // Gluon v3 Graph Execution Engine
pub mod sensors;   // Gluon v3 Browser Instrumentation (Phase 2)
pub mod memory;    // Gluon v3 Holographic Memory (Phase 3)
pub mod interface; // Gluon v3 Interface Layer (Phase 4) - G-Protocol & MCP
pub mod ui;        // Gluon v3 Command Center (Phase 5) - The Cockpit
pub mod local_ai;      // Local AI services (RAG, embeddings)

use apply_system::ApplySystemState;
use editor_bridge::EditorBridge;
use google::google_auth::GoogleAuthState;
use sensors::SensorState;
use ui::commands::UIState;
use tauri::Manager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex as TokioMutex;
use serde::{Serialize, Deserialize};

// Import Local AI types
use local_ai::service_manager::LocalAiService;
use local_ai::vector_map_manager::VectorMapManager;

// ============================================================================
// Application State
// ============================================================================

pub struct AppState {
    pub file_tree_cache: Arc<TokioMutex<HashMap<String, (Vec<TreeNode>, usize, SystemTime)>>>,
    pub context_files_cache: Arc<TokioMutex<HashMap<PathBuf, (ContextConfig, SystemTime)>>>,
    pub local_ai: LocalAiService,
    pub vector_map_manager: Arc<VectorMapManager>,
    pub indexing_cancelled: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum NodeType {
    File,
    Directory,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub node_type: NodeType,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<u128>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContextConfig {
    pub projects: Vec<String>,
    pub selected_files: HashMap<String, Vec<String>>,
    pub attached_files: HashMap<String, Vec<String>>,
    pub environment_id: i64,
    pub prompt_ids: Vec<i64>,
    pub quick_task: Option<String>,
    pub timestamp: String,
    pub include_logs: bool,
    pub logs: Option<String>,
}


// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Initialize Apply System state
            let apply_state = ApplySystemState::new();
            app.manage(apply_state);

            // Initialize Editor Bridge
            app.manage(EditorBridge::new());

            // Initialize Google Auth state
            app.manage(GoogleAuthState::new());

            // Initialize Sensors state (Phase 2)
            app.manage(SensorState::new());

            // Initialize UI/Command Center state (Phase 5)
            app.manage(UIState::new());

            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            apply_system::tauri_commands::parse_model_response_command,
            apply_system::tauri_commands::apply_change_command,
            apply_system::tauri_commands::get_change_queue,
            apply_system::tauri_commands::apply_all_changes,
            apply_system::tauri_commands::undo_change,
            apply_system::tauri_commands::get_config,
            apply_system::tauri_commands::update_config,
            apply_system::tauri_commands::resolve_change_locations,
            apply_system::tauri_commands::refresh_context_graph,
            apply_system::tauri_commands::get_repo_map_prompt,
            apply_system::tauri_commands::preview_backup_content,
            apply_system::tauri_commands::restore_backup_files,
            apply_system::tauri_commands::run_integrity_audit,
            apply_system::tauri_commands::export_audit_report,
            // Advanced Debug Commands
            apply_system::tauri_commands::get_debug_config,
            apply_system::tauri_commands::update_debug_config,
            apply_system::tauri_commands::get_log_statistics,
            apply_system::tauri_commands::get_filtered_logs,
            apply_system::tauri_commands::export_debug_snapshot,
            apply_system::tauri_commands::clear_logs,
            apply_system::tauri_commands::init_log_persistence,
            apply_system::tauri_commands::cleanup_debug_snapshots,
            apply_system::tauri_commands::get_system_diagnostics,
            apply_system::tauri_commands::record_performance_metric,
            apply_system::tauri_commands::start_performance_trace,
            apply_system::tauri_commands::end_performance_trace,
            apply_system::tauri_commands::create_error_report,
            apply_system::tauri_commands::create_debug_snapshot,
            // Regression Testing Commands
            apply_system::tauri_commands::run_regression_tests,
            apply_system::tauri_commands::run_and_export_regression_tests,
            // Google Drive Integration Commands
            google::google_auth::set_google_credentials,
            google::google_auth::has_google_credentials,
            google::google_auth::start_google_login,
            google::google_auth::get_google_access_token,
            google::google_auth::is_google_logged_in,
            google::google_auth::google_logout,
            // Google Drive File Operations
            google::google_drive::list_drive_files,
            google::google_drive::download_file_content,
            google::google_drive::get_drive_file_info,
            // Sensors Subsystem Commands (Phase 2)
            sensors::tauri_commands::sensor_start_browser_session,
            sensors::tauri_commands::sensor_stop_browser_session,
            sensors::tauri_commands::sensor_navigate_to,
            sensors::tauri_commands::sensor_get_active_tabs,
            sensors::tauri_commands::sensor_close_tab,
            sensors::tauri_commands::sensor_get_network_logs,
            sensors::tauri_commands::sensor_get_console_logs,
            sensors::tauri_commands::sensor_get_screenshots,
            sensors::tauri_commands::sensor_clear_data,
            sensors::tauri_commands::sensor_get_config,
            sensors::tauri_commands::sensor_update_config,
            sensors::tauri_commands::sensor_get_status,
            sensors::tauri_commands::sensor_enable_sniffer,
            sensors::tauri_commands::sensor_capture_screenshot,
            // Command Center UI Commands (Phase 5)
            ui::commands::ui_pause_execution,
            ui::commands::ui_resume_execution,
            ui::commands::ui_is_paused,
            ui::commands::ui_get_execution_state,
            ui::commands::ui_create_terminal,
            ui::commands::ui_terminal_write,
            ui::commands::ui_terminal_execute,
            ui::commands::ui_terminal_interrupt,
            ui::commands::ui_terminal_resize,
            ui::commands::ui_terminal_close,
            ui::commands::ui_terminal_list,
            ui::commands::ui_inject_context,
            ui::commands::ui_subscribe_events,
            ui::commands::ui_force_node_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}