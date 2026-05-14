pub mod anchor_extraction;
pub mod anchor_matcher;
pub mod block_matcher;
pub mod coordinator;
pub mod fuzzy_matcher;
pub mod regex_matcher;
pub mod utils;
pub mod weighted_anchor_matcher;

// Re-export matcher structs for easier access in tests
pub use anchor_matcher::AnchorMatcher;
pub use block_matcher::BlockMatcher;
pub use fuzzy_matcher::FuzzyMatcher;
pub use regex_matcher::RegexMatcher;
pub use weighted_anchor_matcher::WeightedAnchorMatcher;

use crate::apply_system::shared::types::{MatchResult, MatchError};

/// Trait implemented by all matching strategies
pub trait Matcher {
    fn find_match(&self, file_content: &str, search_block: &str, file_path: Option<&str>) -> Option<MatchResult>;
}
 
/// Main entry point for matching code.
pub fn match_code(
    file_content: &str,
    old_code: &str,
    _hint_line: usize,
    file_path: Option<&str>,
) -> Result<MatchResult, MatchError> {
    // 0. Pre-process old_code (decode HTML entities only)
    let clean_old_code = utils::decode_html_entities(old_code);

    // ⚠️  DISABLED: normalize_block_indentation dedents code, causing match failures!
    // Problem: AI generates code with correct indentation (e.g., 4 spaces inside function)
    //          Dedenting to 0 makes matcher search at wrong indentation level
    // Solution: Trust AI's indentation, let FuzzyMatcher handle whitespace differences
    //
    // OLD (BROKEN):
    // let normalized_old_code = utils::normalize_block_indentation(&clean_old_code);
    //
    // NEW (FIXED):
    let normalized_old_code = clean_old_code.clone();

    // 1. Find the initial match using standard strategies
    // We try with normalized code first, fall back to raw if needed
    let mut result = coordinator::find_best_match(file_content, &normalized_old_code, file_path)
        .or_else(|_| {
            coordinator::find_best_match(file_content, old_code, file_path)
        })?;
    // [GLUON SEMANTIC ANCHOR] If search_code defines a function/class, verify that the match
    // location in the file ALSO starts with the same def/class signature.
    // Prevents fuzzy matcher from placing a `def foo` replacement inside a different function's body
    // due to structural similarity (BUG B: validate_statistics nested inside validate_ratings_summary).
    if let Some(first_line) = normalized_old_code.lines().next() {
        let trimmed_first = first_line.trim();
        let is_def_or_class = trimmed_first.starts_with("def ")
            || trimmed_first.starts_with("class ")
            || trimmed_first.starts_with("async def ");

        if is_def_or_class {
            let file_lines: Vec<&str> = file_content.lines().collect();
            let match_line_idx = result.matched_line_start.saturating_sub(1);

            if let Some(actual_line) = file_lines.get(match_line_idx) {
                let actual_trimmed = actual_line.trim();
                // Extract "def name" or "class Name" (up to first `(` or `:`)
                let search_sig: String = trimmed_first
                    .chars()
                    .take_while(|c| *c != '(' && *c != ':')
                    .collect();
                let actual_sig: String = actual_trimmed
                    .chars()
                    .take_while(|c| *c != '(' && *c != ':')
                    .collect();

                if search_sig.trim() != actual_sig.trim() {
                    crate::gluon_error!(
                        "match_code",
                        "SEMANTIC ANCHOR MISMATCH: search_code starts with '{}' but match landed on '{}'. \
                        Refusing to apply — this would insert code at the wrong structural position.",
                        trimmed_first,
                        actual_trimmed
                    );
                    return Err(MatchError::AllMatchersFailed {
                        anchor_error: format!(
                            "Semantic anchor mismatch: search_code defines '{}' but match location has '{}'. \
                            The fuzzy matcher found a structurally similar but wrong location. \
                            Extend search_code with more unique context.",
                            search_sig.trim(),
                            actual_sig.trim()
                        ),
                        fuzzy_error: String::new(),
                        regex_error: String::new(),
                    });
                }
            }
        }
    }

    // [GLUON V7.1] Failsafe: Prevent runaway matchers (e.g. uncontrolled fuzzy expansion)
    let search_len = old_code.lines().count().max(1);
    let match_len = result.matched_line_end.saturating_sub(result.matched_line_start) + 1;
    
    if match_len > search_len * 2 && match_len > search_len + 15 {
        crate::gluon_error!("match_code", "CRITICAL: Runaway matcher blocked! Match len {} vs Search len {}. Prevents JSX component shredding.", match_len, search_len);
        return Err(MatchError::AllMatchersFailed {
            anchor_error: format!("Match too large ({} lines) for search block ({} lines). Failsafe triggered.", match_len, search_len),
            fuzzy_error: String::new(),
            regex_error: String::new(),
        });
    }
 
    // [GLUON V3] Hoist scope to definition (Structure Protection)
    // DISABLE for standard Search/Replace to prevent "Header Deletion" bug.
    // We only want to hoist if we are absolutely sure the user intended to replace the whole block,
    // but in SEARCH/REPLACE partial matching is the norm.
    /* 
    let hoisted_start = utils::hoist_scope_to_definition(file_content, result.matched_line_start);
    if hoisted_start != result.matched_line_start {
        result.matched_line_start = hoisted_start;
    } 
    */
 
    // 2. [GLUON INTELLIGENCE] Conditional Scope Expansion
    // Only expand scope to the full function body if the SEARCH block looks like it intends to replace
    // a complete unit. If the search block is just a header or partial fragment, stick to exact match.
    
    let is_partial = utils::is_partial_fragment(&normalized_old_code);
    
    if !is_partial {
        let expanded_end = utils::expand_block_scope(
            file_content, 
            result.matched_line_start, 
            result.matched_line_end
        );
     
        // Logic fix: expand_block_scope returns 1-based exclusive end line.
        let safe_end = if expanded_end > 0 { expanded_end - 1 } else { 0 };
     
        if safe_end > result.matched_line_end {
            result.matched_line_end = safe_end;
            let note = format!(" (Scope expanded to line {})", safe_end);
            result.details = Some(match result.details {
                Some(d) => d + &note,
                None => note.trim_start().to_string(),
            });
        }
    } else {
        // [DEBUG] Log that we preserved partial scope
        let note = " (Partial match preserved)";
        result.details = Some(match result.details {
            Some(d) => d + note,
            None => note.trim_start().to_string(),
        });
    }

    Ok(result)
}