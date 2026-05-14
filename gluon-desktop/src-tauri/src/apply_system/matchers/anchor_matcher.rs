//! Matcher Strategy 1: Anchor Points
//! Locates code based on high-value semantic features.

use super::{Matcher, anchor_extraction};
use crate::apply_system::shared::types::{MatchResult, MatchMethod};

pub struct AnchorMatcher;

impl Matcher for AnchorMatcher {
    fn find_match(&self, file_content: &str, search_block: &str, _file_path: Option<&str>) -> Option<MatchResult> {
        let anchors = anchor_extraction::extract_anchors(search_block);
 
        // If no significant anchors found, skip this matcher
        if anchors.functions.is_empty() && anchors.classes.is_empty() && anchors.literals.is_empty() && anchors.imports.is_empty() {
            return None;
        }

        // Collect all candidate matches with their scores
        let mut candidates: Vec<(usize, i32)> = Vec::new();

        // Iterate lines to find anchors
        // We use a window scoring approach - anchors found closely together are better
        for (i, line) in file_content.lines().enumerate() {
            let mut score = 0;

            // Functions and Classes are strong anchors
            for func in &anchors.functions {
                if line.contains(func) { score += 15; }
            }
            for cls in &anchors.classes {
                if line.contains(cls) { score += 15; }
            }
            // Literals are strong anchors if unique enough
            for lit in &anchors.literals {
                if line.contains(lit) { score += 15; }
            }
            // Imports are weak but helpful context
            for imp in &anchors.imports {
                if line.contains(imp) { score += 5; }
            }

            // Only consider lines with minimum score threshold
            if score >= 15 {
                candidates.push((i, score));
            }
        }

        if candidates.is_empty() {
            return None;
        }

        // ✅ CONTEXT-AWARE DUPLICATE DETECTION & AMBIGUITY CHECK
        // If multiple candidates with same score, use context to disambiguate OR reject
        let best_candidate = if candidates.len() > 1 {
            let max_score = candidates.iter().map(|(_, s)| *s).max().unwrap();
            let top_candidates: Vec<_> = candidates.iter()
                .filter(|(_, s)| *s == max_score)
                .copied()
                .collect();

            // Strict Ambiguity Check: If we have >1 perfect candidates (>90% score match), reject
            // Unless we can disambiguate by context significantly
            if top_candidates.len() > 1 {
                crate::gluon_warn!("AnchorMatcher", "Found {} duplicate anchor matches (score: {}). Disambiguating...", top_candidates.len(), max_score);
                
                let file_lines: Vec<&str> = file_content.lines().collect();
                let search_lines: Vec<&str> = search_block.lines().collect();
 
                // [GLUON IMPROVEMENT] Context Lookahead (Next 3 Lines)
                // Check if subsequent lines match, which is a strong indicator of correct location
                let candidates_with_context: Vec<_> = top_candidates.iter().map(|&(line_idx, base_score)| {
                    let mut context_score = 0;
                    // Check next 3 lines
                    for offset in 1..4 { 
                        if offset < search_lines.len() && (line_idx + offset) < file_lines.len() {
                            let s_line = search_lines[offset].trim();
                            let f_line = file_lines[line_idx + offset].trim();
                            
                            if !s_line.is_empty() {
                                // [GLUON FIX 3.2] Fuzzy Context Verification
                                // Use Levenshtein instead of strict equality to handle whitespace/formatting drift.
                                // This prevents "Ambiguous Match" errors when context is correct but formatted differently.
                                let similarity = crate::apply_system::matchers::utils::calculate_similarity(f_line, s_line);
                                
                                if similarity > 0.90 {
                                    context_score += 10; // Perfect/Near-perfect match
                                } else if similarity > 0.65 {
                                    context_score += 5;  // Good match
                                } else if f_line.contains(s_line) {
                                    context_score += 3;  // Fallback containment
                                }
                            }
                        }
                    }
                    (line_idx, base_score, context_score)
                }).collect();
 
                // Find max context score
                let max_ctx = candidates_with_context.iter().map(|(_, _, c)| *c).max().unwrap_or(0);
                
                // Filter by context score
                let best_by_context: Vec<_> = candidates_with_context.into_iter()
                    .filter(|(_, _, c)| *c == max_ctx)
                    .map(|(l, s, _)| (l, s))
                    .collect();
 
                crate::gluon_info!("AnchorMatcher", "Context Lookahead reduced candidates from {} to {} (Max Context Score: {})", top_candidates.len(), best_by_context.len(), max_ctx);

                // [CRITICAL FIX] If NO candidate has ANY context match (max_ctx == 0),
                // the SEARCH block is ambiguous and we should REJECT IT.
                if max_ctx == 0 && best_by_context.len() > 1 {
                    crate::gluon_error!("AnchorMatcher", "AMBIGUOUS MATCH - No context validation!");
                    crate::gluon_error!("AnchorMatcher", "Found {} candidates with identical anchors but NO matching context.", best_by_context.len());
                    crate::gluon_error!("AnchorMatcher", "This means the SEARCH block is not unique enough.");
                    crate::gluon_error!("AnchorMatcher", "Candidate locations:");
                    for (i, &(l, s)) in best_by_context.iter().enumerate() {
                        let file_lines_vec: Vec<&str> = file_content.lines().collect();
                        let preview = if l < file_lines_vec.len() {
                            file_lines_vec[l].chars().take(60).collect::<String>()
                        } else {
                            "".to_string()
                        };
                        crate::gluon_error!("AnchorMatcher", "[{}] Line {} (Score {}): {:?}", i+1, l+1, s, preview);
                    }
                    crate::gluon_info!("AnchorMatcher", "TIP: Add more unique context (2-3 lines before/after) to the SEARCH block.");
                    return None;
                }

                // Heuristic 2: Indentation Tie-Breaker (if still multiple WITH valid context)
                let search_indent = Self::get_first_line_indent(search_block);
                
                let best_candidate_ref = if best_by_context.len() > 1 {
                    best_by_context.iter()
                        .min_by_key(|(line_idx, _)| {
                            let file_indent = if *line_idx < file_lines.len() {
                                Self::get_indent(file_lines[*line_idx])
                            } else { 0 };
                            (file_indent as i32 - search_indent as i32).abs()
                        })
                } else {
                    best_by_context.first()
                };
 
                if let Some(&(line, score)) = best_candidate_ref {
                    crate::gluon_info!("AnchorMatcher", "Selected match at line {} based on context lookahead & indentation", line + 1);
                    (line, score)
                } else {
                    // CRITICAL CHANGE: If truly ambiguous, do not guess.
                    // Return None to force user/AI to provide more context.
                    crate::gluon_error!("AnchorMatcher", "Ambiguous match! Multiple locations have identical score, context, and indentation.");
                    // Log the lines for debugging
                    for (i, (l, s)) in top_candidates.iter().enumerate() {
                        crate::gluon_error!("AnchorMatcher", "Candidate {}: Line {} (Score {})", i+1, l+1, s);
                    }
                    return None;
                }
            } else {
                candidates[0]
            }
        } else {
            candidates[0]
        };

        let (best_line, max_score) = best_candidate;

        // Rough estimation of end line based on search block length
        // This will be refined by Smart Scope Expansion later
        let block_len = search_block.lines().count();

        Some(MatchResult {
            matched_line_start: best_line + 1, // 1-based index
            matched_line_end: best_line + block_len,
            method_used: MatchMethod::AnchorPoints,
            confidence: 0.95, // High confidence for structural match
            details: Some(format!("Found structural anchors with score {}", max_score)),
            confidence_breakdown: None, // Legacy matcher - no breakdown
        })
    }
}

impl AnchorMatcher {
    /// Get indentation level of first non-empty line
    fn get_first_line_indent(text: &str) -> usize {
        for line in text.lines() {
            if !line.trim().is_empty() {
                return Self::get_indent(line);
            }
        }
        0
    }

    /// Get indentation level (number of leading spaces/tabs)
    fn get_indent(line: &str) -> usize {
        line.len() - line.trim_start().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anchor_match_success() {
        let file = r#"
            // Some noise
            function irrelevant() {}
            
            function targetFunction() {
                console.log("logic");
            }
        "#;
        let search = r#"
            function targetFunction() {
                console.log("logic");
            }
        "#;
        let matcher = AnchorMatcher;
        let res = matcher.find_match(file, search, Some("test.js")).unwrap();
        // targetFunction is on line 5 (0-indexed 4 + 1)
        assert_eq!(res.matched_line_start, 5);
        assert_eq!(res.method_used, MatchMethod::AnchorPoints);
    }
}