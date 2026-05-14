//! Symbol Extraction from AST
//!
//! This module uses tree-sitter to extract symbols (functions, classes, methods)
//! from source code files.

use crate::apply_system::analysis::{AnalysisEngine, queries::QueryMatcher, SupportedLanguage};
use std::path::Path;
use std::collections::HashSet;

/// Represents a symbol in the codebase
#[derive(Debug, Clone)]
pub struct Symbol {
    /// File path relative to project root
    pub file_path: String,

    /// Symbol name (e.g., "calculate_total", "UserService")
    pub name: String,

    /// Kind of symbol
    pub kind: SymbolKind,

    /// Line number where symbol is defined (1-based)
    pub line: usize,

    /// Parent context (e.g., class name for methods)
    pub parent: Option<String>,

    /// Signature/preview of the symbol (first line or type signature)
    pub signature: String,
}

/// Kind of symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Interface,
    Struct,
    Enum,
    Constant,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Method => "method",
            SymbolKind::Interface => "interface",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Constant => "const",
        }
    }
}

/// Extract exact content of a specific symbol from a file
/// [GLUON G-RAG] Returns Minimum Viable Context (MVC):
/// 1. Only relevant imports
/// 2. Parent container header (class/impl)
/// 3. Symbol body
pub fn extract_symbol_content(file_path: &Path, symbol_name: &str) -> Result<String, String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let file_path_str = file_path.to_str().ok_or("Invalid file path")?;

    // Support qualified names like "ClassName.methodName" or "ClassName#methodName"
    let (expected_parent, bare_name) = if let Some(dot_pos) = symbol_name.find('.').or_else(|| symbol_name.find('#')) {
        let (parent, method) = symbol_name.split_at(dot_pos);
        (Some(parent.to_string()), &method[1..]) // strip the separator
    } else {
        (None, symbol_name)
    };

    // Detect language
    let language = SupportedLanguage::from_path(file_path_str)
        .ok_or_else(|| format!("Unsupported language for file: {}", file_path_str))?;

    // Parse
    let parsed = AnalysisEngine::parse_with_heuristics(&content, file_path_str)
        .map_err(|e| format!("Failed to parse: {}", e))?;

    // Traverse tree to find the symbol node
    let mut cursor = parsed.tree.walk();

    // Recursive search helper that returns the NODE, not string
    fn find_node_recursive<'a>(cursor: &mut tree_sitter::TreeCursor<'a>, source: &str, target_name: &str) -> Option<tree_sitter::Node<'a>> {
        loop {
            let node = cursor.node();
            let kind = node.kind();

            // Check definitions
            if kind.contains("function") || kind.contains("class") || kind.contains("method") || 
               kind.contains("impl") || kind.contains("struct") || kind.contains("variable") ||
               kind.contains("declaration") || kind.contains("definition") {
                
                let mut name_cursor = node.walk();
                let children: Vec<_> = node.children(&mut name_cursor).collect();

                for child in children {
                    // 1. Bezpośrednie sprawdzenie identyfikatorów
                    if child.kind().contains("identifier") || child.kind() == "name" || child.kind() == "property_identifier" {
                        if let Ok(name) = child.utf8_text(source.as_bytes()) {
                            if name == target_name {
                                return Some(node);
                            }
                        }
                    }
                    
                    // 2. JS/TS: Wsparcie dla variable_declarator (const func = ...)
                    if child.kind() == "variable_declarator" {
                        if let Some(id_node) = child.child_by_field_name("name") {
                            if let Ok(name) = id_node.utf8_text(source.as_bytes()) {
                                if name == target_name { return Some(node); }
                            }
                        }
                    }

                    // 3. Obsługa zagnieżdżonych deklaratorów (Python/JS/C++)
                    if child.kind() == "function_declarator" || child.kind() == "declarator" {
                         let mut decl_cursor = child.walk();
                         for sub in child.children(&mut decl_cursor) {
                            if sub.kind().contains("identifier") {
                                if let Ok(name) = sub.utf8_text(source.as_bytes()) {
                                    if name == target_name {
                                        return Some(node);
                                    }
                                }
                            }
                         }
                    }
                }

                // 4. Sprawdzenie pól tree-sitter (name, identifier) - najbezpieczniejsza metoda
                if let Some(name_node) = node.child_by_field_name("name").or_else(|| node.child_by_field_name("identifier")) {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        if name == target_name { return Some(node); }
                    }
                }
            }

            if cursor.goto_first_child() {
                if let Some(res) = find_node_recursive(cursor, source, target_name) {
                    return Some(res);
                }
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
        None
    }

    // Search for a method/function inside a specific class by name
    fn find_node_in_class<'a>(
        cursor: &mut tree_sitter::TreeCursor<'a>,
        source: &str,
        method_name: &str,
        class_name: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        // Walk all nodes, find class nodes matching class_name, then look for method inside
        fn walk_for_class<'a>(
            cursor: &mut tree_sitter::TreeCursor<'a>,
            source: &str,
            class_name: &str,
        ) -> Option<tree_sitter::Node<'a>> {
            loop {
                let node = cursor.node();
                let kind = node.kind();
                if kind.contains("class") {
                    // Check if name matches
                    let mut nc = node.walk();
                    for child in node.children(&mut nc) {
                        if child.kind().contains("identifier") || child.kind() == "name" || child.kind() == "type_identifier" {
                            if let Ok(n) = child.utf8_text(source.as_bytes()) {
                                if n == class_name { return Some(node); }
                            }
                        }
                    }
                    if let Some(nn) = node.child_by_field_name("name") {
                        if let Ok(n) = nn.utf8_text(source.as_bytes()) {
                            if n == class_name { return Some(node); }
                        }
                    }
                }
                if cursor.goto_first_child() {
                    if let Some(r) = walk_for_class(cursor, source, class_name) { return Some(r); }
                    cursor.goto_parent();
                }
                if !cursor.goto_next_sibling() { break; }
            }
            None
        }

        if let Some(class_node) = walk_for_class(cursor, source, class_name) {
            let mut inner = class_node.walk();
            if inner.goto_first_child() {
                return find_node_recursive(&mut inner, source, method_name);
            }
        }
        None
    }

    // Find by bare name, then optionally validate parent class
    let found_node = if let Some(ref parent_name) = expected_parent {
        // Qualified lookup: find all matches for bare_name, pick one whose parent class matches
        find_node_in_class(&mut cursor, &content, bare_name, parent_name)
            .or_else(|| {
                // Fallback: reset cursor and try bare name only
                cursor = parsed.tree.walk();
                find_node_recursive(&mut cursor, &content, bare_name)
            })
    } else {
        find_node_recursive(&mut cursor, &content, bare_name)
    };

    if let Some(node) = found_node {
        // 1. Extract Symbol Body
        let body_code = &content[node.byte_range()];

        // 2. Identify Used Types in Body (Simple Heuristic for Phase 1)
        // We scan for capitalized words in the body to guess used types
        let used_identifiers: HashSet<String> = body_code
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| s.len() > 1 && s.chars().next().map_or(false, |c| c.is_uppercase()))
            .map(|s| s.to_string())
            .collect();

        // 3. Extract Relevant Imports (Surgical Import Resolution)
        let relevant_imports = extract_relevant_imports(&parsed.tree, &content, &used_identifiers);

        // 4. Extract Parent Context (Breadcrumb Traversal)
        let parent_context = extract_parent_context(node, &content);

        // 5. Assemble MVC (Minimum Viable Context)
        let mut mvc = String::new();

        mvc.push_str(&format!("// File: {}\n", file_path_str));

        if !relevant_imports.is_empty() {
            mvc.push_str("// Context: Surgical Imports\n");
            for imp in relevant_imports {
                mvc.push_str(&imp);
                mvc.push_str("\n");
            }
            mvc.push_str("\n");
        }

        if let Some(parent) = parent_context {
            mvc.push_str("// Context: Container Signature\n");
            mvc.push_str(&parent);
            mvc.push_str("\n    // ... (symbol implementation below) ...\n\n");
        }

        // Add the actual symbol
        mvc.push_str(body_code);

        Ok(mvc)
    } else {
        // [G-RAG] Return error with suggestion if possible
        Err(format!("Symbol '{}' not found in {}. Check Repo Skeleton for exact names.", symbol_name, file_path_str))
    }
}

