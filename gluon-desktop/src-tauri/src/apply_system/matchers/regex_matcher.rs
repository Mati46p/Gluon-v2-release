//! Matcher Strategy 3: Regex Pattern
//! Converts the search block into a flexible regex, ignoring whitespace differences.

use super::Matcher;
use crate::apply_system::shared::types::{MatchResult, MatchMethod};
use regex::{escape, RegexBuilder};

pub struct RegexMatcher;

impl Matcher for RegexMatcher {
    fn find_match(&self, file_content: &str, search_block: &str, _file_path: Option<&str>) -> Option<MatchResult> {
        // Create a flexible regex from the search block
        // 1. Split by whitespace
        // 2. Escape special characters
        // 3. Replace whitespace runs with \s+ (flexible whitespace)
        let parts: Vec<String> = search_block
            .split_whitespace()
            .map(|part| escape(part))
            .collect();
        
        if parts.is_empty() { return None; }

        // Join with \s+ to allow any amount of whitespace (including newlines) between tokens
        let pattern = parts.join(r"\s+");
        
        // Use RegexBuilder to enable multi-line mode if needed, though \s matches \n
        // Enable dot matches newline if needed, but \s+ usually covers it
        let re = match RegexBuilder::new(&pattern).build() {
            Ok(r) => r,
            Err(_) => return None, // Pattern too complex or invalid
        };

        if let Some(mat) = re.find(file_content) {
            // Convert byte offset to line number
            let start_byte = mat.start();
            let end_byte = mat.end();

            // Count newlines before the match to determine line number
            let pre_content = &file_content[..start_byte];
            let match_content = &file_content[start_byte..end_byte];

            // Count newlines in pre_content to get line start (1-indexed)
            let line_start = pre_content.chars().filter(|&c| c == '\n').count() + 1;

            // Count newlines in match_content to determine line span
            let newlines_in_match = match_content.chars().filter(|&c| c == '\n').count();
            let line_end = line_start + newlines_in_match;

            return Some(MatchResult {
                matched_line_start: line_start,
                matched_line_end: line_end,
                method_used: MatchMethod::RegexPattern,
                confidence: 0.60, // Low confidence by default for Regex
                details: Some("Flexible regex match (whitespace insensitive)".to_string()),
                confidence_breakdown: None,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_match_flexible() {
        let file = r#"
            const x = 1;
            if (x > 0) {
                console.log('test');
            }
        "#;

        // Search block has different newlines but same token structure
        // RegexMatcher splits by whitespace, so tokens must match
        let search = "if (x > 0) { console.log('test'); }";

        let matcher = RegexMatcher;
        let res = matcher.find_match(file, search, None);

        assert!(res.is_some(), "RegexMatcher should find match with flexible whitespace");
        // Should find match starting at line 3
        assert_eq!(res.unwrap().matched_line_start, 3);
    }
}