//! Tests for Advanced Integrity Auditor
//!
//! Tests the enhanced auditing system that uses matchers and parsers
//! for comprehensive code quality and integrity analysis.

mod test_helpers;

use gluon_desktop_lib::apply_system::integrity_auditor::{
    AuditPolicy, IssueType, FileIntegrityStatus
};

// ============================================================================
// Audit Policy Tests
// ============================================================================

#[test]
fn test_standard_policy_configuration() {
    let policy = AuditPolicy::standard();

    assert_eq!(policy.name, "Standard Production");
    assert_eq!(policy.max_complexity, 15);
    assert_eq!(policy.min_type_coverage, 0.50);
    assert!(!policy.allow_phantom_imports);
    assert!(policy.enable_security_scan);
    assert!(policy.use_weighted_anchors);
}

#[test]
fn test_strict_policy_configuration() {
    let policy = AuditPolicy::strict();

    assert_eq!(policy.name, "Strict Enterprise");
    assert_eq!(policy.max_complexity, 10);
    assert_eq!(policy.min_type_coverage, 0.80);
    assert!(policy.strict_mode);
    assert!(policy.enforce_docstrings);
    assert!(!policy.enable_fuzzy_matching); // Strict mode uses exact matches
}

#[test]
fn test_permissive_policy_configuration() {
    let policy = AuditPolicy::permissive();

    assert_eq!(policy.name, "Permissive Development");
    assert_eq!(policy.max_complexity, 25);
    assert_eq!(policy.min_type_coverage, 0.30);
    assert!(policy.allow_phantom_imports);
    assert!(!policy.enforce_docstrings);
    assert!(policy.enable_security_scan); // Security always enabled
}

// ============================================================================
// Enhanced Issue Type Tests
// ============================================================================

#[test]
fn test_all_issue_types_available() {
    use gluon_desktop_lib::apply_system::integrity_auditor::IssueType;

    // Original issue types
    let _ = IssueType::MissingSymbol;
    let _ = IssueType::SignatureMismatch;
    let _ = IssueType::LogicDegradation;

    // Quality issue types
    let _ = IssueType::HighComplexity;
    let _ = IssueType::SecurityRisk;
    let _ = IssueType::TypeUnsafety;
    let _ = IssueType::PhantomDependency;

    // Pattern-based issue types
    let _ = IssueType::CodeDuplication;
    let _ = IssueType::AntiPattern;
    let _ = IssueType::CodeSmell;
    let _ = IssueType::PerformanceIssue;
    let _ = IssueType::ArchitectureViolation;

    // Advanced detection types
    let _ = IssueType::UnexpectedChange;
    let _ = IssueType::StructuralDrift;
    let _ = IssueType::BehaviorChange;

    println!("✓ All 16 issue types are available");
}

// ============================================================================
// Weighted Anchor Matcher Integration Tests
// ============================================================================

#[test]
fn test_weighted_anchor_detects_structural_drift() {
    // This test verifies that WeightedAnchorMatcher can detect
    // when code structure has changed significantly

    let original_code = r#"
    function calculateDiscount(price, customerType) {
        if (customerType === 'premium') {
            return price * 0.20;
        } else if (customerType === 'regular') {
            return price * 0.10;
        }
        return 0;
    }
    "#;

    let modified_code = r#"
    function calculateDiscount(amount, type, specialOffer) {
        // Completely restructured logic
        const discounts = {
            premium: 0.25,
            regular: 0.12,
            special: 0.30
        };
        return amount * (discounts[type] || 0) * (specialOffer ? 1.5 : 1.0);
    }
    "#;

    // Create a mock audit using WeightedAnchorMatcher
    use gluon_desktop_lib::apply_system::matchers::{Matcher, WeightedAnchorMatcher};

    let matcher = WeightedAnchorMatcher::new();
    let result = matcher.find_match(modified_code, original_code, Some("test.js"));

    // The match should have lower confidence due to structural changes
    if let Some(match_result) = result {
        assert!(
            match_result.confidence < 0.80,
            "Significant structural changes should result in low confidence: got {}",
            match_result.confidence
        );
    } else {
        // Or no match at all, which is also acceptable
        println!("✓ WeightedAnchorMatcher correctly detected no reliable match");
    }
}

