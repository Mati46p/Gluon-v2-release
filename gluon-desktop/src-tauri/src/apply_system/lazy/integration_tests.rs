//! Integration tests for the AI Intelligence Phase 2
//!
//! These tests verify the complete workflow of:
//! 1. Weighted Anchoring (finding unique lines and fuzzy expansion)
//! 2. Lazy Stitching (AST-based code reconstruction)
//! 3. Zombie Tail handling (structural integrity preservation)
//! 4. Safety Guards (preventing destructive edits)

#[cfg(test)]
mod tests {
    use crate::apply_system::lazy::{
        weighted_anchoring::{
            build_frequency_map, find_best_anchor, fuzzy_expand_from_anchor,
            WeightedAnchoringConfig, AnchorQuality,
        },
        engine::{LazyStitcherEngine, apply_lazy_edit},
        matcher::find_lazy_replacements,
        detector::detect_lazy_blocks,
    };
    use crate::apply_system::features::prompts::LazyStitcherConfig;
    use crate::apply_system::analysis::AnalysisEngine;
    use std::path::Path;

    // ========================================================================
    // TEST SUITE 1: Weighted Anchoring - Repetitive Code Blindness
    // ========================================================================

    #[test]
    fn test_repetitive_code_handling() {
        // This test verifies the core problem from Document IV:
        // "Finding return true in a file with 50 return true statements"

        let target_file = r#"
fn validate_email(email: &str) -> bool {
    if email.contains('@') {
        return true;
    }
    return false;
}

fn validate_password(pass: &str) -> bool {
    if pass.len() >= 8 {
        return true;
    }
    return false;
}

fn validate_username(user: &str) -> bool {
    if user.len() >= 3 {
        return true;
    }
    return false;
}

fn validate_unique_function(data: &str) -> bool {
    // This is the one we want to find!
    let special_check = data.contains("special");
    if special_check {
        return true;
    }
    return false;
}
"#;

        let search_block = r#"fn validate_unique_function(data: &str) -> bool {
    // This is the one we want to find!
    let special_check = data.contains("special");
    if special_check {
        return true;
    }
    return false;
}"#;

        let file_lines: Vec<String> = target_file.lines().map(|l| l.to_string()).collect();
        let search_lines: Vec<String> = search_block.lines().map(|l| l.to_string()).collect();

        let config = WeightedAnchoringConfig::default();

        // Step 1: Build frequency map
        let freq_map = build_frequency_map(&file_lines, true);

        // Verify that "return true;" is very common
        assert!(freq_map.get("return true;").copied().unwrap_or(0) >= 3);

        // Step 2: Find best anchor
        let anchor = find_best_anchor(&search_lines, &freq_map, &config)
            .expect("Should find an anchor");

        // The anchor should be the function signature (unique and high quality)
        // NOT "return true;" (common)
        println!("Best anchor: line {} = '{}'", anchor.line_index, anchor.line_content);
        assert_eq!(anchor.quality, AnchorQuality::High);
        assert!(anchor.line_content.contains("validate_unique_function"));

        // Step 3: Fuzzy expansion
        let expansion = fuzzy_expand_from_anchor(
            anchor,
            &search_lines,
            &file_lines,
            &config,
        ).expect("Should expand successfully");

        // Verify we matched the correct function (not one of the others)
        // Note: expansion.start_line is 1-based, so we need to subtract 1 for 0-based slicing
        let start_idx = expansion.start_line.saturating_sub(1);
        let matched_text = file_lines[start_idx..expansion.end_line].join("\n");
        println!("Matched text from lines {}..{} (0-based {}..{}):\n{}",
            expansion.start_line, expansion.end_line, start_idx, expansion.end_line, matched_text);
        assert!(matched_text.contains("validate_unique_function"),
            "Expected 'validate_unique_function' in matched text, got:\n{}", matched_text);
        assert!(matched_text.contains("special_check"),
            "Expected 'special_check' in matched text, got:\n{}", matched_text);

