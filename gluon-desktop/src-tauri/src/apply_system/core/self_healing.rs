/// Self-Healing Loop for Failed Patches
///
/// Based on Document IV (Section 6.2: "The 'Self-Healing' Feedback Loop (Aider)")
///
/// ## Algorithm Overview:
///
/// 1. Try to apply patch with current matching
/// 2. IF FAIL → Capture error context (type, location, confidence)
/// 3. Generate repair prompt with actual code at location
/// 4. Send to AI model via IPC (Extension handles communication)
/// 5. Receive repaired patch from model
/// 6. Retry application (max 3 attempts)
/// 7. IF still fails → Ask user (human-in-the-loop)
///
/// ## Success Rate (from Aider):
/// > "This 'Human-in-the-Loop' simulation (where the linter acts as the human)
/// > resolves over 50% of initial patch failures without user intervention."
///
/// ## Safety:
/// - Max 3 retry attempts (prevent infinite loops)
/// - Max 30s total healing time (prevent blocking)
/// - Fail-open strategy (return partial status on timeout)
/// - Detailed metrics tracking (success rate, avg retries)

use serde::{Deserialize, Serialize};
use std::time::Instant;
use crate::apply_system::shared::types::{ChangeQueueItem, MatchError, MatchResult};

/// Result of a healing attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingResult {
    /// Whether healing was successful
    pub success: bool,

    /// Number of retry attempts made
    pub attempts: usize,

    /// Total time spent healing
    pub duration_ms: u128,

    /// Final match result (if successful)
    pub match_result: Option<MatchResult>,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Healing strategy used
    pub strategy: HealingStrategy,
}

/// Strategy used for healing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealingStrategy {
    /// No healing needed (first attempt succeeded)
    NoHealingNeeded,

    /// Repaired via AI feedback loop
    AiFeedback,

    /// Repaired via context reduction (Document IV Section 3.1.2)
    ContextReduction,

    /// Failed after max retries
    Failed,

    /// Timeout exceeded
    Timeout,

    /// User intervention required
    UserInterventionRequired,
}

/// Type of error that caused patch failure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatchFailureType {
    /// No match found (all matchers failed)
    NoMatch {
        /// Errors from each matcher
        matcher_errors: String,
    },

    /// Match found but confidence too low
    LowConfidence {
        /// The confidence score that was too low
        confidence: f64,

        /// Minimum required confidence
        threshold: f64,
    },

    /// Ambiguous match (multiple candidates with similar scores)
    AmbiguousMatch {
        /// Number of candidates found
        candidate_count: usize,

        /// Confidence gap between best and second-best
        confidence_gap: f64,
    },

    /// Syntax error after applying patch
    SyntaxError {
        /// Error message from tree-sitter
        error_message: String,

        /// Line number where error occurred
        line_number: Option<usize>,
    },

    /// Destruction guard blocked the change
    DestructionGuardBlocked {
        /// Reason for blocking
        reason: String,
    },
}

/// Error context extracted from failed patch attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Type of failure
    pub failure_type: PatchFailureType,

    /// The search block that failed to match
    pub search_block: String,

    /// File path where patch was attempted
    pub file_path: String,

    /// Actual code at the attempted location (if found)
    pub actual_code_at_location: Option<String>,

    /// Match result from best attempt (if any)
    pub best_match_attempt: Option<MatchResult>,

    /// Suggested fixes (from heuristics)
    pub suggested_fixes: Vec<String>,
}

/// Configuration for self-healing behavior
#[derive(Debug, Clone)]
pub struct SelfHealingConfig {
    /// Maximum number of retry attempts
    pub max_retries: usize,

    /// Timeout for each retry attempt (ms)
    pub retry_timeout_ms: u64,

    /// Total timeout for entire healing process (ms)
    pub total_timeout_ms: u64,

    /// Minimum confidence threshold to accept a match
    pub min_confidence: f64,

    /// Whether to enable context reduction strategy
    pub enable_context_reduction: bool,

    /// Whether to enable AI feedback loop
    pub enable_ai_feedback: bool,
}

impl Default for SelfHealingConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_timeout_ms: 10_000,  // 10s per retry
            total_timeout_ms: 30_000,  // 30s total
            min_confidence: 0.70,      // 70% minimum
            enable_context_reduction: true,
            enable_ai_feedback: true,
        }
    }
}

