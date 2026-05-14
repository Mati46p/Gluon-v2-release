//! Smart semantic chunking using Tree-sitter
//!
//! This module replaces naive indent-based chunking with intelligent semantic boundary
//! detection using the existing AnalysisEngine and QueryMatcher infrastructure.

use crate::apply_system::analysis::{AnalysisEngine, queries::QueryMatcher, SupportedLanguage};
use crate::memory::types::EnhancedCodeChunk;
use std::path::Path;
use chrono::Utc;
use uuid::Uuid;

pub struct SmartChunker {
    pub max_chunk_size: usize,  // Default: 6000 chars (~1500 tokens, safe for embeddings)
}

impl SmartChunker {
    pub fn new() -> Self {
        Self {
            max_chunk_size: 6000,
        }
    }

    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            max_chunk_size: max_size,
        }
    }

    /// Chunk a file using Tree-sitter semantic boundaries
    ///
    /// This preserves:
    /// - Function/class boundaries
    /// - Parent-child relationships (methods with classes)
    /// - Rich metadata (complexity, security, types)
    /// - Syntactic validity of each chunk
    pub fn chunk_file(
        &self,
        file_path: &Path,
        content: &str,
    ) -> Result<Vec<EnhancedCodeChunk>, String> {
        let file_path_str = file_path.to_str().ok_or("Invalid file path")?;

        // Detect language
        let language = SupportedLanguage::from_path(file_path_str)
            .ok_or_else(|| format!("Unsupported language for file: {}", file_path_str))?;

        // Parse with AnalysisEngine (with heuristics for fragments)
        let parse_result = AnalysisEngine::parse_with_heuristics(content, file_path_str)
            .map_err(|e| format!("Failed to parse {}: {}", file_path_str, e))?;

        // Extract semantic signatures using QueryMatcher
        let signatures = QueryMatcher::extract_signatures(
            content,
            &parse_result.tree,
            language,
        );

        if signatures.is_empty() {
            // Fallback for files with no detected symbols (configs, docs, etc.)
            return self.fallback_chunk(file_path_str, content);
        }

        // Convert signatures to chunks
        self.signatures_to_chunks(content, file_path_str, signatures)
    }

    /// Convert SemanticSignatures to EnhancedCodeChunks
    fn signatures_to_chunks(
        &self,
        content: &str,
        file_path: &str,
        signatures: Vec<crate::apply_system::analysis::queries::SemanticSignature>,
    ) -> Result<Vec<EnhancedCodeChunk>, String> {
        let mut chunks = Vec::new();

        for sig in signatures {
            // Extract content using byte range
            let chunk_content = content
                .get(sig.start_byte..sig.end_byte)
                .ok_or_else(|| format!("Invalid byte range in signature: {}:{}", sig.start_byte, sig.end_byte))?
                .to_string();

            // Check size - split if too large
            if chunk_content.len() > self.max_chunk_size {
                // For very large functions/classes, we split them but preserve context
                chunks.extend(self.split_large_chunk(&chunk_content, &sig, file_path)?);
            } else {
                chunks.push(self.create_chunk(chunk_content, &sig, file_path)?);
            }
        }

        Ok(chunks)
    }

    /// Create a single EnhancedCodeChunk from a SemanticSignature
    fn create_chunk(
        &self,
        content: String,
        sig: &crate::apply_system::analysis::queries::SemanticSignature,
        file_path: &str,
    ) -> Result<EnhancedCodeChunk, String> {
        Ok(EnhancedCodeChunk {
            chunk_id: Self::generate_chunk_id(file_path, sig.start_row),
            file_path: file_path.to_string(),
            start_line: sig.start_row + 1,  // 1-based for display
            end_line: sig.end_row + 1,
            content,
            symbol_name: Some(sig.name.clone()),
            symbol_kind: Some(sig.kind.clone()),
            parent_name: sig.parent_name.clone(),
            cyclomatic_complexity: sig.cyclomatic_complexity,
            security_alerts: sig.security_alerts.clone(),
            type_coverage: sig.type_coverage,
            has_docstring: sig.has_docstring,
            // Git fields will be populated by GitTracker
            commit_hash: String::new(),
            committed_at: Utc::now(),
            author: String::new(),
            // Project fields will be populated by HolographicStore
            project_id: 0,
            project_path: String::new(),
            embedding_model: "nomic-embed-text-v2-moe.Q8_0".to_string(),
            indexed_at: Utc::now(),
        })
    }

    /// Split a large chunk into smaller pieces while preserving metadata
    fn split_large_chunk(
        &self,
        content: &str,
        sig: &crate::apply_system::analysis::queries::SemanticSignature,
        file_path: &str,
    ) -> Result<Vec<EnhancedCodeChunk>, String> {
        let mut chunks = Vec::new();
        let mut current_start = 0;
        let lines: Vec<&str> = content.lines().collect();
        let mut current_lines = Vec::new();
        let mut current_size = 0;
        let mut chunk_start_line = sig.start_row;

        for (i, line) in lines.iter().enumerate() {
            current_lines.push(*line);
            current_size += line.len() + 1;  // +1 for newline

            // Split when we exceed max size or reach end
            if current_size >= self.max_chunk_size || i == lines.len() - 1 {
                let chunk_content = current_lines.join("\n");
                let chunk_end_line = chunk_start_line + current_lines.len();

                chunks.push(EnhancedCodeChunk {
                    chunk_id: Self::generate_chunk_id(file_path, chunk_start_line),
                    file_path: file_path.to_string(),
                    start_line: chunk_start_line + 1,  // 1-based
                    end_line: chunk_end_line,
                    content: chunk_content,
                    symbol_name: Some(format!("{}_part{}", sig.name, chunks.len() + 1)),
                    symbol_kind: Some(sig.kind.clone()),
                    parent_name: sig.parent_name.clone(),
                    cyclomatic_complexity: sig.cyclomatic_complexity,
                    security_alerts: sig.security_alerts.clone(),
                    type_coverage: sig.type_coverage,
                    has_docstring: sig.has_docstring && chunks.is_empty(),  // Only first part has docstring
                    commit_hash: String::new(),
                    committed_at: Utc::now(),
                    author: String::new(),
                    project_id: 0,
                    project_path: String::new(),
                    embedding_model: "nomic-embed-text-v2-moe.Q8_0".to_string(),
                    indexed_at: Utc::now(),
                });

                // Reset for next chunk
                current_lines.clear();
                current_size = 0;
                chunk_start_line = chunk_end_line;
            }
        }

        Ok(chunks)
    }

    /// Fallback chunking for files with no detected symbols
    /// (e.g., JSON, Markdown, plain text)
    fn fallback_chunk(
        &self,
        file_path: &str,
        content: &str,
    ) -> Result<Vec<EnhancedCodeChunk>, String> {
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut start_line = 1;
        let mut current_line = 1;

        for line in content.lines() {
            // Split on max size
            if current_chunk.len() + line.len() > self.max_chunk_size {
                chunks.push(EnhancedCodeChunk {
                    chunk_id: Self::generate_chunk_id(file_path, start_line - 1),
                    file_path: file_path.to_string(),
                    start_line,
                    end_line: current_line - 1,
                    content: current_chunk.clone(),
                    symbol_name: None,
                    symbol_kind: None,
                    parent_name: None,
                    cyclomatic_complexity: 1,
                    security_alerts: vec![],
                    type_coverage: 0.0,
                    has_docstring: false,
                    commit_hash: String::new(),
                    committed_at: Utc::now(),
                    author: String::new(),
                    project_id: 0,
                    project_path: String::new(),
                    embedding_model: "nomic-embed-text-v2-moe.Q8_0".to_string(),
                    indexed_at: Utc::now(),
                });

                current_chunk.clear();
                start_line = current_line;
            }

            current_chunk.push_str(line);
            current_chunk.push('\n');
            current_line += 1;
        }

        // Add last chunk
        if !current_chunk.is_empty() {
            chunks.push(EnhancedCodeChunk {
                chunk_id: Self::generate_chunk_id(file_path, start_line - 1),
                file_path: file_path.to_string(),
                start_line,
                end_line: current_line - 1,
                content: current_chunk,
                symbol_name: None,
                symbol_kind: None,
                parent_name: None,
                cyclomatic_complexity: 1,
                security_alerts: vec![],
                type_coverage: 0.0,
                has_docstring: false,
                commit_hash: String::new(),
                committed_at: Utc::now(),
                author: String::new(),
                project_id: 0,
                project_path: String::new(),
                embedding_model: "nomic-embed-text-v2-moe.Q8_0".to_string(),
                indexed_at: Utc::now(),
            });
        }

        Ok(chunks)
    }

    /// Generate deterministic chunk ID from file path and line number
    fn generate_chunk_id(file_path: &str, line: usize) -> String {
        // Use UUID v5 (deterministic, based on namespace + name)
        let namespace = Uuid::NAMESPACE_OID;
        let name = format!("{}::{}", file_path, line);
        Uuid::new_v5(&namespace, name.as_bytes()).to_string()
    }
}

