//! Syntax Validator for code blocks using Tree-sitter (GLUON ANALYSIS ENGINE).
//!
//! Bridges the gap between the Transaction System and the Analysis Engine.
//! Validates that search/replace blocks are syntactically complete and valid using AST.

use crate::apply_system::analysis::{
    AnalysisEngine, 
    SupportedLanguage, 
    validation::{AstValidator, ValidationErrorType}
};
use tree_sitter::Node;
 
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationSeverity {
    Error,   // Critical error - blocks application
    Warning, // Warning - allows application but logs issue
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub message: String,
    pub line_number: Option<usize>,
}

impl ValidationIssue {
    pub fn error(message: String) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            message,
            line_number: None,
        }
    }

    pub fn error_at_line(message: String, line: usize) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            message,
            line_number: Some(line),
        }
    }

    #[allow(dead_code)]
    pub fn warning(message: String) -> Self {
        Self {
            severity: ValidationSeverity::Warning,
            message,
            line_number: None,
        }
    }

    #[allow(dead_code)]
    pub fn warning_at_line(message: String, line: usize) -> Self {
        Self {
            severity: ValidationSeverity::Warning,
            message,
            line_number: Some(line),
        }
    }
}

pub struct SyntaxValidator;

impl SyntaxValidator {
    /// Validates a code block using Tree-sitter AST.
    ///
    /// This replaces the old Regex-based validation with precise AST analysis.
    /// It handles "snippets" automatically via AnalysisEngine heuristics.
    pub fn validate_block(&self, code: &str, block_name: &str, file_path: &str) -> Result<(), Vec<ValidationIssue>> {
        // Skip validation for empty blocks (valid for DELETE operations)
        if code.trim().is_empty() {
            return Ok(());
        }

        // Check if language is supported
        let language = match SupportedLanguage::from_path(file_path) {
            Some(l) => l,
            None => return Ok(()), // Skip validation for unsupported languages
        };

        // 1. Parse using Analysis Engine (with fragment heuristics)
        let parsed_result = match AnalysisEngine::parse_with_heuristics(code, file_path) {
            Ok(res) => res,
            Err(e) => {
                // If Tree-sitter fails completely (rare), return error
                return Err(vec![ValidationIssue::error(format!("Parser initialization failed: {}", e))]);
            }
        };

        // 2. Run AST Validation
        // CRITICAL FIX: Use `effective_code` (which might be wrapped) to ensure AST indices match the text.
        let ast_errors = AstValidator::validate(
            &parsed_result.effective_code, 
            &parsed_result.tree, 
            language
        );
        if ast_errors.is_empty() {
            return Ok(());
        }

        // 3. Convert AST errors to ValidationIssues
        let mut issues = Vec::new();
        for err in ast_errors {
            let severity = match err.error_type {
                ValidationErrorType::SyntaxError => ValidationSeverity::Error,
                ValidationErrorType::MissingToken => ValidationSeverity::Error,
                // [GLUON V2.2] Strict Semantic Enforcement
                // "self outside class" is now an ERROR unless wrapped. 
                // Since we wrap snippets in AnalysisEngine, any semantic error remaining 
                // means the snippet is fundamentally broken (e.g. self used in a static context 
                // or structure mismatch).
                ValidationErrorType::SemanticError => ValidationSeverity::Error,
            };
 
            // If we wrapped the code (heuristics), adjust line numbers?
            // Currently AnalysisEngine handles wrapping internally, but error lines come from the wrapped tree.
            // If is_wrapped is true, we might need to subtract wrapper_offset.
            let reported_line = if parsed_result.is_wrapped {
                err.line.saturating_sub(parsed_result.wrapper_offset)
            } else {
                err.line
            };

            // Filter out minor issues if code was wrapped (heuristics might introduce noise)
            if parsed_result.is_wrapped && err.error_type == ValidationErrorType::MissingToken {
                // Sometimes wrapping isn't perfect, treat missing tokens in wrapped code as warnings
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    message: format!("(Fragment) {}", err.message),
                    line_number: Some(reported_line),
                });
            } else {
                issues.push(ValidationIssue {
                    severity,
                    message: err.message,
                    line_number: Some(reported_line),
                });
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            // Log for debugging
            eprintln!("\n🔍 AST Analysis for <{}>:", block_name);
            for issue in &issues {
                let icon = if issue.severity == ValidationSeverity::Error { "❌" } else { "⚠️" };
                eprintln!("  {} Line {}: {}", icon, issue.line_number.unwrap_or(0), issue.message);
            }
            
            // Return errors if any (warnings don't fail the result strictly speaking, 
            // but the caller decides. Usually we return Err if list is not empty).
            // However, to mimic old behavior: strictly return Err only if Errors exist?
            // The signature returns Result<(), Vec<Issue>>, so we return Err(issues).
            Err(issues)
        }
    }

    /// [GLUON SAFETY V2] Structural Integrity Check (Deep AST Verification)
    ///
    /// Validates the FULL file content after application (Simulation) using a strict AST pass.
    /// Used as the "System B" fallback guard. If IndentationNormalizer fails, this MUST catch it.
    ///
    /// [GLUON V2 - PHASE 1] Enhanced with Lazy Marker Exception Handling
    /// - Ignores ERROR nodes containing lazy markers: `...`, `// ...`, `# ...`
    /// - Only flags genuine syntax errors
    pub fn validate_structure_integrity(&self, full_content: &str, file_path: &str) -> Result<(), Vec<ValidationIssue>> {
        // [GLUON FIX 2.1] JSON Integrity Guard
        // Treat JSON files as data structures, not code. Any syntax error here is fatal.
        if file_path.ends_with(".json") {
            // Attempt to parse as strict JSON
            if let Err(e) = serde_json::from_str::<serde_json::Value>(full_content) {
                let (line, col) = (e.line(), e.column());
                return Err(vec![ValidationIssue::error_at_line(
                    format!("JSON Integrity Failure: Resulting file is invalid JSON. {} (Line {}, Col {})", e, line, col),
                    line
                )]);
            }
            // If valid JSON, we skip Tree-sitter check as it might be redundant or less strict
            return Ok(());
        }

        let language = match SupportedLanguage::from_path(file_path) {
            Some(l) => l,
            None => return Ok(()),
        };

        // 1. Parse the proposed file state
        let tree = match AnalysisEngine::parse(full_content, file_path) {
            Ok(t) => t,
            Err(e) => return Err(vec![ValidationIssue::error(format!("Integrity Check: Parser crashed: {}", e))]),
        };

        // 2. Check for Root-Level Errors (Tree-sitter specific)
        // [PHASE 1 ENHANCEMENT] Filter out Lazy Marker false positives
        let root = tree.root_node();
        if root.has_error() {
            let mut errors = Vec::new();
            let mut cursor = root.walk();

            // Traverse to find specific error nodes for better reporting
            let mut stack = vec![root];
            while let Some(node) = stack.pop() {
                if node.is_error() || node.is_missing() {
                    let start = node.start_position();
                    let end_byte = node.end_byte().min(full_content.len());
                    let range_code = if node.start_byte() < end_byte {
                        &full_content[node.start_byte()..end_byte]
                    } else {
                        "?"
                    };

                    // [CRITICAL] Lazy Marker Exception Filter
                    // Check if this error is actually a lazy marker (valid in Lazy Stitcher mode)
                    if Self::is_lazy_marker(range_code, full_content, node.start_byte()) {
                        // This is a lazy marker, not a real error - skip it
                        eprintln!("[Syntax Validator] Ignoring lazy marker: '{}'", range_code.trim());
                    } else {
                        // Genuine syntax error
                        errors.push(ValidationIssue::error_at_line(
                            format!("CRITICAL SYNTAX ERROR at line {}: Unexpected token '{}'. This often indicates broken indentation or unclosed brackets.", start.row + 1, range_code.trim()),
                            start.row + 1
                        ));
                    }
                }

                if errors.len() > 5 { break; }

                for child in node.children(&mut cursor) {
                    stack.push(child);
                }
            }

            if !errors.is_empty() {
                return Err(errors);
            }
        }

        // 3. Language-Specific Semantic Checks (System A Validation)
        // [GLUON HARDENING] Now includes validate_python_structure (return outside func, if in class)
        let ast_errors = AstValidator::validate(full_content, &tree, language);

        // Filter critical semantic errors that indicate broken structure
        let mut critical_issues = Vec::new();
        for err in ast_errors {
            // We treat SemanticError as Error for Integrity Check because the parser succeeded (no ERROR nodes),
            // but the structure is logically invalid (e.g. return outside function).
            // This is exactly the "InvoiceCreateSerializer" bug.
            if err.error_type == ValidationErrorType::SemanticError || err.error_type == ValidationErrorType::SyntaxError {
                 critical_issues.push(ValidationIssue {
                    severity: ValidationSeverity::Error,
                    message: format!("Structural Integrity Violation: {}", err.message),
                    line_number: Some(err.line),
                });
            }
        }

        if !critical_issues.is_empty() {
            return Err(critical_issues);
        }

        Ok(())
    }

    /// [GLUON V2 - PHASE 1] Lazy Marker Detection
    ///
    /// Checks if an ERROR node is actually a lazy marker placeholder.
    /// Lazy markers are intentional placeholders used in Lazy Stitcher mode:
    /// - `...` (ellipsis)
    /// - `// ... existing code ...` (C-style comment)
    /// - `# ... existing code ...` (Python comment)
    /// - `/* ... */` (block comment)
    ///
    /// Returns true if the error should be ignored (it's a valid lazy marker)
    fn is_lazy_marker(error_text: &str, full_content: &str, error_start_byte: usize) -> bool {
        let trimmed = error_text.trim();

        // Pattern 1: Pure ellipsis
        if trimmed == "..." || trimmed == "…" {
            return true;
        }

        // Pattern 2: Comment-based lazy markers
        if trimmed.starts_with("//") && trimmed.contains("...") {
            return true;
        }

        if trimmed.starts_with('#') && trimmed.contains("...") {
            return true;
        }

        if trimmed.starts_with("/*") && trimmed.contains("...") && trimmed.ends_with("*/") {
            return true;
        }

        // Pattern 3: Common lazy marker phrases (case-insensitive)
        let lower = trimmed.to_lowercase();
        if lower.contains("existing code") ||
           lower.contains("rest of") ||
           lower.contains("unchanged") ||
           lower.contains("omitted") {
            return true;
        }

        // Pattern 4: Check surrounding context for lazy marker comments
        // Extract the line containing this error
        if let Some(line_text) = Self::get_line_at_byte(full_content, error_start_byte) {
            let line_trimmed = line_text.trim();

            // Check if the entire line is a lazy marker comment
            if (line_trimmed.starts_with("//") || line_trimmed.starts_with('#')) &&
               (line_trimmed.contains("...") || line_trimmed.contains("existing")) {
                return true;
            }
        }

        false
    }

    /// Helper: Extract the line of text at a given byte offset
    fn get_line_at_byte(content: &str, byte_offset: usize) -> Option<&str> {
        if byte_offset >= content.len() {
            return None;
        }

        // Find line start (scan backwards to newline)
        let mut line_start = byte_offset;
        while line_start > 0 && content.as_bytes()[line_start - 1] != b'\n' {
            line_start -= 1;
        }

        // Find line end (scan forwards to newline)
        let mut line_end = byte_offset;
        let bytes = content.as_bytes();
        while line_end < bytes.len() && bytes[line_end] != b'\n' {
            line_end += 1;
        }

        Some(&content[line_start..line_end])
    }
 
    /// [GLUON SAFETY] Global Bracket Balance Check
    ///
    /// A language-agnostic check to ensure we didn't leave dangling braces/parens.
    /// Uses Tree-sitter's error node detection as the source of truth.
    pub fn check_global_bracket_balance(&self, code: &str) -> Result<(), ValidationIssue> {
        // We use a generic parse attempt. If Tree-sitter finds ERROR nodes at root,
        // it usually means unbalanced structure.
        
        // Using "dummy.rs" generic parser for C-style languages, or .py for python
        // Since we don't have file path here, we do a quick heuristic check
        let is_python = code.contains("def ") || code.contains("import ");
        let lang_hint = if is_python { "dummy.py" } else { "dummy.rs" };
 
        let tree = match AnalysisEngine::parse(code, lang_hint) {
            Ok(t) => t,
            Err(_) => return Ok(()), // If parser fails to init, we skip this check (fail open)
        };
 
        let root = tree.root_node();
        if root.has_error() {
            // Find the first error node
            let mut cursor = root.walk();
            let mut error_node: Option<Node> = None;
            
            // Simple DFS to find error
            'search: for child in root.children(&mut cursor) {
                if child.is_error() || child.is_missing() {
                    error_node = Some(child);
                    break 'search;
                }
            }
 
            if let Some(node) = error_node {
                let start = node.start_position();
                return Err(ValidationIssue::error_at_line(
                    format!("Unbalanced syntax detected near line {}. Possible missing brace/parenthesis.", start.row + 1),
                    start.row + 1
                ));
            }
            
            // If root has error but immediate children don't show it clearly
            return Err(ValidationIssue::error("Global syntax structure is broken (Root Error).".to_string()));
        }
 
        Ok(())
    }

    /// Public helper to check if issues contain any errors (vs just warnings).
    pub fn has_errors(issues: &[ValidationIssue]) -> bool {
        issues.iter().any(|i| i.severity == ValidationSeverity::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_python_code() {
        let validator = SyntaxValidator;
        let code = "def foo():\n    return 1";
        assert!(validator.validate_block(code, "test", "test.py").is_ok());
    }

    #[test]
    fn test_invalid_python_syntax() {
        let validator = SyntaxValidator;
        let code = "def foo() return 1"; // Missing colon
        let result = validator.validate_block(code, "test", "test.py");
        assert!(result.is_err());
        let issues = result.unwrap_err();
        assert!(SyntaxValidator::has_errors(&issues));
    }

    #[test]
    fn test_python_snippet_heuristic() {
        let validator = SyntaxValidator;
        // Indented method without class - validator correctly flags this as error
        let code = "    def method(self):\n        pass";
        let result = validator.validate_block(code, "test", "test.py");

        // Validator correctly rejects method with 'self' outside class context
        // This is proper strict validation behavior
        assert!(result.is_err(), "Should reject method with 'self' outside class");
    }
}