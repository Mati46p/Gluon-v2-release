//! KROK 9: Parser #3 - Aggressive Pattern Matching
//!
//! Last resort parser that uses heuristics and pattern matching
//! when structured formats (unified diff, markdown) fail.
//!
//! This parser:
//! - Looks for file paths anywhere in the text
//! - Finds code blocks (even without proper markdown)
//! - Tries to deduce what is "before" and "after"
//! - Returns low confidence results

use crate::apply_system::parsers::Parser;
use crate::apply_system::shared::types::ChangeQueueItem;
use regex::Regex;

pub struct PatternMatchingParser;

impl Parser for PatternMatchingParser {
    fn name(&self) -> &'static str {
        "PatternMatching"
    }

    fn can_handle(&self, raw_response: &str) -> bool {
        // This parser is last resort - it can "handle" anything
        // But we check for at least some code-like content
        self.has_file_extension(raw_response) || self.has_code_blocks(raw_response)
    }

    fn parse(&self, raw_response: &str) -> Result<Vec<ChangeQueueItem>, String> {
        // Try to find file paths
        let file_paths = self.extract_file_paths(raw_response);

        if file_paths.is_empty() {
            return Err("No file paths found in response".to_string());
        }

        // Try to extract code blocks
        let code_blocks = self.extract_code_blocks(raw_response);

        if code_blocks.len() < 2 {
            return Err("Need at least 2 code blocks to infer before/after".to_string());
        }

        // Try to pair file paths with code blocks
        let changes = self.pair_files_with_code(&file_paths, &code_blocks)?;

        if changes.is_empty() {
            return Err("Could not pair file paths with code blocks".to_string());
        }

        Ok(changes)
    }
}

impl PatternMatchingParser {
    /// Check if text contains file extensions
    fn has_file_extension(&self, text: &str) -> bool {
        let ext_patterns = vec![r"\.(ts|tsx|js|jsx|py|rs|go|java|cpp|c|h|cs|rb|php)"];

        ext_patterns.iter().any(|pattern| {
            Regex::new(pattern)
                .map(|re| re.is_match(text))
                .unwrap_or(false)
        })
    }

    /// Check if text has code-like blocks
    fn has_code_blocks(&self, text: &str) -> bool {
        // Look for indented blocks or code-like patterns
        text.contains("```")
            || text.contains("function ")
            || text.contains("class ")
            || text.contains("const ")
            || text.contains("def ")
            || text.contains("import ")
    }

    /// Extract all potential file paths from text
    ///
    /// Looks for patterns like:
    /// - src/auth.ts
    /// - path/to/file.py
    /// - Any text containing .ts, .js, etc.
    fn extract_file_paths(&self, text: &str) -> Vec<String> {
        let mut paths = Vec::new();

        // Pattern: path/to/file.ext
        let path_pattern = r"([a-zA-Z0-9_\-./]+\.(ts|tsx|js|jsx|py|rs|go|java|cpp|c|h|cs|rb|php))";

        if let Ok(re) = Regex::new(path_pattern) {
            for capture in re.captures_iter(text) {
                if let Some(path_match) = capture.get(1) {
                    paths.push(path_match.as_str().to_string());
                }
            }
        }

        paths
    }

    /// Extract all code-like blocks from text
    ///
    /// Finds:
    /// - Markdown code blocks (```)
    /// - Indented blocks (4+ spaces)
    /// - Blocks with function/class keywords
    fn extract_code_blocks(&self, text: &str) -> Vec<String> {
        let mut blocks = Vec::new();

        // Method 1: Markdown code blocks
        blocks.extend(self.extract_markdown_blocks(text));

        // Method 2: Indented blocks
        if blocks.len() < 2 {
            blocks.extend(self.extract_indented_blocks(text));
        }

        // Method 3: Keyword-based blocks
        if blocks.len() < 2 {
            blocks.extend(self.extract_keyword_blocks(text));
        }

        blocks
    }