/// Helper: Extract imports that match used identifiers
fn extract_relevant_imports(tree: &tree_sitter::Tree, source: &str, used_tokens: &HashSet<String>) -> Vec<String> {
    let mut relevant = Vec::new();
    let mut cursor = tree.walk();

    // Iterate root nodes looking for imports
    let root = tree.root_node();
    let mut child_cursor = root.walk();

    for child in root.children(&mut child_cursor) {
        let kind = child.kind();
        if kind.contains("import") || kind.contains("use") || kind == "require" {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                // Check if this import contains any of our used tokens
                // This is a naive intersection but effective for Phase 1
                for token in used_tokens {
                    if text.contains(token) {
                        relevant.push(text.to_string());
                        break;
                    }
                }
            }
        }
    }

    relevant
}

/// Helper: Extract parent class/impl signature
fn extract_parent_context(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut parent = node.parent();
    while let Some(p) = parent {
        let kind = p.kind();
        if kind.contains("class") || kind.contains("impl") || kind.contains("interface") || kind.contains("struct") {
            // Get text up to the body/brace
            // Heuristic: Find the first '{' and cut there
            if let Ok(text) = p.utf8_text(source.as_bytes()) {
                if let Some(idx) = text.find('{') {
                    return Some(format!("{} {{", &text[..idx].trim()));
                } else if let Some(idx) = text.find(':') { // Python
                    return Some(format!("{}:", &text[..idx].trim()));
                }
            }
        }
        parent = p.parent();
    }
    None
}

