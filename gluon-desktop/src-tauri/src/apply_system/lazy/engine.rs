//! Lazy Stitcher Engine
//!
//! This is the main entry point for the lazy stitching system.
//! It implements a Hybrid Strategy:
//! 1. Structural Matching (Tree-sitter anchored patches) - High Precision
//! 2. Heuristic Matching (Text-based/Diff) - High Recall (Fallback)
//! 3. Safety Guard - Verifies logical integrity before saving.

use crate::apply_system::{
    analysis::AnalysisEngine,
    lazy::{
        detector::{detect_lazy_blocks, contains_lazy_markers},
        matcher::find_lazy_replacements,
        reconstructor::reconstruct_file,
        structural_matcher::StructuralMatcher,
        safety::SafetyGuard,
    },
    parsers::IndentationNormalizer, // [ADDED] For context normalization
    features::prompts::LazyStitcherConfig,
};
use std::path::Path;
use std::sync::Mutex;

/// Result of a lazy edit application
#[derive(Debug, Clone)]
pub struct LazyEditResult {
    /// The reconstructed file content
    pub content: String,

    /// Number of lazy blocks processed
    pub lazy_blocks_count: usize,

    /// Whether the result passed syntax validation
    pub is_valid_syntax: bool,

    /// Any warnings generated during processing (Syntax errors, Safety warnings)
    pub warnings: Vec<String>,
}

/// Main lazy stitcher engine
pub struct LazyStitcherEngine {
    config: LazyStitcherConfig,
    /// Mutex protecting the stateful StructuralMatcher (contains compiled Tree-sitter parsers)
    structural_matcher: Mutex<StructuralMatcher>,
}

impl LazyStitcherEngine {
    /// Create a new lazy stitcher engine
    pub fn new(config: LazyStitcherConfig) -> Self {
        Self {
            config,
            structural_matcher: Mutex::new(StructuralMatcher::new()),
        }
    }