/// Extract error context from failed match attempt
///
/// This function analyzes WHY a patch failed and collects relevant information
/// to send to the AI model for repair.
///
/// # Algorithm (from Document IV, Section 6.2):
/// "Aider captures the error message (e.g., 'IndentationError at line 50' or
///  'Could not locate block'). It feeds this error back to the LLM: 'I couldn't
///  apply your edit. The search block didn't match. Here is the actual code at
///  that location: [...]. Please try again.'"
pub fn extract_error_context(
    change: &ChangeQueueItem,
    error: &MatchError,
    file_content: &str,
) -> ErrorContext {
    let failure_type = match error {
        MatchError::AllMatchersFailed { anchor_error, fuzzy_error, regex_error } => {
            PatchFailureType::NoMatch {
                matcher_errors: format!(
                    "Anchor: {}\nFuzzy: {}\nRegex: {}",
                    anchor_error, fuzzy_error, regex_error
                ),
            }
        }
        MatchError::FileReadError { path, error } => {
            PatchFailureType::NoMatch {
                matcher_errors: format!("File read error: {} - {}", path, error),
            }
        }
        MatchError::AmbiguousMatch { locations } => {
            PatchFailureType::AmbiguousMatch {
                candidate_count: locations.len(),
                confidence_gap: 0.0, // Unknown from this error type
            }
        }
    };

    // Try to extract actual code at the location model suggested
    let actual_code_at_location = extract_actual_code_at_hint_location(
        file_content,
        &change.old_code,
        change.line_start,
    );

    // Generate suggested fixes based on error type
    let suggested_fixes = generate_suggested_fixes(&failure_type, &change.old_code, file_content);

    ErrorContext {
        failure_type,
        search_block: change.old_code.clone(),
        file_path: change.file_path.clone(),
        actual_code_at_location,
        best_match_attempt: None, // Filled by caller if available
        suggested_fixes,
    }
}

/// Extract actual code at the location hinted by the model
///
/// Even if exact match failed, we can use the line_start hint to show
/// the model what the ACTUAL code looks like at that approximate location.
fn extract_actual_code_at_hint_location(
    file_content: &str,
    search_block: &str,
    hint_line: usize,
) -> Option<String> {
    let file_lines: Vec<&str> = file_content.lines().collect();
    let search_lines = search_block.lines().count();

    if hint_line == 0 || hint_line > file_lines.len() {
        return None;
    }

    // Extract same number of lines as search block, centered around hint
    let start = hint_line.saturating_sub(2).max(0); // 2 lines before
    let end = (start + search_lines + 4).min(file_lines.len()); // +4 for context

    Some(file_lines[start..end].join("\n"))
}

/// Generate suggested fixes based on error type
///
/// These are heuristics that help the model understand what went wrong.
fn generate_suggested_fixes(
    failure_type: &PatchFailureType,
    search_block: &str,
    file_content: &str,
) -> Vec<String> {
    let mut fixes = Vec::new();

    match failure_type {
        PatchFailureType::NoMatch { .. } => {
            // Check if search block has generic anchors
            let generic_patterns = ["}",  "return", "break", "continue", "//", "/*"];
            let has_generic = search_block.lines().any(|line| {
                let trimmed = line.trim();
                generic_patterns.iter().any(|&pattern| trimmed == pattern)
            });

            if has_generic {
                fixes.push(
                    "Your search block contains generic code like `}` or `return`. \
                     Use unique identifiers (function names, class names, specific variables) \
                     as anchors instead.".to_string()
                );
            }

            // Check if search block is very short
            if search_block.lines().count() < 3 {
                fixes.push(
                    "Your search block is very short. Provide at least 3 lines of \
                     unique context before and after your change.".to_string()
                );
            }

            // Check if code might have changed
            let search_tokens: Vec<&str> = search_block.split_whitespace().collect();
            let file_tokens: Vec<&str> = file_content.split_whitespace().collect();
            let common_tokens = search_tokens.iter().filter(|t| file_tokens.contains(t)).count();
            let token_match_ratio = common_tokens as f64 / search_tokens.len() as f64;

            // [GLUON UX 4.1] Indentation/Formatting Mismatch Detection
            // If tokens match highly (>90%) but exact string match failed, it's a formatting issue.
            if token_match_ratio > 0.9 {
                fixes.push(
                    "Strong token match detected (>90%), but exact match failed. \
                     This indicates an INDENTATION or FORMATTING mismatch. \
                     Use the exact whitespace from the actual file content shown above.".to_string()
                );
            } else if token_match_ratio < 0.5 {
                fixes.push(
                    "The code you're trying to match doesn't seem to exist in this file. \
                     The file may have been modified since you last saw it. \
                     Please check the current file content.".to_string()
                );
            }
        }

        PatchFailureType::LowConfidence { confidence, threshold } => {
            fixes.push(format!(
                "Match confidence ({:.1}%) is below threshold ({:.1}%). \
                 Try providing more unique context or check if the code has changed.",
                confidence * 100.0, threshold * 100.0
            ));
        }

        PatchFailureType::AmbiguousMatch { candidate_count, .. } => {
            fixes.push(format!(
                "Found {} similar matches - your search block is not unique enough. \
                 Add more specific context to disambiguate.",
                candidate_count
            ));
        }

        _ => {}
    }

    fixes
}

