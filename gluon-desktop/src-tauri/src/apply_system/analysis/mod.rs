//! Analysis Engine Module
//!
//! Provides AST-based code analysis using Tree-sitter.
//! Handles language detection, parsing, and query management.

pub mod engine;
pub mod languages;
pub mod validation;
pub mod simulation;
pub mod queries;

// Re-export key types
pub use engine::AnalysisEngine;
pub use languages::SupportedLanguage;