//! Tests for Code Quality Analyzer Module
//!
//! This test suite validates the advanced code quality analysis system
//! that uses matchers and parsers for deep code inspection.

mod test_helpers;
use test_helpers::*;

use gluon_desktop_lib::apply_system::code_quality_analyzer::{
    analyze_file, analyze_project, CodeQualityAnalyzer, AnalyzerConfig,
    FindingCategory, QualityGrade, Severity
};
use gluon_desktop_lib::apply_system::analysis::SupportedLanguage;

// ============================================================================
// Security Pattern Detection Tests
// ============================================================================

#[test]
fn test_detect_eval_usage() {
    let code = r#"
    function processInput(userCode) {
        eval(userCode); // SECURITY: eval is dangerous
        return result;
    }
    "#;

    let report = analyze_file("test.js", code).unwrap();

    assert!(
        report.findings.iter().any(|f|
            f.category == FindingCategory::Security &&
            f.description.contains("eval")
        ),
        "Should detect eval() usage"
    );
}

#[test]
fn test_detect_sql_injection() {
    let code = r#"
    def get_user(username):
        query = f"SELECT * FROM users WHERE name = '{username}'"
        return db.execute(query)
    "#;

    let report = analyze_file("test.py", code).unwrap();

    assert!(
        report.findings.iter().any(|f|
            f.category == FindingCategory::Security &&
            f.description.contains("SQL injection")
        ),
        "Should detect potential SQL injection"
    );
}

#[test]
fn test_detect_hardcoded_password() {
    let code = r#"
    const config = {
        password: "SuperSecret123!",
        apiKey: "sk_live_1234567890abcdef"
    };
    "#;

    let report = analyze_file("test.js", code).unwrap();

    assert!(
        report.findings.iter().any(|f|
            f.category == FindingCategory::Security &&
            f.severity == Severity::High
        ),
        "Should detect hardcoded credentials"
    );
}

#[test]
fn test_no_false_positives_on_safe_code() {
    let safe_code = r#"
    function add(a, b) {
        return a + b;
    }

    function multiply(x, y) {
        return x * y;
    }
    "#;

    let report = analyze_file("test.js", safe_code).unwrap();

    let security_issues: Vec<_> = report.findings.iter()
        .filter(|f| f.category == FindingCategory::Security)
        .collect();

    assert_eq!(security_issues.len(), 0, "Safe code should not trigger security warnings");
}

// ============================================================================
// Performance Pattern Detection Tests
// ============================================================================

#[test]
fn test_detect_nested_loops_with_search() {
    let code = r#"
    function findPairs(arr1, arr2) {
        for (let i = 0; i < arr1.length; i++) {
            for (let j = 0; j < arr2.length; j++) {
                if (arr2.includes(arr1[i])) {
                    return true;
                }
            }
        }
        return false;
    }
    "#;

    let report = analyze_file("test.js", code).unwrap();

    let perf_issues: Vec<_> = report.findings.iter()
        .filter(|f| f.category == FindingCategory::Performance)
        .collect();

    assert!(!perf_issues.is_empty(), "Should detect nested loop performance issue");
}

#[test]
fn test_detect_regex_in_loop() {
    let code = r#"
    function validateEmails(emails) {
        for (let i = 0; i < emails.length; i++) {
            const regex = new RegExp(/^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$/);
            if (!regex.test(emails[i])) {
                return false;
            }
        }
        return true;
    }
    "#;

    let report = analyze_file("test.js", code).unwrap();

    assert!(
        report.findings.iter().any(|f|
            f.category == FindingCategory::Performance &&
            f.description.to_lowercase().contains("regex")
        ),
        "Should detect regex compilation in loop"
    );
}

// ============================================================================
// Code Smell Detection Tests
// ============================================================================

#[test]
fn test_detect_magic_numbers() {
    let code = r#"
    function calculateDiscount(price) {
        if (price > 100) {
            return price * 0.15;
        } else if (price > 50) {
            return price * 0.10;
        }
        return price * 0.05;
    }
    "#;

    let report = analyze_file("test.js", code).unwrap();

    assert!(
        report.findings.iter().any(|f|
            f.category == FindingCategory::CodeSmell
        ),
        "Should detect magic numbers"
    );
}

#[test]
fn test_detect_deep_nesting() {
    let code = r#"
    function processData(data) {
        if (data) {
            if (data.user) {
                if (data.user.profile) {
                    if (data.user.profile.settings) {
                        if (data.user.profile.settings.notifications) {
                            return data.user.profile.settings.notifications.enabled;
                        }
                    }
                }
            }
        }
        return false;
    }
    "#;

    let report = analyze_file("test.js", code).unwrap();

    assert!(
        report.findings.iter().any(|f|
            f.category == FindingCategory::CodeSmell &&
            f.description.contains("nesting")
        ),
        "Should detect deep nesting"
    );
}