/// Generate repair prompt to send to AI model
///
/// This creates a prompt that:
/// 1. Explains what went wrong
/// 2. Shows the actual code at the location
/// 3. Asks the model to provide a corrected SEARCH/REPLACE block
///
/// # Format (from Document IV, Section 6.2):
/// "I couldn't apply your previous patch.
///
///  Error: Could not find unique match for search block (confidence: 45%)
///
///  You tried to change:
///  ```python
///  def process_data(items):
///      return sum(items)
///  ```
///
///  But the actual code at that location is:
///  ```python
///  def process_data(items: List[Item]) -> float:
///      return sum(item.price for item in items)
///  ```
///
///  Please provide a corrected SEARCH/REPLACE block with exact context."
pub fn generate_repair_prompt(context: &ErrorContext) -> String {
    let mut prompt = String::from("🔴 PATCH APPLICATION FAILED - REPAIR NEEDED\n\n");

    // 1. Explain the error
    prompt.push_str(&format!("**Error Type:** {}\n\n", describe_failure_type(&context.failure_type)));

    // 2. Show what the model tried to change
    prompt.push_str("**You tried to change:**\n");
    prompt.push_str(&format!("```\n{}\n```\n\n", context.search_block));

    // 3. Show actual code at location (if available)
    if let Some(ref actual_code) = context.actual_code_at_location {
        prompt.push_str("**But the actual code at that location is:**\n");
        prompt.push_str(&format!("```\n{}\n```\n\n", actual_code));
    } else {
        prompt.push_str("**The code could not be found in the file.**\n\n");
    }

    // 4. Show suggested fixes
    if !context.suggested_fixes.is_empty() {
        prompt.push_str("**Suggested fixes:**\n");
        for (i, fix) in context.suggested_fixes.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, fix));
        }
        prompt.push_str("\n");
    }

    // 5. Request corrected block
    prompt.push_str("**Please provide a corrected SEARCH/REPLACE block:**\n");
    prompt.push_str("- Use EXACT code from the actual file shown above\n");
    prompt.push_str("- Include 3+ unique lines of context before and after\n");
    prompt.push_str("- Avoid generic anchors like `}`, `return`, `//`\n");
    prompt.push_str("\n");
    prompt.push_str(&format!("**File:** {}\n", context.file_path));

    prompt
}

/// Describe failure type in human-readable format
fn describe_failure_type(failure_type: &PatchFailureType) -> String {
    match failure_type {
        PatchFailureType::NoMatch { .. } => {
            "No match found - all matching strategies failed".to_string()
        }
        PatchFailureType::LowConfidence { confidence, threshold } => {
            format!(
                "Low confidence match ({:.1}% < {:.1}% threshold)",
                confidence * 100.0,
                threshold * 100.0
            )
        }
        PatchFailureType::AmbiguousMatch { candidate_count, confidence_gap } => {
            format!(
                "Ambiguous match - {} candidates found (gap: {:.1}%)",
                candidate_count,
                confidence_gap * 100.0
            )
        }
        PatchFailureType::SyntaxError { error_message, line_number } => {
            if let Some(line) = line_number {
                format!("Syntax error at line {}: {}", line, error_message)
            } else {
                format!("Syntax error: {}", error_message)
            }
        }
        PatchFailureType::DestructionGuardBlocked { reason } => {
            format!("Destruction guard blocked: {}", reason)
        }
    }
}

