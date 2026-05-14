//! Code Quality Analyzer - Advanced Code Auditing System
//!
//! This module provides comprehensive code quality analysis by combining:
//! 1. Matchers (structural code analysis)
//! 2. Parsers (pattern recognition)
//! 3. Tree-sitter queries (semantic analysis)
//!
//! Features:
//! - Anti-pattern detection
//! - Code smell identification
//! - Security vulnerability scanning
//! - Performance issue detection
//! - Architecture violation checking
//! - Code duplication analysis

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use crate::apply_system::analysis::{AnalysisEngine, SupportedLanguage};
use crate::apply_system::analysis::queries::{QueryMatcher, SemanticSignature};
use crate::apply_system::matchers::{Matcher, FuzzyMatcher};
use tree_sitter::Tree;

// ============================================================================
// Core Data Structures
// ============================================================================

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QualityReport {
    pub file_path: String,
    pub overall_score: f32, // 0.0 to 100.0
    pub grade: QualityGrade,
    pub findings: Vec<Finding>,
    pub metrics: CodeMetrics,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QualityGrade {
    Excellent, // 90-100
    Good,      // 75-89
    Fair,      // 60-74
    Poor,      // 40-59
    Critical,  // 0-39
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub category: FindingCategory,
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub line_number: usize,
    pub code_snippet: String,
    pub suggestion: String,
    pub confidence: f32, // 0.0 to 1.0
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingCategory {
    Security,
    Performance,
    Maintainability,
    Reliability,
    CodeSmell,
    AntiPattern,
    Duplication,
    Architecture,
    BestPractices,
    Documentation,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CodeMetrics {
    pub total_lines: usize,
    pub code_lines: usize,
    pub comment_lines: usize,
    pub blank_lines: usize,
    pub avg_complexity: f32,
    pub max_complexity: usize,
    pub function_count: usize,
    pub class_count: usize,
    pub duplication_percentage: f32,
    pub comment_ratio: f32,
}

// ============================================================================
// Pattern Definitions
// ============================================================================

/// Security-related code patterns that should be flagged
struct SecurityPattern {
    name: &'static str,
    pattern: &'static str,
    description: &'static str,
    severity: Severity,
    languages: Vec<SupportedLanguage>,
}

impl SecurityPattern {
    fn get_patterns() -> Vec<SecurityPattern> {
        vec![
            SecurityPattern {
                name: "eval_usage",
                pattern: r"\beval\s*\(",
                description: "Use of eval() can execute arbitrary code",
                severity: Severity::Critical,
                languages: vec![SupportedLanguage::JavaScript, SupportedLanguage::TypeScript, SupportedLanguage::Python],
            },
            SecurityPattern {
                name: "sql_injection_risk",
                pattern: r#"(execute|query|rawQuery)\s*\(\s*[^)]*\+|f["'].*SELECT.*FROM.*["']"#,
                description: "Potential SQL injection - use parameterized queries",
                severity: Severity::Critical,
                languages: vec![SupportedLanguage::Python, SupportedLanguage::JavaScript, SupportedLanguage::TypeScript],
            },
            SecurityPattern {
                name: "command_injection",
                pattern: r"(exec|system|popen|spawn)\s*\(\s*[^)]*\+",
                description: "Potential command injection vulnerability",
                severity: Severity::Critical,
                languages: vec![SupportedLanguage::Python, SupportedLanguage::JavaScript, SupportedLanguage::TypeScript],
            },
            SecurityPattern {
                name: "hardcoded_password",
                pattern: r#"(password|passwd|pwd|secret|token|api_?[kK]ey)\s*[:=]\s*["'][^"']{8,}["']"#,
                description: "Hardcoded credentials detected",
                severity: Severity::High,
                languages: vec![SupportedLanguage::Python, SupportedLanguage::JavaScript, SupportedLanguage::TypeScript, SupportedLanguage::Rust],
            },
            SecurityPattern {
                name: "unsafe_deserialization",
                pattern: r"\b(pickle\.loads|yaml\.load|json\.loads)\s*\(",
                description: "Unsafe deserialization can lead to code execution",
                severity: Severity::High,
                languages: vec![SupportedLanguage::Python],
            },
        ]
    }
}

/// Performance anti-patterns
struct PerformancePattern {
    name: &'static str,
    pattern: &'static str,
    description: &'static str,
    severity: Severity,
}

impl PerformancePattern {
    const PATTERNS: &'static [PerformancePattern] = &[
        PerformancePattern {
            name: "nested_loops_array_search",
            pattern: r"for\s*\(.*\)\s*\{[^}]*for\s*\(.*\)\s*\{[^}]*(indexOf|includes|find)",
            description: "Nested loops with array search - consider using Set or Map",
            severity: Severity::Medium,
        },
        PerformancePattern {
            name: "regex_in_loop",
            pattern: r"new\s+RegExp",
            description: "Regex compilation in loop - move outside loop",
            severity: Severity::Medium,
        },
        PerformancePattern {
            name: "string_concatenation_loop",
            pattern: r#"(for|while)\s*\([^)]*\)\s*\{[^}]*\+=\s*["']"#,
            description: "String concatenation in loop - use array.join() or StringBuilder",
            severity: Severity::Low,
        },
    ];
}

/// Code smell patterns
struct CodeSmellPattern {
    name: &'static str,
    pattern: &'static str,
    description: &'static str,
    severity: Severity,
}

impl CodeSmellPattern {
    const PATTERNS: &'static [CodeSmellPattern] = &[
        CodeSmellPattern {
            name: "magic_numbers",
            pattern: r"(\*\s*(0\.\d+|[2-9]\d*|1\d+)|>\s*([5-9]\d*|1\d+)|return\s+[^;]*\s*\*\s*0\.\d+)",
            description: "Magic number detected - use named constants",
            severity: Severity::Low,
        },
        CodeSmellPattern {
            name: "long_parameter_list",
            pattern: r"function\s+\w+\s*\([^)]{100,}\)",
            description: "Long parameter list - consider using object parameter",
            severity: Severity::Medium,
        },
        CodeSmellPattern {
            name: "deep_nesting",
            pattern: r"\{[^\{\}]*\{[^\{\}]*\{[^\{\}]*\{[^\{\}]*\{",
            description: "Deep nesting detected (5+ levels) - refactor for clarity",
            severity: Severity::Medium,
        },
        CodeSmellPattern {
            name: "dead_code",
            pattern: r"(function|const|let|var)\s+\w+\s*[=\(].*//\s*(unused|deprecated|TODO: remove)",
            description: "Potentially dead code",
            severity: Severity::Low,
        },
    ];
}

// ============================================================================
// Code Quality Analyzer Implementation
// ============================================================================

pub struct CodeQualityAnalyzer {
    language: SupportedLanguage,
    config: AnalyzerConfig,
}

#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    pub max_complexity_threshold: usize,
    pub min_comment_ratio: f32,
    pub enable_security_scan: bool,
    pub enable_performance_scan: bool,
    pub enable_duplication_scan: bool,
    pub duplication_min_lines: usize,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            max_complexity_threshold: 15,
            min_comment_ratio: 0.10, // 10% comments
            enable_security_scan: true,
            enable_performance_scan: true,
            enable_duplication_scan: true,
            duplication_min_lines: 6,
        }
    }
}

