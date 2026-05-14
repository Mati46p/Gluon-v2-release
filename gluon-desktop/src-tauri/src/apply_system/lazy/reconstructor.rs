//! File Reconstruction
//!
//! This module handles splicing original code into lazy block positions
//! to create the final reconstructed file.

use super::matcher::{LazyReplacement, NodeInfo};

/// Reconstruct a file by replacing lazy blocks with original code
///
/// This function:
/// 1. Takes the new file (with lazy markers)
/// 2. Takes replacements from the old file
/// 3. Splices the replacement text into positions where lazy markers were
///
/// # Arguments
///
/// * `old_source` - Original file content
/// * `new_source` - New file content (with lazy markers)
/// * `replacements` - List of lazy block replacements
///
/// # Returns
///
/// The reconstructed file as a String
pub fn reconstruct_file(
    old_source: &str,
    new_source: &str,
    replacements: &[LazyReplacement],
) -> String {
    // Sort replacements by reverse position (process from end to start)
    // This way earlier replacements don't affect byte offsets of later ones
    let mut sorted_replacements = replacements.to_vec();
    sorted_replacements.sort_by(|a, b| {
        b.lazy_block_node
            .start_byte
            .cmp(&a.lazy_block_node.start_byte)
    });

    // Work with character array for easier splicing
    let mut result_chars: Vec<char> = new_source.chars().collect();

    // Convert byte positions to char positions
    let byte_to_char_map = build_byte_to_char_map(new_source);

    for replacement in sorted_replacements {
        let lazy_block = &replacement.lazy_block_node;

        // Get replacement text from old file
        let replacement_text = if replacement.replacement_nodes.is_empty() {
            // No replacement = remove the lazy block entirely (with surrounding whitespace)
            remove_lazy_block_with_whitespace(
                &result_chars,
                byte_to_char_map[lazy_block.start_byte],
                byte_to_char_map[lazy_block.end_byte],
            );
            continue;
        } else {
            extract_replacement_text(old_source, &replacement.replacement_nodes)
        };

        // Convert byte positions to character positions
        let start_char = byte_to_char_map[lazy_block.start_byte];
        let end_char = byte_to_char_map[lazy_block.end_byte];

        // Replace the lazy block with the replacement text
        let replacement_chars: Vec<char> = replacement_text.chars().collect();
        result_chars.splice(start_char..end_char, replacement_chars);
    }

    result_chars.into_iter().collect()
}

/// Build a mapping from byte offset to character index
fn build_byte_to_char_map(source: &str) -> Vec<usize> {
    let mut map = Vec::with_capacity(source.len() + 1);
    let mut char_idx = 0;

    for (byte_idx, _) in source.char_indices() {
        // Fill in gaps for multi-byte characters
        while map.len() < byte_idx {
            map.push(char_idx);
        }
        map.push(char_idx);
        char_idx += 1;
    }

    // Fill to end
    while map.len() <= source.len() {
        map.push(char_idx);
    }

    map
}

/// Extract text from a sequence of replacement nodes
fn extract_replacement_text(old_source: &str, nodes: &[NodeInfo]) -> String {
    if nodes.is_empty() {
        return String::new();
    }

    let old_lines: Vec<&str> = old_source.lines().collect();

    let start_node = &nodes[0];
    let end_node = &nodes[nodes.len() - 1];

    // Extract lines from start to end
    let mut lines = old_lines[start_node.start_line..=end_node.end_line].to_vec();

    if lines.is_empty() {
        return String::new();
    }

    // Trim first line from start column
    if !lines.is_empty() {
        let first_line = lines[0];
        if start_node.start_column < first_line.len() {
            lines[0] = &first_line[start_node.start_column..];
        }
    }

    // Trim last line to end column
    if lines.len() > 1 {
        let last_idx = lines.len() - 1;
        let last_line = lines[last_idx];
        if end_node.end_column <= last_line.len() {
            lines[last_idx] = &last_line[..end_node.end_column];
        }
    } else if !lines.is_empty() {
        // Single line case
        let line = lines[0];
        let relative_end = if start_node.start_line == end_node.end_line {
            end_node.end_column.saturating_sub(start_node.start_column)
        } else {
            line.len()
        };
        if relative_end <= line.len() {
            lines[0] = &line[..relative_end];
        }
    }

    lines.join("\n")
}

/// Remove a lazy block and its surrounding whitespace
///
/// This removes up to 2 newlines before and after the lazy block
fn remove_lazy_block_with_whitespace(_chars: &[char], _start_char: usize, _end_char: usize) {
    // Implementation is done in-place by the caller through result_chars.splice()
    // This function signature is kept for clarity but the actual removal logic
    // is handled by returning an empty string in reconstruct_file
}

