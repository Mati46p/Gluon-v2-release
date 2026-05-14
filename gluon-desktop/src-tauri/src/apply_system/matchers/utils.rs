//! KROK 2.3: Matcher Utilities
//! 
//! Provides shared logic for:
//! - Smart Scope Expansion (finding closing braces)
//! - Indentation Normalization (fixing nested code blocks)
//! - Content Sanitization (HTML entity decoding)
 
// [GLUON DEPENDENCY] Ensure tree_sitter is available for AST utils
use tree_sitter;
 
/// Decodes HTML entities often found in LLM responses (e.g. < instead of <).
pub fn decode_html_entities(content: &str) -> String {
    content
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Normalizes indentation of a code block.
///
/// Detects the common leading whitespace (indentation) across all lines
/// and removes it.
#[allow(dead_code)]
pub fn normalize_block_indentation(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    // Find the minimum indentation level (ignoring empty lines)
    let min_indent = lines.iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.len() - line.trim_start().len()
        })
        .min()
        .unwrap_or(0);

    if min_indent == 0 {
        return content.to_string();
    }

    // Reconstruct string with stripped indentation
    lines.iter()
        .map(|line| {
            if line.len() >= min_indent {
                &line[min_indent..]
            } else {
                line.trim()
            }
        })
        .collect::<Vec<&str>>()
        .join("\n")
}
 
/// [GLUON V5 - ROBUST] Relative Indentation Engine
///
/// Inspired by Aider's RelativeIndenter. Instead of simple delta calculation on the first line,
/// this engine calculates the "Indentation Shape" of the new block and projects it onto the 
/// target file's anchor indentation.
///
/// Fixes:
/// - Indentation Drift (when model returns code with 0 indent)
/// - Relative block structure preservation
pub fn smart_adjust_indentation(
    file_content: &str,
    matched_start_line: usize, // 1-based line number in file
    new_code: &str
) -> String {
    let file_lines: Vec<&str> = file_content.lines().collect();
    let new_lines: Vec<&str> = new_code.lines().collect();
 
    if matched_start_line == 0 || new_lines.is_empty() {
        return new_code.to_string();
    }
 
    // Helper: Calculate visual width (Tab = 4 spaces)
    let get_width = |line: &str| -> usize {
        let mut width = 0;
        for c in line.chars() {
            match c {
                ' ' => width += 1,
                '\t' => width += 4, // Assume standard tab width
                _ => break, // Stop at first non-whitespace
            }
        }
        width
    };
 
    // 1. Determine Anchor Indentation (Absolute Truth from File)
    // We look at the line being replaced to establish the "Ground Truth" indentation.
    let start_idx = matched_start_line - 1;
    let mut anchor_indent = 0;
    
    // [Strategy A] AST-Based (Most reliable for empty lines/broken context)
    // We try this, but if it returns 0 and we look like we are inside a block (Strategy C), we might override it.
    let mut ast_indent_opt = None;
    if let Some(val) = calculate_ast_based_indentation(file_content, matched_start_line) {
        ast_indent_opt = Some(val);
        anchor_indent = val;
    }

    // [Strategy B] Visual Inspection (Absolute Truth of existing line)
    // If the line we are replacing has content, its indentation is the best anchor.
    let mut visual_indent_opt = None;
    if start_idx < file_lines.len() {
        let target_line = file_lines[start_idx];
        if !target_line.trim().is_empty() {
             let w = get_width(target_line);
             visual_indent_opt = Some(w);
             anchor_indent = w;
        }
    }

    // [Strategy C] Contextual Inference (Look Upwards)
    // [GLUON V7.1 FIX] Used ONLY if Strategy B failed (empty line). Never override absolute visual truth.
    if visual_indent_opt.is_none() {
        for i in (0..start_idx).rev().take(50) {
            let line = file_lines[i];
            if !line.trim().is_empty() {
                let prev_indent = get_width(line);
                let trimmed = line.trim();
                
                let probable_indent = if trimmed.ends_with(':') || trimmed.ends_with('{') || trimmed.ends_with('(') {
                    prev_indent + 4
                } else {
                    prev_indent
                };
                anchor_indent = probable_indent;
                break;
            }
        }
    }
 
    // 2. Determine Source Baseline (First line indent of the new block)
    // [GLUON FIX] Use FIRST line, not minimum. Using .min() caused issues when
    // a block contained lines at different structural levels (e.g., method body + next method definition).
    // The minimum would incorrectly anchor to the outer definition, causing inner lines to over-indent.
    let source_baseline = new_lines.iter()
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("//")
        })
        .map(|l| get_width(l))
        .next() // Take the FIRST valid line, not the minimum
        .unwrap_or(0);
 
    // 3. Project New Code onto Anchor
    let mut final_lines = Vec::new();
    
    // [GLUON V5.1] Indentation Style Detection
    // Check if the file primarily uses tabs or spaces for indentation
    let use_tabs = file_lines.iter().take(50).any(|l| l.starts_with('\t'));
    
    for line in &new_lines {
        if line.trim().is_empty() {
            final_lines.push(String::new());
            continue;
        }
 
        let current_visual_width = get_width(line);
        
        // Calculate how much this line is indented relative to the block's baseline
        // e.g. if baseline is 4, and line is 8, relative is +4.
        // We use isize to be safe against weird negative edge cases.
        let relative_offset = current_visual_width as isize - source_baseline as isize;
        
        // Calculate new absolute indent: Anchor + Relative Offset
        let new_absolute_width = (anchor_indent as isize + relative_offset).max(0) as usize;
        
        let new_indent_str = if use_tabs {
            // Assume 1 tab = 4 spaces for conversion, but output real tabs
            let tabs_count = new_absolute_width / 4;
            "\t".repeat(tabs_count)
        } else {
            " ".repeat(new_absolute_width)
        };
 
        let content = line.trim_start();
        final_lines.push(format!("{}{}", new_indent_str, content));
    }
 
    final_lines.join("\n")
}

