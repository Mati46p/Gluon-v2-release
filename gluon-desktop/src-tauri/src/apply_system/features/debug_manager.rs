//! Advanced Debug System for Gluon Apply System
//!
//! Provides comprehensive debugging, diagnostics, profiling, and forensic analysis.
//! Features:
//! - Persistent debug snapshots with full context
//! - Performance profiling and metrics
//! - Error tracking and correlation
//! - Execution tracing with call graphs
//! - Visual debugging aids
//! - Export capabilities (JSON, HTML, CSV)

use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};
use serde_json::json;
use chrono::{Local, DateTime, Utc};
use tauri::Manager;
use std::time::{Duration, SystemTime};
use std::collections::HashMap;

use crate::apply_system::shared::types::ChangeQueueItem;
use crate::apply_system::shared::logging::LOG_BUFFER;

// ============================================================================
// Debug Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    /// Enable automatic snapshot creation on failures
    pub auto_snapshot: bool,
    /// Enable performance profiling
    pub profiling_enabled: bool,
    /// Enable execution tracing
    pub tracing_enabled: bool,
    /// Maximum number of snapshots to keep
    pub max_snapshots: usize,
    /// Retention period in days
    pub retention_days: u64,
    /// Enable verbose logging
    pub verbose_logging: bool,
    /// Enable memory profiling
    pub memory_profiling: bool,
    /// Context lines to capture around changes
    pub context_lines: usize,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            auto_snapshot: true,
            profiling_enabled: true,
            tracing_enabled: true,
            max_snapshots: 100,
            retention_days: 30,
            verbose_logging: false,
            memory_profiling: true,
            context_lines: 20,
        }
    }
}

// ============================================================================
// Performance Metrics
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Total operation duration
    pub total_duration: Duration,
    /// Time spent in parsing
    pub parse_time: Duration,
    /// Time spent in matching
    pub match_time: Duration,
    /// Time spent in applying
    pub apply_time: Duration,
    /// Time spent in validation
    pub validation_time: Duration,
    /// Memory usage (bytes)
    pub memory_used: u64,
    /// Peak memory usage (bytes)
    pub peak_memory: u64,
    /// Number of allocations
    pub allocations: u64,
    /// Cache hit rate
    pub cache_hit_rate: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            total_duration: Duration::ZERO,
            parse_time: Duration::ZERO,
            match_time: Duration::ZERO,
            apply_time: Duration::ZERO,
            validation_time: Duration::ZERO,
            memory_used: 0,
            peak_memory: 0,
            allocations: 0,
            cache_hit_rate: 0.0,
        }
    }
}

// ============================================================================
// Execution Trace
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub timestamp: DateTime<Utc>,
    pub phase: String,
    pub module: String,
    pub function: String,
    pub line: u32,
    pub duration_ms: u64,
    pub memory_delta: i64,
    pub details: HashMap<String, String>,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSession {
    pub session_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub traces: Vec<ExecutionTrace>,
    pub total_duration: Duration,
}

// ============================================================================
// Error Tracking
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub error_id: String,
    pub timestamp: DateTime<Utc>,
    pub severity: ErrorSeverity,
    pub category: ErrorCategory,
    pub message: String,
    pub stack_trace: Vec<String>,
    pub context: HashMap<String, String>,
    pub recovery_attempted: bool,
    pub recovery_success: bool,
    pub related_change_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Critical,
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCategory {
    Parsing,
    Matching,
    Application,
    Validation,
    FileSystem,
    Configuration,
    Network,
    Unknown,
}

// ============================================================================
// Enhanced Debug Snapshot
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugSnapshot {
    /// Unique snapshot ID
    pub snapshot_id: String,
    /// Timestamp of snapshot creation
    pub timestamp: DateTime<Utc>,
    /// Associated change
    pub change: ChangeQueueItem,
    /// File content at time of error
    pub file_content: String,
    /// Context window around the change
    pub context_window: String,
    /// All logs up to this point
    pub logs: Vec<String>,
    /// Performance metrics
    pub metrics: PerformanceMetrics,
    /// Error information
    pub error: ErrorRecord,
    /// Execution trace
    pub trace: Option<TraceSession>,
    /// System information
    pub system_info: SystemInfo,
    /// Git information
    pub git_info: Option<GitInfo>,
    /// File diff
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub os_version: String,
    pub architecture: String,
    pub cpu_count: usize,
    pub total_memory: u64,
    pub available_memory: u64,
    pub gluon_version: String,
    pub rust_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    pub branch: String,
    pub commit: String,
    pub dirty: bool,
    pub remote: Option<String>,
    pub last_commit_message: Option<String>,
}

