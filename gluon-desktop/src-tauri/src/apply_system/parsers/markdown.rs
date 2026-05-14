//! KROK 8: Parser #2 - Structured Markdown Format
//!
//! Parses markdown format like:
//! ```
//! File: `src/auth.ts`
//! Lines: 45-52
//!
//! Before:
//! ```typescript
//! old code
//! ```
//!
//! After:
//! ```typescript
//! new code
//! ```
//! ```
//!
//! Also handles variations:
//! - "Old:" / "New:" instead of "Before:" / "After:"
//! - "Current:" / "Proposed:"
//! - Line numbers in comments

use crate::apply_system::parsers::Parser;
use crate::apply_system::shared::types::ChangeQueueItem;
use regex::Regex;

pub struct MarkdownParser;

impl Parser for MarkdownParser {
    fn name(&self) -> &'static str {
        "Markdown"
    }

    fn can_handle(&self, raw_response: &str) -> bool {
        // Quick check for markdown code blocks
        raw_response.contains("```")
            && (raw_response.to_lowercase().contains("before")
                || raw_response.to_lowercase().contains("after")
                || raw_response.to_lowercase().contains("old")
                || raw_response.to_lowercase().contains("new"))
    }

    fn parse(&self, raw_response: &str) -> Result<Vec<ChangeQueueItem>, String> {
        let sections = self.split_into_sections(raw_response)?;

        let mut changes = Vec::new();

        for section in sections {
            match self.parse_section(&section) {
                Ok(change) => changes.push(change),
                Err(e) => {
                    crate::gluon_warn!("Markdown", "Failed to parse markdown section: {}", e);
                }
            }
        }

        if changes.is_empty() {
            return Err("No valid markdown sections found".to_string());
        }

        Ok(changes)
    }
}

impl MarkdownParser {
    /// Split response into sections (one per file change)
    ///
    /// A section typically contains:
    /// - File path
    /// - Before/After code blocks
    /// - Optionally line numbers
    fn split_into_sections(&self, text: &str) -> Result<Vec<String>, String> {
        // Look for file path indicators as section boundaries
        let file_indicators = vec![
            r"File:",
            r"file:",
            r"Path:",
            r"path:",
            r"`[^`]+\.(ts|js|tsx|jsx|py|rs|go|java|cpp|c|h)`",
        ];

        let mut sections = Vec::new();
        let mut current_section = String::new();

        for line in text.lines() {
            let is_new_section = file_indicators.iter().any(|pattern| {
                Regex::new(pattern)
                    .map(|re| re.is_match(line))
                    .unwrap_or(false)
            });

            if is_new_section && !current_section.is_empty() {
                sections.push(current_section.clone());
                current_section = String::new();
            }

            current_section.push_str(line);
            current_section.push('\n');
        }

        // Don't forget last section
        if !current_section.is_empty() {
            sections.push(current_section);
        }

        if sections.is_empty() {
            // No clear sections - treat whole text as one section
            return Ok(vec![text.to_string()]);
        }

        Ok(sections)
    }

    /// Parse a single markdown section into a ChangeQueueItem
    fn parse_section(&self, section: &str) -> Result<ChangeQueueItem, String> {
        // Extract file path
        let file_path = self.extract_file_path(section)?;

        // Extract line numbers (if present)
        let (line_start, line_end) = self.extract_line_numbers(section).unwrap_or((0, 0)); // Default to 0,0 if not found - matching will handle it

        // Extract before/after code blocks
        let (old_code, new_code) = self.extract_code_blocks(section)?;

        let change = ChangeQueueItem::new(file_path, line_start, line_end, old_code, new_code);

        Ok(change)
    }

