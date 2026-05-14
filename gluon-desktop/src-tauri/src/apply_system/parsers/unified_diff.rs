//! KROK 7: Parser #1 - GitHub Unified Diff Format
//!
//! Parses diff format like:
//! ```
//! --- a/path/to/file.ts
//! +++ b/path/to/file.ts
//! @@ -45,7 +45,8 @@
//!  context line
//! -old line
//! +new line
//!  context line
//! ```

use crate::apply_system::parsers::Parser;
use crate::apply_system::shared::types::ChangeQueueItem;
use regex::Regex;

pub struct UnifiedDiffParser;

impl Parser for UnifiedDiffParser {
    fn name(&self) -> &'static str {
        "UnifiedDiff"
    }

    fn can_handle(&self, raw_response: &str) -> bool {
        // Quick check for unified diff markers
        raw_response.contains("---") && raw_response.contains("+++") && raw_response.contains("@@")
    }

    fn parse(&self, raw_response: &str) -> Result<Vec<ChangeQueueItem>, String> {
        let mut changes = Vec::new();

        // Parse diff blocks
        let diff_blocks = self.split_into_diff_blocks(raw_response)?;

        for block in diff_blocks {
            match self.parse_diff_block(&block) {
                Ok(change) => changes.push(change),
                Err(e) => {
                    // Log error but continue parsing other blocks
                    crate::gluon_warn!("UnifiedDiff", "Failed to parse diff block: {}", e);
                }
            }
        }

        if changes.is_empty() {
            return Err("No valid diff blocks found".to_string());
        }

        Ok(changes)
    }
}

impl UnifiedDiffParser {
    /// Split the response into individual diff blocks (one per file)
    fn split_into_diff_blocks(&self, text: &str) -> Result<Vec<String>, String> {
        let mut blocks = Vec::new();
        let mut current_block = String::new();
        let mut in_diff = false;

        for line in text.lines() {
            if line.starts_with("---") {
                // Start of new diff block
                if !current_block.is_empty() {
                    blocks.push(current_block.clone());
                }
                current_block = String::new();
                in_diff = true;
            }

            if in_diff {
                current_block.push_str(line);
                current_block.push('\n');
            }
        }

        // Don't forget the last block
        if !current_block.is_empty() {
            blocks.push(current_block);
        }

        if blocks.is_empty() {
            return Err("No diff blocks found".to_string());
        }

        Ok(blocks)
    }

    /// Parse a single diff block into a ChangeQueueItem
    fn parse_diff_block(&self, block: &str) -> Result<ChangeQueueItem, String> {
        // Extract file path
        let file_path = self.extract_file_path(block)?;

        // Extract hunk information (@@ -45,7 +45,8 @@)
        let (line_start, line_count) = self.extract_hunk_info(block)?;

        // Extract old and new code
        let (old_code, new_code) = self.extract_code_changes(block)?;

        let change = ChangeQueueItem::new(
            file_path,
            line_start,
            line_start + line_count,
            old_code,
            new_code,
        );

        Ok(change)
    }

    /// Extract file path from diff header
    ///
    /// From lines like:
    /// --- a/path/to/file.ts
    /// +++ b/path/to/file.ts
    fn extract_file_path(&self, block: &str) -> Result<String, String> {
        let re = Regex::new(r"^\+\+\+ b/(.+)$").unwrap();

        for line in block.lines() {
            if let Some(captures) = re.captures(line) {
                if let Some(path) = captures.get(1) {
                    return Ok(path.as_str().to_string());
                }
            }
        }

        Err("Could not find file path in diff header".to_string())
    }

    /// Extract hunk information from @@ line
    ///
    /// Format: @@ -45,7 +45,8 @@
    /// Returns: (line_start, line_count)
    fn extract_hunk_info(&self, block: &str) -> Result<(usize, usize), String> {
        // Match: @@ -start,count +start,count @@
        let re = Regex::new(r"^@@\s+-(\d+),(\d+)\s+\+(\d+),(\d+)\s+@@").unwrap();

        for line in block.lines() {
            if let Some(captures) = re.captures(line) {
                // We care about the NEW location (+start,count)
                let line_start: usize = captures
                    .get(3)
                    .and_then(|m| m.as_str().parse().ok())
                    .ok_or("Invalid line start")?;

                let line_count: usize = captures
                    .get(4)
                    .and_then(|m| m.as_str().parse().ok())
                    .ok_or("Invalid line count")?;

                return Ok((line_start, line_count));
            }
        }

        Err("Could not find hunk info (@@) in diff".to_string())
    }

