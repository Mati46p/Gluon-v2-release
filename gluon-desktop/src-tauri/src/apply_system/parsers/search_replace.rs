//! SEARCH/REPLACE Parser - Structured Replacement Format
//!
//! This parser handles structured SEARCH/REPLACE blocks where files
//! are sent to the model with line numbers and the model responds with
//! precise replacement instructions.
//!
//! Supported formats:
//!
//! Format 1 - SEARCH/REPLACE with exact matching:
//! ```
//! <<<< SEARCH path/to/file.ts
//! [exact code to find]
//! ====
//! [new code to replace with]
//! >>>> REPLACE
//! ```
//!
//! Format 2 - EDIT with line numbers:
//! ```
//! <<<< EDIT path/to/file.ts:10-15
//! [new code to replace lines 10-15]
//! >>>> END
//! ```
//!
//! Format 3 - CREATE for new files:
//! ```
//! <<<< CREATE path/to/file.ts
//! [complete file content]
//! >>>> END
//! ```
//!
//! This parser has the HIGHEST priority because it's the most structured
//! format specifically designed for programmatic parsing.

use crate::apply_system::parsers::Parser;
use crate::apply_system::shared::types::ChangeQueueItem;
use regex::Regex;

pub struct SearchReplaceParser;

impl Parser for SearchReplaceParser {
    fn name(&self) -> &'static str {
        "SearchReplace"
    }

    fn can_handle(&self, raw_response: &str) -> bool {
        // Check for our specific markers
        raw_response.contains("<<<< SEARCH")
            || raw_response.contains("<<<< EDIT")
            || raw_response.contains("<<<< CREATE")
    }

    fn parse(&self, raw_response: &str) -> Result<Vec<ChangeQueueItem>, String> {
        let mut changes = Vec::new();

        // Try to find all SEARCH/REPLACE blocks
        changes.extend(self.parse_search_replace_blocks(raw_response)?);

        // Try to find all EDIT blocks
        changes.extend(self.parse_edit_blocks(raw_response)?);

        // Try to find all CREATE blocks
        changes.extend(self.parse_create_blocks(raw_response)?);

        if changes.is_empty() {
            return Err("No valid SEARCH/REPLACE, EDIT, or CREATE blocks found".to_string());
        }

        Ok(changes)
    }
}

impl SearchReplaceParser {

    /// Parse SEARCH/REPLACE blocks
    fn parse_search_replace_blocks(&self, text: &str) -> Result<Vec<ChangeQueueItem>, String> {
        let mut blocks = Vec::new();

        // Pattern: <<<< SEARCH file_path
        let start_pattern = r"<<<<\s+SEARCH\s+([^\s\n]+)";
        let start_re = Regex::new(start_pattern).map_err(|e| format!("Regex error: {}", e))?;

        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            // Look for SEARCH marker
            if let Some(captures) = start_re.captures(lines[i]) {
                let file_path = captures
                    .get(1)
                    .map(|m| m.as_str().to_string())
                    .ok_or("Failed to extract file path")?;

                // Collect old_code until ====
                let mut old_code_lines = Vec::new();
                i += 1;

                while i < lines.len() && !lines[i].trim().starts_with("====") {
                    old_code_lines.push(lines[i]);
                    i += 1;
                }

                if i >= lines.len() {
                    return Err(format!(
                        "Missing ==== separator in SEARCH block for {}",
                        file_path
                    ));
                }

                let old_code = crate::apply_system::parsers::sanitize_code_block(old_code_lines);

                // Skip ==== line
                i += 1;

                // Collect new_code until >>>> REPLACE
                let mut new_code_lines = Vec::new();

                // ROBUSTNESS: Stop at ">>>>", ignore trailing text (e.g. ">>>> REPLACE // comment")
                while i < lines.len() && !lines[i].trim().starts_with(">>>>") {
                    new_code_lines.push(lines[i]);
                    i += 1;
                }

                if i >= lines.len() {
                    return Err(format!("Missing >>>> REPLACE marker for {}", file_path));
                }

                let new_code = crate::apply_system::parsers::sanitize_code_block(new_code_lines);
 
                // [GLUON SAFETY] Lazy Coding Check
                if let Err(e) = crate::apply_system::parsers::detect_lazy_coding(&new_code) {
                    return Err(format!("Lazy coding detected in {}: {}", file_path, e));
                }
 
                // Validate
                if old_code.trim().is_empty() {
                    return Err(format!("Empty old_code in SEARCH block for {}", file_path));
                }
 
                // Create change item
                let change = ChangeQueueItem::new(
                    file_path.clone(),
                    0, // Line numbers will be determined by matcher
                    0,
                    old_code,
                    new_code,
                );

                blocks.push(change);
            }

            i += 1;
        }