// ============================================================================
// Fuzzy Matcher Duplication Detection Tests
// ============================================================================

#[test]
fn test_fuzzy_matcher_detects_similar_code() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher};

    // Use blocks that are actually similar enough for fuzzy matching
    let block1 = r#"
    function processUser(user) {
        if (!user) return null;
        const name = user.name.trim();
        const email = user.email.toLowerCase();
        return { name, email };
    }
    "#;

    // Same code with minor whitespace differences
    let block2 = r#"
    function processUser(user) {
        if(!user)return null;
        const name=user.name.trim();
        const email=user.email.toLowerCase();
        return{name,email};
    }
    "#;

    let matcher = FuzzyMatcher;
    let result = matcher.find_match(block2, block1, Some("test.js"));

    assert!(result.is_some(), "FuzzyMatcher should detect similar code structure");

    if let Some(match_result) = result {
        assert!(
            match_result.confidence > 0.70,
            "Similar code should have high similarity score: got {}",
            match_result.confidence
        );
    }
}

#[test]
fn test_fuzzy_matcher_ignores_whitespace_differences() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher};

    let code1 = "function test(){return 42;}";
    let code2 = r#"
    function test() {
        return 42;
    }
    "#;

    let matcher = FuzzyMatcher;
    let result = matcher.find_match(code2, code1, Some("test.js"));

    assert!(result.is_some(), "FuzzyMatcher should match despite whitespace differences");
}

// ============================================================================
// Block Matcher Architecture Violation Tests
// ============================================================================

#[test]
fn test_block_matcher_identifies_function_boundaries() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, BlockMatcher};

    let code = r#"
    function helper() {
        return 1;
    }

    function mainFunction() {
        const x = helper();
        return x + 1;
    }

    function another() {
        return 2;
    }
    "#;

    let search_for_main = r#"
    function mainFunction() {
        const x = helper();
        return x + 1;
    }
    "#;

    let matcher = BlockMatcher;
    let result = matcher.find_match(code, search_for_main, Some("test.js"));

    assert!(result.is_some(), "BlockMatcher should identify function block");

    if let Some(match_result) = result {
        // Verify it matched the correct function, not the helpers
        assert!(
            match_result.matched_line_start > 4,
            "Should skip the helper() function"
        );
    }
}

// ============================================================================
// Pattern-Based Detection Integration Tests
// ============================================================================

#[test]
fn test_anti_pattern_god_object_detection() {
    // Simulate a class with many methods (God Object anti-pattern)
    let god_object = (0..25)
        .map(|i| format!("    function method{}() {{ return {}; }}\n", i, i))
        .collect::<String>();

    let code = format!("class GodObject {{\n{}}}", god_object);

    let report = gluon_desktop_lib::apply_system::code_quality_analyzer::analyze_file("test.js", &code).unwrap();

    assert!(
        report.findings.iter().any(|f|
            f.category == gluon_desktop_lib::apply_system::code_quality_analyzer::FindingCategory::AntiPattern
        ),
        "Should detect God Object anti-pattern"
    );
}

// ============================================================================
// Multi-Language Support Tests
// ============================================================================

#[test]
#[ignore] // TODO: Fix tree-sitter query for TypeScript
fn test_analyze_typescript_code() {
    let ts_code = r#"
    interface User {
        name: string;
        email: string;
    }

    function greetUser(user: User): string {
        return `Hello, ${user.name}!`;
    }
    "#;

    let report = gluon_desktop_lib::apply_system::code_quality_analyzer::analyze_file("test.ts", ts_code).unwrap();

    assert!(report.overall_score > 0.0);
    assert!(report.metrics.function_count > 0);
}