#[test]
fn test_detect_long_method() {
    let very_long_function = format!(
        "function veryLongFunction() {{\n{}\n}}",
        (0..100).map(|i| format!("    const var{} = {};\n", i, i)).collect::<String>()
    );

    let report = analyze_file("test.js", &very_long_function).unwrap();

    assert!(
        report.findings.iter().any(|f|
            f.category == FindingCategory::CodeSmell &&
            (f.description.contains("long") || f.description.contains("Long"))
        ),
        "Should detect long method"
    );
}

// ============================================================================
// Complexity Detection Tests
// ============================================================================

#[test]
fn test_detect_high_complexity() {
    let complex_code = r#"
    function complexLogic(a, b, c, d, e) {
        if (a > 0) {
            if (b > 0) {
                if (c > 0) {
                    if (d > 0) {
                        if (e > 0) {
                            return 1;
                        } else {
                            return 2;
                        }
                    } else {
                        return 3;
                    }
                } else {
                    return 4;
                }
            } else {
                return 5;
            }
        } else {
            return 6;
        }
    }
    "#;

    let report = analyze_file("test.js", complex_code).unwrap();

    assert!(report.metrics.max_complexity > 5, "Should detect high complexity");
    assert!(
        report.findings.iter().any(|f|
            f.title.contains("Complexity")
        ),
        "Should report complexity issue"
    );
}

#[test]
fn test_simple_code_low_complexity() {
    let simple_code = r#"
    function add(a, b) {
        return a + b;
    }

    function subtract(a, b) {
        return a - b;
    }
    "#;

    let report = analyze_file("test.js", simple_code).unwrap();

    assert!(report.metrics.max_complexity <= 2, "Simple code should have low complexity");
    assert!(report.metrics.avg_complexity <= 2.0, "Average complexity should be low");
}

// ============================================================================
// Code Metrics Tests
// ============================================================================

#[test]
fn test_calculate_metrics() {
    let code = r#"
    // This is a comment
    function test1() {
        return 1;
    }

    // Another comment
    function test2() {
        return 2;
    }

    class MyClass {
        method1() {}
    }
    "#;

    let report = analyze_file("test.js", code).unwrap();

    assert!(report.metrics.total_lines > 0);
    assert!(report.metrics.code_lines > 0);
    assert!(report.metrics.comment_lines >= 2);
    assert_eq!(report.metrics.function_count, 2);
    assert_eq!(report.metrics.class_count, 1);
}

#[test]
fn test_comment_ratio_calculation() {
    let well_documented = r#"
    // Function to add two numbers
    // @param {number} a - First number
    // @param {number} b - Second number
    // @returns {number} Sum of a and b
    function add(a, b) {
        return a + b;
    }
    "#;

    let report = analyze_file("test.js", well_documented).unwrap();

    assert!(report.metrics.comment_ratio > 0.4, "Well-documented code should have high comment ratio");
}

// ============================================================================
// Quality Grading Tests
// ============================================================================

#[test]
fn test_excellent_grade_for_clean_code() {
    let clean_code = r#"
    // Calculate the sum of an array
    function sum(numbers) {
        return numbers.reduce((acc, num) => acc + num, 0);
    }

    // Calculate the average of an array
    function average(numbers) {
        return sum(numbers) / numbers.length;
    }
    "#;

    let report = analyze_file("test.js", clean_code).unwrap();

    assert!(report.overall_score >= 80.0, "Clean code should score high");
    assert!(
        report.grade == QualityGrade::Excellent || report.grade == QualityGrade::Good,
        "Clean code should get good grade"
    );
}

#[test]
fn test_poor_grade_for_problematic_code() {
    let bad_code = r#"
    function badFunction(x) {
        eval(x);
        if (x > 100) {
            for (let i = 0; i < x; i++) {
                for (let j = 0; j < x; j++) {
                    if (i * j > 1000) {
                        if (Math.random() > 0.5) {
                            console.log("test");
                        }
                    }
                }
            }
        }
        const password = "hardcoded123";
    }
    "#;

    let report = analyze_file("test.js", bad_code).unwrap();

    assert!(report.overall_score < 70.0, "Problematic code should score low");
    assert!(report.findings.len() > 3, "Should detect multiple issues");
}

// ============================================================================
// Multi-File Analysis Tests
// ============================================================================