// ============================================================================
// Main Debug Manager
// ============================================================================

pub struct DebugManager {
    config: DebugConfig,
    snapshots: Vec<DebugSnapshot>,
    traces: HashMap<String, TraceSession>,
    errors: Vec<ErrorRecord>,
    metrics: HashMap<String, PerformanceMetrics>,
}

impl DebugManager {
    pub fn new(config: DebugConfig) -> Self {
        Self {
            config,
            snapshots: Vec::new(),
            traces: HashMap::new(),
            errors: Vec::new(),
            metrics: HashMap::new(),
        }
    }

    /// Start a new trace session
    pub fn start_trace(&mut self, session_id: String) -> String {
        let session = TraceSession {
            session_id: session_id.clone(),
            start_time: Utc::now(),
            end_time: None,
            traces: Vec::new(),
            total_duration: Duration::ZERO,
        };
        self.traces.insert(session_id.clone(), session);
        session_id
    }

    /// Add a trace point
    pub fn trace(&mut self, session_id: &str, trace: ExecutionTrace) {
        if let Some(session) = self.traces.get_mut(session_id) {
            session.traces.push(trace);
        }
    }

    /// End a trace session
    pub fn end_trace(&mut self, session_id: &str) {
        if let Some(session) = self.traces.get_mut(session_id) {
            session.end_time = Some(Utc::now());
            if let Some(start) = session.traces.first() {
                if let Some(end) = session.traces.last() {
                    session.total_duration = Duration::from_millis(
                        end.duration_ms.saturating_sub(start.duration_ms)
                    );
                }
            }
        }
    }

    /// Record an error
    pub fn record_error(&mut self, error: ErrorRecord) {
        crate::gluon_error!("DebugManager",
            "Error recorded: {} - {}", error.error_id, error.message);
        self.errors.push(error);
    }

    /// Record performance metrics
    pub fn record_metrics(&mut self, operation: String, metrics: PerformanceMetrics) {
        self.metrics.insert(operation, metrics);
    }

    /// Create enhanced debug snapshot
    pub fn create_enhanced_snapshot(
        &mut self,
        change: &ChangeQueueItem,
        file_content: &str,
        error: ErrorRecord,
        trace: Option<TraceSession>,
    ) -> DebugSnapshot {
        let snapshot_id = uuid::Uuid::new_v4().to_string();

        let context_window = Self::extract_context_window(
            file_content,
            change.line_start,
            change.line_end,
            self.config.context_lines,
        );

        let logs = LOG_BUFFER.lock().unwrap().get_all();

        let system_info = Self::collect_system_info();
        let git_info = Self::collect_git_info(&change.file_path);

        let diff = Self::generate_diff(&change.old_code, &change.new_code);

        let metrics = self.metrics.get(&change.id)
            .cloned()
            .unwrap_or_default();

        let snapshot = DebugSnapshot {
            snapshot_id: snapshot_id.clone(),
            timestamp: Utc::now(),
            change: change.clone(),
            file_content: file_content.to_string(),
            context_window,
            logs,
            metrics,
            error,
            trace,
            system_info,
            git_info,
            diff,
        };

        self.snapshots.push(snapshot.clone());
        snapshot
    }

    /// Extract context window around change location
    fn extract_context_window(content: &str, start: usize, end: usize, context: usize) -> String {
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return String::new();
        }

        let start_idx = start.saturating_sub(1);
        let end_idx = end.min(lines.len());
        let window_start = start_idx.saturating_sub(context);
        let window_end = (end_idx + context).min(lines.len());

        let mut output = String::new();
        output.push_str(&format!("=== Context Window (Lines {}-{}) ===\n",
            window_start + 1, window_end));

        for i in window_start..window_end {
            let marker = if i >= start_idx && i < end_idx {
                ">>>"
            } else if i == start_idx.saturating_sub(1) || i == end_idx {
                "---"
            } else {
                "   "
            };
            output.push_str(&format!("{} {:5} | {}\n", marker, i + 1, lines[i]));
        }

