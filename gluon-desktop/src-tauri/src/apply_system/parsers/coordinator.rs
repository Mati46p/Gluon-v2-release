//! KROK 10-12: Parser Coordinator with Format Detection and Validation
//!
//! Coordinates the 5 parsers:
//! 1. Try to detect format first (optimization)
//! 2. Try parsers in order: XML G-Protocol -> SEARCH/REPLACE -> Unified Diff -> Markdown -> Pattern Matching
//! 3. Validate results before returning
//!
//! If all fail, return aggregated error with details from each parser

use crate::apply_system::parsers::{
    Parser, git_style_search_replace::GitStyleSearchReplaceParser,
    lazy_stitcher::LazyStitcherParser, markdown::MarkdownParser,
    pattern_matching::PatternMatchingParser, search_replace::SearchReplaceParser,
    unified_diff::UnifiedDiffParser, xml_gprotocol::XmlGProtocolParser,
};
use crate::apply_system::shared::types::{ChangeQueueItem, ParseError};

// ============================================================================
// KROK 11: Format Detection (Pre-Parser Optimization)
// ============================================================================

/// Detected format type from quick analysis
#[derive(Debug, Clone, Copy, PartialEq)]
enum DetectedFormat {
    XmlGProtocol,
    GitStyleSearchReplace, // NEW: Git-style conflict markers & Unicode Box
    SearchReplace,
    UnifiedDiff,
    Markdown,
    LazyStitcher, // NEW: Lazy marker format
    Unknown,
}

/// Quick format detection to optimize parser selection
///
/// This is a fast pre-check that looks for format markers
/// without doing full parsing. Helps us try the most likely
/// parser first instead of always starting from #1.
fn detect_format(text: &str) -> DetectedFormat {
    // Check for Lazy Stitcher format (HIGHEST PRIORITY - new lazy marker system)
    use crate::apply_system::lazy::detector::contains_lazy_markers;
    if contains_lazy_markers(text) {
        return DetectedFormat::LazyStitcher;
    }

    // Check for XML G-Protocol format (designed for Gluon)
    if text.contains("<gluon_patch>") {
        return DetectedFormat::XmlGProtocol;
    }

    // Check for Git-Style SEARCH/REPLACE format (7-char delimiters or Unicode Box)
    if text.contains("<<<<<<< SEARCH")
        || text.contains(">>>>>>> REPLACE")
        || text.contains("╔═══════ SEARCH")
        || text.contains("╠═══════ REPLACE")
        || text.contains("╚═══════ END") {
        return DetectedFormat::GitStyleSearchReplace;
    }

    // Check for SEARCH/REPLACE format markers (4-char delimiters)
    if text.contains("<<<< SEARCH") || text.contains("<<<< EDIT") || text.contains("<<<< CREATE") {
        return DetectedFormat::SearchReplace;
    }

    // Check for unified diff markers
    if text.contains("---") && text.contains("+++") && text.contains("@@") {
        // Very likely unified diff
        if text.lines().any(|l| l.starts_with("diff --git")) {
            return DetectedFormat::UnifiedDiff;
        }
        // Could be diff without git header
        if text
            .lines()
            .filter(|l| l.starts_with("+") || l.starts_with("-"))
            .count()
            > 2
        {
            return DetectedFormat::UnifiedDiff;
        }
    }

    // Check for markdown code blocks with before/after indicators
    if text.contains("```") {
        let lower = text.to_lowercase();
        let has_keywords = (lower.contains("before") && lower.contains("after"))
            || (lower.contains("old") && lower.contains("new"))
            || (lower.contains("current") && lower.contains("proposed"));

        if has_keywords {
            return DetectedFormat::Markdown;
        }
    }

    // No clear format detected
    DetectedFormat::Unknown
}

// ============================================================================
// KROK 10: Parser Coordinator
// ============================================================================

