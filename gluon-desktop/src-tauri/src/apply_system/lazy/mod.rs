//! Lazy Stitcher Module
//!
//! This module implements the "Lazy Stitcher" engine for applying AI-proposed
//! code changes using lazy coding markers instead of exact search/replace.
//!
//! ## Architecture
//!
//! The lazy stitcher works in several phases:
//!
//! 1. **Detection**: Scan model response for lazy markers
//! 2. **AST Parsing**: Parse both old and new files with tree-sitter
//! 3. **Replacement Finding**: Use Myers-diff-like algorithm to find what code
//!    should replace each lazy block
//! 4. **Reconstruction**: Splice original code into lazy block positions
//! 5. **Validation**: Verify the result is syntactically valid
//!
//! ## Modules
//!
//! - `engine`: Core lazy stitching algorithm
//! - `detector`: Lazy block detection and validation
//! - `matcher`: AST-based matching for finding replacement nodes
//! - `reconstructor`: File reconstruction logic

pub mod detector;
pub mod engine;
pub mod matcher;
pub mod reconstructor;
pub mod structural_matcher;
pub mod safety;
pub mod weighted_anchoring;

#[cfg(test)]
mod integration_tests;

// Re-export main types
pub use detector::detect_lazy_blocks;
pub use engine::{apply_lazy_edit, LazyStitcherEngine};
pub use matcher::find_lazy_replacements;
pub use reconstructor::reconstruct_file;
pub use weighted_anchoring::{
    find_best_anchor, fuzzy_expand_from_anchor, build_frequency_map,
    WeightedAnchor, WeightedAnchoringConfig, FuzzyExpansionResult, ConfidenceBreakdown,
};