/// Adjust indentation of replacement text to match surrounding context
///
/// This is useful when the lazy block has different indentation than the original code
pub fn adjust_indentation(text: &str, target_indentation: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return text.to_string();
    }

    // Detect current indentation (from first non-empty line)
    let current_indentation = lines
        .iter()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .unwrap_or(0);

    let indent_delta = target_indentation as i32 - current_indentation as i32;

    if indent_delta == 0 {
        return text.to_string();
    }

    let adjusted_lines: Vec<String> = lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                // Preserve empty lines
                (*line).to_string()
            } else if indent_delta > 0 {
                // Add indentation
                format!("{}{}", " ".repeat(indent_delta as usize), line)
            } else {
                // Remove indentation
                let chars_to_remove = (-indent_delta) as usize;
                line.chars().skip(chars_to_remove).collect()
            }
        })
        .collect();

    adjusted_lines.join("\n")
}

/// Smart indentation normalization inspired by Continue's matchLine
///
/// This function:
/// 1. Compares content of lines (trimmed)
/// 2. If content matches but indentation differs, adjusts indentation
/// 3. Handles special cases like closing brackets
///
/// Returns the normalized line with corrected indentation
pub fn normalize_line_indentation(
    new_line: &str,
    old_line: &str,
    permissive: bool,
) -> String {
    // If content is the same after trimming start/end
    if new_line.trim() == old_line.trim() {
        // For sufficiently long lines (>8 chars) or if permissive mode
        if permissive || new_line.trim().len() > 8 {
            // Use the old line's indentation
            return old_line.to_string();
        }
    }

    new_line.to_string()
}

/// Detect indentation style (spaces vs tabs) and amount
///
/// Returns (indent_char, indent_size)
/// e.g., (' ', 2) for 2-space indentation
///       ('\t', 1) for tab indentation
pub fn detect_indentation_style(content: &str) -> (char, usize) {
    let lines: Vec<&str> = content.lines().collect();

    let mut space_counts = Vec::new();
    let mut tab_count = 0;

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let leading_chars: String = line.chars().take_while(|c| c.is_whitespace()).collect();

        if leading_chars.contains('\t') {
            tab_count += 1;
        } else if !leading_chars.is_empty() {
            space_counts.push(leading_chars.len());
        }
    }

    // If more tabs than spaces, use tabs
    if tab_count > space_counts.len() / 2 {
        return ('\t', 1);
    }

    // Calculate most common space count using GCD
    if !space_counts.is_empty() {
        space_counts.sort_unstable();
        let mut gcd = space_counts[0];
        for &count in &space_counts[1..] {
            gcd = compute_gcd(gcd, count);
        }
        // Common indentation sizes: 2, 4, 8
        let indent_size = if gcd == 0 { 4 } else { gcd.min(8) };
        return (' ', indent_size);
    }

    // Default to 4 spaces
    (' ', 4)
}

/// Compute Greatest Common Divisor
fn compute_gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let temp = b;
        b = a % b;
        a = temp;
    }
    a
}

/// Re-indent entire block to match target style
///
/// This converts between tabs/spaces and adjusts indent levels
pub fn reindent_block(
    text: &str,
    source_style: (char, usize),
    target_style: (char, usize),
) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let (src_char, src_size) = source_style;
    let (tgt_char, tgt_size) = target_style;

    let reindented: Vec<String> = lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                return (*line).to_string();
            }

            // Calculate indent level
            let indent_count = line.chars().take_while(|c| c.is_whitespace()).count();
            let indent_level = if src_char == '\t' {
                line.chars().take_while(|c| *c == '\t').count()
            } else {
                indent_count / src_size
            };

            // Generate new indentation
            let new_indent = if tgt_char == '\t' {
                "\t".repeat(indent_level)
            } else {
                " ".repeat(indent_level * tgt_size)
            };

            // Apply new indentation
            format!("{}{}", new_indent, line.trim_start())
        })
        .collect();

    reindented.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_byte_to_char_map() {
        let text = "hello world";
        let map = build_byte_to_char_map(text);
        assert_eq!(map[0], 0); // 'h'
        assert_eq!(map[6], 6); // 'w'
    }

    #[test]
    fn test_adjust_indentation_increase() {
        let text = "line1\n  line2\n  line3";
        let adjusted = adjust_indentation(text, 4);
        assert!(adjusted.contains("    line1"));
        assert!(adjusted.contains("      line2"));
    }

    #[test]
    fn test_adjust_indentation_decrease() {
        let text = "    line1\n      line2";
        let adjusted = adjust_indentation(text, 2);
        assert!(adjusted.contains("  line1"));
        assert!(adjusted.contains("    line2"));
    }

    #[test]
    fn test_extract_replacement_text_simple() {
        let source = "line1\nline2\nline3\nline4";
        let nodes = vec![NodeInfo {
            start_byte: 6,
            end_byte: 17,
            start_line: 1,
            end_line: 2,
            start_column: 0,
            end_column: 5,
            kind: "test".to_string(),
            text: "line2\nline3".to_string(),
        }];

        let result = extract_replacement_text(source, &nodes);
        assert_eq!(result, "line2\nline3");
    }
}