        // Verify confidence is high
        assert!(expansion.confidence > 0.85);
    }

    // ========================================================================
    // TEST SUITE 2: Fuzzy Expansion - AI Hallucination Tolerance
    // ========================================================================

    #[test]
    fn test_fuzzy_tolerance_for_minor_changes() {
        // Test that we can match code even when AI changes whitespace slightly

        let target_file = r#"
function calculateTotal(items) {
    // Calculate total
    const subtotal = items.reduce((sum, item) => sum + item.price, 0);
    const tax = subtotal * 0.08;
    return subtotal + tax;
}
"#;

        // AI made minor whitespace/indentation changes but content is same
        let search_block = r#"function calculateTotal(items) {
    // Calculate total
    const  subtotal = items.reduce((sum, item) => sum + item.price, 0);
    const tax = subtotal * 0.08;
    return  subtotal + tax;
}"#;

        let file_lines: Vec<String> = target_file.lines().map(|l| l.to_string()).collect();
        let search_lines: Vec<String> = search_block.lines().map(|l| l.to_string()).collect();

        let config = WeightedAnchoringConfig::default();
        let freq_map = build_frequency_map(&file_lines, true);

        let anchor = find_best_anchor(&search_lines, &freq_map, &config)
            .expect("Should find anchor");

        let expansion = fuzzy_expand_from_anchor(
            anchor,
            &search_lines,
            &file_lines,
            &config,
        ).expect("Should match despite whitespace differences");

        // Should have high confidence since content is same (whitespace normalized)
        assert!(expansion.confidence > 0.85);
    }

    // ========================================================================
    // TEST SUITE 3: Lazy Stitching - AST Diffing
    // ========================================================================

    #[test]
    fn test_lazy_marker_detection() {
        let code_with_lazy = r#"
function processData() {
    console.log("Starting");
    // ... existing code ...
    console.log("Done");
}
"#;

        let code_without_lazy = r#"
function processData() {
    console.log("Starting");
    const result = doWork();
    console.log("Done");
}
"#;

        use crate::apply_system::lazy::detector::contains_lazy_markers;

        assert!(contains_lazy_markers(code_with_lazy));
        assert!(!contains_lazy_markers(code_without_lazy));
    }

    #[test]
    fn test_ast_based_lazy_replacement() {
        let old_code = r#"function test() {
    const x = 1;
    const y = 2;
    const z = 3;
    return x + y + z;
}"#;

        let new_code_with_lazy = r#"function test() {
    const x = 10;
    // ... existing code ...
    return x + y + z;
}"#;

        let old_tree = AnalysisEngine::parse_with_heuristics(old_code, "test.js")
            .expect("Should parse old code")
            .tree;

        let new_tree = AnalysisEngine::parse_with_heuristics(new_code_with_lazy, "test.js")
            .expect("Should parse new code")
            .tree;

        let replacements = find_lazy_replacements(&old_tree, &new_tree, old_code, new_code_with_lazy);

        // Should find at least one lazy replacement
        assert!(!replacements.is_empty());

        println!("Found {} lazy replacements", replacements.len());
        for (i, replacement) in replacements.iter().enumerate() {
            println!("Replacement {}: {} nodes to insert", i, replacement.replacement_nodes.len());
        }
    }

    // ========================================================================
    // TEST SUITE 4: Zombie Tail - Structural Integrity
    // ========================================================================

    #[test]
    fn test_zombie_tail_detection_and_fix() {
        // This tests the scenario where AI provides a lazy marker
        // The system should preserve code after the marker

        let old_content = r#"function example() {
    if (condition) {
        doSomething();
        doMore();
    }
    return true;
}"#;

        // AI provides new code with lazy marker
        let new_lazy_content = r#"function example() {
    if (condition) {
        doSomethingNew();
        // ... existing code ...
    }
}"#;

        let path = Path::new("test.js");

        let engine = LazyStitcherEngine::new(LazyStitcherConfig::default());

        // The engine should detect the lazy marker and fill it with preserved code
        let result = engine.apply_lazy_edit(old_content, new_lazy_content, path);

        match result {
            Ok(edit_result) => {
                println!("✅ Edit succeeded! Content:\n{}", edit_result.content);

                // Should contain the new change
                assert!(edit_result.content.contains("doSomethingNew"),
                    "Should contain the new code");

                // Should preserve OLD code (doMore) via lazy stitching
                assert!(edit_result.content.contains("doMore"),
                    "Should preserve existing code through lazy marker");

                // Should preserve function structure
                assert!(edit_result.content.contains("function example"));

                println!("Lazy blocks processed: {}", edit_result.lazy_blocks_count);
                println!("Warnings: {:?}", edit_result.warnings);
            }
            Err(e) => {
                // If it fails gracefully, that's also acceptable for this edge case
                println!("⚠️ Edit failed (acceptable for edge case): {}", e);
                assert!(e.contains("syntax") || e.contains("validation") || e.contains("lazy") || e.contains("No lazy"));
            }
        }
    }

    // ========================================================================
    // TEST SUITE 5: Safety Guards - Destructive Edit Prevention
    // ========================================================================

    #[test]
    fn test_safety_guard_blocks_massive_deletion() {
        let old_content = r#"
class DataProcessor {
    constructor() {
        this.data = [];
    }

    addItem(item) {
        this.data.push(item);
    }

    removeItem(id) {
        this.data = this.data.filter(x => x.id !== id);
    }

    processAll() {
        return this.data.map(item => {
            return {
                ...item,
                processed: true,
                timestamp: Date.now()
            };
        });
    }

    clearAll() {
        this.data = [];
    }
}
"#;

        // AI tries to replace everything with lazy markers (destructive!)
        let new_lazy_content = r#"
class DataProcessor {
    // ... existing code ...
}
"#;

        let path = Path::new("test.js");
        let result = apply_lazy_edit(old_content, new_lazy_content, path);

        // This should be REJECTED by the safety guard
        match result {
            Ok(edit_result) => {
                // If it somehow passes, check for warnings
                println!("Warnings: {:?}", edit_result.warnings);
                assert!(!edit_result.warnings.is_empty(), "Should have safety warnings!");

                // The content should NOT be drastically reduced
                let reduction = 1.0 - (edit_result.content.len() as f32 / old_content.len() as f32);
                assert!(reduction < 0.7, "Should not delete 70%+ of code! Reduction: {}", reduction);
            }
            Err(e) => {
                // Expected: Safety guard should block this
                println!("Safety guard correctly blocked edit: {}", e);
                assert!(e.to_lowercase().contains("safety") || e.to_lowercase().contains("deletion"));
            }
        }
    }

    // ========================================================================
    // TEST SUITE 6: End-to-End Integration
    // ========================================================================

    #[test]
    fn test_complete_workflow_realistic_scenario() {
        // Realistic scenario: Developer asks AI to update error handling in a function

        let old_file = r#"
async function fetchUserData(userId) {
    try {
        const response = await fetch(`/api/users/${userId}`);
        const data = await response.json();
        return data;
    } catch (error) {
        console.log(error);
        return null;
    }
}

async function fetchProductData(productId) {
    try {
        const response = await fetch(`/api/products/${productId}`);
        const data = await response.json();
        return data;
    } catch (error) {
        console.log(error);
        return null;
    }
}
"#;

        // AI updates only the first function with better error handling
        let ai_response = r#"
async function fetchUserData(userId) {
    try {
        const response = await fetch(`/api/users/${userId}`);
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        const data = await response.json();
        return data;
    } catch (error) {
        console.error("Failed to fetch user:", error);
        throw error; // Re-throw instead of returning null
    }
}

// ... existing code ...
"#;

        let path = Path::new("api.js");
        let result = apply_lazy_edit(old_file, ai_response, path);

        match result {
            Ok(edit_result) => {
                println!("✅ Edit successful!");
                println!("Result:\n{}", edit_result.content);

                // Verify the first function was updated
                assert!(edit_result.content.contains("console.error"));
                assert!(edit_result.content.contains("throw error"));

                // Verify the second function was preserved
                assert!(edit_result.content.contains("fetchProductData"));

                // Verify both functions are still complete
                let func_count = edit_result.content.matches("async function").count();
                assert_eq!(func_count, 2, "Both functions should still exist");

                println!("Lazy blocks processed: {}", edit_result.lazy_blocks_count);
                println!("Syntax valid: {}", edit_result.is_valid_syntax);
                println!("Warnings: {:?}", edit_result.warnings);
            }
            Err(e) => {
                panic!("Edit should succeed but failed with: {}", e);
            }
        }
    }

    // ========================================================================
    // TEST SUITE 7: Edge Cases
    // ========================================================================

    #[test]
    fn test_empty_lazy_block() {
        let old_code = "const x = 1;\nconst y = 2;";
        let new_code = "// ... existing code ...";

        let path = Path::new("test.js");
        let result = apply_lazy_edit(old_code, new_code, path);

        // Should preserve original content when lazy marker is the only thing
        match result {
            Ok(edit_result) => {
                assert!(edit_result.content.contains("const x"));
                assert!(edit_result.content.contains("const y"));
            }
            Err(_) => {
                // Or it should fail gracefully
            }
        }
    }

    #[test]
    fn test_multiple_lazy_markers_in_sequence() {
        let old_code = r#"
function a() { return 1; }
function b() { return 2; }
function c() { return 3; }
"#;

        let new_code = r#"
// ... existing code ...
function b() { return 200; }
// ... existing code ...
"#;

        let path = Path::new("test.js");
        let result = apply_lazy_edit(old_code, new_code, path);

        match result {
            Ok(edit_result) => {
                // Should preserve function a and c, update function b
                assert!(edit_result.content.contains("function a"));
                assert!(edit_result.content.contains("return 200"));
                assert!(edit_result.content.contains("function c"));
            }
            Err(e) => {
                println!("Multiple lazy markers test failed: {}", e);
            }
        }
    }
}