/// Parse model response using all available parsers with fallback
///
/// Process:
/// 1. Detect format (optimization)
/// 2. Try detected parser first
/// 3. If fail, try remaining parsers in order
/// 4. Validate successful results
/// 5. Return validated changes or aggregated error
pub fn parse_with_fallback(raw_response: &str) -> Result<Vec<ChangeQueueItem>, ParseError> {
    // Quick validation
    if raw_response.trim().is_empty() {
        return Err(ParseError::InvalidInput {
            message: "Empty model response".to_string(),
        });
    }

    // Detect format for optimization
    let detected = detect_format(raw_response);

    // Create parsers
    let lazy_stitcher = LazyStitcherParser;
    let xml_gprotocol = XmlGProtocolParser;
    let git_style = GitStyleSearchReplaceParser;
    let search_replace = SearchReplaceParser;
    let unified = UnifiedDiffParser;
    let markdown = MarkdownParser;
    let pattern = PatternMatchingParser;

    // Try parsers based on detected format
    let (result, errors) = match detected {
        DetectedFormat::LazyStitcher => {
            // Try LazyStitcher first (HIGHEST PRIORITY - new system)
            try_parsers_in_order(
                raw_response,
                vec![&lazy_stitcher, &xml_gprotocol, &git_style, &search_replace, &unified, &markdown, &pattern],
            )
        }
        DetectedFormat::XmlGProtocol => {
            // Try XML G-Protocol first (designed specifically for Gluon)
            try_parsers_in_order(
                raw_response,
                vec![&xml_gprotocol, &git_style, &search_replace, &unified, &markdown, &pattern],
            )
        }
        DetectedFormat::GitStyleSearchReplace => {
            // Try Git-Style SEARCH/REPLACE first (NEW STANDARD - Unicode Box & git markers)
            try_parsers_in_order(
                raw_response,
                vec![&git_style, &xml_gprotocol, &search_replace, &unified, &markdown, &pattern],
            )
        }
        DetectedFormat::SearchReplace => {
            // Try SearchReplace first (structured replacement mode)
            try_parsers_in_order(
                raw_response,
                vec![&git_style, &xml_gprotocol, &search_replace, &unified, &markdown, &pattern],
            )
        }
        DetectedFormat::UnifiedDiff => {
            // Try unified first, then others
            try_parsers_in_order(
                raw_response,
                vec![&xml_gprotocol, &git_style, &search_replace, &unified, &markdown, &pattern],
            )
        }
        DetectedFormat::Markdown => {
            // Try markdown first, then others
            try_parsers_in_order(
                raw_response,
                vec![&xml_gprotocol, &git_style, &search_replace, &markdown, &unified, &pattern],
            )
        }
        DetectedFormat::Unknown => {
            // Try all in default order (XML G-Protocol always first as it's most reliable)
            try_parsers_in_order(
                raw_response,
                vec![&xml_gprotocol, &git_style, &search_replace, &unified, &markdown, &pattern],
            )
        }
    };

    match result {
        Some(changes) => {
            // KROK 12: Validate parsed changes before returning
            validate_parsed_changes(changes)
        }
        None => {
            // All parsers failed - return aggregated error
            // Note: errors vector contains results from all parsers in order tried
            Err(ParseError::AllParsersFailed {
                unified_diff_error: errors
                    .iter()
                    .find(|e| e.starts_with("UnifiedDiff"))
                    .cloned()
                    .unwrap_or_else(|| "Not attempted".to_string()),
                markdown_error: errors
                    .iter()
                    .find(|e| e.starts_with("Markdown"))
                    .cloned()
                    .unwrap_or_else(|| "Not attempted".to_string()),
                pattern_error: errors
                    .iter()
                    .find(|e| e.starts_with("PatternMatching") || e.starts_with("SearchReplace"))
                    .cloned()
                    .unwrap_or_else(|| "Not attempted".to_string()),
            })
        }
    }
}

/// Try parsers in the given order, return first success
///
/// Returns: (Option<changes>, Vec<error_messages>)
fn try_parsers_in_order(
    raw_response: &str,
    parsers: Vec<&dyn Parser>,
) -> (Option<Vec<ChangeQueueItem>>, Vec<String>) {
    let mut errors = Vec::new();

    for parser in parsers {
        // Quick check if this parser can handle the format
        if !parser.can_handle(raw_response) {
            errors.push(format!("{} parser: format not recognized", parser.name()));
            continue;
        }

        // Try to parse
        match parser.parse(raw_response) {
            Ok(changes) => {
                crate::gluon_info!("ParserCoordinator", "Successfully parsed with {} parser", parser.name());
                return (Some(changes), errors);
            }
            Err(e) => {
                crate::gluon_warn!("ParserCoordinator", "{} parser failed: {}", parser.name(), e);
                errors.push(format!("{} parser: {}", parser.name(), e));
                
                // FAIL FAST OPTIMIZATION:
                // If a high-confidence structured parser (XML or SEARCH/REPLACE) detects its markers
                // but fails parsing (e.g. syntax error), do NOT fallback to fuzzy parsers (Markdown/Pattern).
                // It's better to return the specific XML error than a confusing fallback result.
                if parser.name().contains("XmlGProtocol") || parser.name().contains("SearchReplace") {
                    crate::gluon_warn!("ParserCoordinator", "High-confidence parser {} failed. Aborting fallback chain to preserve error context.", parser.name());
                    return (None, errors);
                }
            }
        }
    }

    (None, errors)
}

