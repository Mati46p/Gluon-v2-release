//! Parsers for AI Model Responses
//!
//! Six-tier fallback system:
//! 1. XML G-Protocol Parser (Gluon XML Patch format - highest priority)
//! 2. Git-Style SEARCH/REPLACE Parser (Unicode Box & git conflict markers)
//! 3. SEARCH/REPLACE Parser (Structured replacement format)
//! 4. Unified Diff Parser (GitHub-style)
//! 5. Structured Markdown Parser
//! 6. Aggressive Pattern Matching Parser
//!
//! Each parser attempts to extract code changes from model responses
//! in different formats.

pub mod coordinator;
pub mod git_style_search_replace;
pub mod indentation_normalizer;
pub mod lazy_stitcher;
pub mod markdown;
pub mod pattern_matching;
pub mod regression_tests;
pub mod regression_report;
pub mod search_replace;
pub mod unified_diff;
pub mod xml_gprotocol;

// Re-export new modules for convenience
pub use indentation_normalizer::IndentationNormalizer;
pub use regression_report::{RegressionReport, TestResult, TestCategory, Finding, Severity, ValidationStep, UnwantedChangeDetector};

use crate::apply_system::shared::types::{ChangeQueueItem, ParseError};
use regex::Regex;
 
/// Common trait for all parsers
///
/// Each parser attempts to parse a raw model response into structured changes.
/// Returns Ok(changes) if successful, Err(reason) if this format is not recognized.
pub trait Parser {
    /// Attempt to parse the raw response
    fn parse(&self, raw_response: &str) -> Result<Vec<ChangeQueueItem>, String>;

    /// Get the name of this parser (for logging)
    fn name(&self) -> &'static str;

    /// Quick check if this parser might work for the given input
    /// Used for optimization - can skip expensive parsing if format doesn't match
    fn can_handle(&self, raw_response: &str) -> bool;
}

/// Parse a model response using all available parsers
///
/// Tries parsers in order of priority:
/// 1. Unified Diff (most structured)
/// 2. Markdown (semi-structured)
/// 3. Pattern Matching (last resort)
///
/// Returns the first successful parse, or aggregated error if all fail.
pub fn parse_model_response(raw_response: &str) -> Result<Vec<ChangeQueueItem>, ParseError> {
    coordinator::parse_with_fallback(raw_response)
}

// ============================================================================
// Shared Utilities for All Parsers
// ============================================================================

/// Sanitize code block content - removes AI hallucinations and UI artifacts
///
/// This function filters out common mistakes that AI models make when generating
/// code patches, such as including markdown fences, UI labels, or language names
/// within the actual code content.
///
/// # Removed Artifacts:
/// - Markdown fences: ` ``` `, ` ```javascript `, etc.
/// - UI artifacts: "Code", "code", "Copy code", "Kod", etc.
/// - Language labels: "javascript", "python", "rust", etc. (when standalone)
///
/// # Arguments
/// * `lines` - The lines of code to sanitize
///
/// # Returns
/// A cleaned string with artifacts removed
pub fn sanitize_code_block(lines: Vec<&str>) -> String {
    let filtered: Vec<&str> = lines
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim();
 
            // 1. Remove markdown fences (with or without language labels)
            if trimmed.starts_with("```") {
                return false;
            }

            // 2. Remove common UI artifacts that models hallucinate
            let ui_artifacts = [
                "Code",
                "code",
                "Copy code",
                "Kod",
                "Copy",
                "Kopiuj kod",
                "END", // Sometimes models add "END" marker
                "///", // LLM thought separator
                "...", // Ellipsis on own line
            ];
            
            if ui_artifacts.iter().any(|&artifact| trimmed == artifact) {
                return false;
            }
 
            // Remove dashed lines or separators often used by LLMs (e.g. "----------------")
            if trimmed.len() > 3 && trimmed.chars().all(|c| c == '-' || c == '=' || c == '/') {
                return false;
            }

            // 3. Remove standalone language labels
            let language_labels = [
                "javascript", "typescript", "python", "rust", "java",
                "cpp", "c++", "go", "ruby", "php", "swift", "kotlin",
                "html", "css", "json", "xml", "yaml", "sql", "bash",
                "shell", "powershell", "dockerfile",
            ];

            let trimmed_lower = trimmed.to_lowercase();
            if language_labels.iter().any(|&lang| trimmed_lower == lang) {
                return false;
            }

            true
        })
        .collect();

    // IMPORTANT: Do NOT use .trim() here! It removes indentation from first/last lines.
    // Only trim the END to remove trailing whitespace/newlines.
    // This preserves the original indentation structure of the code block.
    filtered.join("\n").trim_end().to_string()
}