    /// Apply a lazy edit to a file
    pub fn apply_lazy_edit(
        &self,
        old_content: &str,
        new_lazy_content: &str,
        file_path: &Path,
    ) -> Result<LazyEditResult, String> {
        // === STEP 0: Initial Validation ===
        let detection = detect_lazy_blocks(new_lazy_content, file_path, &self.config);

        if !detection.is_lazy_response {
            return Err("No lazy markers detected in the response".to_string());
        }

        if !detection.is_valid() {
            return Err(format!(
                "Lazy block validation failed: {}",
                detection.validation_errors.join("; ")
            ));
        }

        let file_path_str = file_path.to_str().ok_or("Invalid file path")?;
        let extension = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");
        
        // Zmienna na wynik rekonstrukcji
        let reconstructed_content: String;
        let mut strategy_warnings: Vec<String> = Vec::new();
        let mut _used_strategy = "Unknown";

        // === STRATEGY 1: Structural Matching (Tree-sitter) ===
        // We try to find a surgically precise anchor match first.
        let structural_match_result = {
            if let Ok(mut matcher) = self.structural_matcher.lock() {
                matcher.find_best_match(extension, old_content, new_lazy_content)
            } else {
                None // Mutex poisoned
            }
        };

        if let Some(proposal) = structural_match_result {
            println!("[LazyStitcher] Structural match found! Confidence: {}", proposal.confidence);
            
            // Check bounds to be safe
            if proposal.original_range.end <= old_content.len() {
                // [GLUON FIX] Inner Stitching Logic
                let target_block_content = &old_content[proposal.original_range.clone()];
                
                // Klonujemy, bo możemy chcieć zmodyfikować (dodać ogon, znormalizować)
                let mut new_block_candidate = proposal.new_content.clone();

                if contains_lazy_markers(&new_block_candidate) {
                    println!("[LazyStitcher] Structural match contains lazy markers. Performing inner stitching.");
                    
                    // [GLUON FIX 3.0] Context Normalization (Indentation Alignment)
                    // Ensure the new block respects the indentation context of the target file block.
                    // This prevents context lines (like 'read_only_fields') from being treated as new insertions 
                    // due to minor indentation mismatches (e.g. 4 spaces vs 8 spaces relative to slice).
                    let context = IndentationNormalizer::detect_indentation(target_block_content);
                    let normalized_candidate = IndentationNormalizer::normalize_to_context(
                        &new_block_candidate, 
                        &context, 
                        true // preserve relative indentation within the block
                    );
                    
                    // Only apply if normalization changed something meaningful
                    if normalized_candidate.trim() == new_block_candidate.trim() {
                        new_block_candidate = normalized_candidate;
                    }

                    // [GLUON FIX 2.0] Tail Injection (Zombie Tail Fix)
                    let marker = crate::apply_system::features::prompts::LazyMarker::from_extension(extension);
                    let marker_str = marker.as_str();
                    
                    let trimmed_new = new_block_candidate.trim();
                    let has_tail_marker = trimmed_new.ends_with("existing code ...") || 
                                          trimmed_new.ends_with("existing code ... -->") || 
                                          trimmed_new.ends_with("existing code ... */");
                                          
                    if !has_tail_marker {
                        println!("[LazyStitcher] Injecting implicit tail marker to preserve structural integrity.");
                        if !new_block_candidate.ends_with('\n') {
                            new_block_candidate.push('\n');
                        }
                        // Add safe indentation (base level of target block)
                        let base_indent = " ".repeat(context.base_level);
                        new_block_candidate.push_str(&base_indent);
                        new_block_candidate.push_str(marker_str);
                        new_block_candidate.push('\n');
                    }

                    // Recursive stitching on the fragment
                    match self.apply_heuristic_fallback(target_block_content, &new_block_candidate, file_path_str) {
                        Ok(stitched) => {
                            let mut buffer = String::with_capacity(old_content.len() + stitched.len());
                            buffer.push_str(&old_content[..proposal.original_range.start]);
                            buffer.push_str(&stitched);
                            buffer.push_str(&old_content[proposal.original_range.end..]);
                            
                            reconstructed_content = buffer;
                            _used_strategy = "Structural + InnerStitch (Normalized)";
                        },
                        Err(e) => {
                            println!("[LazyStitcher] ⚠️ Inner stitching failed: {}. Fallback to global heuristic.", e);
                            reconstructed_content = self.apply_heuristic_fallback(old_content, new_lazy_content, file_path_str)?;
                            _used_strategy = "Heuristic Fallback (Inner Stitch Failed)";
                        }
                    }
                } else {
                    let mut buffer = String::with_capacity(old_content.len() + new_block_candidate.len());
                    buffer.push_str(&old_content[..proposal.original_range.start]);
                    buffer.push_str(&new_block_candidate);
                    buffer.push_str(&old_content[proposal.original_range.end..]);
                    
                    reconstructed_content = buffer;
                    _used_strategy = "Structural";
                }
            } else {
                println!("[LazyStitcher] Structural match range error. Fallback.");
                reconstructed_content = self.apply_heuristic_fallback(old_content, new_lazy_content, file_path_str)?;
                _used_strategy = "Fallback (Range Error)";
            }
        } else {
            println!("[LazyStitcher] Structural match failed. Executing heuristic fallback.");
            reconstructed_content = self.apply_heuristic_fallback(old_content, new_lazy_content, file_path_str)?;
            _used_strategy = "Heuristic Fallback";
        }

        // === STEP 3: Syntax Validation ===
        let is_valid_syntax = AnalysisEngine::parse_with_heuristics(&reconstructed_content, file_path_str).is_ok();
        if !is_valid_syntax {
            strategy_warnings.push(format!("Resulting code has syntax errors (Strategy: {})", _used_strategy));
        }

        // === STEP 4: Safety Guard ===
        if let Ok(mut matcher) = self.structural_matcher.lock() {
            let old_nodes = matcher.count_significant_nodes(extension, old_content).unwrap_or(100);
            let new_nodes = matcher.count_significant_nodes(extension, &reconstructed_content).unwrap_or(0);
            
            let reduction_ratio = if old_nodes > 0 {
                1.0 - (new_nodes as f32 / old_nodes as f32)
            } else {
                0.0
            };

            let safety_report = SafetyGuard::check_edit(old_content, &reconstructed_content, extension);
            strategy_warnings.extend(safety_report.warnings);

            if reduction_ratio > 0.5 && old_nodes > 20 {
                 let msg = format!("SAFETY BLOCK: Node count dropped by {:.1}% ({} -> {}). This looks like a lazy deletion.", 
                    reduction_ratio * 100.0, old_nodes, new_nodes);
                 if let Some(err) = safety_report.error {
                     return Err(format!("{}; {}", msg, err));
                 }
                 return Err(msg);
            }

            if let Some(err) = safety_report.error {
                return Err(format!("Safety Guard blocked edit: {}", err));
            }
        }

        Ok(LazyEditResult {
            content: reconstructed_content,
            lazy_blocks_count: 1,
            is_valid_syntax,
            warnings: strategy_warnings,
        })
    }

    /// Helper wrapper to execute global heuristic (Strategy 2)
    fn execute_global_heuristic(&self, old: &str, new: &str, path: &str) -> Result<String, String> {
        self.apply_heuristic_fallback(old, new, path)
    }

    /// Helper for the "Old" logic (Strategy 2)
    fn apply_heuristic_fallback(
        &self,
        old_content: &str,
        new_lazy_content: &str,
        file_path_str: &str
    ) -> Result<String, String> {
        let old_parsed = AnalysisEngine::parse_with_heuristics(old_content, file_path_str)
            .map_err(|e| format!("Failed to parse old file: {}", e))?;

        let new_parsed = AnalysisEngine::parse_with_heuristics(new_lazy_content, file_path_str)
            .map_err(|e| format!("Failed to parse new file: {}", e))?;

        let old_tree = old_parsed.tree;
        let new_tree = new_parsed.tree;

        let replacements = find_lazy_replacements(&old_tree, &new_tree, old_content, new_lazy_content);
        let reconstructed = reconstruct_file(old_content, new_lazy_content, &replacements);
        
        Ok(reconstructed)
    }
}

/// Convenience function for applying a lazy edit with default configuration
pub fn apply_lazy_edit(
    old_content: &str,
    new_lazy_content: &str,
    file_path: &Path,
) -> Result<LazyEditResult, String> {
    let engine = LazyStitcherEngine::new(LazyStitcherConfig::default());
    engine.apply_lazy_edit(old_content, new_lazy_content, file_path)
}