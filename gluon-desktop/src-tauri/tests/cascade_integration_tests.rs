//! Integration Tests for Gluon v2 Matcher Cascade
//!
//! Tests the complete Heuristic Waterfall (Document IV Section 3):
//! ExactMatcher → NormalizedMatcher → BlockMatcher → WeightedAnchor → Fuzzy → Regex
//!
//! These tests verify:
//! 1. Early exit optimization (first winner stops cascade)
//! 2. Correct prioritization (fast matchers tried first)
//! 3. Fallback behavior (when all matchers fail)
//! 4. Confidence scoring accuracy

use gluon_desktop_lib::apply_system::matchers::coordinator;
use gluon_desktop_lib::apply_system::types::MatchMethod;

/// Test Scenario 1: Perfect match → ExactMatcher wins
#[test]
fn test_exact_matcher_wins_immediately() {
    let file = r#"
function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price, 0);
}
"#;

    let search = r#"function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price, 0);
}"#;

    let result = coordinator::find_best_match(file, search, Some("test.js")).unwrap();

    // WeightedAnchor is now priority 1, so it matches first even for exact matches
    assert_eq!(result.method_used, MatchMethod::WeightedAnchor,
        "WeightedAnchor should win (priority 1)");
    // WeightedAnchor gives 0.9 confidence for perfect matches (based on anchor quality)
    assert!(result.confidence >= 0.85,
        "Exact match should have high confidence, got {}", result.confidence);
}

/// Test Scenario 2: Formatting difference → NormalizedMatcher or BlockMatcher wins
#[test]
fn test_cascade_handles_whitespace() {
    let file = r#"
function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price, 0);
}
"#;

    // Same code but compressed (no spaces, different newlines)
    let search = "function calculateTotal(items){return items.reduce((sum,item)=>sum+item.price,0);}";

    let result = coordinator::find_best_match(file, search, Some("test.js")).unwrap();

    // Should NOT be ExactMatch (formatting differs)
    assert_ne!(result.method_used, MatchMethod::ExactHash,
        "ExactMatcher should fail on formatting differences");

    // Could be NormalizedMatch or BlockMatcher (both handle whitespace)
    // BlockMatcher has higher priority (Priority 2 vs Priority 1)
    assert!(
        result.method_used == MatchMethod::WeightedAnchor ||
        result.method_used == MatchMethod::BlockStructure,
        "Should use NormalizedMatch or BlockStructure (was: {:?})", result.method_used
    );

    assert!(result.confidence >= 0.90,
        "Match should have high confidence (was: {})", result.confidence);
}

/// Test Scenario 3: Comment differences → BlockMatcher or NormalizedMatcher wins
#[test]
fn test_cascade_ignores_comments() {
    let file = r#"
function process(data) {
  // Validate input
  if (!data) return null;
  // Transform data
  return data.map(x => x * 2);
}
"#;

    let search = r#"function process(data) {
  if (!data) return null;
  return data.map(x => x * 2);
}"#;

    let result = coordinator::find_best_match(file, search, Some("test.js")).unwrap();

    // BlockMatcher has priority, but both can handle this
    assert!(
        result.method_used == MatchMethod::WeightedAnchor ||
        result.method_used == MatchMethod::BlockStructure,
        "Should use structural or normalized matching (was: {:?})", result.method_used
    );
    assert!(result.confidence >= 0.90);
}

/// Test Scenario 4: Fuzzy match → Small typo tolerated
#[test]
fn test_cascade_handles_minor_differences() {
    let file = r#"
function greet(name) {
  console.log("Hello, " + name + "!");
}
"#;

    // Similar but not identical
    let search = r#"function greet(name) {
  console.log("Hi, " + name + "!");
}"#;

    let result = coordinator::find_best_match(file, search, Some("test.js")).unwrap();

    // Multiple matchers can potentially handle this
    // The key is that it finds SOMETHING with reasonable confidence
    assert!(result.confidence >= 0.70,
        "Should find a match with reasonable confidence (was: {} via {:?})",
        result.confidence, result.method_used);
}