/// [GLUON V5 - ROBUST] Look-behind Context Expansion
/// 
/// Checks if the new code starts with headers (decorators, definitions) that already exist
/// immediately before the match point in the file. If so, expands the match backwards
/// to include them, preventing duplication.
pub fn expand_context_backwards(
    file_content: &str,
    matched_start_line: usize,
    new_code: &str
) -> usize {
    if matched_start_line <= 1 { return matched_start_line; }
    
    let file_lines: Vec<&str> = file_content.lines().collect();
    let new_lines: Vec<&str> = new_code.lines().collect();
    
    if new_lines.is_empty() { return matched_start_line; }
    
    // We check the first few lines of new code for structural headers
    // Examples: @decorator, def foo(), class Bar
    let mut current_file_idx = matched_start_line - 1; // 0-based index of match start
    let mut new_code_idx = 0;
    
    // Limits for look-behind to prevent runaway recursion
    let look_behind_limit = 5; 
    let mut expanded_lines = 0;
 
    while new_code_idx < new_lines.len() && expanded_lines < look_behind_limit {
        if current_file_idx == 0 { break; }
        
        let file_line_above = file_lines[current_file_idx - 1]; // Line BEFORE the current match
        let new_code_line = new_lines[new_code_idx];
        
        // Normalize for comparison (ignore whitespace differences)
        let f_trim = file_line_above.trim();
        let n_trim = new_code_line.trim();
        
        // Check if lines are identical (IGNORING WHITESPACE) and look like headers/decorators
        // We compare filtered strings to handle indentation mismatches (e.g. file has indent, model has none)
        let f_clean: String = f_trim.chars().filter(|c| !c.is_whitespace()).collect();
        let n_clean: String = n_trim.chars().filter(|c| !c.is_whitespace()).collect();

        if !f_clean.is_empty() && f_clean == n_clean {
            let is_header = f_trim.starts_with('@') ||
                          f_trim.starts_with("def ") ||     // Python function (with space)
                          f_trim.starts_with("async def ") || // Python async function
                          f_trim.starts_with("class ") ||   // Python/other class (with space)
                          f_trim.starts_with("pub fn") ||   // Rust public function
                          f_trim.starts_with("fn ") ||      // Rust function
                          f_trim.starts_with("function ");  // JavaScript/TypeScript function
            if is_header || f_clean.starts_with('[') { // Also catch Attributes in C#/Rust
                // [GLUON V7 FIX] Strict Indentation Check for Headers
                let f_indent = file_line_above.chars().take_while(|c| c.is_whitespace()).count();
                let n_indent = new_code_line.chars().take_while(|c| c.is_whitespace()).count();
                
                if f_indent == n_indent {
                    // FOUND DUPLICATION PATTERN!
                    // The line above the match in file is identical to the line starting the new code.
                    // We should expand the match upwards to encompass (and overwrite) this line.
                    if current_file_idx > 0 {
                        current_file_idx -= 1;
                        new_code_idx += 1;
                        expanded_lines += 1;
                        continue;
                    }
                }
            }
        }
        
        // If we didn't match, or it wasn't a header, stop looking.
        break;
    }
    
    // Return new 1-based start line
    current_file_idx + 1
}