    /// Extract markdown-style code blocks
    fn extract_markdown_blocks(&self, text: &str) -> Vec<String> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            if lines[i].trim().starts_with("```") {
                let mut code = Vec::new();
                i += 1;

                while i < lines.len() && !lines[i].trim().starts_with("```") {
                    code.push(lines[i]);
                    i += 1;
                }

                if !code.is_empty() {
                    blocks.push(code.join("\n"));
                }
            }
            i += 1;
        }

        blocks
    }

    /// Extract indented blocks (4+ spaces or tab)
    fn extract_indented_blocks(&self, text: &str) -> Vec<String> {
        let mut blocks = Vec::new();
        let mut current_block = Vec::new();

        for line in text.lines() {
            let is_indented = line.starts_with("    ") || line.starts_with("\t");

            if is_indented {
                current_block.push(line.trim_start());
            } else if !current_block.is_empty() {
                blocks.push(current_block.join("\n"));
                current_block = Vec::new();
            }
        }

        // Don't forget last block
        if !current_block.is_empty() {
            blocks.push(current_block.join("\n"));
        }

        blocks
    }

    /// Extract blocks based on keywords (function, class, etc.)
    fn extract_keyword_blocks(&self, text: &str) -> Vec<String> {
        let keywords = vec![
            "function ",
            "class ",
            "const ",
            "let ",
            "var ",
            "def ",
            "impl ",
            "struct ",
            "enum ",
        ];

        let mut blocks = Vec::new();

        for keyword in keywords {
            if let Some(block) = self.extract_block_starting_with(text, keyword) {
                blocks.push(block);
                if blocks.len() >= 2 {
                    break;
                }
            }
        }

        blocks
    }

    /// Extract a block of code starting with a keyword
    fn extract_block_starting_with(&self, text: &str, keyword: &str) -> Option<String> {
        let lines: Vec<&str> = text.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if line.contains(keyword) {
                // Found keyword - collect until empty line or end
                let mut block = Vec::new();
                for j in i..lines.len().min(i + 20) {
                    // Collect up to 20 lines
                    if lines[j].trim().is_empty() && !block.is_empty() {
                        break;
                    }
                    block.push(lines[j]);
                }

                if !block.is_empty() {
                    return Some(block.join("\n"));
                }
            }
        }

        None
    }

    /// Try to pair file paths with code blocks
    ///
    /// Heuristics:
    /// - If 1 file path and 2 code blocks: first is before, second is after
    /// - If multiple files and multiple blocks: try to match by proximity
    fn pair_files_with_code(
        &self,
        file_paths: &[String],
        code_blocks: &[String],
    ) -> Result<Vec<ChangeQueueItem>, String> {
        if code_blocks.len() < 2 {
            return Err("Need at least 2 code blocks".to_string());
        }

        let mut changes = Vec::new();

        // Simple case: 1 file, multiple blocks
        if file_paths.len() == 1 {
            let file_path = &file_paths[0];

            // Pair consecutive blocks as before/after
            for i in (0..code_blocks.len() - 1).step_by(2) {
                let old_code = code_blocks[i].clone();
                let new_code = code_blocks[i + 1].clone();

                let change = ChangeQueueItem::new(
                    file_path.clone(),
                    0, // No line numbers - matching will find them
                    0,
                    old_code,
                    new_code,
                );

                changes.push(change);
            }
        }
        // Multiple files: try to match 1:1
        else if file_paths.len() * 2 <= code_blocks.len() {
            for (i, file_path) in file_paths.iter().enumerate() {
                let block_idx = i * 2;
                if block_idx + 1 < code_blocks.len() {
                    let old_code = code_blocks[block_idx].clone();
                    let new_code = code_blocks[block_idx + 1].clone();

                    let change = ChangeQueueItem::new(file_path.clone(), 0, 0, old_code, new_code);

                    changes.push(change);
                }
            }
        }
        // Fallback: use first file for all block pairs
        else {
            let file_path = &file_paths[0];

            for i in (0..code_blocks.len() - 1).step_by(2) {
                let old_code = code_blocks[i].clone();
                let new_code = code_blocks[i + 1].clone();

                let change = ChangeQueueItem::new(file_path.clone(), 0, 0, old_code, new_code);

                changes.push(change);
            }
        }

        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_file_paths() {
        let parser = PatternMatchingParser;

        let text = "I changed src/auth.ts and also updated utils/helper.js";
        let paths = parser.extract_file_paths(text);

        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"src/auth.ts".to_string()));
        assert!(paths.contains(&"utils/helper.js".to_string()));
    }

    #[test]
    fn test_extract_markdown_blocks() {
        let parser = PatternMatchingParser;

        let text = r#"
Some text

```
code block 1
```

More text

```
code block 2
```
"#;

        let blocks = parser.extract_markdown_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("code block 1"));
        assert!(blocks[1].contains("code block 2"));
    }

    #[test]
    fn test_parse_simple_case() {
        let parser = PatternMatchingParser;

        let text = r#"
Change in src/test.ts:

```
function old() {
  return "old";
}
```

New version:

```
function new() {
  return "new";
}
```
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/test.ts");
        assert!(changes[0].old_code.contains("old"));
        assert!(changes[0].new_code.contains("new"));
    }
}
