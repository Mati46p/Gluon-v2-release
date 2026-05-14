//! Semantic Queries using Tree-sitter
//!
//! Extracts semantic information (definitions, signatures) using S-expression queries.

use super::languages::SupportedLanguage;
use tree_sitter::{Query, QueryCursor, Node, Tree, StreamingIterator};

#[derive(Debug, Clone)]
pub struct SemanticSignature {
    pub name: String,
    pub kind: String, // "function", "class", "method"
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_row: usize, // 0-based
    pub end_row: usize,   // 0-based
    pub parent_name: Option<String>, // Context anchor (e.g. class name)

    // [GLUON AUDITOR - BASIC] Integrity fields
    pub body_size_bytes: usize,
    pub parameters_hash: u64,

    // [GLUON AUDITOR - ENTERPRISE] Deep Analysis Metrics
    pub cyclomatic_complexity: usize,
    pub security_alerts: Vec<String>, // List of detected risky patterns (e.g. "eval", "os.system")
    pub type_coverage: f32,           // 0.0 to 1.0 (percentage of typed arguments/returns)
    pub has_docstring: bool,
}

pub struct QueryMatcher;

impl QueryMatcher {
    /// Extracts semantic signatures from the syntax tree.
    pub fn extract_signatures(code: &str, tree: &Tree, language: SupportedLanguage) -> Vec<SemanticSignature> {
        let query_str = Self::get_query_source(language);
        if query_str.is_empty() {
            return Vec::new();
        }

        let grammar = language.get_grammar();
        let query = match Query::new(&grammar, query_str) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Failed to compile query for {:?}: {:?}", language, e);
                return Vec::new();
            }
        };

        let mut cursor = QueryCursor::new();
        let mut signatures = Vec::new();

        // matches() returns a StreamingIterator over QueryMatch
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            let mut name = String::new();
            let mut def_node: Option<Node> = None;
            let mut kind = "unknown".to_string();

            for capture in m.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                match capture_name {
                    "name" => {
                        if let Ok(n) = capture.node.utf8_text(code.as_bytes()) {
                            name = n.to_string();
                        }
                    },
                    "definition" => {
                        def_node = Some(capture.node);
                        kind = capture.node.kind().to_string();
                    },
                    "function" => kind = "function".to_string(),
                    "class" => kind = "class".to_string(),
                    "method" => kind = "method".to_string(),
                    _ => {}
                }
            }

            if let Some(node) = def_node {
                if !name.is_empty() {
                    let refined_kind = if kind == "unknown" || kind == node.kind() {
                        Self::simplify_kind(node.kind())
                    } else {
                        kind
                    };

                    // Extract parent context (Semantic Anchor)
                    let parent_name = Self::find_parent_name(node, code.as_bytes());

                    // [GLUON AUDITOR] Basic Metrics
                    let body_size = node.end_byte() - node.start_byte();
                    let params_hash = Self::hash_parameters(node, code.as_bytes());

                    // [GLUON AUDITOR] Enterprise Metrics (Deep Analysis)
                    // We only run deep analysis on function/method bodies to save performance
                    let (complexity, security, types, docs) = if refined_kind == "function" || refined_kind == "method" {
                        Self::analyze_deep_metrics(node, code.as_bytes(), language)
                    } else {
                        (1, Vec::new(), 1.0, false) // Default for classes/structs (can be expanded later)
                    };

                    signatures.push(SemanticSignature {
                        name,
                        kind: refined_kind,
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                        start_row: node.start_position().row,
                        end_row: node.end_position().row,
                        parent_name,
                        body_size_bytes: body_size,
                        parameters_hash: params_hash,
                        cyclomatic_complexity: complexity,
                        security_alerts: security,
                        type_coverage: types,
                        has_docstring: docs,
                    });
                }
            }
        }

        Self::deduplicate_signatures(signatures)
    }

    /// [GLUON AUDITOR - ENTERPRISE] Deep Static Analysis
    /// Calculates Complexity, Security Risks, and Type Coverage by walking the AST subtree.
    fn analyze_deep_metrics(node: Node, source: &[u8], language: SupportedLanguage) -> (usize, Vec<String>, f32, bool) {
        let mut complexity = 1; // Base complexity is 1
        let mut security_alerts = Vec::new();
        let mut has_docstring = false;

        let mut total_args = 0;
        let mut typed_args = 0;
        let mut has_return_type = false;

        // --- 1. Docstring Check (Immediate Child) ---
        // Look for string expression as first statement in body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                let kind = child.kind();
                // Python: Expression statement containing string literal
                if kind == "expression_statement" {
                    if let Some(first_child) = child.child(0) {
                        if first_child.kind() == "string" {
                            has_docstring = true;
                        }
                    }
                    break; // Only check first statement
                }
                // JS/Rust: Comments are hidden by default in named_children unless handled, 
                // but usually docstrings are logic-less.
            }
        }

        // --- 2. Type Coverage Analysis ---
        // Check arguments
        if let Some(params) = node.child_by_field_name("parameters") {
            let mut cursor = params.walk();
            for child in params.children(&mut cursor) {
                let kind = child.kind();
                if kind.contains("parameter") || kind == "identifier" {
                    total_args += 1;
                    // Check if it has a type annotation child
                    // Python: type, Rust: type, TS: type_annotation
                    if child.child_by_field_name("type").is_some() || child.child_by_field_name("type_annotation").is_some() {
                        typed_args += 1;
                    }
                }
            }
        }
        // Check return type
        if node.child_by_field_name("return_type").is_some() {
            has_return_type = true;
        }

        let type_score = if total_args == 0 {
            if has_return_type { 1.0 } else { 0.5 } // No args, check return
        } else {
            (typed_args as f32 + (if has_return_type { 1.0 } else { 0.0 })) / (total_args as f32 + 1.0)
        };

        // --- 3. Body Walk (Complexity & Security) ---
        // We traverse the entire subtree of the function
        let mut cursor = node.walk();
        let mut stack = vec![node]; // Simple DFS

        while let Some(curr) = stack.pop() {
            let kind = curr.kind();

            // A. Cyclomatic Complexity
            // Increment for branching/control flow nodes
            match language {
                SupportedLanguage::Python => {
                    if matches!(kind, "if_statement" | "for_statement" | "while_statement" | "except_clause" | "with_statement" | "boolean_operator" | "elif_clause") {
                        complexity += 1;
                    }
                },
                SupportedLanguage::JavaScript | SupportedLanguage::TypeScript | SupportedLanguage::TypeScriptReact => {
                    if matches!(kind, "if_statement" | "else_clause" | "for_statement" | "while_statement" | "catch_clause" | "ternary_expression" | "binary_expression" | "switch_case") {
                        // For JS binary expr, only && and || increase complexity
                        if kind == "binary_expression" {
                            if let Ok(op) = curr.child_by_field_name("operator").unwrap().utf8_text(source) {
                                if op == "&&" || op == "||" { complexity += 1; }
                            }
                        } else {
                            complexity += 1;
                        }
                    }
                },
                SupportedLanguage::Rust => {
                    if matches!(kind, "if_expression" | "for_expression" | "while_expression" | "match_arm" | "loop_expression") {
                        complexity += 1;
                    }
                },
                SupportedLanguage::Kotlin => {
                    if matches!(kind, "if_expression" | "for_statement" | "while_statement" | "when_expression" | "try_expression" | "binary_expression") {
                        // For binary expr, only && and || increase complexity
                        if kind == "binary_expression" {
                            if let Ok(op) = curr.child_by_field_name("operator").unwrap().utf8_text(source) {
                                if op == "&&" || op == "||" { complexity += 1; }
                            }
                        } else {
                            complexity += 1;
                        }
                    }
                },
                _ => {} // Default fallback
            }

            // B. Security Scan (Lightweight SAST)
            // Identify dangerous calls
            if kind == "call" || kind == "call_expression" {
                // Try to find the function name
                let func_node = curr.child_by_field_name("function").or_else(|| curr.child(0));

                if let Some(f) = func_node {
                    if let Ok(name) = f.utf8_text(source) {
                        // Security Risks Database
                        let risk = match language {
                            SupportedLanguage::Python => match name {
                                "eval" | "exec" => Some("Dynamic Code Execution (eval/exec)"),
                                "os.system" | "subprocess.call" | "subprocess.run" | "popen" => Some("Shell Injection Risk"),
                                "pickle.loads" | "yaml.load" => Some("Unsafe Deserialization"),
                                "input" => Some("User Input (Sanitize check needed)"),
                                _ => None
                            },
                            SupportedLanguage::JavaScript | SupportedLanguage::TypeScript | SupportedLanguage::TypeScriptReact => match name {
                                "eval" => Some("Dynamic Code Execution (eval)"),
                                "document.write" => Some("DOM Injection Risk"),
                                "setTimeout" | "setInterval" => None, // usually okay, check if string arg?
                                _ => None
                            },
                            _ => None
                        };

                        if let Some(r) = risk {
                            security_alerts.push(format!("Line {}: {} detected ('{}')", curr.start_position().row + 1, r, name));
                        }
                    }
                }
            }

            // Special checks for properties (innerHTML)
            if (kind == "member_expression" || kind == "attribute") && (language == SupportedLanguage::JavaScript || language == SupportedLanguage::TypeScript) {
                 if let Ok(prop) = curr.child_by_field_name("property").or_else(|| curr.child_by_field_name("attribute")).unwrap_or(curr).utf8_text(source) {
                     if prop == "innerHTML" || prop == "outerHTML" || prop == "dangerouslySetInnerHTML" {
                         security_alerts.push(format!("Line {}: Unsafe DOM Assignment ({})", curr.start_position().row + 1, prop));
                     }
                 }
            }

            // Recurse children
            for child in curr.children(&mut cursor) {
                stack.push(child);
            }
        }

        (complexity, security_alerts, type_score, has_docstring)
    }

    /// Remove duplicate signatures, preferring decorated_definition over function_definition
    fn deduplicate_signatures(signatures: Vec<SemanticSignature>) -> Vec<SemanticSignature> {
        use std::collections::HashMap;

        // Only log for large files (reduces console spam)
        if signatures.len() > 100 {
            eprintln!("[Deduplicate] Processing {} signatures", signatures.len());
        }

        // Group by (name, kind, parent_name)
        let mut seen: HashMap<(String, String, Option<String>), SemanticSignature> = HashMap::new();

        for sig in signatures {
            let key = (sig.name.clone(), sig.kind.clone(), sig.parent_name.clone());

            match seen.get(&key) {
                Some(existing) => {
                    // If new signature starts earlier (includes decorators), replace
                    if sig.start_row < existing.start_row {
                        seen.insert(key, sig);
                    }
                    // Otherwise keep existing (no action needed)
                }
                None => {
                    seen.insert(key, sig);
                }
            }
        }

        seen.into_values().collect()
    }
 
    /// Walks up the AST to find the name of the parent definition (class/impl/etc).
    fn find_parent_name(node: Node, source: &[u8]) -> Option<String> {
        let mut curr = node.parent();
        while let Some(parent) = curr {
            let kind = parent.kind();
            // Check for structural parents
            if kind.contains("class") || kind.contains("struct") || kind.contains("impl") || kind.contains("interface") {
                // Most definitions use 'name' field
                if let Some(name_node) = parent.child_by_field_name("name") {
                    return name_node.utf8_text(source).ok().map(|s| s.to_string());
                }
                // Rust 'impl' blocks often use 'type' field for the struct name
                if kind == "impl_item" {
                     if let Some(type_node) = parent.child_by_field_name("type") {
                        return type_node.utf8_text(source).ok().map(|s| s.to_string());
                     }
                }
            }
            curr = parent.parent();
        }
        None
    }

    /// [GLUON AUDITOR] Hash the parameters/arguments node text to detect API changes
    fn hash_parameters(node: Node, source: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        
        // Try to find the parameters child node
        let child_names = ["parameters", "parameter_list", "formal_parameters", "function_declarator", "argument_list"];
        
        for name in child_names {
            if let Some(child) = node.child_by_field_name(name) {
                if let Ok(text) = child.utf8_text(source) {
                    // Normalize whitespace to avoid false positives on formatting
                    let normalized = text.split_whitespace().collect::<String>();
                    normalized.hash(&mut hasher);
                    return hasher.finish();
                }
            }
        }
        
        // Fallback: If no explicit parameters field, look for children of type "parameter_list" etc.
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind.contains("parameter") || kind.contains("argument") {
                if let Ok(text) = child.utf8_text(source) {
                    let normalized = text.split_whitespace().collect::<String>();
                    normalized.hash(&mut hasher);
                    return hasher.finish();
                }
            }
        }

        0 // No parameters found or failed to extract
    }
 
    fn simplify_kind(raw_kind: &str) -> String {
        if raw_kind.contains("function") || raw_kind.contains("fn") {
            "function".to_string()
        } else if raw_kind.contains("class") || raw_kind.contains("struct") {
            "class".to_string()
        } else if raw_kind.contains("method") {
            "method".to_string()
        } else {
            raw_kind.to_string()
        }
    }

    fn get_query_source(language: SupportedLanguage) -> &'static str {
        match language {
            SupportedLanguage::Python => r#"
                ;; [GLUON V6 FIX] Decorated definitions FIRST (to prefer full match with decorators)
                (decorated_definition
                    (function_definition
                        name: (identifier) @name)) @definition @function
                (decorated_definition
                    (class_definition
                        name: (identifier) @name)) @definition @class
                ;; Fallback: Undecorated definitions
                (function_definition
                    name: (identifier) @name) @definition @function
                (class_definition
                    name: (identifier) @name) @definition @class
            "#,
            SupportedLanguage::Rust => r#"
                (function_item
                    name: (identifier) @name) @definition @function
                (struct_item
                    name: (type_identifier) @name) @definition @class
                (impl_item
                    type: (type_identifier) @name) @definition @class
            "#,
            SupportedLanguage::JavaScript => r#"
                (function_declaration
                    name: (identifier) @name
                ) @definition @function
                (class_declaration
                    name: (identifier) @name
                ) @definition @class
                (method_definition
                    name: [(property_identifier) (identifier)] @name
                ) @definition @method
                (variable_declarator
                    name: (identifier) @name
                    value: [(arrow_function) (function_expression)]
                ) @definition @function
                (pair
                    key: (property_identifier) @name
                    value: [(arrow_function) (function_expression)]
                ) @definition @method
                (assignment_expression
                    left: (member_expression
                        property: (property_identifier) @name)
                    right: [(arrow_function) (function_expression)]
                ) @definition @method
            "#,
            SupportedLanguage::TypeScript | SupportedLanguage::TypeScriptReact => r#"
                (function_declaration
                    name: (identifier) @name
                ) @definition @function
                (class_declaration
                    name: (type_identifier) @name
                ) @definition @class
                (method_definition
                    name: [(property_identifier) (identifier)] @name
                ) @definition @method
                (variable_declarator
                    name: (identifier) @name
                    value: [(arrow_function) (function_expression)]
                ) @definition @function
                (pair
                    key: (property_identifier) @name
                    value: [(arrow_function) (function_expression)]
                ) @definition @method
                (assignment_expression
                    left: (member_expression
                        property: (property_identifier) @name)
                    right: [(arrow_function) (function_expression)]
                ) @definition @method
            "#,
            SupportedLanguage::Go => r#"
                (function_declaration
                    name: (identifier) @name) @definition @function
                (method_declaration
                    name: (field_identifier) @name) @definition @method
                (type_declaration
                    (type_spec
                        name: (type_identifier) @name)) @definition @class
            "#,
            SupportedLanguage::Java => r#"
                (method_declaration
                    name: (identifier) @name) @definition @method
                (class_declaration
                    name: (identifier) @name) @definition @class
                (interface_declaration
                    name: (identifier) @name) @definition @class
                (record_declaration
                    name: (identifier) @name) @definition @class
            "#,
            SupportedLanguage::Kotlin => r#"
                (function_declaration
                    name: (identifier) @name) @definition @function
                (class_declaration
                    name: (type_identifier) @name) @definition @class
                (interface_declaration
                    name: (type_identifier) @name) @definition @class
                (object_declaration
                    name: (type_identifier) @name) @definition @class
            "#,
            SupportedLanguage::Cpp => r#"
                (function_definition
                    declarator: (function_declarator
                        declarator: (identifier) @name)) @definition @function
                (class_specifier
                    name: (type_identifier) @name) @definition @class
                (struct_specifier
                    name: (type_identifier) @name) @definition @class
            "#,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply_system::analysis::AnalysisEngine;

    #[test]
    fn test_extract_python_signatures() {
        let code = r#"
            class MyClass:
                def my_method(self):
                    pass
            
            def standalone():
                pass
        "#;
        let tree = AnalysisEngine::parse(code, "test.py").unwrap();
        let sigs = QueryMatcher::extract_signatures(code, &tree, SupportedLanguage::Python);

        assert!(sigs.iter().any(|s| s.name == "MyClass" && s.kind == "class"));
        assert!(sigs.iter().any(|s| s.name == "my_method" && s.kind == "function")); // Tree-sitter python uses function_definition for methods too
        assert!(sigs.iter().any(|s| s.name == "standalone" && s.kind == "function"));
    }

    #[test]
    fn test_extract_rust_signatures() {
        let code = r#"
            fn main() {}
            struct Config {}
        "#;
        let tree = AnalysisEngine::parse(code, "test.rs").unwrap();
        let sigs = QueryMatcher::extract_signatures(code, &tree, SupportedLanguage::Rust);

        assert!(sigs.iter().any(|s| s.name == "main" && s.kind == "function"));
        assert!(sigs.iter().any(|s| s.name == "Config" && s.kind == "class"));
    }
}