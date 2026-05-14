//! Advanced Logging System for Gluon
//!
//! Features:
//! - Multi-level logging (TRACE, DEBUG, INFO, WARN, ERROR, CRITICAL)
//! - Structured logging with context
//! - Log aggregation and filtering
//! - Performance metrics integration
//! - Log persistence and rotation
//! - Real-time log streaming

use std::sync::{Arc, Mutex};
use std::collections::{VecDeque, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use lazy_static::lazy_static;
use chrono::{Local, DateTime};
use serde::{Serialize, Deserialize};

// ============================================================================
// Global Singleton
// ============================================================================

lazy_static! {
    pub static ref LOG_BUFFER: Arc<Mutex<LogBuffer>> = Arc::new(Mutex::new(LogBuffer::new(5000)));
    pub static ref LOG_MANAGER: Arc<Mutex<LogManager>> = Arc::new(Mutex::new(LogManager::new()));
}

// ============================================================================
// Log Levels
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Critical = 5,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Critical => "CRITICAL",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            LogLevel::Trace => "\x1b[90m",      // Gray
            LogLevel::Debug => "\x1b[36m",      // Cyan
            LogLevel::Info => "\x1b[32m",       // Green
            LogLevel::Warn => "\x1b[33m",       // Yellow
            LogLevel::Error => "\x1b[31m",      // Red
            LogLevel::Critical => "\x1b[35m",   // Magenta
        }
    }
}

// ============================================================================
// Structured Log Entry
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<chrono::Local>,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
    pub context: HashMap<String, String>,
    pub thread_id: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl LogEntry {
    pub fn format(&self, colored: bool) -> String {
        let color = if colored { self.level.color() } else { "" };
        let reset = if colored { "\x1b[0m" } else { "" };

        let timestamp = self.timestamp.format("%H:%M:%S%.3f");
        let level = self.level.as_str();

        let mut formatted = format!(
            "{}[{}] [{}] [{}] {}{}",
            color, timestamp, level, self.module, self.message, reset
        );

        if !self.context.is_empty() {
            formatted.push_str(" | Context: ");
            for (k, v) in &self.context {
                formatted.push_str(&format!("{}={} ", k, v));
            }
        }

        if let (Some(file), Some(line)) = (&self.file, &self.line) {
            formatted.push_str(&format!(" ({}:{})", file, line));
        }

        formatted
    }
}

// ============================================================================
// Log Buffer
// ============================================================================

pub struct LogBuffer {
    capacity: usize,
    buffer: VecDeque<LogEntry>,
    min_level: LogLevel,
    enabled: bool,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: VecDeque::with_capacity(capacity),
            min_level: LogLevel::Debug,
            enabled: true,
        }
    }

    pub fn push(&mut self, entry: LogEntry) {
        if !self.enabled || entry.level < self.min_level {
            return;
        }

        // Print to console
        println!("{}", entry.format(true));

        // Add to buffer
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }

        // Clone for log manager before moving into buffer
        let entry_clone = entry.clone();
        self.buffer.push_back(entry);

        // Notify log manager
        if let Ok(mut manager) = LOG_MANAGER.lock() {
            manager.add_entry(entry_clone);
        }
    }

    pub fn get_all(&self) -> Vec<String> {
        self.buffer
            .iter()
            .map(|e| e.format(false))
            .collect()
    }

    pub fn get_filtered(&self, level: LogLevel, module: Option<&str>) -> Vec<LogEntry> {
        self.buffer
            .iter()
            .filter(|e| {
                e.level >= level && module.map_or(true, |m| e.module.contains(m))
            })
            .cloned()
            .collect()
    }

    pub fn set_min_level(&mut self, level: LogLevel) {
        self.min_level = level;
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}

// ============================================================================
// Log Manager (Persistence and Rotation)
// ============================================================================

pub struct LogManager {
    log_dir: Option<PathBuf>,
    current_log_file: Option<File>,
    max_file_size: u64,
    max_files: usize,
    entries_since_flush: usize,
    flush_interval: usize,
    statistics: LogStatistics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogStatistics {
    pub total_entries: u64,
    pub by_level: HashMap<String, u64>,
    pub by_module: HashMap<String, u64>,
    pub errors_last_hour: u64,
    pub warnings_last_hour: u64,
}

impl LogManager {
    pub fn new() -> Self {
        Self {
            log_dir: None,
            current_log_file: None,
            max_file_size: 10 * 1024 * 1024, // 10 MB
            max_files: 10,
            entries_since_flush: 0,
            flush_interval: 100,
            statistics: LogStatistics::default(),
        }
    }

    pub fn init(&mut self, log_dir: PathBuf) -> Result<(), String> {
        fs::create_dir_all(&log_dir)
            .map_err(|e| format!("Failed to create log directory: {}", e))?;

        self.log_dir = Some(log_dir.clone());
        self.rotate_if_needed()?;

        Ok(())
    }

    pub fn add_entry(&mut self, entry: LogEntry) {
        // Update statistics
        self.statistics.total_entries += 1;
        *self.statistics.by_level.entry(entry.level.as_str().to_string()).or_insert(0) += 1;
        *self.statistics.by_module.entry(entry.module.clone()).or_insert(0) += 1;

        if entry.level == LogLevel::Error {
            self.statistics.errors_last_hour += 1;
        } else if entry.level == LogLevel::Warn {
            self.statistics.warnings_last_hour += 1;
        }

        // Write to file
        if let Err(e) = self.write_to_file(&entry) {
            eprintln!("Failed to write log entry: {}", e);
        }

        self.entries_since_flush += 1;
        if self.entries_since_flush >= self.flush_interval {
            self.flush();
        }
    }

