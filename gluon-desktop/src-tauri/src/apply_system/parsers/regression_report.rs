//! Advanced Regression Testing Report System

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Critical,
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "CRITICAL",
            Severity::Error => "ERROR",
            Severity::Warning => "WARNING",
            Severity::Info => "INFO",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Severity::Critical => "🔴",
            Severity::Error => "❌",
            Severity::Warning => "⚠️",
            Severity::Info => "ℹ️",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TestCategory {
    Syntax,
    Indentation,
    Structure,
    Duplication,
    Context,
    Security,
    FragmentParsing,
    ASTValidation,
    LazyMarkers,
    Integration,
}

impl TestCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            TestCategory::Syntax => "Syntax",
            TestCategory::Indentation => "Indentation",
            TestCategory::Structure => "Structure",
            TestCategory::Duplication => "Duplication",
            TestCategory::Context => "Context",
            TestCategory::Security => "Security",
            TestCategory::FragmentParsing => "Fragment Parsing",
            TestCategory::ASTValidation => "AST Validation",
            TestCategory::LazyMarkers => "Lazy Markers",
            TestCategory::Integration => "Integration",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            TestCategory::Syntax => "📝",
            TestCategory::Indentation => "↔️",
            TestCategory::Structure => "🏗️",
            TestCategory::Duplication => "🔄",
            TestCategory::Context => "🎯",
            TestCategory::Security => "🔒",
            TestCategory::FragmentParsing => "🧩",
            TestCategory::ASTValidation => "🌳",
            TestCategory::LazyMarkers => "💤",
            TestCategory::Integration => "🔗",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub code_snippet: Option<String>,
    pub line_number: Option<usize>,
    pub suggestion: Option<String>,
}

impl Finding {
    pub fn critical(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            severity: Severity::Critical,
            title: title.into(),
            description: description.into(),
            code_snippet: None,
            line_number: None,
            suggestion: None,
        }
    }

    pub fn error(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            title: title.into(),
            description: description.into(),
            code_snippet: None,
            line_number: None,
            suggestion: None,
        }
    }

    pub fn warning(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            title: title.into(),
            description: description.into(),
            code_snippet: None,
            line_number: None,
            suggestion: None,
        }
    }

    pub fn info(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            title: title.into(),
            description: description.into(),
            code_snippet: None,
            line_number: None,
            suggestion: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code_snippet = Some(code.into());
        self
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line_number = Some(line);
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationStep {
    pub name: String,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub passed: bool,
    pub findings: Vec<Finding>,
    pub metadata: HashMap<String, String>,
}

impl ValidationStep {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            started_at: SystemTime::now(),
            completed_at: None,
            passed: false,
            findings: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn complete(mut self, passed: bool) -> Self {
        self.completed_at = Some(SystemTime::now());
        self.passed = passed;
        self
    }

    pub fn add_finding(mut self, finding: Finding) -> Self {
        self.findings.push(finding);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn duration(&self) -> Option<Duration> {
        self.completed_at.and_then(|end| end.duration_since(self.started_at).ok())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_name: String,
    pub category: TestCategory,
    pub passed: bool,
    pub steps: Vec<ValidationStep>,
    pub findings: Vec<Finding>,
    pub duration: Duration,
    pub input_summary: String,
    pub expected: String,
    pub actual: String,
}

impl TestResult {
    pub fn new(test_name: impl Into<String>, category: TestCategory) -> Self {
        Self {
            test_name: test_name.into(),
            category,
            passed: false,
            steps: Vec::new(),
            findings: Vec::new(),
            duration: Duration::from_secs(0),
            input_summary: String::new(),
            expected: String::new(),
            actual: String::new(),
        }
    }

    pub fn add_step(mut self, step: ValidationStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn add_finding(mut self, finding: Finding) -> Self {
        self.findings.push(finding);
        self
    }

    pub fn complete(mut self, passed: bool, duration: Duration) -> Self {
        self.passed = passed;
        self.duration = duration;
        self
    }

    pub fn with_input(mut self, summary: impl Into<String>) -> Self {
        self.input_summary = summary.into();
        self
    }

    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = expected.into();
        self
    }

    pub fn with_actual(mut self, actual: impl Into<String>) -> Self {
        self.actual = actual.into();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionReport {
    pub started_at: SystemTime,
    pub completed_at: SystemTime,
    pub tests: Vec<TestResult>,
    pub summary: ReportSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub duration: Duration,
    pub findings_by_severity: HashMap<String, usize>,
    pub findings_by_category: HashMap<String, usize>,
}

impl RegressionReport {
    pub fn new() -> Self {
        Self {
            started_at: SystemTime::now(),
            completed_at: SystemTime::now(),
            tests: Vec::new(),
            summary: ReportSummary {
                total_tests: 0,
                passed: 0,
                failed: 0,
                duration: Duration::from_secs(0),
                findings_by_severity: HashMap::new(),
                findings_by_category: HashMap::new(),
            },
        }
    }

    pub fn add_test(mut self, test: TestResult) -> Self {
        self.tests.push(test);
        self
    }

    pub fn finalize(mut self) -> Self {
        self.completed_at = SystemTime::now();
        self.summary.total_tests = self.tests.len();
        self.summary.passed = self.tests.iter().filter(|t| t.passed).count();
        self.summary.failed = self.tests.len() - self.summary.passed;
        self.summary.duration = self.completed_at.duration_since(self.started_at).unwrap_or_default();

        for test in &self.tests {
            for finding in &test.findings {
                *self.summary.findings_by_severity
                    .entry(finding.severity.as_str().to_string())
                    .or_insert(0) += 1;
            }
            for step in &test.steps {
                for finding in &step.findings {
                    *self.summary.findings_by_severity
                        .entry(finding.severity.as_str().to_string())
                        .or_insert(0) += 1;
                }
            }
        }

        for test in &self.tests {
            let count = test.findings.len() + test.steps.iter().map(|s| s.findings.len()).sum::<usize>();
            if count > 0 {
                *self.summary.findings_by_category
                    .entry(test.category.as_str().to_string())
                    .or_insert(0) += count;
            }
        }

        self
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub struct UnwantedChangeDetector;

impl UnwantedChangeDetector {
    pub fn detect(original: &str, modified: &str) -> Vec<Finding> {
        let mut findings = Vec::new();

        let orig_lines = original.lines().count();
        let mod_lines = modified.lines().count();

        if orig_lines > 10 && mod_lines < orig_lines / 2 {
            findings.push(
                Finding::warning(
                    "Excessive code deletion",
                    format!("Deleted {} lines out of {} (>50% removal)", orig_lines - mod_lines, orig_lines)
                )
                .with_suggestion("Verify that this large deletion is intentional")
            );
        }

        let orig_imports: Vec<&str> = original.lines()
            .filter(|l| l.trim_start().starts_with("import ") || l.trim_start().starts_with("from "))
            .collect();
        let mod_imports: Vec<&str> = modified.lines()
            .filter(|l| l.trim_start().starts_with("import ") || l.trim_start().starts_with("from "))
            .collect();

        for import in &orig_imports {
            if !mod_imports.contains(import) {
                findings.push(
                    Finding::warning("Import removed", format!("Import statement removed: {}", import.trim()))
                    .with_code(import.trim())
                    .with_suggestion("Ensure this import is no longer needed")
                );
            }
        }

        findings
    }
}
