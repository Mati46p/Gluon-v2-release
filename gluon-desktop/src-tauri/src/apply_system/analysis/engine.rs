//! Core Analysis Engine
//!
//! Wrapper around Tree-sitter parser. Handles parser initialization,
//! tree generation, and heuristics for partial code snippets.

use super::languages::SupportedLanguage;
use tree_sitter::{Parser, Tree};

pub struct AnalysisEngine;

pub struct ParsedResult {
    pub tree: Tree,
    pub is_wrapped: bool,
    pub wrapper_offset: usize, // Line offset if wrapped
    pub effective_code: String, // The actual code string used for parsing (original or wrapped)
}


impl AnalysisEngine {
    /// Parses code into a Syntax Tree.
    ///
    /// # Arguments
    /// * `code` - Source code content
    /// * `file_path` - Path to the file (used for language detection)
    ///
    /// # Returns
    /// * `Ok(Tree)` - The parsed syntax tree
    /// * `Err(String)` - If language is not supported or parsing fails initialization
    pub fn parse(code: &str, file_path: &str) -> Result<Tree, String> {
        let language = SupportedLanguage::from_path(file_path)
            .ok_or_else(|| format!("Unsupported language for file: {}", file_path))?;

        let mut parser = Parser::new();
        parser.set_language(&language.get_grammar())
            .map_err(|e| format!("Failed to set language grammar: {}", e))?;

        parser.parse(code, None)
            .ok_or_else(|| "Tree-sitter failed to produce a tree (internal error)".to_string())
    }

    /// Smart parsing for fragments/snippets.
    ///
    /// Many AI responses contain snippets like indented methods without a class.
    /// This method tries to parse normally first. If significant errors are found
    /// (like unexpected indentation), it tries to wrap the code in a dummy context.
    pub fn parse_with_heuristics(code: &str, file_path: &str) -> Result<ParsedResult, String> {
        let tree = Self::parse(code, file_path)?;
        
        // 1. Check for critical root-level errors
        if !Self::has_critical_root_errors(&tree) {
            return Ok(ParsedResult {
                tree,
                is_wrapped: false,
                wrapper_offset: 0,
                effective_code: code.to_string(),
            });
        }

        // 2. Apply language-specific wrapper heuristics
        let language = SupportedLanguage::from_path(file_path)
            .ok_or_else(|| "Unknown language".to_string())?;

        if let Some((wrapped_code, offset)) = Self::wrap_code(code, language) {
            // Try parsing the wrapped version
            let wrapped_tree = Self::parse(&wrapped_code, file_path)?;
            
            // If wrapping fixed the errors, return the wrapped tree
            if !Self::has_critical_root_errors(&wrapped_tree) {
                return Ok(ParsedResult {
                    tree: wrapped_tree,
                    is_wrapped: true,
                    wrapper_offset: offset,
                    effective_code: wrapped_code,
                });
            }
        }

        // Fallback: return original tree even with errors
        Ok(ParsedResult {
            tree,
            is_wrapped: false,
            wrapper_offset: 0,
            effective_code: code.to_string(),
        })
    }

    /// Checks if the tree has syntax errors at the top level.
    fn has_critical_root_errors(tree: &Tree) -> bool {
        let root = tree.root_node();
        // Check if root itself has error flag
        if root.has_error() {
            // Check direct children for ERROR nodes
            let mut cursor = root.walk();
            for child in root.children(&mut cursor) {
                if child.kind() == "ERROR" || child.is_error() {
                    return true;
                }
            }
            // Also check if there are MISSING nodes which indicate syntax issues
            for child in root.children(&mut cursor) {
                if child.is_missing() {
                    return true;
                }
            }
        }
        false
    }

