///! Weighted Anchor Matcher
///!
///! Implements the Weighted Anchoring algorithm as the primary matching strategy.
///! Based on Document IV (Section 3.3: "Weighted Anchoring and Unique Line Indexing")
///!
///! ## Algorithm Overview:
///!
///! 1. **Frequency Analysis**: Build map of line frequency in target file
///! 2. **Anchor Selection**: Find most unique line in search block
///! 3. **Anchor Locking**: Locate anchor in target file
///! 4. **Fuzzy Expansion**: Expand match outwards from anchor with tolerance
///! 5. **Confidence Calculation**: Score based on similarity + anchor quality
///!
///! ## Advantages over legacy matchers:
///!
///! - **Repetitive Code Handling**: Solves "repetitive code blindness" problem
///! - **Hallucination Tolerance**: Fuzzy expansion handles AI-generated context errors
///! - **Transparency**: Confidence breakdown shows why match was chosen
///! - **Performance**: O(n) frequency map + O(k) fuzzy expansion (k = search block size)

use super::Matcher;
use crate::apply_system::shared::types::{MatchResult, MatchMethod, ConfidenceBreakdown};
use crate::apply_system::lazy::weighted_anchoring::{
    build_frequency_map, find_best_anchor, fuzzy_expand_from_anchor,
    WeightedAnchoringConfig,
};

/// Weighted Anchor Matcher - Primary matching strategy
///
/// This matcher should be tried FIRST in the coordinator hierarchy
/// before falling back to legacy AnchorMatcher, FuzzyMatcher, etc.
pub struct WeightedAnchorMatcher {
    /// Configuration for fuzzy matching behavior
    config: WeightedAnchoringConfig,
}