/// [GLUON V6 - ROBUST] Look-forward Context Expansion
///
/// Checks if the new code ends with lines (closures, returns, etc.) that already exist
/// immediately after the match point in the file. If so, expands the match forwards
/// to include them, preventing duplication and "code appending" bugs.
///
/// This is the mirror function of `expand_context_backwards()`.
///
/// # Arguments
/// * `file_content` - Full file content
/// * `matched_end_line` - Current end line of the match (1-based, exclusive)
/// * `new_code` - The replacement code block
///
/// # Returns
/// * Adjusted end line (1-based, exclusive) that includes duplicate context
pub fn expand_context_forward(
    file_content: &str,
    matched_end_line: usize,
    new_code: &str
) -> usize {
    let file_lines: Vec<&str> = file_content.lines().collect();
    let new_lines: Vec<&str> = new_code.lines().collect();

    if new_lines.is_empty() || matched_end_line >= file_lines.len() {
        return matched_end_line;
    }

    // Convert 1-based exclusive end to 0-based index of first line AFTER match
    let mut current_file_idx = matched_end_line; // 0-based index after match
    let mut new_code_rev_idx = new_lines.len(); // Start from end of new code

    let look_forward_limit = 5;
    let mut expanded_lines = 0;

    while new_code_rev_idx > 0 && current_file_idx < file_lines.len() && expanded_lines < look_forward_limit {
        let file_line_below = file_lines[current_file_idx];
        let new_code_line = new_lines[new_code_rev_idx - 1];

        // Normalize for comparison (ignore whitespace differences)
        let f_trim = file_line_below.trim();
        let n_trim = new_code_line.trim();

        let f_clean: String = f_trim.chars().filter(|c| !c.is_whitespace()).collect();
        let n_clean: String = n_trim.chars().filter(|c| !c.is_whitespace()).collect();

        if !f_clean.is_empty() && f_clean == n_clean {
            // Check if this looks like a structural closure/footer
            // [GLUON FIX] Improved footer detection - must be actual structural element
            let is_footer = f_trim == "}" ||
                          f_trim == "}," ||
                          f_trim == ");" ||
                          f_trim == "});" ||
                          f_trim.starts_with("return ") ||
                          f_trim == "return" ||
                          f_trim.starts_with("def ") ||  // Next function definition
                          f_trim.starts_with("class ");  // Next class definition
            if is_footer {
                // [GLUON V7 FIX] Strict Indentation Check for Closures
                // Prevents JSX / React component shredding when closing tags match by content but not by depth.
                let f_indent = file_line_below.chars().take_while(|c| c.is_whitespace()).count();
                let n_indent = new_code_line.chars().take_while(|c| c.is_whitespace()).count();
                
                let is_structural_closure = f_trim == "}" || f_trim == "}," || f_trim == ");" || f_trim == "});";
                
                if is_structural_closure && f_indent != n_indent {
                    // Do not expand if indents don't match for structural closures
                    break;
                }
                // FOUND DUPLICATION PATTERN!
                // Line after match in file is identical to line at end of new code
                // Expand match downwards to encompass (and overwrite) this line
                current_file_idx += 1;
                new_code_rev_idx -= 1;
                expanded_lines += 1;
                continue;
            }
        }

        // If we didn't match, stop looking
        break;
    }

    // Return new 1-based end line (exclusive)
    current_file_idx
}

/// [GLUON HARDENING] AST-Based Target Indentation
/// Calculates the correct indentation level for a line by inspecting its parent container in the AST.
/// This prevents "Indentation Drift" where code is pasted with 0 indentation because the previous line was empty.
pub fn calculate_ast_based_indentation(file_content: &str, line_num: usize) -> Option<usize> {
    // Only for languages where structure implies indentation (Python, Rust, etc)
    let is_struct_lang = file_content.contains("class ") || file_content.contains("def ") || file_content.contains("fn ");
    if !is_struct_lang {
        return None;
    }
 
    // Attempt parse with generic heuristic if file extension unknown
    let lang_hint = if file_content.contains("def ") { "dummy.py" } else { "dummy.rs" };
    
    if let Ok(tree) = crate::apply_system::analysis::AnalysisEngine::parse(file_content, lang_hint) {
        let root = tree.root_node();
        if line_num == 0 { return Some(0); }
        
        // Convert 1-based line to 0-based row
        let target_row = line_num.saturating_sub(1);
        let point = tree_sitter::Point { row: target_row, column: 0 };
        
        // Find the deepest node at the beginning of the line
        let mut node = root;
        let mut depth_safeguard = 0;
        // [GLUON DEBUG] Safety loop limit
        while let Some(child) = node.named_descendant_for_point_range(point, point) {
             // Loop safety check: ensure we are actually descending
             if child.id() == node.id() { break; } 
             node = child;
             depth_safeguard += 1;
             if depth_safeguard > 100 { 
                 crate::gluon_warn!("MatcherUtils", "Tree-sitter descent depth exceeded 100. Breaking.");
                 break; 
             }
        }
        
        // Walk up to find a semantic container
        let mut curr = Some(node);
        while let Some(n) = curr {
            let kind = n.kind();
            let start_row = n.start_position().row;
 
            // [GLUON FIX] Smart Indentation Context
            // Check if the matched line IS the definition header itself.
            // If we are replacing "def foo():", we want the indentation of "def", not "def" + 4.
            if start_row == target_row {
                if kind == "class_definition" || kind == "function_definition" || kind == "impl_item" {
                    return Some(n.start_position().column);
                }
            }
 
            // List of containers that imply +1 indentation level for their BODY
            if kind == "class_definition" || kind == "function_definition" || kind == "impl_item" || kind == "mod_item" {
                // We are inside the body of this container. Base indent is container's start + 4 spaces.
                let base = n.start_position().column;
                return Some(base + 4);
            }
            curr = n.parent();
        }
        
        return Some(0); // Top level
    }
    None
}
 