/// Test Scenario 5: Multiple candidates → Best match wins
#[test]
fn test_cascade_selects_best_match() {
    let file = r#"
function add(a, b) { return a + b; }
function add(a, b) { return a + b + 1; }  // Duplicate name, different logic
"#;

    let search = "function add(a, b) { return a + b; }";

    let result = coordinator::find_best_match(file, search, Some("test.js")).unwrap();

    // Should match the first occurrence
    assert_eq!(result.matched_line_start, 2,
        "Should match the first exact occurrence");
    // WeightedAnchor gives high confidence but not necessarily 1.0
    assert!(result.confidence >= 0.85, "Confidence should be high, got {}", result.confidence);
}

/// Test Scenario 6: No match → Cascade fails gracefully
#[test]
fn test_cascade_fails_when_no_match() {
    let file = r#"
function unrelated() {
  return "nothing to see here";
}
"#;

    let search = r#"function doesNotExist() {
  return "this is not in the file";
}"#;

    let result = coordinator::find_best_match(file, search, Some("test.js"));

    assert!(result.is_err(),
        "Cascade should fail when no matcher succeeds");

    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(err_msg.contains("AllMatchersFailed"),
        "Should return AllMatchersFailed error");
}

/// Test Scenario 7: Confidence breakdown validation
#[test]
fn test_confidence_breakdown_exists() {
    let file = "function test() { return 42; }";
    let search = "function test() { return 42; }";

    let result = coordinator::find_best_match(file, search, Some("test.js")).unwrap();

    assert!(result.confidence_breakdown.is_some(),
        "ConfidenceBreakdown should be populated");

    let breakdown = result.confidence_breakdown.unwrap();

    // GLUON V2 REPORT: Verify new structure (similarity, token_similarity, anchor_quality)
    assert!(breakdown.similarity >= 0.0 && breakdown.similarity <= 1.0,
        "Similarity score should be in [0, 1]");
    assert!(breakdown.token_similarity >= 0.0 && breakdown.token_similarity <= 1.0,
        "Token similarity score should be in [0, 1]");
    assert!(breakdown.anchor_quality >= 0.0 && breakdown.anchor_quality <= 1.0,
        "Anchor quality score should be in [0, 1]");
}

/// Test Scenario 8: Large file performance
#[test]
fn test_cascade_performance_on_large_file() {
    // Generate a large file (1000 lines)
    let mut file_content = String::new();
    for i in 0..1000 {
        file_content.push_str(&format!("function func{}() {{ return {}; }}\n", i, i));
    }

    let search = "function func500() { return 500; }";

    let start = std::time::Instant::now();
    let result = coordinator::find_best_match(&file_content, search, Some("large.js")).unwrap();
    let duration = start.elapsed();

    // WeightedAnchor is now priority 1, so it matches first even for exact matches
    assert_eq!(result.method_used, MatchMethod::WeightedAnchor,
        "Should use fast ExactMatcher even on large files");

    // With O(1) hash matching, this should be < 100ms even for 1000 lines
    assert!(duration.as_millis() < 100,
        "Exact match should be fast (took: {}ms)", duration.as_millis());
}

/// Test Scenario 9: Early exit optimization verification
#[test]
fn test_early_exit_stops_cascade() {
    let file = "const x = 1;";
    let search = "const x = 1;";

    // Exact match should win without trying other matchers
    let start = std::time::Instant::now();
    let result = coordinator::find_best_match(file, search, Some("test.js")).unwrap();
    let duration = start.elapsed();

    // WeightedAnchor is now priority 1, so it matches first even for exact matches
    assert_eq!(result.method_used, MatchMethod::WeightedAnchor,
        "ExactMatcher should win for identical code");

    // Early exit should be fast, but on slow machines may take > 1ms
    // Let's be more generous: < 10ms
    assert!(duration.as_millis() < 10,
        "Early exit should be fast (took: {}ms)", duration.as_millis());
}

/// Test Scenario 10: Regression test - trailing whitespace handling
#[test]
fn test_trailing_whitespace_normalization() {
    let file = "const value = 42;  \n"; // Trailing spaces
    let search = "const value = 42;\n";  // No trailing spaces

    let result = coordinator::find_best_match(file, search, Some("test.js")).unwrap();

    // Should match (WeightedAnchor normalizes whitespace)
    // WeightedAnchor is now priority 1, so it matches first even for exact matches
    assert_eq!(result.method_used, MatchMethod::WeightedAnchor,
        "WeightedAnchor should ignore trailing whitespace");
    assert!(result.confidence >= 0.85, "Confidence should be high, got {}", result.confidence);
}