impl CodeQualityAnalyzer {
    pub fn new(language: SupportedLanguage) -> Self {
        Self {
            language,
            config: AnalyzerConfig::default(),
        }
    }

    pub fn with_config(language: SupportedLanguage, config: AnalyzerConfig) -> Self {
        Self { language, config }
    }

    /// Main analysis entry point
    pub fn analyze(&self, file_path: &str, code: &str) -> Result<QualityReport, String> {
        crate::gluon_info!("CodeQualityAnalyzer", "Analyzing file: {}", file_path);

        // Parse code using tree-sitter
        let tree = AnalysisEngine::parse(code, file_path)?;

        // Extract semantic signatures
        let signatures = QueryMatcher::extract_signatures(code, &tree, self.language);

        // Calculate metrics
        let metrics = self.calculate_metrics(code, &signatures);

        // Run all detectors
        let mut findings = Vec::new();

        if self.config.enable_security_scan {
            findings.extend(self.detect_security_issues(code, &tree));
        }

        if self.config.enable_performance_scan {
            findings.extend(self.detect_performance_issues(code, &tree));
        }

        findings.extend(self.detect_code_smells(code, &signatures));
        findings.extend(self.detect_anti_patterns(code, &tree));
        findings.extend(self.detect_complexity_issues(&signatures));

        if self.config.enable_duplication_scan {
            findings.extend(self.detect_duplication(code));
        }

        // Generate recommendations
        let recommendations = self.generate_recommendations(&findings, &metrics);

        // Calculate overall score and grade
        let overall_score = self.calculate_overall_score(&findings, &metrics);
        let grade = Self::score_to_grade(overall_score);

        Ok(QualityReport {
            file_path: file_path.to_string(),
            overall_score,
            grade,
            findings,
            metrics,
            recommendations,
        })
    }