/// Extract symbols from a file
/// Returns a list of all symbols (functions, classes, etc.) found in the file
pub fn extract_symbols(file_path: &Path, content: &str) -> Result<Vec<Symbol>, String> {
    let file_path_str = file_path.to_str().ok_or("Invalid file path")?;

    // Detect language needed for query extraction
    let language = SupportedLanguage::from_path(file_path_str)
        .ok_or_else(|| format!("Unsupported language for file: {}", file_path_str))?;

    // Parse with tree-sitter
    let parsed = AnalysisEngine::parse_with_heuristics(content, file_path_str)
        .map_err(|e| format!("Failed to parse {}: {}", file_path_str, e))?;

    // Extract semantic signatures using QueryMatcher
    let signatures = QueryMatcher::extract_signatures(content, &parsed.tree, language);

    // Convert to Symbol structs
    let mut symbols = Vec::new();

    for sig in signatures {
        let kind = match sig.kind.as_str() {
            "function_declaration" | "function" | "function_definition" | "arrow_function" => SymbolKind::Function,
            "class_declaration" | "class" | "class_definition" => SymbolKind::Class,
            "method_declaration" | "method" | "method_definition" => SymbolKind::Method,
            "interface_declaration" | "interface" => SymbolKind::Interface,
            "struct" => SymbolKind::Struct,
            "enum" => SymbolKind::Enum,
            "const" | "constant" | "variable_declarator" | "lexical_declaration" => SymbolKind::Constant,
            _ => continue, // Skip unknown types
        };

        symbols.push(Symbol {
            file_path: file_path_str.to_string(),
            name: sig.name.clone(),
            kind,
            // POPRAWKA: Używamy start_row + 1 zamiast nieistniejącego pola `line`
            line: sig.start_row + 1,
            parent: sig.parent_name.clone(),
            signature: format_signature(&sig.name, kind, &sig.parent_name),
        });
    }

    Ok(symbols)
}

/// Format a symbol signature for display
fn format_signature(name: &str, kind: SymbolKind, parent: &Option<String>) -> String {
    match (kind, parent) {
        (SymbolKind::Method, Some(parent_name)) => {
            format!("{}.{}()", parent_name, name)
        }
        (SymbolKind::Method, None) => {
            // Fallback for methods without detected parent
            format!("{}()", name)
        }
        (SymbolKind::Function, _) => {
            format!("{}()", name)
        }
        (SymbolKind::Class, _) => {
            format!("class {}", name)
        }
        (SymbolKind::Struct, _) => {
            format!("struct {}", name)
        }
        (SymbolKind::Interface, _) => {
            format!("interface {}", name)
        }
        (SymbolKind::Enum, _) => {
            format!("enum {}", name)
        }
        (SymbolKind::Constant, _) => {
            format!("const {}", name)
        }
    }
}

/// Extract imports/dependencies from a file
///
/// This helps build the call graph by finding what other files this file depends on
pub fn extract_imports(content: &str, file_ext: &str) -> Vec<String> {
    let mut imports = Vec::new();

    let patterns = match file_ext {
        "ts" | "tsx" | "js" | "jsx" => {
            vec![
                r#"import\s+.*\s+from\s+['"](.+)['""]"#,
                r#"require\(['"](.+)['"]\)"#,
            ]
        }
        "py" => {
            vec![
                r"import\s+([\w\.]+)",
                r"from\s+([\w\.]+)\s+import",
            ]
        }
        "rs" => {
            vec![
                r"use\s+([\w:]+)",
            ]
        }
        _ => return imports,
    };

    for pattern_str in patterns {
        if let Ok(re) = regex::Regex::new(pattern_str) {
            for cap in re.captures_iter(content) {
                if let Some(import_path) = cap.get(1) {
                    imports.push(import_path.as_str().to_string());
                }
            }
        }
    }

    imports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_signature() {
        assert_eq!(
            format_signature("calculate", SymbolKind::Function, &None),
            "calculate()"
        );

        assert_eq!(
            format_signature("process", SymbolKind::Method, &Some("DataProcessor".to_string())),
            "DataProcessor.process()"
        );

        assert_eq!(
            format_signature("User", SymbolKind::Class, &None),
            "class User"
        );
    }
}