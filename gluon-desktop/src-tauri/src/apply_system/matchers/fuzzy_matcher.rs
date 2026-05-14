//! Matcher Strategy 2: Fuzzy Text Matching
//! Uses normalized Levenshtein distance to find similar code blocks.
//! 
//! Optimized in Gluon v2.2 to use O(min(N,M)) memory space instead of O(N*M).

use super::Matcher;
use crate::apply_system::shared::types::{MatchResult, MatchMethod};
use std::cmp::min;

pub struct FuzzyMatcher;

impl Matcher for FuzzyMatcher {
    fn find_match(&self, file_content: &str, search_block: &str, _file_path: Option<&str>) -> Option<MatchResult> {
        let file_lines: Vec<&str> = file_content.lines().collect();
        let search_lines: Vec<&str> = search_block.lines().collect();
        
        if search_lines.is_empty() || file_lines.is_empty() {
            return None;
        }
 
        // [GLUON V5.1] Optimization: Token Bag Pre-check (Jaccard Similarity)
        // If the file doesn't contain a significant portion of the search tokens,
        // don't bother with expensive sliding window Levenshtein.
        // This is similar to Aider's "find_similar_lines" thresholding.
        if !self.has_token_overlap(file_content, search_block, 0.3) {
             return None;
        }
 
        let search_text = search_lines.join("\n");
 
        // Strategy A: Standard Whitespace Normalization
        let normalized_search = normalize_whitespace(&search_text);
        
        // Strategy B: Skeleton Normalization (Alphanumeric only)
        let skeleton_search = normalize_skeleton(&search_text);
 
        // Strategy C: Token Stream Normalization (Gluon v2.1)
        // Keeps semantic structure but ignores formatting entirely
        let token_search = crate::apply_system::matchers::utils::normalize_token_stream(&search_text);
 
        let window_size = search_lines.len();

        // Allow window flexibility (+/- 20%) because model might add/remove blank lines
        // But also allow matching search to a single line if file has compressed code
        let min_window = if file_lines.len() == 1 { 1 } else { (window_size as f32 * 0.8).max(1.0) as usize };
        let max_window = (window_size as f32 * 1.2).ceil() as usize;

        let mut best_similarity = 0.0;
        // Store candidates as (score, start_idx, end_idx)
        let mut candidates: Vec<(f32, usize, usize)> = Vec::new();

        // Optimized Sliding Window
        for i in 0..file_lines.len() {
            // Calculate max possible window size at this position
            let max_possible_window = file_lines.len() - i;
            if max_possible_window == 0 { break; }

            // Determine window range: start from min_window or 1 if file is smaller
            let actual_min_window = min(min_window, max_possible_window);
            let actual_max_window = min(max_window, max_possible_window);

            // Try different window sizes
            for ws in actual_min_window..=actual_max_window {
                let current_window = &file_lines[i..i + ws];

                // [GLUON OPTIMIZATION 3.1] First-Line Filter (Short Circuit)
                // Before processing the whole block (expensive Levenshtein), check if the first line matches.
                // This reduces complexity from O(N*M) to nearly O(N) for mismatching regions.
                if !search_lines.is_empty() {
                    let first_search = normalize_whitespace(search_lines[0]);
                    let first_window = normalize_whitespace(current_window[0]);
                    // Using a loose threshold (40%) to be safe, but skip obviously wrong starts
                    if calculate_similarity(&first_search, &first_window) < 0.4 {
                        continue;
                    }
                }

                let window_text = current_window.join("\n");
                
                // Score A: Standard
                let normalized_window = normalize_whitespace(&window_text);
                let sim_a = calculate_similarity(&normalized_search, &normalized_window);
                
                // Score B & C: Advanced Structural Matching
                let mut final_score = sim_a;
 
                // If standard match is weak, try deeper structural matching
                if sim_a > 0.6 {
                    let skeleton_window = normalize_skeleton(&window_text);
                    let sim_b = calculate_similarity(&skeleton_search, &skeleton_window);
 
                    let token_window = crate::apply_system::matchers::utils::normalize_token_stream(&window_text);
                    let sim_c = calculate_similarity(&token_search, &token_window);
                    
                    // Prioritize Token Match (sim_c) as it is most robust to formatting
                    let best_structural = sim_b.max(sim_c);
 
                    if best_structural > sim_a {
                        // Strong boost for structural match
                        final_score = (sim_a * 0.3) + (best_structural * 0.7);
                    }
                }
 
                // Optimization: Track global best to prune weak candidates early
                if final_score > best_similarity {
                    best_similarity = final_score;
                }
 
                // Only store potentially valid candidates
                if final_score > 0.65 {
                    candidates.push((final_score, i, i + ws));
                }
            }
        }
 
        // Adaptive Threshold Calculation
        let text_len = normalized_search.len();
        let threshold = match text_len {
            0..=50 => 0.90,   // Very short snippets: must be almost exact
            51..=200 => 0.80, // Short-medium blocks: high precision
            _ => 0.70         // Long blocks: allows for more noise/formatting diffs
        };
 
        if best_similarity < threshold {
            return None;
        }
 
        // [GLUON FAIL-SAFE] Ambiguity Check
        // Filter candidates that meet the threshold and are close to the best score
        let top_candidates: Vec<&(f32, usize, usize)> = candidates.iter()
            .filter(|(score, _, _)| *score >= best_similarity * 0.95 && *score >= threshold)
            .collect();
 
        if top_candidates.is_empty() { return None; }
 
        let (best_score, best_start, best_end) = top_candidates[0];
 
        // Check for DISTINCT matches (non-overlapping) with similar high scores
        // If we find another match that is far away from the best match, it's ambiguous.
        let is_ambiguous = top_candidates.iter().any(|(_, start, end)| {
            // Check if ranges overlap
            let overlap = std::cmp::max(*start, *best_start) < std::cmp::min(*end, *best_end);
            !overlap // If they DON'T overlap, we have two distinct locations = Ambiguity
        });
 
        if is_ambiguous {
            crate::gluon_error!("FuzzyMatcher", "Ambiguous match detected! Multiple distinct blocks have similarity ~{:.2}%. Rejecting to prevent wrong replacement.", best_similarity * 100.0);
            return None;
        }
 
        Some(MatchResult {
            matched_line_start: best_start + 1,
            matched_line_end: *best_end,
            method_used: MatchMethod::FuzzyMatch,
            confidence: *best_score,
            details: Some(format!("Fuzzy similarity: {:.2}%", best_score * 100.0)),
            confidence_breakdown: None,
        })
    }
}
    
    impl FuzzyMatcher {
        /// [GLUON V5.1] Quick Jaccard Similarity Check
        /// Returns true if file contains enough unique tokens from search block.
        fn has_token_overlap(&self, file_content: &str, search_block: &str, threshold: f32) -> bool {
        let search_tokens: std::collections::HashSet<&str> = search_block.split_whitespace().collect();
        if search_tokens.is_empty() { return true; } 
        
        // Optimization: Don't tokenize whole file if search is small.
        // Just check if random sample of search tokens exist in file.
        let sample_size = (search_tokens.len() as f32 * threshold).ceil() as usize;
        let mut found = 0;
        
        for token in &search_tokens {
            if file_content.contains(token) {
                found += 1;
            }
            if found >= sample_size {
                return true;
            }
        }
        
        false
    }
}

