//! SearchCodeTool - Search codebase for patterns
//!
//! This tool allows LLMs to search through code files to find specific patterns,
//! functions, classes, or text matches.
//!
//! ## Search Types
//!
//! 1. **Text search**: Simple string matching
//! 2. **Regex search**: Pattern-based matching
//! 3. **File name search**: Find files by name pattern
//!
//! ## Example Usage
//!
//! ```json
//! {
//!   "query": "function.*calculate",
//!   "path": "src/",
//!   "regex": true,
//!   "max_results": 50
//! }
//! ```

use crate::interface::types::{
    GTool, ToolCategory, ToolContext, ToolError, ToolOutput, ToolResult,
};
use async_trait::async_trait;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs;
use walkdir::WalkDir;

/// Maximum number of results to return
const MAX_RESULTS_LIMIT: usize = 200;

/// Default max results
const DEFAULT_MAX_RESULTS: usize = 50;

/// SearchCode tool implementation
///
/// Searches through code files for patterns.
pub struct SearchCodeTool;

impl SearchCodeTool {
    pub fn new() -> Self {
        Self
    }
}

/// Parameters for SearchCodeTool
#[derive(Debug, Deserialize, JsonSchema)]
struct SearchCodeParams {
    /// Search query (text or regex pattern)
    query: String,

    /// Optional path to search in (relative to working_dir)
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,

    /// Whether query is a regex pattern (default: false)
    #[serde(default)]
    regex: bool,

    /// Case-sensitive search (default: false)
    #[serde(default)]
    case_sensitive: bool,

    /// File extension filter (e.g., "rs", "js")
    #[serde(skip_serializing_if = "Option::is_none")]
    file_extension: Option<String>,

    /// Maximum number of results (default: 50, max: 200)
    #[serde(default = "default_max_results")]
    max_results: usize,

    /// Include line numbers in results (default: true)
    #[serde(default = "default_true")]
    include_line_numbers: bool,

    /// Number of context lines before/after match (default: 2)
    #[serde(default = "default_context_lines")]
    context_lines: usize,
}

fn default_max_results() -> usize {
    DEFAULT_MAX_RESULTS
}

fn default_true() -> bool {
    true
}

fn default_context_lines() -> usize {
    2
}

/// A single search result match
#[derive(Debug, Serialize)]
struct SearchMatch {
    /// File path (relative to search root)
    file_path: String,

    /// Line number where match was found
    line_number: usize,

    /// The matching line content
    line_content: String,

    /// Context lines before the match
    #[serde(skip_serializing_if = "Vec::is_empty")]
    context_before: Vec<String>,

    /// Context lines after the match
    #[serde(skip_serializing_if = "Vec::is_empty")]
    context_after: Vec<String>,
}

/// Result of a code search
#[derive(Debug, Serialize)]
struct SearchCodeResult {
    /// Number of matches found
    match_count: usize,

    /// Number of files searched
    files_searched: usize,

    /// Whether results were truncated
    truncated: bool,

    /// Search matches
    matches: Vec<SearchMatch>,
}

#[async_trait]
impl GTool for SearchCodeTool {
    fn name(&self) -> &str {
        "gluon.search_code"
    }

    fn description(&self) -> &str {
        "Search through code files for text patterns or regex matches. \
         Use this to find function definitions, variable usage, or any code pattern. \
         Supports filtering by file extension and returns matches with context. \
         Examples: search for 'fn main', 'class.*Component', or specific error messages."
    }

    fn parameters_schema(&self) -> Value {
        let schema = schemars::schema_for!(SearchCodeParams);
        serde_json::to_value(schema).unwrap_or(json!({}))
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        eprintln!("[SearchCodeTool] Starting code search");

        // 1. Parse parameters
        let mut params: SearchCodeParams = serde_json::from_value(params)
            .map_err(|e| ToolError::invalid_params(&format!("Invalid parameters: {}", e)))?;

        // Limit max_results
        if params.max_results > MAX_RESULTS_LIMIT {
            params.max_results = MAX_RESULTS_LIMIT;
        }

        // 2. Determine search path
        let search_root = if let Some(ref path) = params.path {
            context.working_dir.join(path)
        } else {
            context.working_dir.clone()
        };

        if !search_root.exists() {
            return Err(ToolError::execution_failed(&format!(
                "Search path does not exist: {}",
                search_root.display()
            )));
        }

        eprintln!("[SearchCodeTool] Query: {}", params.query);
        eprintln!("[SearchCodeTool] Search root: {}", search_root.display());
        eprintln!("[SearchCodeTool] Regex: {}", params.regex);

        // 3. Compile regex if needed
        let regex = if params.regex {
            let pattern = if params.case_sensitive {
                params.query.clone()
            } else {
                format!("(?i){}", params.query)
            };

            Some(Regex::new(&pattern).map_err(|e| {
                ToolError::invalid_params(&format!("Invalid regex pattern: {}", e))
            })?)
        } else {
            None
        };

        // 4. Search files
        let (matches, files_searched) =
            Self::search_files(&search_root, &params, regex.as_ref()).await?;

        let match_count = matches.len();
        let truncated = match_count >= params.max_results;

        let result = SearchCodeResult {
            match_count,
            files_searched,
            truncated,
            matches,
        };

        let summary = if truncated {
            format!(
                "Found {} matches in {} files (truncated to {} results)",
                match_count, files_searched, params.max_results
            )
        } else {
            format!(
                "Found {} matches in {} files",
                match_count, files_searched
            )
        };

        eprintln!("[SearchCodeTool] {}", summary);

        Ok(ToolOutput {
            result: serde_json::to_value(result)?,
            summary,
            artifacts: vec![],
        })
    }