    fn write_to_file(&mut self, entry: &LogEntry) -> Result<(), String> {
        if let Err(e) = self.rotate_if_needed() {
            return Err(format!("Log rotation failed: {}", e));
        }

        if let Some(ref mut file) = self.current_log_file {
            let json = serde_json::to_string(entry)
                .map_err(|e| format!("Failed to serialize log entry: {}", e))?;

            writeln!(file, "{}", json)
                .map_err(|e| format!("Failed to write to log file: {}", e))?;
        }

        Ok(())
    }

    fn rotate_if_needed(&mut self) -> Result<(), String> {
        let log_dir = match &self.log_dir {
            Some(dir) => dir.clone(),
            None => return Ok(()),
        };

        let should_rotate = if let Some(ref file) = self.current_log_file {
            file.metadata()
                .map(|m| m.len() >= self.max_file_size)
                .unwrap_or(false)
        } else {
            true
        };

        if !should_rotate {
            return Ok(());
        }

        // Close current file
        if self.current_log_file.is_some() {
            self.flush();
            self.current_log_file = None;
        }

        // Create new log file
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
        let log_file_path = log_dir.join(format!("gluon_{}.log", timestamp));

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .map_err(|e| format!("Failed to open log file: {}", e))?;

        self.current_log_file = Some(file);

        // Clean up old log files
        self.cleanup_old_logs(&log_dir)?;

        Ok(())
    }

    fn cleanup_old_logs(&self, log_dir: &PathBuf) -> Result<(), String> {
        let mut log_files: Vec<_> = fs::read_dir(log_dir)
            .map_err(|e| format!("Failed to read log directory: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "log")
                    .unwrap_or(false)
            })
            .collect();

        if log_files.len() <= self.max_files {
            return Ok(());
        }

        // Sort by modification time (oldest first)
        log_files.sort_by_key(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .ok()
        });

        // Remove oldest files
        let to_remove = log_files.len() - self.max_files;
        for entry in log_files.iter().take(to_remove) {
            if let Err(e) = fs::remove_file(entry.path()) {
                eprintln!("Failed to remove old log file: {}", e);
            }
        }

        Ok(())
    }

    fn flush(&mut self) {
        if let Some(ref mut file) = self.current_log_file {
            let _ = file.flush();
        }
        self.entries_since_flush = 0;
    }

    pub fn get_statistics(&self) -> LogStatistics {
        self.statistics.clone()
    }
}

// ============================================================================
// Logging Macros
// ============================================================================

#[macro_export]
macro_rules! gluon_trace {
    ($module:expr, $($arg:tt)*) => {
        $crate::apply_system::logging::log_entry(
            $crate::apply_system::logging::LogLevel::Trace,
            $module,
            &format!($($arg)*),
            None,
            None,
        );
    };
}

#[macro_export]
macro_rules! gluon_debug {
    ($module:expr, $($arg:tt)*) => {
        $crate::apply_system::shared::logging::log_entry(
            $crate::apply_system::shared::logging::LogLevel::Debug,
            $module,
            &format!($($arg)*),
            Some(file!()),
            Some(line!()),
        );
    };
}

#[macro_export]
macro_rules! gluon_info {
    ($module:expr, $($arg:tt)*) => {
        $crate::apply_system::shared::logging::log_entry(
            $crate::apply_system::shared::logging::LogLevel::Info,
            $module,
            &format!($($arg)*),
            None,
            None,
        );
    };
}

#[macro_export]
macro_rules! gluon_warn {
    ($module:expr, $($arg:tt)*) => {
        $crate::apply_system::shared::logging::log_entry(
            $crate::apply_system::shared::logging::LogLevel::Warn,
            $module,
            &format!($($arg)*),
            Some(file!()),
            Some(line!()),
        );
    };
}

#[macro_export]
macro_rules! gluon_error {
    ($module:expr, $($arg:tt)*) => {
        $crate::apply_system::shared::logging::log_entry(
            $crate::apply_system::shared::logging::LogLevel::Error,
            $module,
            &format!($($arg)*),
            Some(file!()),
            Some(line!()),
        );
    };
}

#[macro_export]
macro_rules! gluon_critical {
    ($module:expr, $($arg:tt)*) => {
        $crate::apply_system::shared::logging::log_entry(
            $crate::apply_system::shared::logging::LogLevel::Critical,
            $module,
            &format!($($arg)*),
            Some(file!()),
            Some(line!()),
        );
    };
}

// Helper function for macros
pub fn log_entry(
    level: LogLevel,
    module: &str,
    message: &str,
    file: Option<&str>,
    line: Option<u32>,
) {
    let entry = LogEntry {
        timestamp: Local::now(),
        level,
        module: module.to_string(),
        message: message.to_string(),
        context: HashMap::new(),
        thread_id: format!("{:?}", std::thread::current().id()),
        file: file.map(|s| s.to_string()),
        line,
    };

    if let Ok(mut buffer) = LOG_BUFFER.lock() {
        buffer.push(entry);
    }
}

// ============================================================================
// Performance Timer
// ============================================================================

pub struct PerformanceTimer {
    name: String,
    start: std::time::Instant,
}

impl PerformanceTimer {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        gluon_debug!("Performance", "Starting timer: {}", name);
        Self {
            name,
            start: std::time::Instant::now(),
        }
    }

    pub fn lap(&self, label: &str) {
        let elapsed = self.start.elapsed();
        gluon_debug!("Performance", "{} - {}: {:?}", self.name, label, elapsed);
    }
}

impl Drop for PerformanceTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        gluon_info!("Performance", "{} completed in {:?}", self.name, elapsed);
    }
}

#[macro_export]
macro_rules! perf_timer {
    ($name:expr) => {
        $crate::apply_system::logging::PerformanceTimer::new($name)
    };
}