        output
    }

    /// Collect system information
    fn collect_system_info() -> SystemInfo {
        SystemInfo {
            os: std::env::consts::OS.to_string(),
            os_version: sys_info::os_release().unwrap_or_default(),
            architecture: std::env::consts::ARCH.to_string(),
            cpu_count: num_cpus::get(),
            total_memory: sys_info::mem_info().map(|m| m.total).unwrap_or(0),
            available_memory: sys_info::mem_info().map(|m| m.avail).unwrap_or(0),
            gluon_version: env!("CARGO_PKG_VERSION").to_string(),
            rust_version: std::env::var("RUSTC_VERSION").unwrap_or_else(|_| "unknown".to_string()),
        }
    }

    /// Collect Git information
    fn collect_git_info(file_path: &str) -> Option<GitInfo> {
        use std::process::Command;

        let path = Path::new(file_path);
        let repo_root = path.parent()?;

        let branch = Command::new("git")
            .args(&["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(repo_root)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string());

        let commit = Command::new("git")
            .args(&["rev-parse", "HEAD"])
            .current_dir(repo_root)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string());

        let dirty = Command::new("git")
            .args(&["status", "--porcelain"])
            .current_dir(repo_root)
            .output()
            .ok()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

        let remote = Command::new("git")
            .args(&["config", "--get", "remote.origin.url"])
            .current_dir(repo_root)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string());

        let last_commit_message = Command::new("git")
            .args(&["log", "-1", "--pretty=%B"])
            .current_dir(repo_root)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string());

        Some(GitInfo {
            branch: branch?,
            commit: commit?,
            dirty,
            remote,
            last_commit_message,
        })
    }

    /// Generate unified diff
    fn generate_diff(old: &str, new: &str) -> Option<String> {
        use similar::TextDiff;

        let diff = TextDiff::from_lines(old, new);
        let mut output = String::new();

        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                similar::ChangeTag::Delete => "-",
                similar::ChangeTag::Insert => "+",
                similar::ChangeTag::Equal => " ",
            };
            output.push_str(&format!("{} {}", sign, change));
        }

        Some(output)
    }

    /// Export snapshot to multiple formats
    pub fn export_snapshot(
        &self,
        snapshot: &DebugSnapshot,
        format: ExportFormat,
        output_path: &Path,
    ) -> Result<(), String> {
        match format {
            ExportFormat::Json => self.export_json(snapshot, output_path),
            ExportFormat::Html => self.export_html(snapshot, output_path),
            ExportFormat::Markdown => self.export_markdown(snapshot, output_path),
            ExportFormat::Csv => self.export_csv(snapshot, output_path),
        }
    }

    fn export_json(&self, snapshot: &DebugSnapshot, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(snapshot)
            .map_err(|e| format!("JSON serialization failed: {}", e))?;
        fs::write(path, json)
            .map_err(|e| format!("Failed to write JSON: {}", e))
    }

    fn export_html(&self, snapshot: &DebugSnapshot, path: &Path) -> Result<(), String> {
        let html = format!(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Gluon Debug Snapshot - {}</title>
    <style>
        body {{ font-family: monospace; padding: 20px; background: #1e1e1e; color: #d4d4d4; }}
        .section {{ margin: 20px 0; padding: 15px; background: #252526; border-left: 3px solid #007acc; }}
        .error {{ color: #f48771; }}
        .success {{ color: #89d185; }}
        .warning {{ color: #dcdcaa; }}
        .code {{ background: #1e1e1e; padding: 10px; overflow-x: auto; }}
        .metric {{ display: inline-block; margin: 5px 10px; }}
        pre {{ white-space: pre-wrap; }}
    </style>
</head>
<body>
    <h1>🐛 Gluon Debug Snapshot</h1>

    <div class="section">
        <h2>📊 Overview</h2>
        <p><strong>Snapshot ID:</strong> {}</p>
        <p><strong>Timestamp:</strong> {}</p>
        <p><strong>Change ID:</strong> {}</p>
        <p><strong>File:</strong> {}</p>
    </div>

    <div class="section">
        <h2>❌ Error</h2>
        <p class="error"><strong>Severity:</strong> {:?}</p>
        <p class="error"><strong>Category:</strong> {:?}</p>
        <p class="error"><strong>Message:</strong> {}</p>
    </div>

    <div class="section">
        <h2>⚡ Performance Metrics</h2>
        <div class="metric">
            <strong>Total:</strong> {}ms
        </div>
        <div class="metric">
            <strong>Parse:</strong> {}ms
        </div>
        <div class="metric">
            <strong>Match:</strong> {}ms
        </div>
        <div class="metric">
            <strong>Apply:</strong> {}ms
        </div>
        <div class="metric">
            <strong>Memory:</strong> {} KB
        </div>
    </div>

    <div class="section">
        <h2>📝 Context Window</h2>
        <pre class="code">{}</pre>
    </div>

    <div class="section">
        <h2>🔄 Diff</h2>
        <pre class="code">{}</pre>
    </div>

    <div class="section">
        <h2>💻 System Info</h2>
        <p><strong>OS:</strong> {} {}</p>
        <p><strong>Architecture:</strong> {}</p>
        <p><strong>CPUs:</strong> {}</p>
        <p><strong>Memory:</strong> {} MB / {} MB</p>
        <p><strong>Gluon Version:</strong> {}</p>
    </div>

    <div class="section">
        <h2>📋 Logs</h2>
        <pre class="code">{}</pre>
    </div>
</body>
</html>"#,
            snapshot.snapshot_id,
            snapshot.snapshot_id,
            snapshot.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            snapshot.change.id,
            snapshot.change.file_path,
            snapshot.error.severity,
            snapshot.error.category,
            snapshot.error.message,
            snapshot.metrics.total_duration.as_millis(),
            snapshot.metrics.parse_time.as_millis(),
            snapshot.metrics.match_time.as_millis(),
            snapshot.metrics.apply_time.as_millis(),
            snapshot.metrics.memory_used / 1024,
            snapshot.context_window,
            snapshot.diff.as_ref().unwrap_or(&"N/A".to_string()),
            snapshot.system_info.os,
            snapshot.system_info.os_version,
            snapshot.system_info.architecture,
            snapshot.system_info.cpu_count,
            snapshot.system_info.available_memory / 1024 / 1024,
            snapshot.system_info.total_memory / 1024 / 1024,
            snapshot.system_info.gluon_version,
            snapshot.logs.join("\n")
        );

        fs::write(path, html)
            .map_err(|e| format!("Failed to write HTML: {}", e))
    }

    fn export_markdown(&self, snapshot: &DebugSnapshot, path: &Path) -> Result<(), String> {
        let md = format!(r#"# 🐛 Gluon Debug Snapshot

## 📊 Overview
- **Snapshot ID:** `{}`
- **Timestamp:** {}
- **Change ID:** `{}`
- **File:** `{}`

## ❌ Error
- **Severity:** {:?}
- **Category:** {:?}
- **Message:** {}

## ⚡ Performance Metrics
| Metric | Value |
|--------|-------|
| Total Duration | {}ms |
| Parse Time | {}ms |
| Match Time | {}ms |
| Apply Time | {}ms |
| Memory Used | {} KB |
| Peak Memory | {} KB |

## 📝 Context Window
```
{}
```

## 🔄 Diff
```diff
{}
```

## 💻 System Info
- **OS:** {} {}
- **Architecture:** {}
- **CPUs:** {}
- **Memory:** {} MB / {} MB
- **Gluon Version:** {}

## 🌳 Git Info
{}

## 📋 Logs
```
{}
```
"#,
            snapshot.snapshot_id,
            snapshot.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            snapshot.change.id,
            snapshot.change.file_path,
            snapshot.error.severity,
            snapshot.error.category,
            snapshot.error.message,
            snapshot.metrics.total_duration.as_millis(),
            snapshot.metrics.parse_time.as_millis(),
            snapshot.metrics.match_time.as_millis(),
            snapshot.metrics.apply_time.as_millis(),
            snapshot.metrics.memory_used / 1024,
            snapshot.metrics.peak_memory / 1024,
            snapshot.context_window,
            snapshot.diff.as_ref().unwrap_or(&"N/A".to_string()),
            snapshot.system_info.os,
            snapshot.system_info.os_version,
            snapshot.system_info.architecture,
            snapshot.system_info.cpu_count,
            snapshot.system_info.available_memory / 1024 / 1024,
            snapshot.system_info.total_memory / 1024 / 1024,
            snapshot.system_info.gluon_version,
            if let Some(git) = &snapshot.git_info {
                format!("- **Branch:** {}\n- **Commit:** {}\n- **Dirty:** {}\n- **Remote:** {}",
                    git.branch, git.commit, git.dirty,
                    git.remote.as_ref().unwrap_or(&"N/A".to_string()))
            } else {
                "Not available".to_string()
            },
            snapshot.logs.join("\n")
        );

        fs::write(path, md)
            .map_err(|e| format!("Failed to write Markdown: {}", e))
    }

    fn export_csv(&self, snapshot: &DebugSnapshot, path: &Path) -> Result<(), String> {
        let csv = format!("Metric,Value\nSnapshot ID,{}\nTimestamp,{}\nChange ID,{}\nFile,{}\nError Severity,{:?}\nError Category,{:?}\nError Message,{}\nTotal Duration (ms),{}\nParse Time (ms),{}\nMatch Time (ms),{}\nApply Time (ms),{}\nMemory Used (KB),{}\nPeak Memory (KB),{}\n",
            snapshot.snapshot_id,
            snapshot.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            snapshot.change.id,
            snapshot.change.file_path,
            snapshot.error.severity,
            snapshot.error.category,
            snapshot.error.message.replace(",", ";"),
            snapshot.metrics.total_duration.as_millis(),
            snapshot.metrics.parse_time.as_millis(),
            snapshot.metrics.match_time.as_millis(),
            snapshot.metrics.apply_time.as_millis(),
            snapshot.metrics.memory_used / 1024,
            snapshot.metrics.peak_memory / 1024,
        );

        fs::write(path, csv)
            .map_err(|e| format!("Failed to write CSV: {}", e))
    }

    /// Clean up old snapshots based on retention policy
    pub fn cleanup_old_snapshots(&mut self, app_handle: &tauri::AppHandle) -> Result<usize, String> {
        let app_data_dir = app_handle.path().app_data_dir()
            .map_err(|e| e.to_string())?;
        let debug_root = app_data_dir.join(".gluon").join("debug_snapshots");

        if !debug_root.exists() {
            return Ok(0);
        }

        let cutoff = SystemTime::now() - Duration::from_secs(self.config.retention_days * 24 * 3600);
        let mut removed = 0;

        if let Ok(entries) = fs::read_dir(&debug_root) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(created) = metadata.created() {
                        if created < cutoff {
                            if fs::remove_dir_all(entry.path()).is_ok() {
                                removed += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(removed)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Json,
    Html,
    Markdown,
    Csv,
}

// ============================================================================
// Legacy Compatibility Layer
// ============================================================================

pub struct DebugSnapshotManager;

impl DebugSnapshotManager {
    /// Legacy method - creates a basic snapshot (backward compatibility)
    pub fn create_snapshot(
        app_handle: &tauri::AppHandle,
        change: &ChangeQueueItem,
        _html_snippet: &str,
        ui_error: &str,
    ) -> Result<String, String> {
        let app_data_dir = app_handle.path().app_data_dir()
            .map_err(|e| e.to_string())?;
        let debug_root = app_data_dir.join(".gluon").join("debug_snapshots");

        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
        let safe_id = change.id.chars().take(8).collect::<String>();
        let snapshot_dir = debug_root.join(format!("fail_{}_{}", timestamp, safe_id));

        fs::create_dir_all(&snapshot_dir)
            .map_err(|e| format!("Failed to create debug dir: {}", e))?;

        // Logs
        let logs = LOG_BUFFER.lock().unwrap().get_all().join("\n");
        fs::write(snapshot_dir.join("backend_logs.txt"), logs)
            .map_err(|e| e.to_string())?;
            // Context
            let target_path = Path::new(&change.file_path);
            if target_path.exists() {
                let original_content = fs::read_to_string(target_path).unwrap_or_default();

                // [GLUON FIX] Smart Location Resolution for Snapshot
                // 1. Prefer MatchResult (Exact location found by matcher)
                // 2. Fallback to change.line_start (Hint from parser)
                // 3. Last resort: Find first line of old_code in content manually
                let (real_start, real_end) = if let Some(ref match_res) = change.match_result {
                    (match_res.matched_line_start, match_res.matched_line_end)
                } else if change.line_start > 0 {
                    (change.line_start, change.line_end)
                } else {
                    // Fallback search: find the first non-empty line of old_code
                    let search_snippet = change.old_code.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                    if !search_snippet.is_empty() {
                        if let Some(idx) = original_content.lines().position(|l| l.contains(search_snippet.trim())) {
                            (idx + 1, idx + 1 + change.old_code.lines().count())
                        } else {
                            (0, 0)
                        }
                    } else {
                        (0, 0)
                    }
                };

                let context_snippet = Self::extract_context_window(
                    &original_content,
                    real_start,
                    real_end,
                    10, // 10 lines context
                );
                fs::write(snapshot_dir.join("context_before.txt"), context_snippet)
                    .map_err(|e| e.to_string())?;
            } else {
            fs::write(snapshot_dir.join("context_before.txt"), "[FILE NOT FOUND ON DISK]")
                .map_err(|e| e.to_string())?;
        }

        // Change dump
        let clean_change_dump = format!(
            "=== SEARCH BLOCK (OLD) ===\n{}\n\n=== REPLACE BLOCK (NEW) ===\n{}",
            change.old_code, change.new_code
        );
        fs::write(snapshot_dir.join("change_extracted.txt"), clean_change_dump)
            .map_err(|e| e.to_string())?;

        // Manifest
        let manifest = json!({
            "change_id": change.id,
            "timestamp": timestamp.to_string(),
            "file_path": change.file_path,
            "ui_error": ui_error,
            "match_result": change.match_result,
            "line_start": change.line_start,
            "line_end": change.line_end,
            "change_spec": {
                "search_block": change.old_code,
                "replace_block": change.new_code
            }
        });

        fs::write(
            snapshot_dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .map_err(|e| e.to_string())?;

        // Forensic script
        let repro_script = Self::generate_forensic_script(&change.file_path, ui_error);
        fs::write(snapshot_dir.join("reproduce_issue.py"), repro_script)
            .map_err(|e| e.to_string())?;

        Ok(snapshot_dir.to_string_lossy().to_string())
    }

    fn extract_context_window(
        content: &str,
        start_line: usize,
        end_line: usize,
        context_lines: usize,
    ) -> String {
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return String::new();
        }

        let target_start = start_line.saturating_sub(1);
        let target_end = end_line.min(lines.len());
        let window_start = target_start.saturating_sub(context_lines);
        let window_end = (target_end + context_lines).min(lines.len());

        let mut output = String::new();
        output.push_str(&format!(
            "--- Lines {}-{} (Context window) ---\n",
            window_start + 1,
            window_end
        ));

        for i in window_start..window_end {
            let marker = if i >= target_start && i < target_end {
                ">"
            } else {
                " "
            };
            output.push_str(&format!("{} {:4} | {}\n", marker, i + 1, lines[i]));
        }

        output
    }

    fn generate_forensic_script(file_path: &str, error_msg: &str) -> String {
        format!(
            r#"#!/usr/bin/env python3
import json
import os
import sys
import difflib

# GLUON FORENSIC TOOL v2.0
# Target File: {}
# Original Error: {}

def load_manifest():
    with open('manifest.json', 'r', encoding='utf-8') as f:
        return json.load(f)

def load_original_file():
    if os.path.exists('original_file.txt'):
        with open('original_file.txt', 'r', encoding='utf-8') as f:
            return f.read()
    return ""

def normalize(text):
    return ' '.join(text.split())

def analyze_match_failure(original_content, search_block):
    print("\n🔍 FORENSIC ANALYSIS:")
    print("-" * 50)

    if search_block in original_content:
        print("✅ EXACT MATCH FOUND!")
        return

    norm_search = normalize(search_block)
    norm_content = normalize(original_content)

    if norm_search in norm_content:
        print("⚠️  NORMALIZED MATCH FOUND!")
        print("Reason: Whitespace mismatch")
        return

    print("❌ No exact or normalized match.")
    print("Running fuzzy scan...")

    matcher = difflib.SequenceMatcher(None, original_content, search_block)
    match = matcher.find_longest_match(0, len(original_content), 0, len(search_block))

    if match.size > 10:
        print(f"\nFound partial match of {{match.size}} characters")

def main():
    print("🚀 Starting Gluon Debug Reproduction...")
    manifest = load_manifest()
    original_content = load_original_file()
    search_block = manifest['change_spec']['search_block']

    print(f"Target File: {{manifest['file_path']}}")
    print(f"Search Block Length: {{len(search_block)}} chars")

    if not original_content:
        print("⚠️ Original file not saved. Forensic analysis limited.")
        return

    analyze_match_failure(original_content, search_block)
    print("\n⚠️  Check 'backend_logs.txt' for Rust-side traces.")

if __name__ == "__main__":
    main()
"#,
            file_path, error_msg
        )
    }
}