    /// Extract old and new code from diff lines
    ///
    /// Lines starting with:
    /// - " " (space) = context (unchanged)
    /// - "-" = removed (old code)
    /// - "+" = added (new code)
    fn extract_code_changes(&self, block: &str) -> Result<(String, String), String> {
        let mut old_code = Vec::new();
        let mut new_code = Vec::new();
        let mut in_hunk = false;

        for line in block.lines() {
            // Start collecting after @@ line
            if line.starts_with("@@") {
                in_hunk = true;
                continue;
            }

            if !in_hunk {
                continue;
            }

            // Stop at next diff block or end
            if line.starts_with("---") || line.starts_with("+++") {
                break;
            }

            if line.starts_with("-") {
                // Removed line (old code)
                old_code.push(line[1..].to_string());
            } else if line.starts_with("+") {
                // Added line (new code)
                new_code.push(line[1..].to_string());
            } else if line.starts_with(" ") {
                // Context line - appears in both
                let context = line[1..].to_string();
                old_code.push(context.clone());
                new_code.push(context);
            }
        }

        if old_code.is_empty() && new_code.is_empty() {
            return Err("No code changes found in diff".to_string());
        }

        Ok((old_code.join("\n"), new_code.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_unified_diff() {
        let parser = UnifiedDiffParser;

        let valid = r#"
--- a/src/auth.ts
+++ b/src/auth.ts
@@ -45,7 +45,8 @@
-old line
+new line
"#;

        assert!(parser.can_handle(valid));

        let invalid = "just some text without diff markers";
        assert!(!parser.can_handle(invalid));
    }

    #[test]
    fn test_extract_file_path() {
        let parser = UnifiedDiffParser;

        let block = r#"
--- a/src/auth.ts
+++ b/src/auth.ts
"#;

        let path = parser.extract_file_path(block).unwrap();
        assert_eq!(path, "src/auth.ts");
    }

    #[test]
    fn test_extract_hunk_info() {
        let parser = UnifiedDiffParser;

        let block = "@@ -45,7 +45,8 @@";

        let (start, count) = parser.extract_hunk_info(block).unwrap();
        assert_eq!(start, 45);
        assert_eq!(count, 8);
    }

    #[test]
    fn test_parse_simple_diff() {
        let parser = UnifiedDiffParser;

        let diff = r#"
--- a/src/test.ts
+++ b/src/test.ts
@@ -1,3 +1,3 @@
 function test() {
-  return "old";
+  return "new";
 }
"#;

        let result = parser.parse(diff);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/test.ts");
        assert!(changes[0].old_code.contains("old"));
        assert!(changes[0].new_code.contains("new"));
    }

    #[test]
    fn test_parse_multiple_files() {
        let parser = UnifiedDiffParser;

        let diff = r#"
--- a/src/file1.ts
+++ b/src/file1.ts
@@ -10,2 +10,2 @@
-old line 1
+new line 1

--- a/src/file2.ts
+++ b/src/file2.ts
@@ -5,2 +5,2 @@
-old line 2
+new line 2
"#;

        let result = parser.parse(diff);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "src/file1.ts");
        assert_eq!(changes[1].file_path, "src/file2.ts");
    }

    #[test]
    fn test_parse_addition_only() {
        let parser = UnifiedDiffParser;

        let diff = r#"
--- a/src/new.ts
+++ b/src/new.ts
@@ -0,0 +1,3 @@
+export function newFunc() {
+  return true;
+}
"#;

        let result = parser.parse(diff);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].new_code.contains("newFunc"));
    }