        Ok(blocks)
    }

    /// Parse EDIT blocks with line numbers
    fn parse_edit_blocks(&self, text: &str) -> Result<Vec<ChangeQueueItem>, String> {
        let mut blocks = Vec::new();

        // Pattern: <<<< EDIT file_path:line_start-line_end
        let start_pattern = r"<<<<\s+EDIT\s+([^\s:]+):(\d+)-(\d+)";
        let start_re = Regex::new(start_pattern).map_err(|e| format!("Regex error: {}", e))?;

        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            // Look for EDIT marker
            if let Some(captures) = start_re.captures(lines[i]) {
                let file_path = captures
                    .get(1)
                    .map(|m| m.as_str().to_string())
                    .ok_or("Failed to extract file path")?;

                let line_start = captures
                    .get(2)
                    .and_then(|m| m.as_str().parse::<usize>().ok())
                    .ok_or("Failed to parse line_start")?;

                let line_end = captures
                    .get(3)
                    .and_then(|m| m.as_str().parse::<usize>().ok())
                    .ok_or("Failed to parse line_end")?;

                // Collect new_code until >>>> END
                let mut new_code_lines = Vec::new();
                i += 1;

                // ROBUSTNESS: Stop at ">>>>"
                while i < lines.len() && !lines[i].trim().starts_with(">>>>") {
                    new_code_lines.push(lines[i]);
                    i += 1;
                }

                if i >= lines.len() {
                    return Err(format!("Missing >>>> END marker for {}", file_path));
                }

                let new_code = crate::apply_system::parsers::sanitize_code_block(new_code_lines);
 
                // [GLUON SAFETY] Lazy Coding Check
                if let Err(e) = crate::apply_system::parsers::detect_lazy_coding(&new_code) {
                    return Err(format!("Lazy coding detected in {}: {}", file_path, e));
                }
 
                // For EDIT format, we don't have old_code
                // We'll use a placeholder - the matcher will read the actual lines from the file
                let old_code = format!("<<LINES {}-{}>>", line_start, line_end);

                // Create change item
                let change = ChangeQueueItem::new(
                    file_path.clone(),
                    line_start,
                    line_end,
                    old_code,
                    new_code,
                );

                blocks.push(change);
            }

            i += 1;
        }

        Ok(blocks)
    }

    /// Parse CREATE blocks for new files
    fn parse_create_blocks(&self, text: &str) -> Result<Vec<ChangeQueueItem>, String> {
        let mut blocks = Vec::new();

        // Pattern: <<<< CREATE file_path
        let start_pattern = r"<<<<\s+CREATE\s+([^\s\n]+)";
        let start_re = Regex::new(start_pattern).map_err(|e| format!("Regex error: {}", e))?;

        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            // Look for CREATE marker
            if let Some(captures) = start_re.captures(lines[i]) {
                let file_path = captures
                    .get(1)
                    .map(|m| m.as_str().to_string())
                    .ok_or("Failed to extract file path")?;

                // Collect file content until >>>> END
                let mut content_lines = Vec::new();
                i += 1;

                // ROBUSTNESS: Stop at ">>>>"
                while i < lines.len() && !lines[i].trim().starts_with(">>>>") {
                    content_lines.push(lines[i]);
                    i += 1;
                }

                if i >= lines.len() {
                    return Err(format!("Missing >>>> END marker for {}", file_path));
                }

                let new_code = crate::apply_system::parsers::sanitize_code_block(content_lines);
 
                // [GLUON SAFETY] Lazy Coding Check
                if let Err(e) = crate::apply_system::parsers::detect_lazy_coding(&new_code) {
                    return Err(format!("Lazy coding detected in {}: {}", file_path, e));
                }
 
                // For CREATE, old_code is empty (new file)
                let old_code = "";

                // Create change item
                let change = ChangeQueueItem::new(
                    file_path.clone(),
                    1, // New file starts at line 1
                    1,
                    old_code.to_string(),
                    new_code,
                );

                blocks.push(change);
            }

            i += 1;
        }

        Ok(blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_search_replace() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< SEARCH src/test.ts
old code
====
new code
>>>> REPLACE
"#;

        assert!(parser.can_handle(text));
    }

    #[test]
    fn test_can_handle_edit() {
        let parser = SearchReplaceParser;

        let text = "<<<< EDIT src/test.ts:10-15\nnew code\n>>>> END";

        assert!(parser.can_handle(text));
    }

    #[test]
    fn test_parse_search_replace_block() {
        let parser = SearchReplaceParser;

        let text = r#"
Some explanation text...

<<<< SEARCH src/auth.ts
export function login(username: string) {
  return authenticate(username);
}
====
export function login(username: string, password: string) {
  return authenticate(username, password);
}
>>>> REPLACE

More text...
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/auth.ts");
        assert!(changes[0].old_code.contains("authenticate(username)"));
        assert!(
            changes[0]
                .new_code
                .contains("authenticate(username, password)")
        );
    }

    #[test]
    fn test_parse_edit_block() {
        let parser = SearchReplaceParser;

        let text = r#"
Let me update the login function:

<<<< EDIT src/auth.ts:15-17
export function login(username: string, password: string) {
  return authenticate(username, password);
}
>>>> END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/auth.ts");
        assert_eq!(changes[0].line_start, 15);
        assert_eq!(changes[0].line_end, 17);
        assert!(
            changes[0]
                .new_code
                .contains("authenticate(username, password)")
        );
    }

    #[test]
    fn test_parse_create_block() {
        let parser = SearchReplaceParser;

        let text = r#"
I'll create a new utility file:

<<<< CREATE src/utils/helper.ts
export function formatDate(date: Date): string {
  return date.toISOString();
}
>>>> END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/utils/helper.ts");
        assert_eq!(changes[0].old_code, ""); // New file
        assert!(changes[0].new_code.contains("formatDate"));
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let parser = SearchReplaceParser;

        let text = r#"
I'll make two changes:

<<<< SEARCH src/auth.ts
old code 1
====
new code 1
>>>> REPLACE

<<<< EDIT src/db.ts:20-25
new code 2
>>>> END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "src/auth.ts");
        assert_eq!(changes[1].file_path, "src/db.ts");
    }

    #[test]
    fn test_parse_missing_separator() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< SEARCH src/test.ts
