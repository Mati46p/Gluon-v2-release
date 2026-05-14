use tree_sitter::{Parser, Language, Node};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PatchProposal {
    pub original_range: std::ops::Range<usize>, // Bajtowy zakres do wymiany w pliku
    pub new_content: String,
    pub confidence: f32, // 0.0 - 1.0
}

pub struct StructuralMatcher {
    parsers: HashMap<String, Language>,
}

// Implementacja Send, ponieważ Language (wskaźnik C) nie jest automatycznie Send,
// a używamy go w Mutexie w engine.rs.
// tree_sitter::Language jest thread-safe.
unsafe impl Send for StructuralMatcher {}

impl StructuralMatcher {
    pub fn new() -> Self {
        let mut parsers = HashMap::new();
        
        // ZMIANA: W nowszych wersjach tree-sitter (0.24+) używamy stałych LANGUAGE
        // i konwertujemy je na strukturę Language przez .into()
        parsers.insert("rs".into(), tree_sitter_rust::LANGUAGE.into());
        parsers.insert("ts".into(), tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into());
        parsers.insert("tsx".into(), tree_sitter_typescript::LANGUAGE_TSX.into());
        parsers.insert("js".into(), tree_sitter_javascript::LANGUAGE.into());
        parsers.insert("py".into(), tree_sitter_python::LANGUAGE.into());

        Self { parsers }
    }

    /// Główna funkcja: Próbuje znaleźć miejsce pasujące do 'snippet' w 'file_content'
    pub fn find_best_match(&mut self, file_extension: &str, file_content: &str, snippet: &str) -> Option<PatchProposal> {
        let language = self.parsers.get(file_extension)?;
        
        let mut parser = Parser::new();
        // ZMIANA: set_language oczekuje &Language, a language to &Language (z HashMapy)
        // Nie dereferencjujemy go (*language).
        parser.set_language(language).ok()?;

        // 1. Parsuj oba fragmenty
        let file_tree = parser.parse(file_content, None)?;
        let snippet_tree = parser.parse(snippet, None)?;

        let file_root = file_tree.root_node();
        let snippet_root = snippet_tree.root_node();

        // 2. Znajdź główny węzeł w kodzie AI (np. FunctionDeclaration)
        let snippet_anchor = self.find_significant_node(snippet_root)?;

        // 3. Szukaj pasującej kotwicy w pliku użytkownika
        self.find_matching_node_recursive(file_root, snippet_anchor, file_content, snippet)
    }

    /// Rekurencyjne szukanie pasującego węzła w drzewie pliku
    fn find_matching_node_recursive<'a>(&self, cursor_node: Node<'a>, target_node: Node, file_source: &str, snippet_source: &str) -> Option<PatchProposal> {
        // Sprawdź czy węzły są "strukturalnie identyczne" (ta sama nazwa, typ)
        if self.are_nodes_anchored(&cursor_node, &target_node, file_source, snippet_source) {
            return Some(PatchProposal {
                original_range: cursor_node.byte_range(),
                new_content: snippet_source[target_node.byte_range()].to_string(),
                confidence: 1.0, // Pełne dopasowanie kotwicy
            });
        }

        // ZMIANA: Używamy ręcznej iteracji zamiast node.children(), aby uniknąć błędów borrow checkera
        let mut cursor = cursor_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if let Some(proposal) = self.find_matching_node_recursive(child, target_node, file_source, snippet_source) {
                    return Some(proposal);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        None
    }

    /// Kluczowa logika: Czy to ta sama funkcja/klasa?
    fn are_nodes_anchored(&self, file_node: &Node, snippet_node: &Node, file_source: &str, snippet_source: &str) -> bool {
        // 1. Muszą mieć ten sam typ (np. "function_item")
        if file_node.kind() != snippet_node.kind() {
            return false;
        }

        // 2. Muszą mieć tę samą nazwę (identyfikator)
        let file_name = self.extract_name(file_node, file_source);
        let snippet_name = self.extract_name(snippet_node, snippet_source);

        match (file_name, snippet_name) {
            (Some(fn_name), Some(sn_name)) => fn_name == sn_name,
            _ => false, // Jeśli nie mają nazw (np. blok if), nie ryzykujemy podmiany
        }
    }

    /// Pomocnicza: Wyciąga nazwę funkcji/klasy z węzła
    fn extract_name(&self, node: &Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        
        // ZMIANA: Ręczna iteracja po dzieciach, aby móc bezpiecznie wołać cursor.field_name()
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                
                // Sprawdź czy to pole "name"
                if let Some(field_name) = cursor.field_name() {
                    if field_name == "name" {
                        return Some(source[child.byte_range()].to_string());
                    }
                }
                
                // Fallback: sprawdź czy to identyfikator (dla JS/TS)
                if child.kind() == "identifier" || child.kind() == "type_identifier" {
                    return Some(source[child.byte_range()].to_string());
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }

    /// Znajduje pierwszy "ważny" węzeł (omija komentarze i puste znaki na początku snippeta)
    fn find_significant_node<'a>(&self, root: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = root.walk();
        
        // ZMIANA: Ręczna iteracja
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                
                // Ignoruj komentarze i błędy parsowania
                if child.kind() != "comment" && !child.is_error() {
                    // Jeśli to Function/Class/Impl/Method - to jest nasza kotwica
                    if child.kind().contains("function") 
                       || child.kind().contains("class") 
                       || child.kind().contains("impl") 
                       || child.kind().contains("method") {
                        return Some(child);
                    }
                }
                
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        
        // Fallback: zwróć pierwsze dziecko jeśli nie znaleziono nic specyficznego
        root.child(0)
    }

    /// Liczy "istotne" węzły (bez komentarzy i interpunkcji)
    pub fn count_significant_nodes(&mut self, extension: &str, content: &str) -> Option<usize> {
        let language = self.parsers.get(extension)?;
        let mut parser = Parser::new();
        parser.set_language(language).ok()?;
        
        let tree = parser.parse(content, None)?;
        let root = tree.root_node();
        
        let mut count = 0;
        let mut cursor = root.walk();
        
        // Prosta iteracja po całym drzewie (pre-order)
        let mut visited_children = false;
        loop {
            if !visited_children {
                let node = cursor.node();
                if node.is_named() && node.kind() != "comment" {
                    count += 1;
                }
                if cursor.goto_first_child() {
                    visited_children = false;
                    continue;
                }
            }
            
            if cursor.goto_next_sibling() {
                visited_children = false;
                continue;
            }
            
            if cursor.goto_parent() {
                visited_children = true;
                continue;
            }
            break;
        }
        
        Some(count)
    }

    pub fn has_syntax_error(&mut self, extension: &str, content: &str) -> bool {
        let language = if let Some(l) = self.parsers.get(extension) { l } else { return false };
        let mut parser = Parser::new();
        if parser.set_language(language).is_err() { return false; }
        
        if let Some(tree) = parser.parse(content, None) {
             return tree.root_node().has_error();
        }
        true // Jeśli nie udało się sparsować, zakładamy błąd
    }
}