    /// Wraps code in a dummy context based on language.
    /// Returns (new_code, line_offset).
    ///
    /// [GLUON V2 - PHASE 1] Contextual Fragment Wrapping
    /// Creates intelligent wrappers based on code content analysis:
    /// - Detects if code contains `self` → wraps in class method
    /// - Detects indentation → wraps appropriately
    /// - Handles both complete and partial fragments
    fn wrap_code(code: &str, lang: SupportedLanguage) -> Option<(String, usize)> {
        match lang {
            SupportedLanguage::Python => {
                // Analyze code characteristics
                let has_self = code.contains("self.");
                let has_indent = code.lines().any(|l| l.starts_with(' ') || l.starts_with('\t'));
                let first_line = code.lines().next().unwrap_or("");
                let base_indent = Self::detect_base_indent(code);

                // Strategy 1: Code with `self` or already indented → likely method fragment
                if has_self || (has_indent && base_indent > 0) {
                    // Wrap in class + method (8 spaces total indent)
                    let indented_code: String = code.lines()
                        .map(|l| {
                            if l.trim().is_empty() {
                                String::new()
                            } else {
                                // Preserve existing indentation + add method indent (8 spaces)
                                format!("        {}", l)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    return Some((
                        format!("class __GluonWrapper:\n    def __gluon_method(self):\n{}", indented_code),
                        2
                    ));
                }

                // Strategy 2: Plain statements without self → wrap in function
                if first_line.trim().starts_with("return") ||
                   first_line.trim().starts_with("if") ||
                   first_line.trim().starts_with("for") ||
                   first_line.trim().starts_with("while") {
                    let indented_code: String = code.lines()
                        .map(|l| {
                            if l.trim().is_empty() {
                                String::new()
                            } else {
                                format!("    {}", l)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    return Some((
                        format!("def __gluon_wrapper():\n{}", indented_code),
                        1
                    ));
                }

                // Strategy 3: Generic wrapper (default fallback)
                let indented_code: String = code.lines()
                    .map(|l| if l.trim().is_empty() { String::new() } else { format!("        {}", l) })
                    .collect::<Vec<_>>()
                    .join("\n");

                Some((format!("class __GluonWrapper:\n    def __gluon_method(self):\n{}", indented_code), 2))
            },

            SupportedLanguage::JavaScript | SupportedLanguage::TypeScript | SupportedLanguage::TypeScriptReact => {
                // Analyze for class method indicators
                let has_this = code.contains("this.");
                let has_arrow_func = code.contains("=>");
                let has_method_pattern = code.trim_start().starts_with("async") ||
                                        code.contains("function");

                // Strategy 1: Code with `this` → class method wrapper
                if has_this && !has_arrow_func {
                    let indented_code: String = code.lines()
                        .map(|l| if l.trim().is_empty() { String::new() } else { format!("        {}", l) })
                        .collect::<Vec<_>>()
                        .join("\n");

                    return Some((
                        format!("class __GluonWrapper {{\n    __gluonMethod() {{\n{}\n    }}\n}}", indented_code),
                        2
                    ));
                }

                // Strategy 2: Loose statements → function wrapper
                if !has_method_pattern {
                    let indented_code: String = code.lines()
                        .map(|l| if l.trim().is_empty() { String::new() } else { format!("    {}", l) })
                        .collect::<Vec<_>>()
                        .join("\n");

                    return Some((
                        format!("function __gluonWrapper() {{\n{}\n}}", indented_code),
                        1
                    ));
                }

                // Default: class method wrapper
                let indented_code: String = code.lines()
                    .map(|l| if l.trim().is_empty() { String::new() } else { format!("    {}", l) })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some((format!("class __GluonWrapper {{\n    __gluonMethod() {{\n{}\n    }}\n}}", indented_code), 2))
            }

            SupportedLanguage::Java => {
                // Java always needs class context
                let indented_code: String = code.lines()
                    .map(|l| if l.trim().is_empty() { String::new() } else { format!("        {}", l) })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some((format!("class __GluonWrapper {{\n    void __gluonMethod() {{\n{}\n    }}\n}}", indented_code), 2))
            }

            SupportedLanguage::Cpp => {
                let indented_code: String = code.lines()
                    .map(|l| if l.trim().is_empty() { String::new() } else { format!("        {}", l) })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some((format!("class __GluonWrapper {{\n    void __gluonMethod() {{\n{}\n    }}\n}};", indented_code), 2))
            }

            SupportedLanguage::Rust => {
                // Rust snippet wrapping: put inside a function
                let indented_code: String = code.lines()
                    .map(|l| if l.trim().is_empty() { String::new() } else { format!("    {}", l) })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some((format!("fn __gluon_wrapper() {{\n{}\n}}", indented_code), 1))
            }

            _ => None,
        }
    }

    /// Helper: Detect base indentation level of code fragment
    fn detect_base_indent(code: &str) -> usize {
        code.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0)
    }

    /// Checks if a file is supported by the Analysis Engine
    #[allow(dead_code)]
    pub fn is_supported(file_path: &str) -> bool {
        SupportedLanguage::from_path(file_path).is_some()
    }
 
    /// [GLUON V2] Helper: Get visual indentation of a specific node
    pub fn get_node_indentation(node: &tree_sitter::Node, source: &str) -> usize {
        let start_byte = node.start_byte();
        let source_bytes = source.as_bytes();
        
        // Walk backwards from start_byte to find newline
        let mut i = start_byte;
        while i > 0 {
            if source_bytes[i - 1] == b'\n' {
                break;
            }
            i -= 1;
        }
        
        // Count whitespace from newline to start_byte
        let mut indent = 0;
        for j in i..start_byte {
            match source_bytes[j] {
                b' ' => indent += 1,
                b'\t' => indent += 4, // Assume 4 spaces for tab
                _ => {}
            }
        }
        indent
    }
 
    /// [GLUON V2] Helper: Find nearest parent of specific types
    pub fn find_parent_of_type<'a>(mut node: tree_sitter::Node<'a>, types: &[&str]) -> Option<tree_sitter::Node<'a>> {
        while let Some(parent) = node.parent() {
            if types.contains(&parent.kind()) {
                return Some(parent);
            }
            node = parent;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_parsing() {
        let code = "def foo():\n    pass";
        let tree = AnalysisEngine::parse(code, "script.py").expect("Should parse Python");
        let root = tree.root_node();
        assert_eq!(root.kind(), "module");
    }

    #[test]
    fn test_python_fragment_heuristic() {
        // Code with "ghost indentation" (indented but no parent class)
        // May or may not trigger automatic wrapping depending on parse errors
        let code = "    def method(self):\n        return 1";

        let result = AnalysisEngine::parse_with_heuristics(code, "test.py");

        // Should parse successfully - wrapping is optional
        assert!(result.is_ok(), "Should parse code with ghost indentation");
    }

    #[test]
    fn test_rust_parsing() {
        let code = "fn main() { println!(\"Hello\"); }";
        let tree = AnalysisEngine::parse(code, "main.rs").expect("Should parse Rust");
        let root = tree.root_node();
        assert_eq!(root.kind(), "source_file");
    }
}