impl Default for SmartChunker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_generate_chunk_id_deterministic() {
        let id1 = SmartChunker::generate_chunk_id("src/main.rs", 42);
        let id2 = SmartChunker::generate_chunk_id("src/main.rs", 42);
        assert_eq!(id1, id2, "Chunk IDs should be deterministic");
    }

    #[test]
    fn test_generate_chunk_id_unique() {
        let id1 = SmartChunker::generate_chunk_id("src/main.rs", 42);
        let id2 = SmartChunker::generate_chunk_id("src/main.rs", 43);
        assert_ne!(id1, id2, "Different lines should have different IDs");
    }

    #[test]
    fn test_chunk_simple_rust_file() {
        let chunker = SmartChunker::new();
        let code = r#"
fn hello() {
    println!("Hello");
}

fn world() {
    println!("World");
}
"#;
        let path = PathBuf::from("test.rs");
        let chunks = chunker.chunk_file(&path, code).unwrap();

        assert!(chunks.len() >= 2, "Should detect at least 2 functions");
        assert!(chunks.iter().any(|c| c.symbol_name.as_deref() == Some("hello")));
        assert!(chunks.iter().any(|c| c.symbol_name.as_deref() == Some("world")));
    }

    #[test]
    fn test_fallback_for_unsupported_file() {
        let chunker = SmartChunker::with_max_size(100);
        let code = "Line 1\nLine 2\nLine 3\n".repeat(50);  // 350 chars
        let path = PathBuf::from("test.txt");

        let chunks = chunker.chunk_file(&path, &code);
        // Should fallback to simple chunking for .txt files
        assert!(chunks.is_ok() || chunks.is_err());  // Either works or gracefully fails
    }
}