/// [GLUON INTELLIGENCE] Detects if a code block is a partial fragment vs a complete unit.
/// Used to decide if we should aggressively expand the replacement scope to the whole function.
pub fn is_partial_fragment(code: &str) -> bool {
    let trimmed = code.trim();
    if trimmed.is_empty() { return false; }
 
    // Python heuristics
    if trimmed.ends_with(':') { return true; } // "def foo():" -> Header only
    
    // Check structural braces balance (if code opens { but doesn't close, it's partial)
    if count_structural_braces(code) > 0 { return true; }
 
    // Indentation heuristic: if the last line is indented deeper than the first line,
    // it implies the block continues.
    let lines: Vec<&str> = code.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() >= 2 {
        let first_indent = lines.first().map(|l| l.len() - l.trim_start().len()).unwrap_or(0);
        let last_indent = lines.last().map(|l| l.len() - l.trim_start().len()).unwrap_or(0);
        if last_indent > first_indent {
            return true;
        }
    }
 
    false
}

/// Smart Scope Expansion (AST-Enhanced)
///
/// Uses Tree-sitter to find the *true* semantic end of a block.
/// Falls back to brace/indentation counting only if parsing fails.
pub fn expand_block_scope(file_content: &str, start_line: usize, original_end_line: usize) -> usize {
    // [GLUON V3 - FORTRESS] Surgical Scope Resolution
    // Instead of trusting indentation (which fails on empty lines/comments),
    // we query the AST for the exact byte-range of the node starting at `start_line`.
    
    // Heuristic: determine language from context
    let is_python = file_content.contains("def ") || file_content.contains("class ") || file_content.contains("import ");
    let dummy_path = if is_python { "dummy.py" } else { "dummy.rs" };
 
    if let Ok(tree) = crate::apply_system::analysis::AnalysisEngine::parse(file_content, dummy_path) {
        let root = tree.root_node();
        let target_byte = get_byte_offset(file_content, start_line);
        
        // Strategy: Find the most specific "Block Node" that starts near our target line.
        let mut cursor = root.walk();
        let mut best_node = root;
        let mut found_exact = false;
 
        'outer: loop {
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    let start = child.start_byte();
                    let end = child.end_byte();
                    
                    // Does this node contain our start line?
                    if target_byte >= start && target_byte < end {
                        
                        // Check if this node STARTS on or near our target line.
                        // Allow variance for decorators (e.g. @action line vs def line).
                        let node_start_row = child.start_position().row;
                        let target_row = start_line.saturating_sub(1);
                        
                        // Check intersection or proximity
                        if node_start_row <= target_row && target_row <= child.end_position().row {
                            best_node = child;
                            
                            let kind = child.kind();
                            // List of nodes that constitute "Blocks" we want to fully replace
                            if kind == "function_definition" || kind == "class_definition" 
                               || kind == "decorated_definition" || kind == "impl_item" 
                               || kind == "function_item" {
                                
                                // Check if the definition actually starts here (precision check)
                                // We accept if the node starts within 2 lines of target (handling decorators)
                                if node_start_row.abs_diff(target_row) <= 2 {
                                    found_exact = true;
                                    // Stop going deeper - we found the container we likely want to replace.
                                    // If we go deeper, we might select just the `name` node or `parameters` node.
                                    break 'outer;
                                }
                            }
                            
                            continue 'outer; // Go deeper to find more specific match
                        }
                    }
                    
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
                cursor.goto_parent();
            }
            break;
        }
        
        if found_exact {
            let end_row = best_node.end_position().row; // 0-based
            // Return 1-based line number AFTER the block.
            // Tree-sitter end_position is inclusive of the last char.
            // So if function ends on line 10, end_row is 9. We want to return 11.
            return end_row + 2; 
        }
    }
 
    // --- FALLBACK TO HEURISTICS (Existing Code) ---
    let lines: Vec<&str> = file_content.lines().collect();
 
    if start_line == 0 || start_line > lines.len() {
        return original_end_line;
    }
 
    let start_idx = start_line - 1;
    let start_line_text = lines[start_idx].trim();

    // Heuristic: Check if this looks like a block definition
    let is_block_def = start_line_text.contains("fn ") ||
                       start_line_text.contains("function ") ||
                       start_line_text.contains("class ") ||
                       start_line_text.contains("struct ") ||
                       start_line_text.contains("impl ") ||
                       start_line_text.contains("pub ") ||
                       start_line_text.contains("interface ") ||
                       start_line_text.contains("mod ") ||
                       (start_line_text.contains("const ") && start_line_text.contains("=>")) ||
                       start_line_text.contains("def ");

    if !is_block_def || (!start_line_text.contains('{') && !start_line_text.ends_with(':')) {
        return original_end_line;
    }

    // Python handling (Robust Multi-line support)
    // Check if this looks like a definition (def/class/async def) even if multi-line
    if start_line_text.contains("def ") || start_line_text.contains("class ") || start_line_text.contains("try:") || start_line_text.contains("if ") {
        // 1. Find the colon that ends the definition header
        let mut colon_line_idx = start_idx;
        let mut found_colon = false;
 
        if start_line_text.trim().ends_with(':') {
            found_colon = true;
        } else {
            // Scan forward for colon (max 20 lines to avoid runaway)
            for i in start_idx..lines.len().min(start_idx + 20) {
                if lines[i].trim().ends_with(':') {
                    colon_line_idx = i;
                    found_colon = true;
                    break;
                }
            }
        }
 
        // 2. Scan body indentation
        if found_colon {
            let base_indent = lines[start_idx].len() - lines[start_idx].trim_start().len();
            
            // Scan from the line AFTER the colon
            for (i, line) in lines.iter().enumerate().skip(colon_line_idx + 1) {
                if !line.trim().is_empty() {
                    let current_indent = line.len() - line.trim_start().len();
                    // If indentation drops to (or below) base level, block has ended
                    if current_indent <= base_indent {
                        return i + 1; // Return 1-based line number of the start of NEXT block
                    }
                }
            }
            return lines.len() + 1; // End of file
        }
    }

    // [FIX 5] ENHANCED C-STYLE BRACE COUNTING (With JSON Support)
    let mut brace_balance = 0;   // {}
    let mut bracket_balance = 0; // []
    let mut found_structure = false;
    let mut real_end_idx = start_idx;
    // [GLUON V8 FIX] Hard limit scope expansion to prevent runaway parsing in JSX/React
    let max_scan_lines = 40; // Don't scan more than 40 lines forward to find closing brackets
    // Scan forward to find the logical end of the block/structure
    for (i, line) in lines.iter().enumerate().skip(start_idx) {
        if i.saturating_sub(start_idx) > max_scan_lines {
            // Failsafe: Runaway structure expansion detected. Stop scanning.
            crate::gluon_warn!("MatcherUtils", "Scope expansion aborted: limit of {} lines reached. Preventing runaway deletion in JSX/TSX.", max_scan_lines);
            return original_end_line;
        }
        // Simple char counting is safer/faster here than full State Machine for just balance
        // We only care about root-level balance
        for c in line.chars() {
            match c {
                '{' => { brace_balance += 1; found_structure = true; }
                '}' => { brace_balance -= 1; }
                '[' => { bracket_balance += 1; found_structure = true; }
                ']' => { bracket_balance -= 1; }
                _ => {}
            }
        }

        // Check if we just closed a root structure
        if found_structure {
            if brace_balance == 0 && bracket_balance == 0 {
                // Check if the line ends with common continuations (like comma in JSON/Rust)
                let trimmed = line.trim();
                if !trimmed.ends_with(',') && !trimmed.ends_with('.') {
                    real_end_idx = i;
                    break;
                }
                // If it ends with comma, we might be inside a larger list, but this object is done.
                // However, we usually want to include the comma if it's part of the line.
                // Let's assume the end of the line closing the structure is the end.
                real_end_idx = i; 
                // We keep scanning if balance is still positive (nested), but here balance is 0.
                // So we break.
                break; 
            }
        }

        // Safety: negative balance usually means we started in the middle or matched wrong
        if brace_balance < 0 || bracket_balance < 0 {
            // [GLUON LOG] Negative balance detected, aborting scope expansion
            return original_end_line;
        }
    }

    if found_structure {
        let calculated_end_line = real_end_idx + 1;
        // Only expand, never shrink below original estimation
        return calculated_end_line.max(original_end_line);
    }

    original_end_line
}
 
