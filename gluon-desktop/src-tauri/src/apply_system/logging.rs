/**
 * KROK 45: Logging and Analytics
 *
 * Structured logging for debugging and performance monitoring
 */
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

/// Log level for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// Log entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub module: String,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub parsing_time_ms: Option<u128>,
    pub matching_time_ms: Option<u128>,
    pub apply_time_ms: Option<u128>,
    pub total_changes_processed: usize,
    pub successful_applies: usize,
    pub failed_applies: usize,
}

/// Logger instance
pub struct ApplySystemLogger {
    log_file: Arc<Mutex<BufWriter<File>>>,
    min_level: LogLevel,
    metrics: Arc<Mutex<PerformanceMetrics>>,
}

impl ApplySystemLogger {
    /// Create new logger
    pub fn new(log_path: PathBuf) -> Result<Self, String> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("Failed to open log file: {}", e))?;

        let writer = BufWriter::new(file);

        Ok(Self {
            log_file: Arc::new(Mutex::new(writer)),
            min_level: LogLevel::Info,
            metrics: Arc::new(Mutex::new(PerformanceMetrics::default())),
        })
    }

    /// Set minimum log level
    pub fn set_min_level(&mut self, level: LogLevel) {
        self.min_level = level;
    }

    /// Log a message
    pub fn log(
        &self,
        level: LogLevel,
        module: &str,
        message: &str,
        metadata: Option<serde_json::Value>,
    ) {
        // Filter by level
        if !self.should_log(level) {
            return;
        }

        let entry = LogEntry {
            timestamp: chrono::Local::now().to_rfc3339(),
            level,
            module: module.to_string(),
            message: message.to_string(),
            metadata,
        };

        // Write to file
        if let Ok(mut writer) = self.log_file.lock() {
            if let Ok(json) = serde_json::to_string(&entry) {
                let _ = writeln!(writer, "{}", json);
                let _ = writer.flush();
            }
        }

        // Also print to console in debug mode
        if level == LogLevel::Error || level == LogLevel::Warning {
            eprintln!("[{}] {}: {}", format!("{:?}", level), module, message);
        } else {
            println!("[{}] {}: {}", format!("{:?}", level), module, message);
        }
    }

    /// Check if should log based on level
    fn should_log(&self, level: LogLevel) -> bool {
        let min_priority = match self.min_level {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warning => 2,
            LogLevel::Error => 3,
        };

        let current_priority = match level {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warning => 2,
            LogLevel::Error => 3,
        };

        current_priority >= min_priority
    }

    /// Log debug message
    pub fn debug(&self, module: &str, message: &str) {
        self.log(LogLevel::Debug, module, message, None);
    }

    /// Log info message
    pub fn info(&self, module: &str, message: &str) {
        self.log(LogLevel::Info, module, message, None);
    }

    /// Log warning message
    pub fn warning(&self, module: &str, message: &str) {
        self.log(LogLevel::Warning, module, message, None);
    }

    /// Log error message
    pub fn error(&self, module: &str, message: &str) {
        self.log(LogLevel::Error, module, message, None);
    }

    /// Update performance metrics
    pub fn record_parsing_time(&self, duration_ms: u128) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.parsing_time_ms = Some(duration_ms);
        }
    }

    pub fn record_matching_time(&self, duration_ms: u128) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.matching_time_ms = Some(duration_ms);
        }
    }

    pub fn record_apply_time(&self, duration_ms: u128) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.apply_time_ms = Some(duration_ms);
        }
    }

    pub fn record_apply_result(&self, success: bool) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.total_changes_processed += 1;
            if success {
                metrics.successful_applies += 1;
            } else {
                metrics.failed_applies += 1;
            }
        }
    }

    /// Get current metrics
    pub fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.lock().unwrap().clone()
    }

    /// Reset metrics
    pub fn reset_metrics(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            *metrics = PerformanceMetrics::default();
        }
    }

    /// Log metrics summary
    pub fn log_metrics_summary(&self) {
        let metrics = self.get_metrics();

        let metadata = serde_json::json!({
            "parsing_time_ms": metrics.parsing_time_ms,
            "matching_time_ms": metrics.matching_time_ms,
            "apply_time_ms": metrics.apply_time_ms,
            "total_processed": metrics.total_changes_processed,
            "successful": metrics.successful_applies,
            "failed": metrics.failed_applies,
            "success_rate": if metrics.total_changes_processed > 0 {
                (metrics.successful_applies as f64 / metrics.total_changes_processed as f64) * 100.0
            } else {
                0.0
            }
        });

        self.log(
            LogLevel::Info,
            "metrics",
            "Performance summary",
            Some(metadata),
        );
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            parsing_time_ms: None,
            matching_time_ms: None,
            apply_time_ms: None,
            total_changes_processed: 0,
            successful_applies: 0,
            failed_applies: 0,
        }
    }
}

