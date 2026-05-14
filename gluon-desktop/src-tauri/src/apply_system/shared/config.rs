//! Configuration Management for Gluon Apply System
//!
//! Handles user preferences and settings:
//! - Legacy vs Enhanced mode
//! - UI variant toggles
//! - Security whitelist/blacklist
//! - Performance settings

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================================
// KROK 6: Configuration for Dual Mode Operation
// ============================================================================

/// Main configuration structure for the Apply System
///
/// This is persisted to disk and loaded on app start.
/// User can modify settings through the Tauri UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplySystemConfig {
    /// Security and path configuration
    pub path_config: PathConfig,

    /// Performance and behavior settings
    pub performance: PerformanceConfig,
}

/// Path whitelist and blacklist configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathConfig {
    /// Paths where changes are allowed
    /// Default: only workspace root
    pub whitelisted_paths: Vec<PathBuf>,

    /// Additional paths to exclude (beyond hardcoded blacklist)
    /// User can add custom folders to ignore
    pub custom_blacklist: Vec<String>,

    /// Whether to enforce strict whitelist
    /// If true, ONLY whitelisted paths are allowed
    /// If false, any path except blacklisted is allowed
    pub strict_whitelist: bool,
}

/// Performance and behavior settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceConfig {
    /// Maximum search range for fuzzy matching (lines before/after)
    pub fuzzy_search_range: usize,

    /// Minimum confidence score to auto-apply (0.0 - 1.0)
    /// Changes below this require user confirmation
    pub min_confidence_threshold: f32,

    /// Maximum number of changes to keep in history
    pub max_history_size: usize,

    /// Enable parallel processing for batch operations
    pub parallel_batch_apply: bool,

    /// Maximum concurrent apply operations (if parallel enabled)
    pub max_concurrent_applies: usize,
}

// ============================================================================
// Hardcoded Security Blacklist
// ============================================================================

/// Get the hardcoded blacklist that cannot be disabled
///
/// These paths are NEVER allowed to be modified for security reasons
pub fn get_hardcoded_blacklist() -> Vec<&'static str> {
    vec![
        // Environment files with secrets
        ".env",
        ".env.local",
        ".env.development",
        ".env.production",
        ".env.test",
        ".env.*",
        // Git directory
        ".git/",
        ".gitignore",
        // Node modules and dependencies
        "node_modules/",
        "vendor/",
        "bower_components/",
        // System files (Unix)
        "/etc/",
        "/sys/",
        "/proc/",
        "/dev/",
        // System files (Windows)
        "C:\\Windows\\",
        "C:\\Program Files\\",
        "C:\\Program Files (x86)\\",
        // System files (macOS)
        "/System/",
        "/Library/System/",
        // Build artifacts
        "dist/",
        "build/",
        "target/",
        ".next/",
        // IDE files
        ".vscode/",
        ".idea/",
        // Lock files (usually auto-generated)
        "package-lock.json",
        "yarn.lock",
        "Cargo.lock",
    ]
}

/// Check if a path is blacklisted (hardcoded or custom)
pub fn is_path_blacklisted(path: &str, config: &PathConfig) -> bool {
    // Check hardcoded blacklist
    let hardcoded = get_hardcoded_blacklist();
    for pattern in hardcoded {
        if path.contains(pattern) {
            return true;
        }
    }

    // Check custom blacklist
    for pattern in &config.custom_blacklist {
        if path.contains(pattern) {
            return true;
        }
    }

    false
}

/// Check if a path is whitelisted
pub fn is_path_whitelisted(path: &str, config: &PathConfig) -> bool {
    if !config.strict_whitelist {
        // In non-strict mode, everything except blacklist is allowed
        return !is_path_blacklisted(path, config);
    }

    // In strict mode, must be in whitelist
    let path_buf = PathBuf::from(path);
    for allowed in &config.whitelisted_paths {
        if path_buf.starts_with(allowed) {
            return true;
        }
    }

    false
}

/// Check for path traversal attacks
pub fn has_path_traversal(path: &str) -> bool {
    path.contains("../") || path.contains("..\\")
}

/// Validate a file path for security
pub fn validate_file_path(path: &str, config: &PathConfig) -> Result<(), String> {
    // Check for path traversal
    if has_path_traversal(path) {
        return Err(format!("Path traversal detected: {}", path));
    }

    // Check blacklist
    if is_path_blacklisted(path, config) {
        return Err(format!("Path is blacklisted: {}", path));
    }

    // Check whitelist
    if !is_path_whitelisted(path, config) {
        return Err(format!("Path is not whitelisted: {}", path));
    }

    Ok(())
}

// ============================================================================
// Default Configuration
// ============================================================================

impl Default for ApplySystemConfig {
    fn default() -> Self {
        Self {
            path_config: PathConfig::default(),
            performance: PerformanceConfig::default(),
        }
    }
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            // Empty whitelist means "workspace root only"
            // Will be populated with actual workspace path at runtime
            whitelisted_paths: Vec::new(),

            // No custom blacklist by default
            custom_blacklist: Vec::new(),

            // Non-strict by default (more permissive)
            strict_whitelist: false,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            // Search ±50 lines for fuzzy matching
            fuzzy_search_range: 50,

            // Require 70% confidence for auto-apply
            min_confidence_threshold: 0.7,

            // Keep last 100 changes in history
            max_history_size: 100,

            // Enable parallel processing (faster)
            parallel_batch_apply: true,

            // Max 4 concurrent applies
            max_concurrent_applies: 4,
        }
    }
}

// ============================================================================
// Configuration Persistence
// ============================================================================
// Note: Configuration persistence is reserved for future use
// Currently config is loaded from defaults only

impl ApplySystemConfig {
    // Methods for load, save, default_path, and validate are reserved for future use
}