/// Normalizes code by reducing multiple whitespaces to single space and trimming
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Aggressive normalization: keeps only alphanumeric characters.
/// Removes punctuation, brackets, quotes, whitespace.
/// Useful for matching logic when formatting/syntax details differ.
fn normalize_skeleton(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}
 
// Token Stream Normalizer moved to matchers/utils.rs to be shared with TransactionManager

/// Calculates similarity between two strings (0.0 to 1.0)
fn calculate_similarity(s1: &str, s2: &str) -> f32 {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    let max_len = std::cmp::max(len1, len2);

    if max_len == 0 { return 1.0; }

    // [GLUON OPTIMIZATION] Avoid expensive Levenshtein for very long strings
    // For strings > 5000 chars, use faster approximate similarity (Jaccard-style)
    if max_len > 5000 {
        return calculate_jaccard_similarity(s1, s2);
    }

    let distance = levenshtein_distance(s1, s2);
    1.0 - (distance as f32 / max_len as f32)
}

/// Fast approximate similarity for very long strings using token overlap (Jaccard-style)
fn calculate_jaccard_similarity(s1: &str, s2: &str) -> f32 {
    use std::collections::HashSet;

    let tokens1: HashSet<&str> = s1.split_whitespace().collect();
    let tokens2: HashSet<&str> = s2.split_whitespace().collect();

    if tokens1.is_empty() && tokens2.is_empty() {
        return 1.0;
    }

    let intersection = tokens1.intersection(&tokens2).count();
    let union = tokens1.union(&tokens2).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f32 / union as f32
}

/// Optimized Levenshtein distance algorithm (Two-Row Space Efficient)
/// Reduces memory complexity from O(N*M) to O(min(N,M))
///
/// This implementation uses only two rows (conceptually) to calculate the distance,
/// vastly reducing memory allocation for large files/strings.
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let len1 = v1.len();
    let len2 = v2.len();

    // Ensure we iterate over the shorter string to minimize memory
    if len1 > len2 {
        return levenshtein_distance(s2, s1);
    }

    // We only need two rows: previous and current.
    // 'cache' stores the previous row.
    let mut cache: Vec<usize> = (0..=len1).collect();
    let mut dist_diag;
    let mut dist_left;

    for j in 1..=len2 {
        dist_diag = cache[0];
        cache[0] = j;

        for i in 1..=len1 {
            dist_left = cache[i];
            let cost = if v1[i - 1] == v2[j - 1] { 0 } else { 1 };
            
            cache[i] = min(
                min(cache[i] + 1, cache[i - 1] + 1), // insertion, deletion
                dist_diag + cost                     // substitution
            );
            
            dist_diag = dist_left;
        }
    }

    cache[len1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_exact() {
        assert_eq!(levenshtein_distance("test", "test"), 0);
    }

    #[test]
    fn test_levenshtein_diff() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn test_fuzzy_match_whitespace_insensitive() {
        let file = "function  test ( ) { return 1; }";
        let search = "function test() {\n  return 1;\n}";
        
        let matcher = FuzzyMatcher;
        let res = matcher.find_match(file, search, None).unwrap();
        
        assert!(res.confidence > 0.95);
        assert_eq!(res.matched_line_start, 1);
    }
 
    #[test]
    fn test_skeleton_match_punctuation_insensitive() {
        // Test ignoring extra comma and semicolon differences
        let file = "items = [1, 2, 3];"; 
        let search = "items = [1, 2, 3]"; // Missing semicolon in search
        
        let matcher = FuzzyMatcher;
        let res = matcher.find_match(file, search, None).unwrap();
        
        // Skeleton match should boost confidence high enough
        assert!(res.confidence > 0.9, "Confidence was {}", res.confidence);
    }
}