    // ========================================================================
    // Metric Calculation
    // ========================================================================

    fn calculate_metrics(&self, code: &str, signatures: &[SemanticSignature]) -> CodeMetrics {
        let lines: Vec<&str> = code.lines().collect();
        let total_lines = lines.len();

        let mut code_lines = 0;
        let mut comment_lines = 0;
        let mut blank_lines = 0;

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                blank_lines += 1;
            } else if trimmed.starts_with("//") || trimmed.starts_with("#") || trimmed.starts_with("/*") {
                comment_lines += 1;
            } else {
                code_lines += 1;
            }
        }

        // Count only standalone functions (not methods inside classes)
        let function_count = signatures.iter()
            .filter(|s| (s.kind == "function" || s.kind == "method") && s.parent_name.is_none())
            .count();
        let class_count = signatures.iter().filter(|s| s.kind == "class").count();

        let complexities: Vec<usize> = signatures.iter()
            .filter(|s| s.kind == "function" || s.kind == "method")
            .map(|s| s.cyclomatic_complexity)
            .collect();

        let avg_complexity = if !complexities.is_empty() {
            complexities.iter().sum::<usize>() as f32 / complexities.len() as f32
        } else {
            1.0
        };

        let max_complexity = complexities.iter().max().copied().unwrap_or(1);

        let comment_ratio = if total_lines > 0 {
            comment_lines as f32 / total_lines as f32
        } else {
            0.0
        };

        CodeMetrics {
            total_lines,
            code_lines,
            comment_lines,
            blank_lines,
            avg_complexity,
            max_complexity,
            function_count,
            class_count,
            duplication_percentage: 0.0, // Will be calculated by duplication detector
            comment_ratio,
        }
    }

    // ========================================================================
    // Security Detection
    // ========================================================================

    fn detect_security_issues(&self, code: &str, _tree: &Tree) -> Vec<Finding> {
        let mut findings = Vec::new();

        for pattern in &SecurityPattern::get_patterns() {
            // Check if pattern applies to current language
            if !pattern.languages.is_empty() && !pattern.languages.contains(&self.language) {
                continue;
            }

            if let Ok(regex) = regex::Regex::new(pattern.pattern) {
                // First try line-by-line matching
                for (line_num, line) in code.lines().enumerate() {
                    if regex.is_match(line) {
                        findings.push(Finding {
                            category: FindingCategory::Security,
                            severity: pattern.severity.clone(),
                            title: format!("Security: {}", pattern.name.replace('_', " ")),
                            description: pattern.description.to_string(),
                            line_number: line_num + 1,
                            code_snippet: line.trim().to_string(),
                            suggestion: self.get_security_suggestion(pattern.name),
                            confidence: 0.85,
                        });
                    }
                }

                // Also try whole-code matching for multiline patterns
                if findings.is_empty() || pattern.name == "sql_injection_risk" {
                    let normalized = code.replace('\n', " ").replace('\r', " ");
                    if regex.is_match(&normalized) {
                        // Find the line number where the match occurs
                        if let Some(mat) = regex.find(&normalized) {
                            let prefix = &normalized[..mat.start()];
                            let line_num = code[..prefix.len().min(code.len())].lines().count();

                            // Avoid duplicates
                            if !findings.iter().any(|f| f.line_number == line_num && f.title.contains(pattern.name)) {
                                findings.push(Finding {
                                    category: FindingCategory::Security,
                                    severity: pattern.severity.clone(),
                                    title: format!("Security: {}", pattern.name.replace('_', " ")),
                                    description: pattern.description.to_string(),
                                    line_number: line_num,
                                    code_snippet: code.lines().nth(line_num.saturating_sub(1)).unwrap_or("").trim().to_string(),
                                    suggestion: self.get_security_suggestion(pattern.name),
                                    confidence: 0.85,
                                });
                            }
                        }
                    }
                }
            }
        }

        findings
    }

    fn get_security_suggestion(&self, pattern_name: &str) -> String {
        match pattern_name {
            "eval_usage" => "Replace eval() with safer alternatives like JSON.parse() or Function constructor".to_string(),
            "sql_injection_risk" => "Use parameterized queries or prepared statements".to_string(),
            "command_injection" => "Validate and sanitize all user input, use safe command execution methods".to_string(),
            "hardcoded_password" => "Store credentials in environment variables or secure vaults".to_string(),
            "unsafe_deserialization" => "Use safe deserialization methods with proper validation".to_string(),
            _ => "Review security best practices for this pattern".to_string(),
        }
    }

    // ========================================================================
    // Performance Detection
    // ========================================================================

    fn detect_performance_issues(&self, code: &str, _tree: &Tree) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Special case: regex in loop detection using context awareness
        if let Ok(regex_pattern) = regex::Regex::new(r"new\s+RegExp") {
            for (line_num, line) in code.lines().enumerate() {
                if regex_pattern.is_match(line) {
                    // Check if this line is inside a loop by looking at surrounding context
                    let context_start = line_num.saturating_sub(10);
                    let context_end = (line_num + 5).min(code.lines().count());
                    let context_lines: Vec<&str> = code.lines().skip(context_start).take(context_end - context_start).collect();
                    let context = context_lines.join("\n").to_lowercase();

                    // Look for loop keywords in the context
                    if context.contains("for") || context.contains("while") {
                        findings.push(Finding {
                            category: FindingCategory::Performance,
                            severity: Severity::Medium,
                            title: "Performance: regex in loop".to_string(),
                            description: "Regex compilation in loop - move outside loop".to_string(),
                            line_number: line_num + 1,
                            code_snippet: line.trim().to_string(),
                            suggestion: self.get_performance_suggestion("regex_in_loop"),
                            confidence: 0.75,
                        });
                    }
                }
            }
        }

        for pattern in PerformancePattern::PATTERNS {
            // Skip regex_in_loop since we handled it above
            if pattern.name == "regex_in_loop" {
                continue;
            }

            if let Ok(regex) = regex::Regex::new(pattern.pattern) {
                // Try line-by-line first
                for (line_num, line) in code.lines().enumerate() {
                    if regex.is_match(line) {
                        findings.push(Finding {
                            category: FindingCategory::Performance,
                            severity: pattern.severity.clone(),
                            title: format!("Performance: {}", pattern.name.replace('_', " ")),
                            description: pattern.description.to_string(),
                            line_number: line_num + 1,
                            code_snippet: line.trim().to_string(),
                            suggestion: self.get_performance_suggestion(pattern.name),
                            confidence: 0.75,
                        });
                    }
                }

                // Also try multiline matching for complex patterns
                if findings.is_empty() || pattern.name.contains("loop") {
                    let normalized = code.replace('\n', " ").replace('\r', " ");
                    if regex.is_match(&normalized) {
                        if let Some(mat) = regex.find(&normalized) {
                            let prefix = &normalized[..mat.start()];
                            let line_num = code[..prefix.len().min(code.len())].lines().count();

                            if !findings.iter().any(|f| f.line_number == line_num && f.title.contains(pattern.name)) {
                                findings.push(Finding {
                                    category: FindingCategory::Performance,
                                    severity: pattern.severity.clone(),
                                    title: format!("Performance: {}", pattern.name.replace('_', " ")),
                                    description: pattern.description.to_string(),
                                    line_number: line_num,
                                    code_snippet: code.lines().nth(line_num.saturating_sub(1)).unwrap_or("").trim().to_string(),
                                    suggestion: self.get_performance_suggestion(pattern.name),
                                    confidence: 0.75,
                                });
                            }
                        }
                    }
                }
            }
        }

        findings
    }

    fn get_performance_suggestion(&self, pattern_name: &str) -> String {
        match pattern_name {
            "nested_loops_array_search" => "Convert inner array to Set or Map for O(1) lookups".to_string(),
            "regex_in_loop" => "Compile regex outside the loop and reuse".to_string(),
            "string_concatenation_loop" => "Use array.join() or StringBuilder pattern".to_string(),
            _ => "Consider optimizing this code pattern".to_string(),
        }
    }

    // ========================================================================
    // Code Smell Detection
    // ========================================================================

    fn detect_code_smells(&self, code: &str, signatures: &[SemanticSignature]) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Pattern-based smells
        for pattern in CodeSmellPattern::PATTERNS {
            if let Ok(regex) = regex::Regex::new(pattern.pattern) {
                // Try line-by-line first
                for (line_num, line) in code.lines().enumerate() {
                    if regex.is_match(line) {
                        findings.push(Finding {
                            category: FindingCategory::CodeSmell,
                            severity: pattern.severity.clone(),
                            title: format!("Code Smell: {}", pattern.name.replace('_', " ")),
                            description: pattern.description.to_string(),
                            line_number: line_num + 1,
                            code_snippet: line.trim().to_string(),
                            suggestion: self.get_code_smell_suggestion(pattern.name),
                            confidence: 0.70,
                        });
                    }
                }

                // For deep nesting, also check multiline
                if pattern.name == "deep_nesting" && findings.is_empty() {
                    let normalized = code.replace('\n', " ").replace('\r', " ");
                    if regex.is_match(&normalized) {
                        if let Some(mat) = regex.find(&normalized) {
                            let prefix = &normalized[..mat.start()];
                            let line_num = code[..prefix.len().min(code.len())].lines().count();

                            findings.push(Finding {
                                category: FindingCategory::CodeSmell,
                                severity: pattern.severity.clone(),
                                title: format!("Code Smell: {}", pattern.name.replace('_', " ")),
                                description: pattern.description.to_string(),
                                line_number: line_num,
                                code_snippet: code.lines().nth(line_num.saturating_sub(1)).unwrap_or("").trim().to_string(),
                                suggestion: self.get_code_smell_suggestion(pattern.name),
                                confidence: 0.70,
                            });
                        }
                    }
                }
            }
        }

        // Signature-based smells
        for sig in signatures {
            // God class detection
            if sig.kind == "class" && sig.body_size_bytes > 5000 {
                findings.push(Finding {
                    category: FindingCategory::CodeSmell,
                    severity: Severity::Medium,
                    title: "God Class".to_string(),
                    description: format!("Class '{}' is very large ({} bytes)", sig.name, sig.body_size_bytes),
                    line_number: sig.start_row + 1,
                    code_snippet: format!("class {} {{ ... }}", sig.name),
                    suggestion: "Consider splitting into smaller, focused classes".to_string(),
                    confidence: 0.80,
                });
            }

            // Long method detection
            if (sig.kind == "function" || sig.kind == "method") && sig.body_size_bytes > 1000 {
                findings.push(Finding {
                    category: FindingCategory::CodeSmell,
                    severity: Severity::Low,
                    title: "Long Method".to_string(),
                    description: format!("Function '{}' is very long ({} bytes)", sig.name, sig.body_size_bytes),
                    line_number: sig.start_row + 1,
                    code_snippet: format!("function {}() {{ ... }}", sig.name),
                    suggestion: "Consider breaking into smaller functions".to_string(),
                    confidence: 0.75,
                });
            }
        }

        findings
    }

    fn get_code_smell_suggestion(&self, pattern_name: &str) -> String {
        match pattern_name {
            "magic_numbers" => "Extract magic numbers into named constants".to_string(),
            "long_parameter_list" => "Use an options object or configuration parameter".to_string(),
            "deep_nesting" => "Extract nested logic into separate functions".to_string(),
            "dead_code" => "Remove unused code to improve maintainability".to_string(),
            _ => "Refactor to improve code quality".to_string(),
        }
    }

    // ========================================================================
    // Anti-Pattern Detection
    // ========================================================================

    fn detect_anti_patterns(&self, code: &str, _tree: &Tree) -> Vec<Finding> {
        let mut findings = Vec::new();

        // God object pattern
        let class_methods = code.matches("function ").count();
        if class_methods > 20 {
            findings.push(Finding {
                category: FindingCategory::AntiPattern,
                severity: Severity::High,
                title: "God Object".to_string(),
                description: "Class has too many methods (20+), violating Single Responsibility Principle".to_string(),
                line_number: 1,
                code_snippet: "class { ... 20+ methods ... }".to_string(),
                suggestion: "Split into multiple focused classes".to_string(),
                confidence: 0.80,
            });
        }

        // Copy-paste programming
        let duplicate_blocks = self.find_duplicate_blocks(code);
        if duplicate_blocks > 3 {
            findings.push(Finding {
                category: FindingCategory::AntiPattern,
                severity: Severity::Medium,
                title: "Copy-Paste Programming".to_string(),
                description: format!("Found {} duplicate code blocks", duplicate_blocks),
                line_number: 1,
                code_snippet: "".to_string(),
                suggestion: "Extract common code into reusable functions".to_string(),
                confidence: 0.75,
            });
        }

        findings
    }

    // ========================================================================
    // Complexity Issues
    // ========================================================================

    fn detect_complexity_issues(&self, signatures: &[SemanticSignature]) -> Vec<Finding> {
        let mut findings = Vec::new();

        for sig in signatures {
            // Report complexity issues if:
            // 1. Exceeds custom threshold, OR
            // 2. Exceeds universal "concerning" threshold of 10
            let should_report = sig.cyclomatic_complexity > self.config.max_complexity_threshold
                || sig.cyclomatic_complexity > 10;

            if should_report {
                let severity = if sig.cyclomatic_complexity > 25 {
                    Severity::High
                } else if sig.cyclomatic_complexity > 15 {
                    Severity::Medium
                } else {
                    Severity::Low
                };

                findings.push(Finding {
                    category: FindingCategory::Maintainability,
                    severity,
                    title: "High Cyclomatic Complexity".to_string(),
                    description: format!(
                        "Function '{}' has complexity of {} (threshold: {})",
                        sig.name, sig.cyclomatic_complexity, self.config.max_complexity_threshold
                    ),
                    line_number: sig.start_row + 1,
                    code_snippet: format!("function {}() {{ ... }}", sig.name),
                    suggestion: "Reduce complexity by extracting conditions into separate functions".to_string(),
                    confidence: 0.95,
                });
            }
        }

        findings
    }

    // ========================================================================
    // Duplication Detection
    // ========================================================================

    fn detect_duplication(&self, code: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        let lines: Vec<&str> = code.lines().collect();

        if lines.len() < self.config.duplication_min_lines {
            return findings;
        }

        let mut blocks: Vec<(String, usize)> = Vec::new();
        let fuzzy_matcher = FuzzyMatcher;

        // Limit analysis for performance - sample blocks instead of checking all
        const MAX_BLOCKS_TO_ANALYZE: usize = 300;
        let step_size = if lines.len() > MAX_BLOCKS_TO_ANALYZE * self.config.duplication_min_lines {
            (lines.len() / MAX_BLOCKS_TO_ANALYZE).max(1)
        } else {
            1
        };

        // Collect all blocks (with sampling for large files)
        let mut i = 0;
        while i <= lines.len().saturating_sub(self.config.duplication_min_lines) {
            let block: String = lines[i..i + self.config.duplication_min_lines]
                .iter()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");

            if block.len() > 50 { // Minimum block size
                blocks.push((block, i));
                if blocks.len() >= MAX_BLOCKS_TO_ANALYZE {
                    break;
                }
            }
            i += step_size;
        }

        // Use fuzzy matching to find similar blocks
        let mut found_duplicates: HashSet<usize> = HashSet::new();
        const MAX_DUPLICATION_REPORTS: usize = 10; // Limit findings to avoid overwhelming reports

        for i in 0..blocks.len() {
            // Stop if we've found enough duplicates
            if findings.len() >= MAX_DUPLICATION_REPORTS {
                break;
            }
            if found_duplicates.contains(&i) {
                continue;
            }

            let mut duplicate_count = 1;
            let (ref block1, line_num) = blocks[i];

            for j in (i + 1)..blocks.len() {
                if found_duplicates.contains(&j) {
                    continue;
                }

                let (ref block2, _) = blocks[j];

                // Use fuzzy matcher to compare blocks
                if let Some(result) = fuzzy_matcher.find_match(block1, block2, None) {
                    if result.confidence > 0.7 { // Similar enough to be considered duplicate
                        duplicate_count += 1;
                        found_duplicates.insert(j);
                    }
                }
            }

            if duplicate_count > 1 {
                found_duplicates.insert(i);
                findings.push(Finding {
                    category: FindingCategory::Duplication,
                    severity: Severity::Low,
                    title: "Code Duplication".to_string(),
                    description: format!("Code block duplicated {} times (fuzzy matched)", duplicate_count),
                    line_number: line_num + 1,
                    code_snippet: block1.lines().take(3).collect::<Vec<_>>().join("\n") + "\n...",
                    suggestion: "Extract into a reusable function".to_string(),
                    confidence: 0.85,
                });
            }
        }

        findings
    }

    fn find_duplicate_blocks(&self, code: &str) -> usize {
        // Simplified duplicate detection for anti-pattern analysis
        let findings = self.detect_duplication(code);
        findings.len()
    }

    // ========================================================================
    // Scoring and Recommendations
    // ========================================================================

    fn calculate_overall_score(&self, findings: &[Finding], metrics: &CodeMetrics) -> f32 {
        let mut score = 100.0;

        // Deduct points based on findings severity
        for finding in findings {
            let penalty = match finding.severity {
                Severity::Critical => 15.0,
                Severity::High => 10.0,
                Severity::Medium => 5.0,
                Severity::Low => 2.0,
                Severity::Info => 0.5,
            };
            score -= penalty * finding.confidence;
        }

        // Deduct points for poor metrics
        if metrics.avg_complexity > self.config.max_complexity_threshold as f32 {
            score -= 5.0;
        }

        if metrics.comment_ratio < self.config.min_comment_ratio {
            score -= 3.0;
        }

        if metrics.duplication_percentage > 10.0 {
            score -= 5.0;
        }

        score.max(0.0).min(100.0)
    }

    fn score_to_grade(score: f32) -> QualityGrade {
        match score as u32 {
            90..=100 => QualityGrade::Excellent,
            75..=89 => QualityGrade::Good,
            60..=74 => QualityGrade::Fair,
            40..=59 => QualityGrade::Poor,
            _ => QualityGrade::Critical,
        }
    }

    fn generate_recommendations(&self, findings: &[Finding], metrics: &CodeMetrics) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Group findings by category
        let mut category_counts: HashMap<FindingCategory, usize> = HashMap::new();
        for finding in findings {
            *category_counts.entry(finding.category.clone()).or_insert(0) += 1;
        }

        // Generate category-specific recommendations
        if let Some(&count) = category_counts.get(&FindingCategory::Security) {
            if count > 0 {
                recommendations.push(format!(
                    "🔒 Address {} security issue(s) - these should be highest priority",
                    count
                ));
            }
        }

        if let Some(&count) = category_counts.get(&FindingCategory::Performance) {
            if count > 2 {
                recommendations.push(format!(
                    "⚡ Review {} performance issue(s) - consider profiling to identify bottlenecks",
                    count
                ));
            }
        }

        if metrics.avg_complexity > self.config.max_complexity_threshold as f32 {
            recommendations.push(format!(
                "🔄 Refactor complex functions - average complexity is {:.1}, target is {}",
                metrics.avg_complexity, self.config.max_complexity_threshold
            ));
        }

        if metrics.comment_ratio < self.config.min_comment_ratio {
            recommendations.push(format!(
                "📝 Improve documentation - comment ratio is {:.1}%, target is {:.1}%",
                metrics.comment_ratio * 100.0,
                self.config.min_comment_ratio * 100.0
            ));
        }

        if recommendations.is_empty() {
            recommendations.push("✅ Code quality is good! Keep up the good work.".to_string());
        }

        recommendations
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Analyze a single file for code quality issues
pub fn analyze_file(file_path: &str, code: &str) -> Result<QualityReport, String> {
    let language = SupportedLanguage::from_path(file_path)
        .ok_or_else(|| format!("Unsupported file type: {}", file_path))?;

    let analyzer = CodeQualityAnalyzer::new(language);
    analyzer.analyze(file_path, code)
}

