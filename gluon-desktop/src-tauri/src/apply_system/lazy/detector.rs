// Plik: gluon-desktop/src-tauri/src/apply_system/lazy/detector.rs

//! Lazy Block Detection
//!
//! This module handles detection and validation of lazy markers in model responses.

use crate::apply_system::features::prompts::{LazyBlock, LazyBlockDetection, LazyMarker, LazyStitcherConfig};
use std::path::Path;

/// Detect lazy blocks in a file content
pub fn detect_lazy_blocks(
    content: &str,
    file_path: &Path,
    config: &LazyStitcherConfig,
) -> LazyBlockDetection {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let marker = LazyMarker::from_extension(ext);

    let lines: Vec<&str> = content.lines().collect();
    let mut blocks = Vec::new();

    for (line_num, line) in lines.iter().enumerate() {
        if marker.matches(line) {
            // Calculate indentation
            let indentation = line.len() - line.trim_start().len();

            // Extract context before
            // [GLUON FIX] Relaxed context extraction to avoid panic on usize underflow
            let context_start = line_num.saturating_sub(config.min_context_before);
            let context_before: Vec<String> = lines[context_start..line_num]
                .iter()
                .map(|s| s.to_string())
                .collect();

            // Extract context after
            let context_end = (line_num + 1 + config.min_context_after).min(lines.len());
            let context_after: Vec<String> = lines[line_num + 1..context_end]
                .iter()
                .map(|s| s.to_string())
                .collect();

            blocks.push(LazyBlock {
                line_number: line_num,
                marker: marker.clone(),
                context_before,
                context_after,
                indentation,
            });
        }
    }

    let is_lazy_response = !blocks.is_empty();
    let mut detection = LazyBlockDetection {
        blocks,
        is_lazy_response,
        validation_errors: Vec::new(),
    };

    // Validate all blocks
    detection.validate(config);

    detection
}

/// Check if a given text contains any lazy marker patterns
pub fn contains_lazy_markers(content: &str) -> bool {
    lazy_static::lazy_static! {
        static ref LAZY_REGEX: regex::Regex = regex::Regex::new(
            r"(?://|#|<!--|/\*)\s*\.\.\.\s*existing code\s*\.\.\.\s*(?:-->|\*/)?",
        ).unwrap();
    }

    LAZY_REGEX.is_match(content)
}

impl LazyBlockDetection {
    /// Validate that all lazy blocks have sufficient context
    pub fn validate(&mut self, config: &LazyStitcherConfig) {
        self.validation_errors.clear();
        let blocks_count = self.blocks.len();

        for (idx, block) in self.blocks.iter().enumerate() {
            // [GLUON FIX] Relaxed validation for FIRST block
            // If it's the first block, we allow slightly less context before (e.g. 1 line header)
            let min_before = if idx == 0 { 1 } else { config.min_context_before };
            
            if block.context_before.len() < min_before {
                self.validation_errors.push(format!(
                    "Block {} at line {}: Insufficient context before (need {}, got {})",
                    idx + 1,
                    block.line_number,
                    min_before,
                    block.context_before.len()
                ));
            }

            // [GLUON FIX] Relaxed validation for LAST block (Tail Marker)
            // If it's the last block, having 0 context lines after is VALID.
            // It simply means "keep everything until the end of the file".
            let is_last = idx == blocks_count - 1;
            let min_after = if is_last { 0 } else { config.min_context_after };

            if block.context_after.len() < min_after {
                self.validation_errors.push(format!(
                    "Block {} at line {}: Insufficient context after (need {}, got {})",
                    idx + 1,
                    block.line_number,
                    min_after,
                    block.context_after.len()
                ));
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        self.is_lazy_response && self.validation_errors.is_empty()
    }
}