    /// Extract file path from section
    ///
    /// Looks for patterns like:
    /// - File: `src/auth.ts`
    /// - file: src/auth.ts
    /// - Path: src/auth.ts
    /// - In `src/auth.ts`...
    fn extract_file_path(&self, section: &str) -> Result<String, String> {
        // Pattern 1: File: `path` or File: path
        let patterns = vec![
            r"[Ff]ile:\s*`?([^`\n]+?\.(?:ts|js|tsx|jsx|py|rs|go|java|cpp|c|h))`?",
            r"[Pp]ath:\s*`?([^`\n]+?\.(?:ts|js|tsx|jsx|py|rs|go|java|cpp|c|h))`?",
            r"`([^`]+?\.(?:ts|js|tsx|jsx|py|rs|go|java|cpp|c|h))`",
        ];

        for pattern in patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(captures) = re.captures(section) {
                    if let Some(path) = captures.get(1) {
                        return Ok(path.as_str().trim().to_string());
                    }
                }
            }
        }

        Err("Could not find file path in markdown section".to_string())
    }

    /// Extract line numbers from section
    ///
    /// Looks for patterns like:
    /// - Lines: 45-52
    /// - Line 45
    /// - (lines 45-52)
    fn extract_line_numbers(&self, section: &str) -> Option<(usize, usize)> {
        // Pattern: Lines: 45-52
        let range_re = Regex::new(r"[Ll]ines?:\s*(\d+)-(\d+)").ok()?;
        if let Some(captures) = range_re.captures(section) {
            let start = captures.get(1)?.as_str().parse().ok()?;
            let end = captures.get(2)?.as_str().parse().ok()?;
            return Some((start, end));
        }

        // Pattern: Line 45 (single line)
        let single_re = Regex::new(r"[Ll]ine:\s*(\d+)").ok()?;
        if let Some(captures) = single_re.captures(section) {
            let line = captures.get(1)?.as_str().parse().ok()?;
            return Some((line, line));
        }

        None
    }

    /// Extract before/after code blocks from section
    ///
    /// Looks for patterns like:
    /// Before:
    /// ```typescript
    /// code
    /// ```
    ///
    /// After:
    /// ```typescript
    /// code
    /// ```
    fn extract_code_blocks(&self, section: &str) -> Result<(String, String), String> {
        // Keywords that indicate "before" section
        let before_keywords = vec!["before", "old", "current", "original"];

        // Keywords that indicate "after" section
        let after_keywords = vec!["after", "new", "proposed", "updated"];

        let mut before_code = None;
        let mut after_code = None;

        // Find all code blocks
        let code_blocks = self.find_all_code_blocks(section);

        if code_blocks.len() < 2 {
            return Err("Need at least 2 code blocks (before and after)".to_string());
        }

        // Match code blocks to before/after based on surrounding text
        for (_i, (code, context_before)) in code_blocks.iter().enumerate() {
            let context_lower = context_before.to_lowercase();

            // Check if this is a "before" block
            if before_keywords.iter().any(|kw| context_lower.contains(kw)) {
                before_code = Some(code.clone());
            }

            // Check if this is an "after" block
            if after_keywords.iter().any(|kw| context_lower.contains(kw)) {
                after_code = Some(code.clone());
            }
        }

        // If keywords didn't work, assume first block is before, second is after
        if before_code.is_none() || after_code.is_none() {
            if code_blocks.len() >= 2 {
                before_code = Some(code_blocks[0].0.clone());
                after_code = Some(code_blocks[1].0.clone());
            }
        }

        match (before_code, after_code) {
            (Some(before), Some(after)) => Ok((before, after)),
            _ => Err("Could not identify before and after code blocks".to_string()),
        }
    }

    /// Find all code blocks in text
    ///
    /// Returns: Vec<(code, context_before)>
    /// - code: the content inside ```
    /// - context_before: 2 lines before the code block (for keyword matching)
    fn find_all_code_blocks(&self, text: &str) -> Vec<(String, String)> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // Check if this is start of code block
            if line.trim().starts_with("```") {
                // Collect context (2 lines before)
                let context_start = i.saturating_sub(2);
                let context = lines[context_start..i].join("\n");

                // Collect code inside block
                let mut code_lines = Vec::new();
                i += 1;

                while i < lines.len() {
                    let code_line = lines[i];
                    if code_line.trim().starts_with("```") {
                        // End of code block
                        break;
                    }
                    code_lines.push(code_line);
                    i += 1;
                }

                // Sanitize code to remove AI hallucinations
                let code_refs: Vec<&str> = code_lines.iter().map(|s| s.as_ref()).collect();
                let code = crate::apply_system::parsers::sanitize_code_block(code_refs);
                blocks.push((code, context));
            }

            i += 1;
        }

        blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_markdown() {
        let parser = MarkdownParser;

        let valid = r#"
File: `src/test.ts`

Before:
```typescript
old code
```

After:
```typescript
new code
```
"#;

        assert!(parser.can_handle(valid));

        let invalid = "just some text";
        assert!(!parser.can_handle(invalid));
    }

    #[test]
    fn test_extract_file_path() {
        let parser = MarkdownParser;

        let section1 = "File: `src/auth.ts`";
        assert_eq!(parser.extract_file_path(section1).unwrap(), "src/auth.ts");

        let section2 = "file: src/test.py";
        assert_eq!(parser.extract_file_path(section2).unwrap(), "src/test.py");
    }

    #[test]
    fn test_extract_line_numbers() {
        let parser = MarkdownParser;

        let section1 = "Lines: 45-52";
        assert_eq!(parser.extract_line_numbers(section1), Some((45, 52)));

        let section2 = "Line: 100";
        assert_eq!(parser.extract_line_numbers(section2), Some((100, 100)));
    }

    #[test]
    fn test_parse_markdown_section() {
        let parser = MarkdownParser;

        let markdown = r#"
File: `src/test.ts`
Lines: 10-15

Before:
```typescript
function old() {
  return "old";
}
```

After:
```typescript
function new() {
  return "new";
}
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/test.ts");
        assert_eq!(changes[0].line_start, 10);
        assert!(changes[0].old_code.contains("old"));
        assert!(changes[0].new_code.contains("new"));
    }

    #[test]
    fn test_parse_old_new_keywords() {
        let parser = MarkdownParser;

        let markdown = r#"
File: src/config.ts

Old:
```typescript
const PORT = 3000;
```

New:
```typescript
const PORT = 8080;
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("3000"));
        assert!(changes[0].new_code.contains("8080"));
    }

    #[test]
    fn test_parse_current_proposed_keywords() {
        let parser = MarkdownParser;

        let markdown = r#"
File: `src/api.ts`

Current:
```typescript
export const API_URL = "http://localhost";
```

Proposed:
```typescript
export const API_URL = "https://api.example.com";
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("localhost"));
        assert!(changes[0].new_code.contains("example.com"));
    }

    #[test]
    fn test_parse_multiple_files() {
        let parser = MarkdownParser;

        let markdown = r#"
File: `src/file1.ts`

Before:
```typescript
const a = 1;
```

After:
```typescript
const a = 2;
```

File: `src/file2.ts`

Before:
```typescript
const b = 3;
```

After:
```typescript
const b = 4;
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "src/file1.ts");
        assert_eq!(changes[1].file_path, "src/file2.ts");
    }

    #[test]
    fn test_parse_without_line_numbers() {
        let parser = MarkdownParser;

        let markdown = r#"
File: src/utils.ts

Before:
```typescript
export function helper() {}
```

After:
```typescript
export function helper(param: string) {}
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].line_start, 0);
        assert_eq!(changes[0].line_end, 0);
    }

    #[test]
    fn test_parse_single_line_number() {
        let parser = MarkdownParser;

        let markdown = r#"
File: src/const.ts
Line: 42

Before:
```typescript
const VALUE = 100;
```

After:
```typescript
const VALUE = 200;
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].line_start, 42);
        assert_eq!(changes[0].line_end, 42);
    }

    #[test]
    fn test_extract_file_path_backticks() {
        let parser = MarkdownParser;

        let section = "File: `src/deep/path/file.ts`";
        assert_eq!(parser.extract_file_path(section).unwrap(), "src/deep/path/file.ts");
    }

    #[test]
    fn test_extract_file_path_no_backticks() {
        let parser = MarkdownParser;

        let section = "file: src/simple.py";
        assert_eq!(parser.extract_file_path(section).unwrap(), "src/simple.py");
    }

    #[test]
    fn test_extract_file_path_path_keyword() {
        let parser = MarkdownParser;

        let section = "Path: src/module.rs";
        assert_eq!(parser.extract_file_path(section).unwrap(), "src/module.rs");
    }

    #[test]
    fn test_extract_line_numbers_range() {
        let parser = MarkdownParser;

        let section = "Lines: 100-150";
        assert_eq!(parser.extract_line_numbers(section), Some((100, 150)));
    }

    #[test]
    fn test_cannot_handle_without_markdown() {
        let parser = MarkdownParser;

        let text = "This is plain text without code blocks";
        assert!(!parser.can_handle(text));
    }

    #[test]
    fn test_cannot_handle_without_keywords() {
        let parser = MarkdownParser;

        let text = r#"
```typescript
some code
```
"#;
        assert!(!parser.can_handle(text));
    }

    #[test]
    fn test_parse_with_python_code() {
        let parser = MarkdownParser;

        let markdown = r#"
File: `src/main.py`

Before:
```python
def greet():
    print("hello")
```

After:
```python
def greet(name):
    print(f"hello {name}")
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("hello"));
        assert!(changes[0].new_code.contains("hello {name}"));
    }

    #[test]
    fn test_parse_with_rust_code() {
        let parser = MarkdownParser;

        let markdown = r#"
File: src/lib.rs

Before:
```rust
pub fn main() {
    println!("start");
}
```

After:
```rust
pub fn main() {
    println!("starting application");
}
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("start"));
        assert!(changes[0].new_code.contains("starting application"));
    }

    #[test]
    fn test_parse_without_language_specifier() {
        let parser = MarkdownParser;

        // Note: File extension must match the regex pattern (ts, js, py, rs, etc.)
        let markdown = r#"
File: src/note.ts

Before:
```
old text
```

After:
```
new text
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("old text"));
        assert!(changes[0].new_code.contains("new text"));
    }

    #[test]
    fn test_parse_missing_file_path() {
        let parser = MarkdownParser;

        let markdown = r#"
Before:
```typescript
old
```

After:
```typescript
new
```
"#;

        let result = parser.parse(markdown);
        // Should fail because no file path
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_only_one_code_block() {
        let parser = MarkdownParser;

        let markdown = r#"
File: src/test.ts

Before:
```typescript
only one block
```
"#;

        let result = parser.parse(markdown);
        // Should fail because we need at least 2 blocks (before and after)
        assert!(result.is_err());
    }

    #[test]
    fn test_find_all_code_blocks() {
        let parser = MarkdownParser;

        let text = r#"
Some text

```typescript
code block 1
```

More text

```python
code block 2
```
"#;

        let blocks = parser.find_all_code_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].0.contains("code block 1"));
        assert!(blocks[1].0.contains("code block 2"));
    }

    #[test]
    fn test_parse_with_indented_code() {
        let parser = MarkdownParser;

        let markdown = r#"
File: src/class.py

Before:
```python
class MyClass:
    def __init__(self):
        self.value = 0
```

After:
```python
class MyClass:
    def __init__(self, value=0):
        self.value = value
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("self.value = 0"));
        assert!(changes[0].new_code.contains("self.value = value"));
    }

    #[test]
    fn test_parse_case_insensitive_keywords() {
        let parser = MarkdownParser;

        let markdown = r#"
File: src/test.ts

BEFORE:
```typescript
old
```

AFTER:
```typescript
new
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_split_into_sections_multiple_files() {
        let parser = MarkdownParser;

        // Use complete file references in a more realistic format
        let text = r#"
File: `file1.ts`

Before:
code here

File: `file2.ts`

Before:
more code
"#;

        let sections = parser.split_into_sections(text).unwrap();
        // Should split when it sees "File: `file2.ts`"
        assert!(sections.len() >= 2, "Expected at least 2 sections, got {}", sections.len());
    }

    #[test]
    fn test_parse_with_inline_file_reference() {
        let parser = MarkdownParser;

        let markdown = r#"
In `src/inline.ts`, update:

Before:
```typescript
const x = 1;
```

After:
```typescript
const x = 2;
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/inline.ts");
    }

    #[test]
    fn test_parse_fallback_to_position() {
        let parser = MarkdownParser;

        // No keywords, should use first block as before, second as after
        let markdown = r#"
File: src/fallback.ts

```typescript
first block
```

```typescript
second block
```
"#;

        let result = parser.parse(markdown);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("first block"));
        assert!(changes[0].new_code.contains("second block"));
    }
}