/// Analyze multiple files and generate aggregate report
pub fn analyze_project(files: Vec<(String, String)>) -> Result<Vec<QualityReport>, String> {
    let mut reports = Vec::new();

    for (file_path, code) in files {
        match analyze_file(&file_path, &code) {
            Ok(report) => reports.push(report),
            Err(e) => {
                crate::gluon_warn!("CodeQualityAnalyzer", "Failed to analyze {}: {}", file_path, e);
            }
        }
    }

    if reports.is_empty() {
        return Err("No files were successfully analyzed".to_string());
    }

    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_pattern_detection() {
        let code = r#"
        function dangerousCode() {
            eval("alert('xss')");
            const password = "hardcoded123";
        }
        "#;

        let report = analyze_file("test.js", code).unwrap();
        assert!(report.findings.iter().any(|f| f.category == FindingCategory::Security));
    }

    #[test]
    fn test_complexity_detection() {
        let code = r#"
        function complex() {
            if (a) {
                if (b) {
                    if (c) {
                        if (d) {
                            if (e) {
                                return true;
                            }
                        }
                    }
                }
            }
            return false;
        }
        "#;

        let report = analyze_file("test.js", code).unwrap();
        assert!(report.metrics.max_complexity > 5);
    }

    #[test]
    fn test_quality_scoring() {
        let clean_code = r#"
        // Well documented function
        function add(a, b) {
            return a + b;
        }
        "#;

        let report = analyze_file("test.js", clean_code).unwrap();
        assert!(report.overall_score > 80.0);
        assert_eq!(report.grade, QualityGrade::Excellent);
    }
}