impl WeightedAnchorMatcher {
    /// Create new weighted anchor matcher with default configuration
    pub fn new() -> Self {
        Self {
            config: WeightedAnchoringConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: WeightedAnchoringConfig) -> Self {
        Self { config }
    }
}

impl Default for WeightedAnchorMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Matcher for WeightedAnchorMatcher {
    fn find_match(
        &self,
        file_content: &str,
        search_block: &str,
        _file_path: Option<&str>,
    ) -> Option<MatchResult> {
        crate::gluon_info!("WeightedAnchorMatcher", "Starting match attempt...");

        // Step 1: Split content into lines
        let file_lines: Vec<String> = file_content.lines().map(|l| l.to_string()).collect();
        let search_lines: Vec<String> = search_block.lines().map(|l| l.to_string()).collect();

        crate::gluon_info!("WeightedAnchorMatcher", "File lines: {}, Search lines: {}", file_lines.len(), search_lines.len());

        if search_lines.is_empty() || file_lines.is_empty() {
            crate::gluon_warn!("WeightedAnchorMatcher", "Empty input - returning None");
            return None;
        }

        // Step 2: Build frequency map of target file
        let frequency_map = build_frequency_map(&file_lines, self.config.normalize_whitespace);
        crate::gluon_info!("WeightedAnchorMatcher", "Built frequency map with {} unique lines", frequency_map.len());

        // Step 3: Find best anchor in search block
        let anchor = match find_best_anchor(&search_lines, &frequency_map, &self.config) {
            Some(a) => {
                crate::gluon_info!("WeightedAnchorMatcher", "Found anchor at search line {}: '{}' (uniqueness={:.2}, quality={:?})",
                    a.line_index,
                    a.line_content.chars().take(50).collect::<String>(),
                    a.uniqueness_score,
                    a.quality
                );
                a
            }
            None => {
                crate::gluon_warn!("WeightedAnchorMatcher", "No suitable anchor found - returning None");
                return None;
            }
        };

        // Step 4: Fuzzy expand from anchor to find full match
        let expansion_result = match fuzzy_expand_from_anchor(
            anchor,
            &search_lines,
            &file_lines,
            &self.config,
        ) {
            Some(r) => {
                crate::gluon_info!("WeightedAnchorMatcher", "Expansion successful: lines {}-{}, confidence={:.2}",
                    r.start_line, r.end_line, r.confidence
                );
                r
            }
            None => {
                crate::gluon_warn!("WeightedAnchorMatcher", "Expansion failed - returning None");
                return None;
            }
        };

        // Step 5: Check if confidence meets threshold
        // CRITICAL FIX: Adaptive threshold based on match quality (must match fuzzy_expand_from_anchor logic!)
        let effective_threshold = if expansion_result.confidence_breakdown.similarity >= 0.99
            && expansion_result.confidence_breakdown.token_similarity >= 0.99 {
            // Near-perfect match - very confident
            let relaxed = 0.80;
            crate::gluon_info!("WeightedAnchorMatcher", "Near-perfect match (sim={:.2}, token={:.2}), using relaxed threshold {:.2}",
                expansion_result.confidence_breakdown.similarity,
                expansion_result.confidence_breakdown.token_similarity,
                relaxed
            );
            relaxed
        } else if expansion_result.confidence_breakdown.token_similarity >= 0.95 {
            // High token similarity means structure matches well (code is correct)
            // Even if some lines differ (markdown artifacts, comments), the match is valid
            let relaxed = 0.70;
            crate::gluon_info!("WeightedAnchorMatcher", "High token similarity detected ({:.2}), using relaxed threshold {:.2}",
                expansion_result.confidence_breakdown.token_similarity,
                relaxed
            );
            relaxed
        } else if expansion_result.confidence_breakdown.similarity >= 0.999 && expansion_result.confidence_breakdown.token_similarity >= 0.88 {
            // Perfect line similarity with good token coverage - reliable match (BUG FIX)
            let relaxed = 0.75;
            crate::gluon_info!("WeightedAnchorMatcher", "Perfect similarity + good token match, using relaxed threshold {:.2}", relaxed);
            relaxed
        } else {
            self.config.fuzzy_threshold
        };

        if expansion_result.confidence < effective_threshold {
            crate::gluon_warn!("WeightedAnchorMatcher", "Confidence {:.2} < threshold {:.2} - returning None",
                expansion_result.confidence, effective_threshold
            );
            return None;
        }

        crate::gluon_info!("WeightedAnchorMatcher", "Match found! Returning MatchResult");

        // Step 6: Convert confidence breakdown to types::ConfidenceBreakdown
        let confidence_breakdown = ConfidenceBreakdown {
            similarity: expansion_result.confidence_breakdown.similarity,
            token_similarity: expansion_result.confidence_breakdown.token_similarity,
            anchor_quality: expansion_result.confidence_breakdown.anchor_quality,
        };

        // Step 7: Build MatchResult
        Some(MatchResult {
            matched_line_start: expansion_result.start_line,
            matched_line_end: expansion_result.end_line,
            method_used: MatchMethod::WeightedAnchor,
            confidence: expansion_result.confidence as f32,
            details: Some(format!(
                "Weighted Anchor Match: anchor='{}' (uniqueness={:.2}, quality={:?}) at line {} | similarity={:.2}% | token_similarity={:.2}% | anchor_quality={:.2}%",
                expansion_result.anchor.line_content,
                expansion_result.anchor.uniqueness_score,
                expansion_result.anchor.quality,
                expansion_result.anchor.target_position.unwrap_or(0),
                confidence_breakdown.similarity * 100.0,
                confidence_breakdown.token_similarity * 100.0,
                confidence_breakdown.anchor_quality * 100.0,
            )),
            confidence_breakdown: Some(confidence_breakdown),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weighted_anchor_unique_function() {
        let matcher = WeightedAnchorMatcher::new();

        let file_content = r#"
// Some code
function processData() {
    const result = calculate();
    return result;
}

function other() {
    return 42;
}
"#;

        let search_block = r#"function processData() {
    const result = calculate();
    return result;
}"#;

        let result = matcher.find_match(file_content, search_block, Some("test.js"));

        assert!(result.is_some());
        let res = result.unwrap();
        assert_eq!(res.method_used, MatchMethod::WeightedAnchor);
        assert!(res.confidence > 0.85);
        assert!(res.confidence_breakdown.is_some());
    }

    #[test]
    fn test_weighted_anchor_repetitive_code() {
        // Test the "repetitive code blindness" problem from Document IV
        let matcher = WeightedAnchorMatcher::new();

        let file_content = r#"
if err != nil {
    return err
}

function processUserData(id string) error {
    // Process user
    if err != nil {
        return err
    }
}

if err != nil {
    return err
}
"#;

        // We want to match the SECOND error block (inside processUserData)
        let search_block = r#"function processUserData(id string) error {
    // Process user
    if err != nil {
        return err
    }
}"#;

        let result = matcher.find_match(file_content, search_block, Some("test.go"));

        assert!(result.is_some());
        let res = result.unwrap();

        // Should match the function block, not the standalone error blocks
        assert!(res.matched_line_start > 3); // Not the first error block
        assert_eq!(res.method_used, MatchMethod::WeightedAnchor);
    }

    #[test]
    fn test_weighted_anchor_fuzzy_tolerance() {
        // Test tolerance to minor AI hallucinations
        let matcher = WeightedAnchorMatcher::new();

        let file_content = r#"
function calculateTotal(items) {
    // Calculate total price with tax
    const subtotal = sum(items);
    return subtotal;
}
"#;

        // AI generated with slightly different comment (just one word difference)
        let search_block = r#"function calculateTotal(items) {
    // Calculate total price and tax
    const subtotal = sum(items);
    return subtotal;
}"#;

        // Use lower threshold config for this test to demonstrate fuzzy tolerance
        let mut config = WeightedAnchoringConfig::default();
        config.fuzzy_threshold = 0.80; // Lower threshold to allow minor differences
        let matcher = WeightedAnchorMatcher::with_config(config);

        let result = matcher.find_match(file_content, search_block, Some("test.js"));

        // Should match despite minor comment difference with relaxed threshold
        assert!(result.is_some(), "Should match despite minor comment difference");
        let res = result.unwrap();
        assert_eq!(res.method_used, MatchMethod::WeightedAnchor);
        assert!(res.confidence > 0.75, "Confidence should be > 0.75, got {}", res.confidence);
    }

    #[test]
    fn test_weighted_anchor_no_match_low_confidence() {
        let matcher = WeightedAnchorMatcher::new();

        let file_content = "function foo() { return 1; }";
        let search_block = "function bar() { return 999; }"; // Completely different

        let result = matcher.find_match(file_content, search_block, Some("test.js"));

        // Should return None due to low confidence
        assert!(result.is_none());
    }

    #[test]
    fn test_confidence_breakdown_populated() {
        let matcher = WeightedAnchorMatcher::new();

        let file_content = "function unique() { return 42; }";
        let search_block = "function unique() { return 42; }";

        let result = matcher.find_match(file_content, search_block, Some("test.js")).unwrap();

        // Check confidence breakdown exists
        assert!(result.confidence_breakdown.is_some());

        let breakdown = result.confidence_breakdown.unwrap();
        assert!(breakdown.similarity > 0.0);
        assert!(breakdown.token_similarity > 0.0);
        assert!(breakdown.anchor_quality > 0.0);

        // For exact match, all should be high
        assert!(breakdown.similarity > 0.95);
        assert!(breakdown.token_similarity > 0.95);
    }
}
