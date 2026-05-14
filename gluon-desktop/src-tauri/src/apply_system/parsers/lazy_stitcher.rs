//! Lazy Stitcher Parser
//!
//! Parses model responses that use lazy coding markers (// ... existing code ...)
//! instead of explicit search/replace blocks.
//!
//! This parser:
//! 1. Detects lazy markers in the response
//! 2. Reads the original file from disk
//! 3. Uses the LazyStitcher engine to reconstruct the file
//! 4. Generates a diff to create ChangeQueueItems

use super::Parser;
use crate::apply_system::{
    lazy::apply_lazy_edit,
    shared::types::ChangeQueueItem,
};
use std::path::Path;

pub struct LazyStitcherParser;

impl Parser for LazyStitcherParser {
    fn parse(&self, response_text: &str) -> Result<Vec<ChangeQueueItem>, String> {
        // Step 1: Extract file path from response
        // We expect the response to contain a code block with language identifier
        // e.g., ```typescript
        // or we look for file path comments like "// File: src/example.ts"

        let file_path = extract_file_path(response_text)
            .ok_or("LazyStitcher: Could not determine file path from response")?;

        // Step 2: Extract code content (remove markdown fences)
        let code_content = extract_code_content(response_text)?;

        // Step 3: Read original file
        let original_content = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("LazyStitcher: Failed to read original file {}: {}", file_path, e))?;

        // Step 4: Apply lazy edit
        let path = Path::new(&file_path);

        let edit_result = apply_lazy_edit(&original_content, &code_content, path)
            .map_err(|e| format!("LazyStitcher: Failed to apply lazy edit: {}", e))?;

        // Step 5: Generate diff and create ChangeQueueItem
        // For now, we create a single change that replaces the entire file
        // (In future, we could create granular changes based on the diff)

        let change = ChangeQueueItem::new(
            file_path.clone(),
            1,
            original_content.lines().count(),
            original_content,
            edit_result.content.clone(),
        );

        Ok(vec![change])
    }

    fn name(&self) -> &'static str {
        "LazyStitcher"
    }

    fn can_handle(&self, response_text: &str) -> bool {
        use crate::apply_system::lazy::detector::contains_lazy_markers;
        contains_lazy_markers(response_text)
    }
}

/// Extract file path from response text
///
/// Looks for patterns like:
/// - `// File: src/example.ts`
/// - `# File: src/example.py`
/// - Code fence with file path: ```typescript:src/example.ts
fn extract_file_path(text: &str) -> Option<String> {
    // Strategy 1: Look for explicit file comments
    let file_comment_patterns = [
        r"//\s*[Ff]ile:\s*(.+)",
        r"#\s*[Ff]ile:\s*(.+)",
        r"<!--\s*[Ff]ile:\s*(.+)\s*-->",
    ];

    for pattern in &file_comment_patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(cap) = re.captures(text) {
                if let Some(path) = cap.get(1) {
                    return Some(path.as_str().trim().to_string());
                }
            }
        }
    }

    // Strategy 2: Look for code fence with file path
    // ```typescript:src/example.ts
    if let Ok(re) = regex::Regex::new(r"```\w+:(.+)") {
        if let Some(cap) = re.captures(text) {
            if let Some(path) = cap.get(1) {
                return Some(path.as_str().trim().to_string());
            }
        }
    }

    None
}

/// Extract code content from response (remove markdown fences)
fn extract_code_content(text: &str) -> Result<String, String> {
    // Look for code blocks
    let fence_pattern = r"```(?:\w+)?(?::.+)?\n([\s\S]+?)```";

    if let Ok(re) = regex::Regex::new(fence_pattern) {
        if let Some(cap) = re.captures(text) {
            if let Some(code) = cap.get(1) {
                return Ok(code.as_str().to_string());
            }
        }
    }

    // If no code fence found, return the whole text
    // (Maybe the model returned raw code without fences)
    Ok(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_file_path() {
        let text = "// File: src/example.ts\nfunction foo() {}";
        assert_eq!(extract_file_path(text), Some("src/example.ts".to_string()));

        let text2 = "# File: main.py\ndef hello():";
        assert_eq!(extract_file_path(text2), Some("main.py".to_string()));

        let text3 = "```typescript:src/app.ts\nconst x = 1;";
        assert_eq!(extract_file_path(text3), Some("src/app.ts".to_string()));
    }

    #[test]
    fn test_extract_code_content() {
        let text = "```typescript\nconst x = 1;\n```";
        let result = extract_code_content(text).unwrap();
        assert_eq!(result, "const x = 1;\n");

        let text2 = "```python:main.py\ndef foo():\n    pass\n```";
        let result2 = extract_code_content(text2).unwrap();
        assert!(result2.contains("def foo()"));
    }
}