/// [GLUON V4] Scope Hoisting (AST + Heuristic)
///
/// Moves the start line UP to the beginning of the logical container (function/class)
/// to ensure we replace the WHOLE unit, not just a fragment inside 'try/except'.
pub fn hoist_scope_to_definition(file_content: &str, start_line: usize) -> usize {
    if start_line <= 1 { return start_line; }
 
    // [SYSTEM A] AST Scope Detection
    // Try to parse the file and find the "Parent Node" of the target line
    // If the target line is inside a function/class, return the start of that function/class.
    let is_python = file_content.contains("def ") || file_content.contains("class ");
    let dummy_path = if is_python { "dummy.py" } else { "dummy.rs" };
 
    if let Ok(tree) = crate::apply_system::analysis::AnalysisEngine::parse(file_content, dummy_path) {
        let root = tree.root_node();
        let target_byte = get_byte_offset(file_content, start_line);
        
        let mut cursor = root.walk();
        let mut target_node = None;
 
        // Drill down to find the most specific node at the target line
        'search: loop {
            if cursor.goto_first_child() {
                loop {
                    let node = cursor.node();
                    if target_byte >= node.start_byte() && target_byte < node.end_byte() {
                        // Found a candidate, go deeper
                        target_node = Some(node);
                        continue 'search;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
                cursor.goto_parent();
            }
            break;
        }
 
        // Walk UP from the target node to find a definition container
        if let Some(mut node) = target_node {
            while let Some(parent) = node.parent() {
                let kind = parent.kind();
                // If we are inside a function/class body, hoist to the definition
                if kind == "function_definition" || kind == "class_definition" || kind == "impl_item" {
                    let start_row = parent.start_position().row;
                    return start_row + 1; // 1-based index
                }
                node = parent;
            }
        }
    }
 
    // [SYSTEM B] Heuristic Fallback (Line Scanning)
    // Used if AST fails or language is unsupported
    let lines: Vec<&str> = file_content.lines().collect();
    if start_line > lines.len() { return start_line; }
 
    let mut current_idx = start_line - 1;
    
    for _ in 0..25 { // Look back 25 lines
        if current_idx == 0 { break; }
        
        let line = lines[current_idx];
        let trimmed = line.trim();
        
        if trimmed.starts_with("def ") || trimmed.starts_with("class ") || trimmed.starts_with("async def ") {
            // Found a definition header
            // Check if there are decorators above it
            let mut check_idx = current_idx;
            while check_idx > 0 {
                let prev = lines[check_idx - 1].trim();
                if prev.starts_with('@') {
                    check_idx -= 1;
                } else {
                    break;
                }
            }
            return check_idx + 1;
        }
        
        current_idx -= 1;
    }
    
    start_line
}
 
/// [GLUON V2.1] Structural State Machine
/// Keeps track of parsing context across lines to handle multi-line strings/comments correctly.
#[derive(Debug, Clone, Default)]
pub struct StructuralState {
    pub in_string: bool,
    pub string_delimiter: char,
    pub in_char_literal: bool,
    pub in_raw_string: bool,
    pub raw_string_hashes: usize,
    pub in_block_comment: bool,
    pub balance: i32, // Bracket balance: ( [ { increment, ) ] } decrement
}
 
impl StructuralState {
    /// Update state based on a single line of code.
    /// Returns the net change in bracket balance for this line.
    pub fn update(&mut self, line: &str) -> i32 {
        let start_balance = self.balance;
        let mut chars = line.chars().peekable();
        let mut escape_next = false;
 
        while let Some(c) = chars.next() {
            // Handle escape sequences
            if escape_next {
                escape_next = false;
                continue;
            }
 
            // Handle block comments (C-style /* ... */)
            if self.in_block_comment {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next(); // consume '/'
                    self.in_block_comment = false;
                }
                continue;
            }
 
            // Check for line comments (// or #) - ONLY if not in string
            if !self.in_string && !self.in_char_literal && !self.in_raw_string {
                if c == '/' && chars.peek() == Some(&'/') {
                    break; // Rest of line is comment
                }
                if c == '#' {
                    break; // Python/Shell comment
                }
                if c == '/' && chars.peek() == Some(&'*') {
                    chars.next(); // consume '*'
                    self.in_block_comment = true;
                    continue;
                }
            }
 
            match c {
                '\\' if !self.in_raw_string => {
                    escape_next = true;
                }
 
                // Rust raw string: r#"..."#
                'r' if !self.in_string && !self.in_char_literal && chars.peek() == Some(&'#') => {
                    self.in_raw_string = true;
                    self.raw_string_hashes = 0;
                    // Count hashes
                    while chars.peek() == Some(&'#') {
                        self.raw_string_hashes += 1;
                        chars.next();
                    }
                    // Consume opening "
                    if chars.peek() == Some(&'"') {
                        chars.next();
                    }
                }
 
                '"' if self.in_raw_string => {
                    // Check if followed by matching hashes
                    let mut hash_count = 0;
                    let saved_position = chars.clone();
                    while chars.peek() == Some(&'#') && hash_count < self.raw_string_hashes {
                        hash_count += 1;
                        chars.next();
                    }
                    if hash_count == self.raw_string_hashes {
                        self.in_raw_string = false;
                        self.raw_string_hashes = 0;
                    } else {
                        // Not end of raw string, restore position
                        chars = saved_position;
                    }
                }
 
                '"' if !self.in_raw_string && !self.in_char_literal => {
                    if self.in_string && self.string_delimiter == '"' {
                        self.in_string = false;
                    } else if !self.in_string {
                        self.in_string = true;
                        self.string_delimiter = '"';
                    }
                }
 
                '\'' if !self.in_string && !self.in_raw_string => {
                    if self.in_char_literal {
                        self.in_char_literal = false;
                    } else {
                        self.in_char_literal = true;
                    }
                }
 
                '`' if !self.in_char_literal && !self.in_raw_string => {
                    // JS/TS template literal
                    if self.in_string && self.string_delimiter == '`' {
                        self.in_string = false;
                    } else if !self.in_string {
                        self.in_string = true;
                        self.string_delimiter = '`';
                    }
                }
 
                '(' | '[' | '{' if !self.in_string && !self.in_char_literal && !self.in_raw_string => {
                    self.balance += 1;
                }
 
                ')' | ']' | '}' if !self.in_string && !self.in_char_literal && !self.in_raw_string => {
                    self.balance -= 1;
                }
 
                _ => {}
            }
        }
        
        if self.in_char_literal {
            self.in_char_literal = false; 
        }
 
        self.balance - start_balance
    }
}
 
