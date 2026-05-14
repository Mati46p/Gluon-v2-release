//! Gluon Apply System - Core Module
//!
//! This module implements the complete system for applying AI-proposed code changes
//! directly to files, with conflict detection, multiple parsing strategies,
//! and intelligent code matching.

pub mod analysis;
pub mod context;
pub mod extraction;
pub mod lazy;
pub mod matchers;
pub mod parsers;
pub mod validators;
pub mod tauri_commands;
pub mod preset_library;

// Shared modules (types, config, logging, protocol)
pub mod shared {
    pub mod config;
    pub mod logging;
    pub mod protocol;
    pub mod types;
}

// Core functionality modules
pub mod core {
    pub mod self_healing;
    pub mod transaction;
}

// Feature modules
pub mod features {
    pub mod backup_system;
    pub mod code_quality_analyzer;
    pub mod debug_manager;
    pub mod integrity_auditor;
    pub mod prompts;
    pub mod snapshot;
}


// Re-exports for convenience
pub use shared::config::*;
pub use shared::logging::*;
pub use shared::protocol::*;
pub use shared::types::*;
pub use core::self_healing;
pub use core::transaction::TransactionManager;
pub use features::snapshot::SnapshotManager;
pub use tauri_commands::ApplySystemState;