// Plik: gluon-desktop/src-tauri/src/apply_system/context/repo_map.rs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use tree_sitter::{Parser, Language, Node};
use walkdir::WalkDir;

pub struct RepoMap {
    symbol_index: HashMap<String, Vec<PathBuf>>,
}

impl RepoMap {
    pub fn new() -> Self {
        Self {
            symbol_index: HashMap::new(),
        }
    }

    pub fn build_index(&mut self, root_path: &Path) {
        println!("[RepoMap] Rozpoczynam indeksowanie projektu: {:?}", root_path);
        let mut count = 0;

        for entry in WalkDir::new(root_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if self.is_ignored(path) { continue; }

            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if let Some(language) = self.get_language_for_ext(ext) {
                    if let Ok(content) = fs::read_to_string(path) {
                        self.index_file(path, &content, language);
                        count += 1;
                    }
                }
            }
        }
        println!("[RepoMap] Zindeksowano {} plików. Unikalnych symboli: {}", count, self.symbol_index.len());
    }

    /// Generuje lekki szkielet projektu (Skeleton) dla Agenta.
    /// Zawiera strukturę katalogów i sygnatury funkcji, ale BEZ ciał funkcji.
    pub fn generate_skeleton(&self, root_path: &Path) -> String {
        let mut output = String::new();
        output.push_str(&format!("Project Skeleton for: {:?}\n\n", root_path));

        let mut walker = WalkDir::new(root_path).into_iter();
        
        // Sortowanie, aby wynik był deterministyczny
        let mut entries: Vec<_> = walker
            .filter_map(|e| e.ok())
            .filter(|e| !self.is_ignored(e.path()))
            .collect();
        
        entries.sort_by_key(|e| e.path().to_path_buf());

        for entry in entries {
            let path = entry.path();
            if !path.is_file() { continue; }
            
            // Relatywna ścieżka
            let relative = path.strip_prefix(root_path).unwrap_or(path).to_string_lossy();
            
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if let Some(language) = self.get_language_for_ext(ext) {
                    output.push_str(&format!("File: {}\n", relative));
                    
                    if let Ok(content) = fs::read_to_string(path) {
                        // Używamy extract_symbols z symbol_extractor, aby pobrać sygnatury
                        // Uwaga: To wymaga, żeby symbol_extractor był publiczny w module context
                        // Zoptymalizowana wersja mogłaby używać już zbudowanego indeksu self.symbol_index,
                        // ale tutaj chcemy pełną hierarchię dla pliku.
                        
                        // Parsujemy "na lekko" - tylko nagłówki
                        let names = self.parse_signatures(&content, language);
                        for sig in names {
                            output.push_str(&format!("  {}\n", sig));
                        }
                    }
                    output.push('\n');
                }
            }
        }
        
        output
    }

    /// Pobiera sygnatury (zamiast samych nazw) dla Skeleton View
    fn parse_signatures(&self, content: &str, language: Language) -> Vec<String> {
        let mut parser = Parser::new();
        if parser.set_language(&language).is_err() { return vec![]; }
        
        let tree = match parser.parse(content, None) {
            Some(t) => t,
            None => return vec![],
        };

        let mut signatures = Vec::new();
        let mut cursor = tree.walk();
        
        // Prosta nawigacja po top-level
        let mut visited_children = false;
        loop {
            if !visited_children {
                let node = cursor.node();
                let kind = node.kind();
                
                // Interesują nas definicje
                if kind.contains("function") || kind.contains("class") || kind.contains("impl") || kind.contains("method") {
                    // Pobieramy pierwszą linię jako sygnaturę
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let node_text = &content[start_byte..end_byte];
                    
                    if let Some(first_line) = node_text.lines().next() {
                        let clean_sig = first_line.trim_end_matches('{').trim();
                        signatures.push(clean_sig.to_string());
                    }
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
        signatures
    }

    /// Znajduje plik z bardzo agresywnym podejściem (Manual Scan)
    pub fn find_path_for_snippet(&self, snippet: &str, known_roots: &[String]) -> Option<PathBuf> {
        // DEBUG: Pokaż początek snippeta, żebyśmy widzieli co dostajemy
        let preview = snippet.chars().take(100).collect::<String>().replace('\n', "\\n");
        println!("[RepoMap] Analyzing snippet start: \"{}\"", preview);

        // 1. MANUAL SCAN (Zamiast Regexa)
        // Sprawdzamy pierwsze 5 linii tekstu w poszukiwaniu "Plik:" lub "File:"
        let mut explicit_path: Option<String> = None;

        for line in snippet.lines().take(5) {
            let clean = line.trim();
            // Szukamy wariantów: # Plik:, // Plik:, <!-- Plik:
            // Ignorujemy znaki komentarza, szukamy słowa kluczowego
            let lower = clean.to_lowercase();
            
            if let Some(idx) = lower.find("plik:") {
                // Pobierz wszystko po dwukropku
                let path_part = &clean[idx + 5..].trim();
                // Usuń ewentualne znaki kończące komentarz HTML/CSS (--> lub */)
                let clean_path = path_part.trim_end_matches("-->").trim_end_matches("*/").trim();
                explicit_path = Some(clean_path.to_string());
                break;
            }
            if let Some(idx) = lower.find("file:") {
                let path_part = &clean[idx + 5..].trim();
                let clean_path = path_part.trim_end_matches("-->").trim_end_matches("*/").trim();
                explicit_path = Some(clean_path.to_string());
                break;
            }
        }

        // Jeśli znaleźliśmy ścieżkę w komentarzu
        if let Some(rel_path) = explicit_path {
            // Normalizacja separatorów (na Windows \)
            let clean_rel_path = rel_path.replace('/', std::path::MAIN_SEPARATOR_STR).replace('\\', std::path::MAIN_SEPARATOR_STR);
            let unix_rel_path = rel_path.replace('\\', "/"); // Do szukania w indeksie

            println!("[RepoMap] 🎯 Found explicit file marker: '{}'", clean_rel_path);
            
            // A. DIRECT DISK CHECK (Sprawdzamy fizycznie na dysku)
            for root in known_roots {
                let root_path = Path::new(root);
                let candidate = root_path.join(&clean_rel_path);
                
                // Debug log dla każdej próby
                // println!("[RepoMap] Checking candidate: {:?}", candidate);

                if candidate.exists() && candidate.is_file() {
                    println!("[RepoMap] ✅ Direct hit! File found: {:?}", candidate);
                    return Some(candidate);
                }
            }
            
            println!("[RepoMap] ⚠️ File not found on disk in known roots. Trying index fallback...");

            // B. INDEX FALLBACK (Dla niepełnych ścieżek lub błędnych separatorów)
            for paths_list in self.symbol_index.values() {
                for indexed_path in paths_list {
                    let indexed_str = indexed_path.to_string_lossy().replace('\\', "/");
                    if indexed_str.ends_with(&unix_rel_path) {
                        println!("[RepoMap] ✅ Index match found: {:?}", indexed_path);
                        return Some(indexed_path.clone());
                    }
                }
            }

            // C. FILESYSTEM SUFFIX SCAN (Dla plików bez zaindeksowanych symboli)
            println!("[RepoMap] 💡 Index fallback failed. Trying filesystem scan by suffix...");
            if let Some(found_path) = self.find_by_path_suffix(&unix_rel_path, known_roots) {
                println!("[RepoMap] ✅ Filesystem suffix match found: {:?}", found_path);
                return Some(found_path);
            }
        } else {
            println!("[RepoMap] ℹ️ No explicit file marker found in first 5 lines.");
        }

        // 2. SYMBOL HEURISTIC (Ostatnia deska ratunku)
        let snippet_symbols = self.extract_top_level_names(snippet);
        for symbol in snippet_symbols {
            if let Some(paths) = self.symbol_index.get(&symbol) {
                println!("[RepoMap] Heuristic match: Symbol '{}' found in {:?}", symbol, paths[0]);
                return Some(paths[0].clone());
            }
        }
        
        None
    }

    /// Szuka pliku na podstawie końcówki ścieżki (suffix matching)
    /// 1. Sprawdza symbol_index (szybkie)
    /// 2. Jeśli nie znajdzie - skanuje known_roots (WalkDir)
    fn find_by_path_suffix(&self, suffix: &str, known_roots: &[String]) -> Option<PathBuf> {
        let mut candidates = Vec::new();
        let normalized_suffix = suffix.replace('/', std::path::MAIN_SEPARATOR_STR);

        // STEP 1: Szybkie sprawdzenie w symbol_index
        for paths_list in self.symbol_index.values() {
            for path in paths_list {
                let path_str = path.to_string_lossy();
                if path_str.ends_with(&normalized_suffix) {
                    candidates.push(path.clone());
                }
            }
        }

        // Jeśli znaleziono w symbol_index - zwróć wynik
        if !candidates.is_empty() {
            return Self::return_best_candidate(candidates, suffix);
        }

        // STEP 2: Full filesystem scan (dla plików bez symboli, np. .json, .txt)
        println!("[RepoMap] 🔎 Symbol index miss. Scanning known_roots for suffix...");

        for root in known_roots {
            let root_path = Path::new(root);

            // Optymalizacja: używamy WalkDir z max_depth dla szybkości
            for entry in WalkDir::new(root_path)
                .max_depth(10) // Limit głębokości dla wydajności
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let path = entry.path();
                if self.is_ignored(path) { continue; }

                let path_str = path.to_string_lossy();
                if path_str.ends_with(&normalized_suffix) {
                    candidates.push(path.to_path_buf());
                }
            }
        }

        Self::return_best_candidate(candidates, suffix)
    }

    /// Helper: zwraca najlepszy kandydat z listy
    fn return_best_candidate(candidates: Vec<PathBuf>, suffix: &str) -> Option<PathBuf> {
        match candidates.len() {
            0 => {
                println!("[RepoMap] ⚠️ No files found matching suffix: {}", suffix);
                None
            }
            1 => {
                println!("[RepoMap] ✨ Found unique file by suffix: {:?}", candidates[0]);
                Some(candidates[0].clone())
            }
            _ => {
                println!("[RepoMap] ⚠️ Multiple files match suffix '{}'. Using first one:", suffix);
                for (i, cand) in candidates.iter().enumerate() {
                    println!("[RepoMap]   {}. {:?}", i + 1, cand);
                }
                Some(candidates[0].clone())
            }
        }
    }

    // --- Helpers (Bez zmian) ---
    fn is_ignored(&self, path: &Path) -> bool {
        let p = path.to_string_lossy();
        p.contains("node_modules") || p.contains("target") || p.contains(".git") || p.contains("dist") || p.contains("__pycache__")
    }

    fn get_language_for_ext(&self, ext: &str) -> Option<Language> {
        match ext {
            "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
            "ts" | "tsx" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            "js" | "jsx" => Some(tree_sitter_javascript::LANGUAGE.into()),
            "py" => Some(tree_sitter_python::LANGUAGE.into()),
            _ => None,
        }
    }

    fn index_file(&mut self, path: &Path, content: &str, language: Language) {
        let names = self.parse_names(content, language);
        for name in names {
            self.symbol_index
                .entry(name)
                .or_insert_with(Vec::new)
                .push(path.to_path_buf());
        }
    }

    fn extract_top_level_names(&self, snippet: &str) -> Vec<String> {
        let langs = [
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            tree_sitter_python::LANGUAGE.into(),
        ];
        
        let mut found = Vec::new();
        for lang in langs {
            let names = self.parse_names(snippet, lang);
            found.extend(names);
        }
        found
    }

    fn parse_names(&self, content: &str, language: Language) -> Vec<String> {
        let mut parser = Parser::new();
        if parser.set_language(&language).is_err() { return vec![]; }
        
        let tree = match parser.parse(content, None) {
            Some(t) => t,
            None => return vec![],
        };

        let mut names = Vec::new();
        let mut cursor = tree.walk();
        
        let mut visited_children = false;
        loop {
            if !visited_children {
                let node = cursor.node();
                let kind = node.kind();
                if kind.contains("function") || kind.contains("class") || kind.contains("impl") || kind.contains("method") {
                    if let Some(name) = self.extract_name_from_node(node, content) {
                        names.push(name);
                    }
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
        names
    }

    fn extract_name_from_node(&self, node: Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if let Some(field) = cursor.field_name() {
                    if field == "name" {
                        return Some(source[child.byte_range()].to_string());
                    }
                }
                if child.kind() == "identifier" || child.kind() == "type_identifier" {
                     return Some(source[child.byte_range()].to_string());
                }
                if !cursor.goto_next_sibling() { break; }
            }
        }
        None
    }
}