old code
new code without separator
>>>> REPLACE
"#;

        let result = parser.parse(text);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing ==== separator"));
    }

    #[test]
    fn test_parse_missing_end_marker() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< EDIT src/test.ts:10-15
new code without end marker
"#;

        let result = parser.parse(text);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing >>>> END"));
    }

    #[test]
    fn test_parse_multiple_search_replace_blocks() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< SEARCH src/auth.ts
function login() {}
====
function login(user: string) {}
>>>> REPLACE

<<<< SEARCH src/db.ts
const conn = null;
====
const conn = createConnection();
>>>> REPLACE
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "src/auth.ts");
        assert_eq!(changes[1].file_path, "src/db.ts");
    }

    #[test]
    fn test_parse_multiple_edit_blocks() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< EDIT src/main.ts:10-12
new code for main
>>>> END

<<<< EDIT src/utils.ts:20-25
new code for utils
>>>> END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "src/main.ts");
        assert_eq!(changes[0].line_start, 10);
        assert_eq!(changes[0].line_end, 12);
        assert_eq!(changes[1].file_path, "src/utils.ts");
        assert_eq!(changes[1].line_start, 20);
        assert_eq!(changes[1].line_end, 25);
    }

    #[test]
    fn test_parse_mixed_formats() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< SEARCH src/config.ts