    fn requires_confirmation(&self) -> bool {
        false // Reading files is safe
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }
}

impl SearchCodeTool {
    /// Search through files for matches
    async fn search_files(
        root: &Path,
        params: &SearchCodeParams,
        regex: Option<&Regex>,
    ) -> Result<(Vec<SearchMatch>, usize), ToolError> {
        let mut matches = Vec::new();
        let mut files_searched = 0;

        // Collect files to search
        let mut files_to_search = Vec::new();

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !Self::is_hidden(e) && !Self::is_ignored_dir(e))
        {
            let entry = entry.map_err(|e| {
                ToolError::execution_failed(&format!("Failed to walk directory: {}", e))
            })?;

            if entry.file_type().is_file() {
                // Check extension filter
                if let Some(ref ext_filter) = params.file_extension {
                    if let Some(ext) = entry.path().extension() {
                        if ext.to_string_lossy() != ext_filter.as_str() {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }

                // Only search text files
                if Self::is_text_file(entry.path()) {
                    files_to_search.push(entry.path().to_path_buf());
                }
            }
        }

        // Search each file
        for file_path in files_to_search {
            files_searched += 1;

            if let Ok(file_matches) = Self::search_file(&file_path, root, params, regex).await {
                for m in file_matches {
                    matches.push(m);

                    // Stop if we've hit the limit
                    if matches.len() >= params.max_results {
                        return Ok((matches, files_searched));
                    }
                }
            }
        }

        Ok((matches, files_searched))
    }

    /// Search a single file for matches
    async fn search_file(
        file_path: &Path,
        root: &Path,
        params: &SearchCodeParams,
        regex: Option<&Regex>,
    ) -> Result<Vec<SearchMatch>, ToolError> {
        let content = fs::read_to_string(file_path).await.map_err(|e| {
            ToolError::execution_failed(&format!("Failed to read file: {}", e))
        })?;

        let lines: Vec<&str> = content.lines().collect();
        let mut matches = Vec::new();

        for (line_idx, line) in lines.iter().enumerate() {
            let is_match = if let Some(regex) = regex {
                regex.is_match(line)
            } else if params.case_sensitive {
                line.contains(&params.query)
            } else {
                line.to_lowercase()
                    .contains(&params.query.to_lowercase())
            };

            if is_match {
                let relative_path = file_path
                    .strip_prefix(root)
                    .unwrap_or(file_path)
                    .to_string_lossy()
                    .to_string();

                let context_before = if params.context_lines > 0 {
                    let start = line_idx.saturating_sub(params.context_lines);
                    lines[start..line_idx]
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    vec![]
                };

                let context_after = if params.context_lines > 0 {
                    let end = std::cmp::min(line_idx + params.context_lines + 1, lines.len());
                    lines[line_idx + 1..end]
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    vec![]
                };

                matches.push(SearchMatch {
                    file_path: relative_path,
                    line_number: line_idx + 1,
                    line_content: line.to_string(),
                    context_before,
                    context_after,
                });
            }
        }

        Ok(matches)
    }

    /// Check if entry is hidden
    fn is_hidden(entry: &walkdir::DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    }

    /// Check if directory should be ignored
    fn is_ignored_dir(entry: &walkdir::DirEntry) -> bool {
        const IGNORED_DIRS: &[&str] = &[
            "node_modules",
            "target",
            "dist",
            "build",
            ".git",
            ".svn",
            "__pycache__",
            "venv",
        ];

        if entry.file_type().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                return IGNORED_DIRS.contains(&name);
            }
        }
        false
    }

    /// Check if file is likely a text file
    fn is_text_file(path: &Path) -> bool {
        const TEXT_EXTENSIONS: &[&str] = &[
            "rs", "js", "ts", "jsx", "tsx", "py", "java", "c", "cpp", "h", "hpp", "cs", "go",
            "rb", "php", "swift", "kt", "scala", "r", "m", "mm", "vue", "svelte", "html",
            "css", "scss", "sass", "less", "json", "xml", "yaml", "yml", "toml", "md", "txt",
            "sh", "bash", "zsh", "fish", "sql", "graphql", "proto", "gradle", "cmake",
        ];

        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                return TEXT_EXTENSIONS.contains(&ext_str);
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_metadata() {
        let tool = SearchCodeTool::new();

        assert_eq!(tool.name(), "gluon.search_code");
        assert!(!tool.requires_confirmation());
        assert!(matches!(tool.category(), ToolCategory::Analysis));
    }

    #[test]
    fn test_parameters_schema() {
        let tool = SearchCodeTool::new();
        let schema = tool.parameters_schema();

        // Should have type = object
        assert_eq!(schema["type"], "object");

        // Should have 'query' property
        let properties = &schema["properties"];
        assert!(properties.is_object());
        assert!(properties["query"].is_object());
    }

    #[test]
    fn test_is_text_file() {
        assert!(SearchCodeTool::is_text_file(Path::new("test.rs")));
        assert!(SearchCodeTool::is_text_file(Path::new("test.js")));
        assert!(SearchCodeTool::is_text_file(Path::new("test.py")));
        assert!(!SearchCodeTool::is_text_file(Path::new("test.exe")));
        assert!(!SearchCodeTool::is_text_file(Path::new("test.bin")));
    }
}