// ============================================================================
// KROK 12: Validation of Parsed Changes
// ============================================================================

/// Validate parsed changes before accepting them
///
/// Checks:
/// - File paths are not empty
/// - old_code and new_code are different
/// - Line numbers are valid (start <= end)
/// - No path traversal attacks
/// - File extensions are supported
fn validate_parsed_changes(
    changes: Vec<ChangeQueueItem>,
) -> Result<Vec<ChangeQueueItem>, ParseError> {
    let mut validated = Vec::new();

    for change in changes {
        // Validate file path is not empty
        if change.file_path.trim().is_empty() {
            return Err(ParseError::ValidationFailed {
                reason: "File path is empty".to_string(),
            });
        }

        // Validate old_code and new_code are different
        if change.old_code.trim() == change.new_code.trim() {
            return Err(ParseError::ValidationFailed {
                reason: format!(
                    "old_code and new_code are identical in {}",
                    change.file_path
                ),
            });
        }

        // Validate line numbers
        if change.line_start > change.line_end && change.line_end != 0 {
            return Err(ParseError::ValidationFailed {
                reason: format!(
                    "Invalid line range: {} > {} in {}",
                    change.line_start, change.line_end, change.file_path
                ),
            });
        }

        // Validate no path traversal
        if change.file_path.contains("../") || change.file_path.contains("..\\") {
            return Err(ParseError::ValidationFailed {
                reason: format!("Path traversal detected in {}", change.file_path),
            });
        }

        // Validate file extension is supported
        if !has_supported_extension(&change.file_path) {
            crate::gluon_warn!(
                "ParserCoordinator",
                "Unusual file extension in {}, but accepting",
                change.file_path
            );
            // Don't fail - just warn. Pattern matching might pick up weird extensions
        }

        validated.push(change);
    }

    if validated.is_empty() {
        return Err(ParseError::ValidationFailed {
            reason: "No valid changes after validation".to_string(),
        });
    }

    Ok(validated)
}

/// Check if file has a supported extension
fn has_supported_extension(path: &str) -> bool {
    let supported = vec![
        ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".py", ".pyw", ".rs", ".go", ".java", ".cpp",
        ".cc", ".cxx", ".c", ".h", ".hpp", ".cs", ".rb", ".php", ".swift", ".kt", ".kts", ".scala",
        ".html", ".htm", ".css", ".scss", ".sass", ".less", ".json", ".yaml", ".yml", ".toml",
        ".xml", ".md", ".txt",
    ];

    supported.iter().any(|ext| path.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_format_unified_diff() {
        let text = r#"
--- a/src/test.ts
+++ b/src/test.ts
@@ -1,3 +1,3 @@
-old
+new
"#;
        assert_eq!(detect_format(text), DetectedFormat::UnifiedDiff);
    }

    #[test]
    fn test_detect_format_markdown() {
        let text = r#"
Before:
```typescript
old code
```

After:
```typescript
new code
```
"#;
        assert_eq!(detect_format(text), DetectedFormat::Markdown);
    }

    #[test]
    fn test_detect_format_unknown() {
        let text = "just some random text";
        assert_eq!(detect_format(text), DetectedFormat::Unknown);
    }

    #[test]
    fn test_validate_empty_path() {
        let change =
            ChangeQueueItem::new("".to_string(), 1, 2, "old".to_string(), "new".to_string());

        let result = validate_parsed_changes(vec![change]);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identical_code() {
        let change = ChangeQueueItem::new(
            "test.ts".to_string(),
            1,
            2,
            "same code".to_string(),
            "same code".to_string(),
        );

        let result = validate_parsed_changes(vec![change]);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_path_traversal() {
        let change = ChangeQueueItem::new(
            "../../../etc/passwd".to_string(),
            1,
            2,
            "old".to_string(),
            "new".to_string(),
        );

        let result = validate_parsed_changes(vec![change]);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_valid_change() {
        let change = ChangeQueueItem::new(
            "src/test.ts".to_string(),
            10,
            15,
            "old code".to_string(),
            "new code".to_string(),
        );

        let result = validate_parsed_changes(vec![change]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_with_fallback_unified() {
        let text = r#"
--- a/src/test.ts
+++ b/src/test.ts
@@ -1,2 +1,2 @@
-old line
+new line
"#;

        let result = parse_with_fallback(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_parse_with_fallback_markdown() {
        let text = r#"
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

        let result = parse_with_fallback(text);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_with_fallback_empty() {
        let result = parse_with_fallback("");
        assert!(result.is_err());
    }
}
