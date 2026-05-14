//! Context Graph Module
//!
//! This module implements an AST-based semantic graph of the codebase,
//! inspired by Aider's repo map system.

pub mod graph;
pub mod symbol_extractor;
pub mod ranker;
pub mod repo_map;

// Re-export main types
pub use graph::ContextGraph;
pub use symbol_extractor::{Symbol, SymbolKind, extract_symbols};
pub use ranker::{rank_files, map_target_files};