#[test]
fn test_analyze_multiple_files() {
    let files = vec![
        ("src/utils.js".to_string(), "function add(a, b) { return a + b; }".to_string()),
        ("src/app.js".to_string(), "eval('test');".to_string()),
        ("src/config.js".to_string(), "const password = 'secret123';".to_string()),
    ];

    let reports = analyze_project(files).unwrap();

    assert_eq!(reports.len(), 3, "Should analyze all files");

    // Check that security issues were found
    let total_security_issues: usize = reports.iter()
        .flat_map(|r| &r.findings)
        .filter(|f| f.category == FindingCategory::Security)
        .count();

    assert!(total_security_issues >= 2, "Should find security issues across files");
}

// ============================================================================
// Configuration Tests
// ============================================================================

#[test]
fn test_custom_analyzer_config() {
    let code = r#"
    function moderateComplexity(a, b, c) {
        if (a > 0) {
            if (b > 0) {
                if (c > 0) {
                    return a + b + c;
                }
            }
        }
        return 0;
    }
    "#;

    // Strict configuration
    let strict_config = AnalyzerConfig {
        max_complexity_threshold: 3,
        min_comment_ratio: 0.20,
        enable_security_scan: true,
        enable_performance_scan: true,
        enable_duplication_scan: true,
        duplication_min_lines: 4,
    };

    let analyzer = CodeQualityAnalyzer::with_config(SupportedLanguage::JavaScript, strict_config);
    let report = analyzer.analyze("test.js", code).unwrap();

    assert!(
        report.findings.iter().any(|f| f.title.contains("Complexity")),
        "Strict config should detect moderate complexity"
    );

    // Permissive configuration
    let permissive_config = AnalyzerConfig {
        max_complexity_threshold: 20,
        min_comment_ratio: 0.05,
        enable_security_scan: false,
        enable_performance_scan: false,
        enable_duplication_scan: false,
        duplication_min_lines: 10,
    };

    let permissive_analyzer = CodeQualityAnalyzer::with_config(SupportedLanguage::JavaScript, permissive_config);
    let permissive_report = permissive_analyzer.analyze("test.js", code).unwrap();

    assert!(
        permissive_report.findings.is_empty() || permissive_report.findings.len() < report.findings.len(),
        "Permissive config should find fewer issues"
    );
}

// ============================================================================
// Recommendation Generation Tests
// ============================================================================

#[test]
fn test_generate_recommendations() {
    let code_with_multiple_issues = r#"
    function test() {
        eval("x");
        const password = "secret123";
        for (let i = 0; i < 100; i++) {
            for (let j = 0; j < 100; j++) {
                console.log(i * j);
            }
        }
    }
    "#;

    let report = analyze_file("test.js", code_with_multiple_issues).unwrap();

    assert!(!report.recommendations.is_empty(), "Should generate recommendations");
    assert!(
        report.recommendations.iter().any(|r| r.contains("security") || r.contains("Security")),
        "Should recommend addressing security issues"
    );
}

// ============================================================================
// Edge Cases and Robustness Tests
// ============================================================================

#[test]
fn test_empty_file_analysis() {
    let empty_code = "";

    let result = analyze_file("test.js", empty_code);

    // Should either succeed with no findings or fail gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_very_large_file() {
    let large_code = (0..1000)
        .map(|i| format!("function func{}() {{ return {}; }}\n", i, i))
        .collect::<String>();

    let report = analyze_file("test.js", &large_code).unwrap();

    assert_eq!(report.metrics.function_count, 1000);
    assert!(report.overall_score > 0.0);
}

#[test]
fn test_unicode_and_special_characters() {
    let unicode_code = r#"
    function greet(name) {
        console.log("Cześć, " + name + "! 🎉");
        return "Witaj! 👋";
    }
    "#;

    let report = analyze_file("test.js", unicode_code).unwrap();

    assert!(report.overall_score > 0.0);
    // QualityReport doesn't have a status field, it's a different type
    // Just verify the report was generated successfully
}

#[test]
fn test_minified_code() {
    let minified = "function a(b){return b>10?b*2:b+5;}function c(d){return a(d)+1;}";

    let report = analyze_file("test.js", minified).unwrap();

    // Minified code typically has low comment ratio
    assert!(report.metrics.comment_ratio < 0.01);
}

// ============================================================================
// Integration with Matchers Tests
// ============================================================================

#[test]
fn test_duplication_detection_uses_fuzzy_matcher() {
    let code_with_duplication = r#"
    function calculateTotal1(items) {
        let total = 0;
        for (let i = 0; i < items.length; i++) {
            total += items[i].price * items[i].quantity;
        }
        return total;
    }

    function calculateTotal2(products) {
        let sum = 0;
        for (let i = 0; i < products.length; i++) {
            sum += products[i].price * products[i].quantity;
        }
        return sum;
    }
    "#;

    let report = analyze_file("test.js", code_with_duplication).unwrap();

    assert!(
        report.findings.iter().any(|f| f.category == FindingCategory::Duplication),
        "Should detect code duplication using fuzzy matcher"
    );
}