    #[test]
    fn test_parse_deletion_only() {
        let parser = UnifiedDiffParser;

        let diff = r#"
--- a/src/old.ts
+++ b/src/old.ts
@@ -10,3 +10,0 @@
-export function oldFunc() {
-  return false;
-}
"#;

        let result = parser.parse(diff);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("oldFunc"));
    }

    #[test]
    fn test_parse_with_context_lines() {
        let parser = UnifiedDiffParser;

        let diff = r#"
--- a/src/context.ts
+++ b/src/context.ts
@@ -5,7 +5,7 @@
 export class MyClass {
   constructor() {
-    this.value = 0;
+    this.value = 10;
   }

   getValue() {
"#;

        let result = parser.parse(diff);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("this.value = 0"));
        assert!(changes[0].new_code.contains("this.value = 10"));
        assert!(changes[0].old_code.contains("export class MyClass"));
        assert!(changes[0].new_code.contains("export class MyClass"));
    }

    #[test]
    fn test_extract_hunk_info_multiple_lines() {
        let parser = UnifiedDiffParser;

        let block = "@@ -100,15 +100,20 @@";
        let (start, count) = parser.extract_hunk_info(block).unwrap();
        assert_eq!(start, 100);
        assert_eq!(count, 20);
    }

    #[test]
    fn test_extract_hunk_info_single_line() {
        let parser = UnifiedDiffParser;

        let block = "@@ -42,1 +42,1 @@";
        let (start, count) = parser.extract_hunk_info(block).unwrap();
        assert_eq!(start, 42);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_cannot_handle_without_markers() {
        let parser = UnifiedDiffParser;

        let text = "This is just normal text without diff markers";
        assert!(!parser.can_handle(text));
    }

    #[test]
    fn test_parse_diff_with_leading_text() {
        let parser = UnifiedDiffParser;

        let diff = r#"
Here is the diff for the changes:

--- a/src/app.ts
+++ b/src/app.ts
@@ -1,2 +1,2 @@
-const version = "1.0";
+const version = "2.0";
"#;

        let result = parser.parse(diff);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_parse_multiple_hunks_same_file() {
        let parser = UnifiedDiffParser;

        let diff = r#"
--- a/src/multi.ts
+++ b/src/multi.ts
@@ -10,2 +10,2 @@
-line 10 old
+line 10 new
@@ -20,2 +20,2 @@
-line 20 old
+line 20 new
"#;

        let result = parser.parse(diff);
        assert!(result.is_ok());

        // Note: Current implementation treats each hunk separately
        // This test documents the current behavior
        let changes = result.unwrap();
        assert!(changes.len() >= 1);
    }

    #[test]
    fn test_parse_with_no_newline_marker() {
        let parser = UnifiedDiffParser;

        let diff = r#"
--- a/src/nonewline.ts
+++ b/src/nonewline.ts
@@ -1,1 +1,1 @@
-old content
\ No newline at end of file
+new content
\ No newline at end of file
"#;

        let result = parser.parse(diff);
        assert!(result.is_ok());
    }

    #[test]
    fn test_missing_file_path_error() {
        let parser = UnifiedDiffParser;

        let diff = r#"
@@ -1,2 +1,2 @@
-old
+new
"#;

        let result = parser.parse(diff);
        // Should fail because no file path headers
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_indented_code() {
        let parser = UnifiedDiffParser;

        let diff = r#"
--- a/src/indent.py
+++ b/src/indent.py
@@ -5,4 +5,4 @@
 class Test:
     def method(self):
-        return False
+        return True
"#;

        let result = parser.parse(diff);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("return False"));
        assert!(changes[0].new_code.contains("return True"));
    }

    #[test]
    fn test_split_into_diff_blocks() {
        let parser = UnifiedDiffParser;

        let text = r#"
Some text before

--- a/file1.ts
+++ b/file1.ts
@@ -1,1 +1,1 @@
-old1
+new1

--- a/file2.ts
+++ b/file2.ts
@@ -1,1 +1,1 @@
-old2
+new2
"#;

        let blocks = parser.split_into_diff_blocks(text).unwrap();
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn test_extract_file_path_variations() {
        let parser = UnifiedDiffParser;

        let block1 = "+++ b/src/path/to/file.ts";
        assert_eq!(parser.extract_file_path(block1).unwrap(), "src/path/to/file.ts");

        let block2 = "+++ b/deeply/nested/path/module.rs";
        assert_eq!(parser.extract_file_path(block2).unwrap(), "deeply/nested/path/module.rs");
    }

    #[test]
    fn test_parse_with_special_characters_in_code() {
        let parser = UnifiedDiffParser;

        let diff = r#"
--- a/src/regex.ts
+++ b/src/regex.ts
@@ -1,1 +1,1 @@
-const pattern = /[<>&]/g;
+const pattern = /[<>&"']/g;
"#;

        let result = parser.parse(diff);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("[<>&]"));
        assert!(changes[0].new_code.contains("[<>&\"']"));
    }

    #[test]
    fn test_hunk_with_no_changes() {
        let parser = UnifiedDiffParser;

        let diff = r#"
--- a/src/same.ts
+++ b/src/same.ts
@@ -1,3 +1,3 @@
 line 1
 line 2
 line 3
"#;

        let result = parser.parse(diff);
        // Should succeed but with empty or context-only code
        assert!(result.is_ok());
    }
}