/// [GLUON SAFETY] Detects "Lazy Coding" (Zombie Code) in REPLACE blocks
///
/// Checks if the AI returned placeholders instead of full code in REPLACE blocks.
///
/// ⚠️ CRITICAL: This function is called ONLY on REPLACE/new_code blocks.
/// Truncation markers ARE allowed in SEARCH/old_code blocks for optimized matching,
/// but are STRICTLY FORBIDDEN in REPLACE blocks which must contain complete implementations.
///
/// Examples of forbidden patterns in REPLACE blocks:
/// - "// ... existing code ..."
/// - "// ... rest of function ..."
/// - "# ... rest of function ..."
/// - "/* ... */"
pub fn detect_lazy_coding(code: &str) -> Result<(), String> {
    let lazy_patterns = [
        // C-style comments (JS, TS, Rust, C++, etc.)
        r"(?m)^\s*//\s*\.{3,}",                 // // ...
        r"(?m)^\s*//.*existing\s+code",         // // ... existing code
        r"(?m)^\s*//.*rest\s+of",               // // ... rest of function
        r"(?m)^\s*//.*reszta\s+funkcji",        // // ... reszta funkcji (Polish)
        r"(?m)^\s*/\*\s*\.{3,}\s*\*/",          // /* ... */

        // Python/Shell comments
        r"(?m)^\s*#\s*\.{3,}",                  // # ...
        r"(?m)^\s*#.*existing\s+code",          // # ... existing code
        r"(?m)^\s*#.*rest\s+of",                // # ... rest of code
        r"(?m)^\s*#.*reszta\s+funkcji",         // # ... reszta funkcji (Polish)

        // Generic text markers
        r"(?m)^\s*\.\.\.\s*$",                  // ... (on explicit line)
        r"(?i)rest of the file",                // "rest of the file" text
        r"(?i)reszta pliku",                    // "reszta pliku" (Polish)
    ];

    for pattern in lazy_patterns {
        if let Ok(re) = Regex::new(pattern) {
            if re.is_match(code) {
                return Err(format!(
                    "🔴 LAZY CODING DETECTED in REPLACE block! Pattern: '{}'. \
                    \n\n⚠️ CRITICAL RULE VIOLATION: \
                    \n- REPLACE blocks MUST contain COMPLETE code (no placeholders!) \
                    \n- Truncation markers like '// ... rest of function ...' are ONLY allowed in SEARCH blocks \
                    \n- This patch will be REJECTED until you provide full implementation",
                    pattern
                ));
            }
        }
    }

    Ok(())
}

/// Detects truncation markers in SEARCH blocks (these are ALLOWED and expected)
///
/// Returns true if the code contains truncation markers indicating it's an abbreviated
/// SEARCH block (showing only first 5 + last 5 lines of a long function).
///
/// Recognized truncation markers:
/// - `// ... rest of function ...` (JavaScript/TypeScript/Rust/C++)
/// - `# ... rest of function ...` (Python/Bash)
/// - `// ... reszta funkcji ...` (Polish)
/// - `# ... reszta funkcji ...` (Polish)
pub fn has_truncation_marker(code: &str) -> bool {
    let truncation_patterns = [
        r"(?m)^\s*//\s*\.{3,}\s*rest\s+of\s+function",     // // ... rest of function ...
        r"(?m)^\s*#\s*\.{3,}\s*rest\s+of\s+function",      // # ... rest of function ...
        r"(?m)^\s*//\s*\.{3,}\s*reszta\s+funkcji",         // // ... reszta funkcji ...
        r"(?m)^\s*#\s*\.{3,}\s*reszta\s+funkcji",          // # ... reszta funkcji ...
        r"(?m)^\s*//\s*\.{3,}\s*rest\s+of\s+code",         // // ... rest of code ...
        r"(?m)^\s*#\s*\.{3,}\s*rest\s+of\s+code",          // # ... rest of code ...
    ];

    for pattern in truncation_patterns {
        if let Ok(re) = Regex::new(pattern) {
            if re.is_match(code) {
                return true;
            }
        }
    }

    false
}