const DEBUG = false;
====
const DEBUG = true;
>>>> REPLACE

<<<< EDIT src/app.ts:5-10
export function start() {
  initialize();
}
>>>> END

<<<< CREATE src/new-file.ts
export const VERSION = "1.0.0";
>>>> END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0].file_path, "src/config.ts");
        assert_eq!(changes[1].file_path, "src/app.ts");
        assert_eq!(changes[2].file_path, "src/new-file.ts");
        assert_eq!(changes[2].old_code, ""); // CREATE has empty old_code
    }

    #[test]
    fn test_create_with_multiline_content() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< CREATE src/types.ts
export interface User {
  id: number;
  name: string;
  email: string;
}

export interface Post {
  id: number;
  title: string;
}
>>>> END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/types.ts");
        assert!(changes[0].new_code.contains("interface User"));
        assert!(changes[0].new_code.contains("interface Post"));
    }

    #[test]
    fn test_search_replace_with_special_characters() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< SEARCH src/regex.ts
const pattern = /[a-z]+/;
====
const pattern = /[a-zA-Z0-9_]+/;
>>>> REPLACE
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("[a-z]+"));
        assert!(changes[0].new_code.contains("[a-zA-Z0-9_]+"));
    }

    #[test]
    fn test_edit_single_line_range() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< EDIT src/const.ts:42-42
export const API_KEY = "new-key";
>>>> END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].line_start, 42);
        assert_eq!(changes[0].line_end, 42);
    }

    #[test]
    fn test_empty_old_code_in_search_block() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< SEARCH src/test.ts
====
new code
>>>> REPLACE
"#;

        let result = parser.parse(text);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty old_code"));
    }

    #[test]
    fn test_can_handle_only_search() {
        let parser = SearchReplaceParser;
        assert!(parser.can_handle("<<<< SEARCH file.ts"));
        assert!(parser.can_handle("<<<< EDIT file.ts:1-10"));
        assert!(parser.can_handle("<<<< CREATE file.ts"));
        assert!(!parser.can_handle("some random text"));
    }

    #[test]
    fn test_search_replace_preserves_indentation() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< SEARCH src/class.py
class MyClass:
    def method(self):
        return True
====
class MyClass:
    def method(self):
        return self.validate()

    def validate(self):
        return True
>>>> REPLACE
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("    def method"));
        assert!(changes[0].new_code.contains("    def validate"));
    }

    #[test]
    fn test_create_empty_file() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< CREATE src/empty.ts
>>>> END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].old_code, "");
        assert_eq!(changes[0].new_code.trim(), "");
    }

    #[test]
    fn test_robustness_trailing_text_after_markers() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< SEARCH src/app.ts
old code
====
new code
>>>> REPLACE // this is a comment
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_edit_with_line_numbers_preserves_placeholder() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< EDIT src/test.ts:100-110
replacement code here
>>>> END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].old_code, "<<LINES 100-110>>");
        assert_eq!(changes[0].line_start, 100);
        assert_eq!(changes[0].line_end, 110);
    }

    #[test]
    fn test_multiple_creates() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< CREATE src/file1.ts
export const A = 1;
>>>> END

<<<< CREATE src/file2.ts
export const B = 2;
>>>> END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        assert!(changes[0].new_code.contains("A = 1"));
        assert!(changes[1].new_code.contains("B = 2"));
    }

    #[test]
    fn test_search_replace_with_blank_lines() {
        let parser = SearchReplaceParser;

        let text = r#"
<<<< SEARCH src/spacing.ts
function test() {
  console.log("a");
  console.log("b");
}
====
function test() {
  console.log("a");

  console.log("b");
}
>>>> REPLACE
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        // Verify blank line is preserved in new_code
        assert!(changes[0].new_code.contains("\n\n"));
    }
}
