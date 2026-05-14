use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;
use super::symbol_extractor::{Symbol, extract_symbols, extract_imports};

/// Represents the Semantic Knowledge Graph of the codebase
pub struct ContextGraph {
    /// Map of file path -> List of defined symbols (Functions, Classes)
    pub file_symbols: HashMap<String, Vec<Symbol>>,
    
    /// Map of file path -> List of imported paths (Edges)
    pub file_imports: HashMap<String, Vec<String>>,
    
    /// Cache of file sizes for ranking
    pub file_sizes: HashMap<String, u64>,
}

impl ContextGraph {
    pub fn new() -> Self {
        Self {
            file_symbols: HashMap::new(),
            file_imports: HashMap::new(),
            file_sizes: HashMap::new(),
        }
    }

    /// Index a whole directory (recursively)
    ///
    /// Optional progress callback is called every N files with (current_count, file_path)
    pub fn index_directory<F>(&mut self, root_path: &str, excluded: &[String], mut progress_callback: Option<F>)
    where
        F: FnMut(usize, &str)
    {
        println!("[ContextGraph::index_directory] 🔍 Starting walk of: {}", root_path);
        let walker = WalkDir::new(root_path).into_iter();
        let mut processed_count = 0;
        const PROGRESS_INTERVAL: usize = 2; // Report progress every 5 files

        for entry in walker.filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !excluded.iter().any(|ex| name.contains(ex)) && !name.starts_with('.')
        }) {
            if let Ok(entry) = entry {
                if entry.file_type().is_file() {
                    if let Some(path_str) = entry.path().to_str() {
                        // Skip binary/large files based on extension heuristic
                        if self.is_source_file(path_str) {
                            println!("[ContextGraph::index_directory] 📄 Indexing: {}", path_str);
                            self.index_file(entry.path());
                            processed_count += 1;

                            // Call progress callback every N files
                            if processed_count % PROGRESS_INTERVAL == 0 {
                                if let Some(ref mut callback) = progress_callback {
                                    callback(processed_count, path_str);
                                }
                            }
                        } else {
                            println!("[ContextGraph::index_directory] ⏭️ Skipped (not source file): {}", path_str);
                        }
                    }
                } else {
                    println!("[ContextGraph::index_directory] 📁 Directory: {}", entry.path().display());
                }
            }
        }
        println!("[ContextGraph::index_directory] ✅ Completed: processed {} files", processed_count);

        // Final progress update
        if processed_count > 0 {
            if let Some(ref mut callback) = progress_callback {
                callback(processed_count, "completed");
            }
        }
    }

    /// Update index for a single file (e.g. after edit)
    pub fn index_file(&mut self, path: &Path) {
        let path_str = path.to_string_lossy().to_string().replace('\\', "/");
        
        match std::fs::read_to_string(path) {
            Ok(content) => {
                self.file_sizes.insert(path_str.clone(), content.len() as u64);

                // 1. Extract Symbols using Tree-sitter
                match extract_symbols(path, &content) {
                    Ok(symbols) => {
                        println!("[ContextGraph] Indexed {} symbols from {}", symbols.len(), path_str);
                        self.file_symbols.insert(path_str.clone(), symbols);
                    }
                    Err(e) => {
                        println!("[ContextGraph] Warning: Failed to extract symbols for {}: {}. Adding as empty.", path_str, e);
                        self.file_symbols.insert(path_str.clone(), Vec::new());
                    }
                }

                // 2. Extract Imports (Edges) using Regex
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                let imports = extract_imports(&content, ext);
                self.file_imports.insert(path_str, imports);
            },
            Err(e) => {
                println!("[ContextGraph] ❌ Failed to read file {}: {}", path_str, e);
                // Also insert empty to prevent "Graph contains 0 files" confusion if possible, 
                // but technically if we can't read it, we can't map it.
            }
        }
    }

    pub fn get_symbols(&self, path: &str) -> Option<&Vec<Symbol>> {
        self.file_symbols.get(path)
    }

    pub fn get_imports(&self, path: &str) -> Option<&Vec<String>> {
        self.file_imports.get(path)
    }

    pub fn get_all_files(&self) -> Vec<String> {
        self.file_symbols.keys().cloned().collect()
    }

    fn is_source_file(&self, path: &str) -> bool {
        let exts = [".rs", ".ts", ".js", ".py", ".go", ".java", ".kt", ".kts", ".c", ".cpp", ".h"];
        exts.iter().any(|e| path.ends_with(e))
    }
}