/// [NEW FUNCTION] Counts structural braces using the robust State Machine
pub fn count_structural_braces(line: &str) -> i32 {
    let mut state = StructuralState::default();
    state.update(line)
}
 
/// [GLUON V2.1] Token Stream Normalizer (Moved from FuzzyMatcher)
/// Converts code to a stream of essential tokens, preserving specific punctuation
/// but ignoring all whitespace and quote types. Used for Fuzzy Matching and Destruction Guard.
pub fn normalize_token_stream(s: &str) -> String {
    let mut tokens = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
 
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        
        // Skip comments (basic)
        if c == '/' && chars.peek() == Some(&'/') {
            while let Some(n) = chars.next() { if n == '\n' { break; } }
            continue;
        }
        if c == '#' {
            while let Some(n) = chars.next() { if n == '\n' { break; } }
            continue;
        }
 
        // Normalize quotes to single generic symbol
        if c == '"' || c == '\'' {
            tokens.push('"'); 
            continue;
        }
 
        // Keep alphanumeric and structural chars
        // Lowercase alphanumeric to handle casing diffs (though rarer in code)
        if c.is_alphanumeric() {
             tokens.push(c.to_ascii_lowercase());
        } else {
             tokens.push(c);
        }
    }
    tokens
}
 
/// Helper: Convert 1-based line number to byte offset (CRLF/LF safe)
fn get_byte_offset(content: &str, line_num: usize) -> usize {
    if line_num <= 1 { return 0; }
    
    // Scan for newlines directly to handle mixed/platform-specific line endings correctly.
    // lines() iterator strips endings, making byte math unreliable for CRLF.
    let mut current_line = 1;
    for (i, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            current_line += 1;
            if current_line == line_num {
                return i + 1; // Start of the requested line
            }
        }
    }
    
    // Fallback: EOF
    content.len()
}
 