/// Main self-healing loop
///
/// Attempts to apply a change with automatic retry and repair.
///
/// # Algorithm:
/// 1. Try initial application
/// 2. IF FAIL → Extract error context
/// 3. Generate repair prompt
/// 4. Request repair from model (via callback)
/// 5. Parse repaired patch
/// 6. Retry application
/// 7. Repeat up to max_retries times
/// 8. IF all fail → Return RequiresUserIntervention
///
/// # Safety:
/// - Max retries enforced
/// - Total timeout enforced
/// - Fail-open on timeout
pub async fn apply_with_healing<F>(
    change: &ChangeQueueItem,
    file_content: &str,
    config: &SelfHealingConfig,
    mut request_repair: F,
) -> HealingResult
where
    F: FnMut(&ErrorContext) -> Option<ChangeQueueItem>,
{
    let start_time = Instant::now();

    // Attempt 0: Try without healing
    // (This is handled by caller - transaction.rs)

    // If we're here, first attempt failed
    // Start healing loop

    let mut attempts = 0;
    let mut last_error: Option<String> = None;

    while attempts < config.max_retries {
        attempts += 1;

        // Check total timeout
        if start_time.elapsed().as_millis() > config.total_timeout_ms as u128 {
            return HealingResult {
                success: false,
                attempts,
                duration_ms: start_time.elapsed().as_millis(),
                match_result: None,
                error: Some("Healing timeout exceeded".to_string()),
                strategy: HealingStrategy::Timeout,
            };
        }

        // Extract error context from previous failure
        let error_context = ErrorContext {
            failure_type: PatchFailureType::NoMatch {
                matcher_errors: last_error.clone().unwrap_or_default(),
            },
            search_block: change.old_code.clone(),
            file_path: change.file_path.clone(),
            actual_code_at_location: extract_actual_code_at_hint_location(
                file_content,
                &change.old_code,
                change.line_start,
            ),
            best_match_attempt: None,
            suggested_fixes: generate_suggested_fixes(
                &PatchFailureType::NoMatch {
                    matcher_errors: String::new(),
                },
                &change.old_code,
                file_content,
            ),
        };

        // Request repair from model
        let _repaired_change = match request_repair(&error_context) {
            Some(repaired) => repaired,
            None => {
                last_error = Some("Model failed to provide repair".to_string());
                continue;
            }
        };

        // Try to apply repaired change
        // (Caller will use transaction manager to attempt apply)
        // For now, we return the repaired change for caller to try

        // This is a placeholder - actual retry logic happens in transaction.rs
        // We're just generating the repair prompt and returning it

        break;
    }

    // If we exhausted retries
    HealingResult {
        success: false,
        attempts,
        duration_ms: start_time.elapsed().as_millis(),
        match_result: None,
        error: last_error,
        strategy: if attempts >= config.max_retries {
            HealingStrategy::Failed
        } else {
            HealingStrategy::UserInterventionRequired
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_actual_code_at_hint_location() {
        let file_content = r#"line 1
line 2
line 3
line 4
line 5
line 6
line 7"#;

        let search_block = "line 3\nline 4";
        let actual = extract_actual_code_at_hint_location(file_content, search_block, 3);

        assert!(actual.is_some());
        let code = actual.unwrap();
        assert!(code.contains("line 3"));
        assert!(code.contains("line 4"));
    }

    #[test]
    fn test_generate_suggested_fixes_generic_anchors() {
        let search_block = "}\n    return result;\n}";
        let file_content = "some file content";

        let failure = PatchFailureType::NoMatch {
            matcher_errors: "test".to_string(),
        };

        let fixes = generate_suggested_fixes(&failure, search_block, file_content);

        assert!(!fixes.is_empty());
        assert!(fixes.iter().any(|f| f.contains("generic code")));
    }

    #[test]
    fn test_generate_suggested_fixes_short_block() {
        let search_block = "return value;";
        let file_content = "some content";

        let failure = PatchFailureType::NoMatch {
            matcher_errors: "test".to_string(),
        };

        let fixes = generate_suggested_fixes(&failure, search_block, file_content);

        assert!(fixes.iter().any(|f| f.contains("very short")));
    }

    #[test]
    fn test_generate_repair_prompt() {
        let context = ErrorContext {
            failure_type: PatchFailureType::LowConfidence {
                confidence: 0.65,
                threshold: 0.85,
            },
            search_block: "function foo() {\n    return 42;\n}".to_string(),
            file_path: "src/test.js".to_string(),
            actual_code_at_location: Some("function foo(x) {\n    return x * 2;\n}".to_string()),
            best_match_attempt: None,
            suggested_fixes: vec!["Add more context".to_string()],
        };

        let prompt = generate_repair_prompt(&context);

        assert!(prompt.contains("FAILED"));
        assert!(prompt.contains("You tried to change"));
        assert!(prompt.contains("actual code"));
        assert!(prompt.contains("function foo"));
        assert!(prompt.contains("Suggested fixes"));
    }

    #[test]
    fn test_describe_failure_type() {
        let no_match = PatchFailureType::NoMatch {
            matcher_errors: "test".to_string(),
        };
        let desc = describe_failure_type(&no_match);
        assert!(desc.contains("No match found"));

        let low_conf = PatchFailureType::LowConfidence {
            confidence: 0.7,
            threshold: 0.85,
        };
        let desc = describe_failure_type(&low_conf);
        assert!(desc.contains("70.0%"));
        assert!(desc.contains("85.0%"));
    }
}