/// Global logger instance
static GLOBAL_LOGGER: OnceLock<Arc<ApplySystemLogger>> = OnceLock::new();

/// Initialize global logger
pub fn init_logger(log_path: PathBuf) -> Result<(), String> {
    let logger = ApplySystemLogger::new(log_path)?;
    GLOBAL_LOGGER
        .set(Arc::new(logger))
        .map_err(|_| "Logger already initialized".to_string())?;
    Ok(())
}

/// Get global logger
pub fn get_logger() -> Option<Arc<ApplySystemLogger>> {
    GLOBAL_LOGGER.get().cloned()
}

/// Convenience macros for logging
#[macro_export]
macro_rules! log_debug {
    ($module:expr, $message:expr) => {
        if let Some(logger) = $crate::apply_system::logging::get_logger() {
            logger.debug($module, $message);
        }
    };
}

#[macro_export]
macro_rules! log_info {
    ($module:expr, $message:expr) => {
        if let Some(logger) = $crate::apply_system::logging::get_logger() {
            logger.info($module, $message);
        }
    };
}

#[macro_export]
macro_rules! log_warning {
    ($module:expr, $message:expr) => {
        if let Some(logger) = $crate::apply_system::logging::get_logger() {
            logger.warning($module, $message);
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($module:expr, $message:expr) => {
        if let Some(logger) = $crate::apply_system::logging::get_logger() {
            logger.error($module, $message);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_logger_creation() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let logger = ApplySystemLogger::new(log_path.clone());
        assert!(logger.is_ok());

        assert!(log_path.exists());
    }

    #[test]
    fn test_logging() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let logger = ApplySystemLogger::new(log_path.clone()).unwrap();

        logger.info("test", "Test message");
        logger.warning("test", "Warning message");
        logger.error("test", "Error message");

        // Read log file
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("Test message"));
        assert!(content.contains("Warning message"));
        assert!(content.contains("Error message"));
    }

    #[test]
    fn test_metrics() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let logger = ApplySystemLogger::new(log_path).unwrap();

        logger.record_parsing_time(100);
        logger.record_matching_time(200);
        logger.record_apply_time(300);
        logger.record_apply_result(true);
        logger.record_apply_result(false);

        let metrics = logger.get_metrics();

        assert_eq!(metrics.parsing_time_ms, Some(100));
        assert_eq!(metrics.matching_time_ms, Some(200));
        assert_eq!(metrics.apply_time_ms, Some(300));
        assert_eq!(metrics.total_changes_processed, 2);
        assert_eq!(metrics.successful_applies, 1);
        assert_eq!(metrics.failed_applies, 1);
    }

    #[test]
    fn test_log_filtering() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let mut logger = ApplySystemLogger::new(log_path.clone()).unwrap();
        logger.set_min_level(LogLevel::Warning);

        logger.debug("test", "Debug message");
        logger.info("test", "Info message");
        logger.warning("test", "Warning message");

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(!content.contains("Debug message"));
        assert!(!content.contains("Info message"));
        assert!(content.contains("Warning message"));
    }
}