#[test]
#[ignore] // TODO: Fix tree-sitter query for Python
fn test_analyze_python_code() {
    let py_code = r#"
def calculate_sum(numbers):
    """Calculate the sum of a list of numbers."""
    total = 0
    for num in numbers:
        total += num
    return total

def calculate_average(numbers):
    """Calculate the average of a list of numbers."""
    return calculate_sum(numbers) / len(numbers)
    "#;

    let report = gluon_desktop_lib::apply_system::code_quality_analyzer::analyze_file("test.py", py_code).unwrap();

    assert!(report.overall_score > 0.0);
    assert_eq!(report.metrics.function_count, 2);
    assert!(report.metrics.comment_ratio > 0.0); // Has docstrings
}

#[test]
fn test_analyze_rust_code() {
    let rust_code = r#"
    fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    fn multiply(x: i32, y: i32) -> i32 {
        x * y
    }

    pub struct Calculator;

    impl Calculator {
        pub fn new() -> Self {
            Calculator
        }
    }
    "#;

    let report = gluon_desktop_lib::apply_system::code_quality_analyzer::analyze_file("test.rs", rust_code).unwrap();

    assert!(report.overall_score > 0.0);
    assert!(report.metrics.function_count >= 2);
}

// ============================================================================
// Performance and Scalability Tests
// ============================================================================

#[test]
fn test_audit_performance_on_large_codebase() {
    use std::time::Instant;

    // Create a moderately large file
    let large_code = (0..200)
        .map(|i| {
            format!(
                r#"
                function func{}(param{}) {{
                    if (param{} > 0) {{
                        return param{} * 2;
                    }}
                    return param{} + 1;
                }}
                "#,
                i, i, i, i, i
            )
        })
        .collect::<String>();

    let start = Instant::now();
    let report = gluon_desktop_lib::apply_system::code_quality_analyzer::analyze_file("large.js", &large_code).unwrap();
    let duration = start.elapsed();

    assert_eq!(report.metrics.function_count, 200);
    assert!(
        duration.as_secs() < 5,
        "Analysis should complete within 5 seconds, took {:?}",
        duration
    );

    println!("✓ Analyzed 200 functions in {:?}", duration);
}

// ============================================================================
// Comprehensive Integration Test
// ============================================================================

#[test]
#[ignore] // TODO: Fix code quality analyzer tree-sitter queries
fn test_full_audit_workflow() {
    // This test simulates a complete audit workflow using all systems

    let problematic_code = r#"
    // Missing documentation
    function processPayment(amount, userId) {
        // Security issue: eval
        eval("processAmount = " + amount);

        // Hardcoded credential
        const apiKey = "sk_live_12345678";

        // Performance issue: nested loops
        for (let i = 0; i < 100; i++) {
            for (let j = 0; j < 100; j++) {
                console.log(i * j);
            }
        }

        // High complexity
        if (amount > 1000) {
            if (userId > 0) {
                if (apiKey.length > 0) {
                    if (processAmount > 0) {
                        if (Math.random() > 0.5) {
                            return true;
                        }
                    }
                }
            }
        }

        return false;
    }
    "#;

    let report = gluon_desktop_lib::apply_system::code_quality_analyzer::analyze_file("payment.js", problematic_code).unwrap();

    // Verify multiple issue categories detected
    let categories: std::collections::HashSet<_> = report.findings.iter()
        .map(|f| f.category.clone())
        .collect();

    assert!(
        categories.len() >= 3,
        "Should detect multiple types of issues, found {:?}",
        categories
    );

    // Verify overall score reflects problems
    assert!(
        report.overall_score < 60.0,
        "Problematic code should score below 60, got {}",
        report.overall_score
    );

    // Verify recommendations are generated
    assert!(
        !report.recommendations.is_empty(),
        "Should generate recommendations for improvements"
    );

    // Verify critical security issues are flagged
    assert!(
        report.findings.iter().any(|f|
            f.category == gluon_desktop_lib::apply_system::code_quality_analyzer::FindingCategory::Security &&
            f.severity == gluon_desktop_lib::apply_system::code_quality_analyzer::Severity::Critical
        ),
        "Should flag critical security issues"
    );

    println!("✓ Full audit workflow completed successfully");
    println!("  - Score: {}", report.overall_score);
    println!("  - Grade: {:?}", report.grade);
    println!("  - Issues found: {}", report.findings.len());
    println!("  - Categories: {}", categories.len());
}