/// [GLUON SHARED] Text Similarity Calculator (Normalized Levenshtein)
/// Used by BlockMatcher (System B) and FuzzyMatcher.
pub fn calculate_similarity(s1: &str, s2: &str) -> f32 {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    let max_len = std::cmp::max(len1, len2);
 
    if max_len == 0 { return 1.0; }
    
    // [GLUON SAFETY] Performance Guard & Fast Fallback
    // If strings are too massive for Levenshtein, use Jaccard Token Similarity.
    if max_len > 3000 {
        let set1: std::collections::HashSet<&str> = s1.split_whitespace().collect();
        let set2: std::collections::HashSet<&str> = s2.split_whitespace().collect();
        
        let intersection = set1.intersection(&set2).count();
        let union = set1.union(&set2).count();
        
        if union == 0 { return 0.0; }
        return intersection as f32 / union as f32;
    }
 
    let distance = levenshtein_distance(s1, s2);
    1.0 - (distance as f32 / max_len as f32)
}
 
/// Optimized Levenshtein distance (Two-Row Space Efficient)
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let len1 = v1.len();
    let len2 = v2.len();
 
    if len1 > len2 {
        return levenshtein_distance(s2, s1);
    }
 
    let mut cache: Vec<usize> = (0..=len1).collect();
    let mut dist_diag;
    let mut dist_left;
 
    for j in 1..=len2 {
        dist_diag = cache[0];
        cache[0] = j;
 
        for i in 1..=len1 {
            dist_left = cache[i];
            let cost = if v1[i - 1] == v2[j - 1] { 0 } else { 1 };
            cache[i] = std::cmp::min(
                std::cmp::min(cache[i] + 1, cache[i - 1] + 1),
                dist_diag + cost
            );
            dist_diag = dist_left;
        }
    }
    cache[len1]
}
/// [GLUON ELASTIC MATCHING] Fuzzy Block End Detection
///
/// Finds the actual end of a code block by fuzzy matching the "tail signature"
/// from the search block against the file content. This handles AI-generated
/// SEARCH blocks that may be incomplete or have slightly different line counts.
///
/// ## Algorithm:
/// 1. Extract tail signature (last N lines) from search block
/// 2. Search in file starting from estimated_end ± search_window
/// 3. Find line with best fuzzy match to tail signature
/// 4. Return that line as the true end
///
/// ## Parameters:
/// - `file_lines`: All lines from the target file
/// - `start_line`: 1-based start line of the match
/// - `search_lines`: Lines from the SEARCH block
/// - `fuzzy_threshold`: Minimum similarity (0.0-1.0, default 0.85)
///
/// ## Returns:
/// 1-based line number of the actual block end (exclusive)
pub fn find_fuzzy_block_end(
    file_lines: &[String],
    start_line: usize,
    search_lines: &[String],
    fuzzy_threshold: f32,
) -> usize {
    crate::gluon_info!("FuzzyBlockEnd", "Starting fuzzy end detection...");
    crate::gluon_info!("FuzzyBlockEnd", "Start line: {}, Search block: {} lines", start_line, search_lines.len());

    if search_lines.is_empty() || start_line == 0 || start_line > file_lines.len() {
        crate::gluon_warn!("FuzzyBlockEnd", "Invalid input - returning start + search_len");
        return start_line + search_lines.len();
    }

    // Step 1: Extract tail signature (last 3-5 lines of search block)
    let tail_size = search_lines.len().min(5).max(1);
    let tail_lines: Vec<&String> = search_lines.iter().rev().take(tail_size).collect();
    let tail_signature = tail_lines.iter().rev().map(|s| s.as_str()).collect::<Vec<_>>().join("\n");

    crate::gluon_info!("FuzzyBlockEnd", "Tail signature ({} lines):", tail_size);
    for (idx, line) in tail_lines.iter().rev().enumerate() {
        crate::gluon_info!("FuzzyBlockEnd", "[{}] {}", idx, line.chars().take(80).collect::<String>());
    }

    // Step 2: Define search window
    // Start from estimated end (start + search_len) and search ±10 lines
    let estimated_end = start_line + search_lines.len();
    let search_window = 10;
    let search_start = estimated_end.saturating_sub(search_window);
    let search_end = (estimated_end + search_window).min(file_lines.len() + 1);

    crate::gluon_info!("FuzzyBlockEnd", "Searching range: {}-{} (estimated: {})", search_start, search_end, estimated_end);

    // Step 3: Find best match in search window
    let mut best_match_line = estimated_end;
    let mut best_similarity = 0.0f32;

    for candidate_end in search_start..search_end {
        if candidate_end <= start_line || candidate_end > file_lines.len() {
            continue;
        }

        // Extract candidate tail from file (last N lines before candidate_end)
        let candidate_start = (candidate_end.saturating_sub(tail_size)).max(start_line - 1);
        let candidate_tail_lines = &file_lines[candidate_start..candidate_end];
        let candidate_signature = candidate_tail_lines.join("\n");

        // Calculate similarity
        let similarity = calculate_similarity(&tail_signature, &candidate_signature);

        if similarity > best_similarity {
            best_similarity = similarity;
            best_match_line = candidate_end;

            crate::gluon_info!("FuzzyBlockEnd", "Candidate end={}, similarity={:.2}", candidate_end, similarity);
        }
    }

    // Step 4: Accept match if above threshold
    if best_similarity >= fuzzy_threshold {
        crate::gluon_info!("FuzzyBlockEnd", "Found fuzzy match! End line: {} (similarity: {:.2})", best_match_line, best_similarity);
        best_match_line
    } else {
        crate::gluon_warn!("FuzzyBlockEnd", "No good match found (best: {:.2} < {:.2}), using estimated end: {}",
            best_similarity, fuzzy_threshold, estimated_end);
        
        // [GLUON FIX 1.2] Zombie Tail Prevention (Look-ahead Fallback)
        // If we didn't find a fuzzy match for the tail, check if the estimated end leaves dangling braces.
        // This is a common AI error where they omit the closing brace in the SEARCH block.
        if estimated_end <= file_lines.len() {
            let context_end = expand_block_scope(&file_lines.join("\n"), start_line, estimated_end);
            if context_end > estimated_end {
                crate::gluon_info!("FuzzyBlockEnd", "Zombie Tail Guard: Extended scope from {} to {} based on structural closure.", estimated_end, context_end);
                return context_end;
            }
        }
        
        estimated_end
    }
}