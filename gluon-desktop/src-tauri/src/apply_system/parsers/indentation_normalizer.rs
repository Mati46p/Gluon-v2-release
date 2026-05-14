//! Indentation Normalization Module
//!
//! Based on research findings:
//! - Dynamic AST with Semantic Anchoring (Roo-Code)
//! - Context-aware indentation detection and normalization
//! - AST-based validation using abstract syntax trees
//!
//! This module provides robust indentation handling for code blocks,
//! especially important for Python where indentation is syntactically significant.

use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum IndentStyle {
    Spaces(usize),  // Number of spaces per indent level
    Tabs,
    Mixed(usize, usize), // (spaces_count, tabs_count) - problematic
}

#[derive(Debug, Clone)]
pub struct IndentationContext {
    pub style: IndentStyle,
    pub base_level: usize,  // Base indentation level (e.g., method inside class = 1)
    #[allow(dead_code)]
    pub detected_levels: Vec<usize>, // All detected indent levels in code
}
pub struct IndentationNormalizer;

impl IndentationNormalizer {
    /// a comment is indented deeper than the comment itself without a block opener.
    ///
    /// Pattern causing bug:
    ///   # Comment
    ///       code_that_should_be_aligned_with_comment
    ///
    /// Returns normalized code with artifacts removed.
    /// 
    /// [GLUON V2.1] Structural Integrity Indentation Fixer
    /// Uses a full State Machine (TokenWalker) to correctly track bracket balance across lines.
    /// This prevents lists/dicts/multiline strings from being corrupted by indentation fixes.
    pub fn fix_ghost_indentation(code: &str) -> String {
        use crate::apply_system::matchers::utils::StructuralState;
 
        let lines: Vec<&str> = code.lines().collect();
        let mut result_lines = Vec::new();
        
        // State to track if we are in a "ghost block" following a comment
        let mut active_ghost_dedent: Option<usize> = None;
        let mut last_comment_indent: Option<usize> = None;
        
        // [Lexer State] Tracks strings, comments, and bracket balance
        let mut state = StructuralState::default();
 
        for line in lines {
            // Check state BEFORE processing line (are we deep inside a structure from previous lines?)
            let is_inside_structure = state.balance > 0 || state.in_string || state.in_block_comment;
 
            // Update state with current line content
            state.update(line);
 
            // If we are INSIDE a multi-line structure, we SKIP ghost indentation logic completely.
            // Indentation inside lists/dicts/strings is semantic and must be preserved verbatim.
            if is_inside_structure {
                result_lines.push(line.to_string());
                continue;
            }
 
            if line.trim().is_empty() {
                result_lines.push(line.to_string());
                continue;
            }
 
            let current_indent = Self::get_indent_level(line);
            let trimmed = line.trim();
 
            // If line is a comment (starting from beginning), it resets the context
            if trimmed.starts_with('#') {
                last_comment_indent = Some(current_indent);
                active_ghost_dedent = None; // Reset ghost tracking
                result_lines.push(line.to_string());
                continue;
            }
 
            // If line is code...
            if let Some(comment_indent) = last_comment_indent {
                // Check if this code triggered a ghost block (first line after comment)
                if active_ghost_dedent.is_none() {
                    // Logic: If code is indented deeper than the comment above it, it's likely a ghost indent
                    // UNLESS checking indentation flow would show it belongs there (handled by SyntaxValidator later)
                    if current_indent > comment_indent {
                        let excess = current_indent - comment_indent;
                        active_ghost_dedent = Some(excess);
                    } else {
                        // Code is aligned or dedented relative to comment - normal behavior
                        last_comment_indent = None;
                    }
                }
            }
 
            // If we are in a ghost block, dedent the line
            if let Some(dedent_amount) = active_ghost_dedent {
                if current_indent >= dedent_amount {
                    let new_line = format!("{}{}", 
                        " ".repeat(current_indent - dedent_amount), 
                        trimmed
                    );
                    result_lines.push(new_line);
                } else {
                    // Indentation dropped back below the ghost level - end the ghost block
                    active_ghost_dedent = None;
                    last_comment_indent = None;
                    result_lines.push(line.to_string());
                }
            } else {
                result_lines.push(line.to_string());
            }
        }
 
        result_lines.join("\n")
    }

    /// Detects the indentation style and base level from a code block.
    /// Uses histogram analysis to determine the most common indentation pattern.
    ///
    /// Based on: https://github.com/sindresorhus/detect-indent
    pub fn detect_indentation(code: &str) -> IndentationContext {
        let lines: Vec<&str> = code.lines().collect();

        let mut indent_histogram: HashMap<usize, usize> = HashMap::new();
        let mut uses_tabs = false;
        let mut uses_spaces = false;
        let mut min_indent = usize::MAX;

        // Analyze each line
        for line in &lines {
            if line.trim().is_empty() || line.trim().starts_with('#') {
                continue; // Skip empty lines and comments
            }

            let indent = Self::get_indent_level(line);

            // Detect tab vs space usage
            if line.starts_with('\t') {
                uses_tabs = true;
            } else if line.starts_with(' ') {
                uses_spaces = true;
            }

            // Track indent levels
            if indent > 0 {
                *indent_histogram.entry(indent).or_insert(0) += 1;
                if indent < min_indent {
                    min_indent = indent;
                }
            }
        }

        // Determine style
        let style = if uses_tabs && !uses_spaces {
            IndentStyle::Tabs
        } else if uses_spaces && !uses_tabs {
            // Find most common indent difference (GCD-like approach)
            let indent_unit = Self::detect_indent_unit(&indent_histogram);
            IndentStyle::Spaces(indent_unit)
        } else if uses_tabs && uses_spaces {
            crate::gluon_warn!("IndentationNormalizer", "Mixed tabs and spaces detected - this may cause issues");
            let space_count = indent_histogram.keys().min().copied().unwrap_or(4);
            IndentStyle::Mixed(space_count, 1)
        } else {
            // No indentation detected, default to 4 spaces (PEP 8)
            IndentStyle::Spaces(4)
        };

        // Base level is the minimum indentation found
        let base_level = if min_indent == usize::MAX { 0 } else { min_indent };

        let detected_levels: Vec<usize> = indent_histogram.keys().copied().collect();

        IndentationContext {
            style,
            base_level,
            detected_levels,
        }
    }

    /// Detects the unit of indentation (e.g., 2, 4, or 8 spaces).
    /// Uses GCD (Greatest Common Divisor) approach on indent levels.
    fn detect_indent_unit(histogram: &HashMap<usize, usize>) -> usize {
        let mut levels: Vec<usize> = histogram.keys().copied().collect();
        levels.sort_unstable();

        if levels.is_empty() {
            return 4; // Default
        }

        if levels.len() == 1 {
            return levels[0]; // Single level detected
        }

        // Calculate differences between consecutive levels
        let mut diffs = Vec::new();
        for i in 1..levels.len() {
            let diff = levels[i] - levels[i - 1];
            if diff > 0 {
                diffs.push(diff);
            }
        }

        if diffs.is_empty() {
            return 4;
        }

        // Find GCD of all differences
        let mut gcd = diffs[0];
        for &diff in &diffs[1..] {
            gcd = Self::gcd(gcd, diff);
        }

        // Sanity check: indent unit should be reasonable (2, 3, 4, 8)
        if gcd == 0 || gcd > 8 {
            4 // Default to PEP 8
        } else {
            gcd
        }
    }

    /// Greatest Common Divisor using Euclidean algorithm
    fn gcd(mut a: usize, mut b: usize) -> usize {
        while b != 0 {
            let temp = b;
            b = a % b;
            a = temp;
        }
        a
    }

    /// Normalizes code block indentation to match target context.
    ///
    /// # Arguments
    /// * `code` - The code block to normalize
    /// * `target_context` - The target indentation context (from file)
    /// * `preserve_relative` - If true, preserves relative indentation within block
    ///
    /// # Returns
    /// Normalized code with corrected indentation
    #[allow(dead_code)]
    pub fn normalize_to_context(
        code: &str,
        target_context: &IndentationContext,
        preserve_relative: bool
    ) -> String {
        // source_context removed (unused)

        let lines: Vec<&str> = code.lines().collect();
        let mut normalized_lines = Vec::new();

        // Calculate indent offset
        let base_offset = match &target_context.style {
            IndentStyle::Spaces(unit) => target_context.base_level * unit,
            IndentStyle::Tabs => target_context.base_level,
            IndentStyle::Mixed(spaces, _) => target_context.base_level * spaces,
        };

        // [GLUON V2.2 FIX] Pre-calculate source base char index to avoid O(N^2) and Borrow Checker issues
        let source_base_chars = if preserve_relative {
            if let Some(first_non_empty) = lines.iter().find(|l| !l.trim().is_empty()) {
                Self::get_indent_level(first_non_empty)
            } else {
                0
            }
        } else {
            0
        };

        for line in lines {
            if line.trim().is_empty() {
                normalized_lines.push(String::new());
                continue;
            }

            let trimmed = line.trim_start();
            let original_indent = Self::get_indent_level(line);

            if !preserve_relative {
                // Simple normalization: apply target base level
                let new_indent = Self::create_indent(&target_context.style, base_offset);
                normalized_lines.push(format!("{}{}", new_indent, trimmed));
            } else {
                // [GLUON V2.2] Relative Delta Algorithm (Optimized)
                // 1. Calculate Delta (Target - Source)
                let target_base_i32 = base_offset as i32;
                let source_base_i32 = source_base_chars as i32;
                let shift = target_base_i32 - source_base_i32;

                // 2. Apply Shift
                let current_indent_i32 = original_indent as i32;
                let new_indent_i32 = current_indent_i32 + shift;
                
                // Ensure we don't have negative indentation
                let final_indent = if new_indent_i32 < 0 { 0 } else { new_indent_i32 as usize };
                
                // Safety cap
                let safe_indent = final_indent.min(80);

                let new_indent_str = Self::create_indent(&target_context.style, safe_indent);
                normalized_lines.push(format!("{}{}", new_indent_str, trimmed));
            }
        }

        normalized_lines.join("\n")
    }

    /// Validates that code block indentation is consistent with target file.
    ///
    /// Returns warnings if indentation looks suspicious (e.g., method without class context).
    #[allow(dead_code)]
    pub fn validate_context_compatibility(
        code: &str,
        target_file_content: &str,
        anchor_line: usize
    ) -> Result<(), Vec<String>> {
        let mut warnings = Vec::new();

        let file_context = Self::detect_indentation(target_file_content);
        let code_context = Self::detect_indentation(code);

        // Check 1: Style mismatch
        match (&file_context.style, &code_context.style) {
            (IndentStyle::Tabs, IndentStyle::Spaces(_)) |
            (IndentStyle::Spaces(_), IndentStyle::Tabs) => {
                warnings.push(format!(
                    "⚠️  Indentation style mismatch: file uses {:?}, code block uses {:?}",
                    file_context.style, code_context.style
                ));
            }
            (IndentStyle::Spaces(file_unit), IndentStyle::Spaces(code_unit)) if file_unit != code_unit => {
                warnings.push(format!(
                    "⚠️  Indent unit mismatch: file uses {} spaces, code block uses {} spaces",
                    file_unit, code_unit
                ));
            }
            _ => {}
        }

        // Check 2: Detect if code looks like a class method but has no indentation
        if Self::looks_like_class_method(code) && code_context.base_level == 0 {
            warnings.push(
                "⚠️  Code appears to be a class method but has no base indentation (should be inside class)".to_string()
            );
        }

        // Check 3: Analyze target line context
        if let Some(expected_indent) = Self::get_expected_indent_at_line(target_file_content, anchor_line) {
            if code_context.base_level != expected_indent {
                warnings.push(format!(
                    "⚠️  Expected indentation level {} at line {}, but code block has level {}",
                    expected_indent, anchor_line, code_context.base_level
                ));
            }
        }

        if warnings.is_empty() {
            Ok(())
        } else {
            Err(warnings)
        }
    }

    /// Detects if code contains patterns suggesting it's a class method.
    #[allow(dead_code)]
    fn looks_like_class_method(code: &str) -> bool {
        let method_with_self_re = Regex::new(r"def\s+\w+\s*\(\s*self").unwrap();
        let decorator_re = Regex::new(r"@(action|property|staticmethod|classmethod)").unwrap();

        method_with_self_re.is_match(code) || decorator_re.is_match(code)
    }

    /// Gets expected indentation level at a specific line in file.
    #[allow(dead_code)]
    fn get_expected_indent_at_line(file_content: &str, line_num: usize) -> Option<usize> {
        let lines: Vec<&str> = file_content.lines().collect();

        if line_num >= lines.len() {
            return None;
        }

        // Get indent of target line
        let target_line = lines[line_num];
        if !target_line.trim().is_empty() {
            return Some(Self::get_indent_level(target_line));
        }

        // If empty, search backwards for nearest non-empty line
        for i in (0..line_num).rev() {
            let line = lines[i];
            if !line.trim().is_empty() && !line.trim().starts_with('#') {
                return Some(Self::get_indent_level(line));
            }
        }

        Some(0)
    }

    /// Helper: Get indentation level (number of leading whitespace chars).
    pub fn get_indent_level(line: &str) -> usize {
        line.len() - line.trim_start().len()
    }


    /// Helper: Create indentation string based on style.
    #[allow(dead_code)]
    fn create_indent(style: &IndentStyle, total_chars: usize) -> String {
        match style {
            IndentStyle::Tabs => "\t".repeat(total_chars),
            IndentStyle::Spaces(unit) => {
                let levels = total_chars / unit;
                " ".repeat(levels * unit)
            }
            IndentStyle::Mixed(spaces, _) => " ".repeat(total_chars / spaces * spaces),
        }
    }

    /// [GLUON V4] Indentation Re-Flow (Defense in Depth)
    ///
    /// Combines [System A] Relative Delta Calculation with [System B] Visual Alignment Fallback
    /// to guarantee PEP-8 compliance regardless of LLM hallucinations.
    pub fn reconstruct_python_indentation(code_block: &str) -> String {
        let lines: Vec<&str> = code_block.lines().collect();
        if lines.is_empty() { return String::new(); }
 
        // [SYSTEM A] Anchor Identification
        let def_idx = lines.iter().position(|l| {
            let trimmed = l.trim();
            trimmed.starts_with("def ") || trimmed.starts_with("class ") || trimmed.starts_with("async def ")
        });
 
        // Fallback to Ghost Fixing if no definition found
        let def_idx = match def_idx {
            Some(idx) => idx,
            None => return Self::fix_ghost_indentation(code_block),
        };
 
        // Anchor Point: The definition header
        let def_line = lines[def_idx];
        let header_indent = Self::get_indent_level(def_line);
        let mut normalized = Vec::new();
 
        // 1. Normalize Header & Decorators
        for i in 0..=def_idx {
            let line = lines[i];
            let trimmed = line.trim_start();
            if i < def_idx && trimmed.starts_with('@') {
                // Force decorators to align with function header
                normalized.push(format!("{}{}", " ".repeat(header_indent), trimmed));
            } else {
                // Ensure header itself is clean (though we respect its base indent)
                normalized.push(line.to_string());
            }
        }
 
        // 2. Normalize Body (Relative Delta Algorithm)
        let target_body_base = header_indent + 4; // PEP-8 Enforcement
        let mut input_body_base = None;
 
        for line in lines.iter().skip(def_idx + 1) {
            if line.trim().is_empty() {
                normalized.push(String::new());
                continue;
            }
 
            let current_indent = Self::get_indent_level(line);
 
            // Establish the "Ground Truth" indentation of the body from the first line
            if input_body_base.is_none() {
                input_body_base = Some(current_indent);
            }
            let input_base = input_body_base.unwrap();
 
            // [SYSTEM A Logic] Relative Delta
            // NewIndent = TargetBase + (CurrentIndent - InputBase)
            let relative_offset = if current_indent >= input_base {
                current_indent - input_base
            } else {
                // [SYSTEM B Logic] Fallback for broken structures
                // If a line is less indented than the body start, it might be a dedent.
                // We clamp it to 0 relative offset to prevent negative overflows or massive jumps.
                0
            };
 
            let new_indent = target_body_base + relative_offset;
            normalized.push(format!("{}{}", " ".repeat(new_indent), line.trim_start()));
        }
 
        normalized.join("\n")
    }
 
    /// Auto-adjusts code block indentation to match surrounding context.
    ///
    /// This is the main entry point for indentation normalization during parsing.
    #[allow(dead_code)]
    pub fn auto_adjust(
        code_block: &str,
        target_file: &str,
        anchor_line: usize
    ) -> Result<String, String> {
        // 1. Detect both contexts
        let file_context = Self::detect_indentation(target_file);

        // 2. Validate compatibility (log warnings but don't fail)
        if let Err(warnings) = Self::validate_context_compatibility(code_block, target_file, anchor_line) {
            for warning in warnings {
                crate::gluon_warn!("IndentationNormalizer", "{}", warning);
            }
        }

        // 3. Normalize code block to match file context
        let normalized = Self::normalize_to_context(
            code_block,
            &file_context,
            true // Preserve relative indentation within block
        );

        Ok(normalized)
    }

    /// [GLUON V2 - PHASE 1] Smart Adjust Indentation (Relative Delta Algorithm)
    ///
    /// This is the MAIN entry point for AST Hardening Phase 1 indentation handling.
    /// Implements the "Relative Delta" algorithm that projects code fragments onto
    /// the target file's anchor indentation.
    ///
    /// # Algorithm Steps:
    /// 1. **Anchor Detection**: Find the target indentation at `insert_line` in `target_file`
    /// 2. **Source Baseline**: Calculate minimum indentation in `new_fragment`
    /// 3. **Delta Calculation**: `delta = Target Base - Source Base`
    /// 4. **Projection**: For each line: `New Indent = Local Indent + Delta`
    ///
    /// # Arguments
    /// * `target_file` - Full content of the target file
    /// * `insert_line` - 1-based line number where code will be inserted
    /// * `new_fragment` - Code fragment from AI (may have wrong base indentation)
    ///
    /// # Returns
    /// * Adjusted code with correct indentation matching target context
    ///
    /// # Example
    /// ```text
    /// Target file at line 10: "        self.save()" (8 spaces - inside method)
    /// New fragment: "x = 1\nreturn x" (0 spaces - flat)
    /// Delta: +8
    /// Result: "        x = 1\n        return x"
    /// ```
    pub fn smart_adjust_indentation(
        target_file: &str,
        insert_line: usize,
        new_fragment: &str
    ) -> String {
        // [GLUON FIX 3.0] Pre-process Python Structure
        // If the model returns code where statements and definitions are at the same indentation level,
        // we must restore the relative structure BEFORE calculating target indentation.
        // This ensures that "Body Statements" act as the anchor for depth, while "Next Definitions"
        // correctly float back up to the parent scope.
        let processed_fragment = Self::ensure_python_structural_integrity(new_fragment);

        let file_lines: Vec<&str> = target_file.lines().collect();
        let new_lines: Vec<&str> = processed_fragment.lines().collect();

        if insert_line == 0 || new_lines.is_empty() {
            return processed_fragment;
        }

        // Helper: Get visual width (handles tabs and spaces)
        let get_width = |line: &str| -> usize {
            let mut width = 0;
            for c in line.chars() {
                match c {
                    ' ' => width += 1,
                    '\t' => width += 4, // Standard tab width
                    _ => break,
                }
            }
            width
        };

        // Step 1: Determine Target Base Indentation (Anchor)
        // Look at the line being replaced/inserted at to establish ground truth
        let start_idx = insert_line.saturating_sub(1);
        let mut target_base_indent = 0;

        // Strategy A: Visual inspection of target line
        if start_idx < file_lines.len() {
            let target_line = file_lines[start_idx];
            if !target_line.trim().is_empty() {
                target_base_indent = get_width(target_line);
            } else {
                // Strategy B: Look backwards for nearest non-empty line
                for i in (0..start_idx).rev().take(50) {
                    let line = file_lines[i];
                    if !line.trim().is_empty() {
                        let prev_indent = get_width(line);
                        let trimmed = line.trim();

                        // Predict indent based on previous line structure
                        target_base_indent = if trimmed.ends_with(':') || trimmed.ends_with('{') {
                            prev_indent + 4 // Block opener
                        } else {
                            prev_indent // Sibling statement
                        };
                        break;
                    }
                }
            }
        }

        // Step 2: Determine Source Baseline (ANCHOR indent of new fragment)
        // [GLUON FIX 2.0] Use the indentation of the FIRST CODE LINE as the baseline.
        // Using .min() caused issues when a block contained dedented code (e.g. closing a function),
        // causing the anchor line to be shifted unnecessarily.
        let source_base_indent = new_lines.iter()
            .filter(|l| {
                let trimmed = l.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("//")
            })
            .map(|l| get_width(l))
            .next() // Take the FIRST valid line, not the minimum
            // Fallback: if block is ONLY comments/empty, take first non-empty
            .unwrap_or_else(|| {
                new_lines.iter()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| get_width(l))
                    .next()
                    .unwrap_or(0)
            });

        // Step 3: Calculate Delta
        let delta = target_base_indent as i32 - source_base_indent as i32;

        // Step 4: Project each line onto target indentation
        // Detect indentation style from target file
        let use_tabs = file_lines.iter().take(50).any(|l| l.starts_with('\t'));

        let mut result_lines = Vec::new();
        for line in &new_lines {
            if line.trim().is_empty() {
                result_lines.push(String::new());
                continue;
            }

            let current_width = get_width(line);

            // Apply delta: New Indent = Current + Delta
            let new_width = (current_width as i32 + delta).max(0) as usize;

            // Create indent string (respecting file style)
            let indent_str = if use_tabs {
                "\t".repeat(new_width / 4)
            } else {
                " ".repeat(new_width)
            };

            let content = line.trim_start();
            result_lines.push(format!("{}{}", indent_str, content));
        }

        result_lines.join("\n")
    }

    /// [GLUON INTELLIGENCE] Restore Structural Integrity for Python Blocks
    ///
    /// Detects if a block mixes "Definitions" (def/class) and "Statements" at the same indentation level.
    /// If so, it assumes the Statements belong to the *previous* scope (body) and indents them by 4 spaces,
    /// while keeping Definitions at the base level.
    fn ensure_python_structural_integrity(code: &str) -> String {
        let lines: Vec<&str> = code.lines().collect();
        if lines.len() < 2 { return code.to_string(); }

        // Analyze indentation levels
        let mut min_def_indent = usize::MAX;
        let mut min_stmt_indent = usize::MAX;
        let mut has_defs = false;
        let mut has_stmts = false;

        for line in &lines {
            if line.trim().is_empty() { continue; }
            let indent = Self::get_indent_level(line);
            let trimmed = line.trim();
            
            // Check if definition
            if trimmed.starts_with("def ") || trimmed.starts_with("class ") || trimmed.starts_with("async def ") || trimmed.starts_with("@") {
                has_defs = true;
                if indent < min_def_indent { min_def_indent = indent; }
            } else if !trimmed.starts_with("#") && !trimmed.starts_with(")") && !trimmed.starts_with("]") { 
                // Ignore comments and closing brackets for statement detection (they often align with parent)
                has_stmts = true;
                if indent < min_stmt_indent { min_stmt_indent = indent; }
            }
        }

        // Trigger Condition: 
        // 1. Block contains BOTH definitions and statements
        // 2. Statements are NOT indented deeper than definitions (Collision)
        if has_defs && has_stmts && min_stmt_indent <= min_def_indent {
            let mut fixed_lines = Vec::new();
            
            for line in lines {
                if line.trim().is_empty() {
                    fixed_lines.push(line.to_string());
                    continue;
                }
                
                let trimmed = line.trim();
                let is_def = trimmed.starts_with("def ") || 
                             trimmed.starts_with("class ") || 
                             trimmed.starts_with("async def ") || 
                             trimmed.starts_with("@");
                             
                if is_def {
                    // Keep definitions at their current level (relative 0)
                    fixed_lines.push(line.to_string());
                } else {
                    // Indent statements by +4 spaces to push them into the "body"
                    // This creates the relative offset needed for smart_adjust_indentation
                    fixed_lines.push(format!("    {}", line));
                }
            }
            return fixed_lines.join("\n");
        }

        // Return original if structure seems fine
        code.to_string()
    }

    /// Detects if a code block is "Flat Python" (indentation lost) AND mixes definitions with statements.
    fn is_flat_python_mix(code: &str) -> bool {
        let lines: Vec<&str> = code.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.len() < 2 { return false; }
        
        // 1. Check if flat (all lines have same indentation)
        let first_indent = Self::get_indent_level(lines[0]);
        if !lines.iter().all(|l| Self::get_indent_level(l) == first_indent) {
            return false;
        }
        
        // 2. Check for mix of Defs and Statements
        let has_def = lines.iter().any(|l| l.trim().starts_with("def ") || l.trim().starts_with("class "));
        
        // Heuristic for statement: not a structure keyword, not a decorator, not an import
        let has_stmt = lines.iter().any(|l| {
            let t = l.trim();
            !t.starts_with("def ") && 
            !t.starts_with("class ") && 
            !t.starts_with('@') && 
            !t.starts_with("import ") && 
            !t.starts_with("from ") &&
            !t.starts_with(")") // Closing bracket often aligns with def
        });
        
        has_def && has_stmt
    }

    /// Restores relative indentation for flat python blocks.
    /// Assumes 'def'/'class' are outer blocks (base indent) and other statements are inner (base + 4).
    fn restore_python_structure(code: &str) -> String {
        let lines: Vec<&str> = code.lines().collect();
        let mut output = Vec::new();
        
        for line in lines {
            if line.trim().is_empty() {
                output.push(line.to_string());
                continue;
            }
            
            let trimmed = line.trim();
            // These keywords indicate the "Outer" scope in a mixed block
            let is_outer = trimmed.starts_with("def ") || 
                           trimmed.starts_with("class ") || 
                           trimmed.starts_with('@') || 
                           trimmed.starts_with("import ") || 
                           trimmed.starts_with("from ");
            
            if is_outer {
                // Keep at base level (relative 0)
                output.push(trimmed.to_string());
            } else {
                // Indent statements (relative +4)
                // We add 4 spaces to whatever existing indent was there (usually 0 in flat block)
                output.push(format!("    {}", trimmed));
            }
        }
        output.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_indentation_spaces() {
        let code = r#"
def foo():
    x = 1
    if True:
        y = 2
"#;
        let context = IndentationNormalizer::detect_indentation(code);
        assert_eq!(context.style, IndentStyle::Spaces(4));
        assert_eq!(context.base_level, 4);
    }

    #[test]
    fn test_detect_indentation_nested() {
        let code = r#"
    def method(self):
        x = 1
        if True:
            y = 2
"#;
        let context = IndentationNormalizer::detect_indentation(code);
        assert_eq!(context.style, IndentStyle::Spaces(4));
        assert_eq!(context.base_level, 4); // Base is 4 (minimum non-zero)
    }

    #[test]
    fn test_normalize_to_context() {
        let source_code = r#"def foo():
    x = 1
    return x"#;

        let target_context = IndentationContext {
            style: IndentStyle::Spaces(4),
            base_level: 1, // Inside class, so base = 1 level = 4 spaces
            detected_levels: vec![4, 8],
        };

        let normalized = IndentationNormalizer::normalize_to_context(
            source_code,
            &target_context,
            true
        );

        // Should add 4 spaces (1 level) to each line
        assert!(normalized.contains("    def foo():"));
        assert!(normalized.contains("        x = 1"));
    }

    #[test]
    fn test_looks_like_class_method() {
        let method_code = "def my_method(self, arg):";
        assert!(IndentationNormalizer::looks_like_class_method(method_code));

        let decorator_code = "@action(detail=True)\ndef foo():";
        assert!(IndentationNormalizer::looks_like_class_method(decorator_code));

        let function_code = "def standalone_func(arg):";
        assert!(!IndentationNormalizer::looks_like_class_method(function_code));
    }

    #[test]
    fn test_gcd() {
        assert_eq!(IndentationNormalizer::gcd(12, 8), 4);
        assert_eq!(IndentationNormalizer::gcd(6, 9), 3);
        assert_eq!(IndentationNormalizer::gcd(4, 8), 4);
    }

    #[test]
    fn test_detect_indent_unit() {
        let mut histogram = HashMap::new();
        histogram.insert(4, 5);  // 5 lines with 4 spaces
        histogram.insert(8, 3);  // 3 lines with 8 spaces
        histogram.insert(12, 2); // 2 lines with 12 spaces
 
        let unit = IndentationNormalizer::detect_indent_unit(&histogram);
        assert_eq!(unit, 4); // GCD of (8-4=4, 12-8=4) = 4
    }
 
    #[test]
    fn test_fix_ghost_indentation() {
        // Symulacja błędu LLM: kod po komentarzu jest wcięty o 4 spacje za dużo
        let input = r#"
    # To jest komentarz
        instance.updated_by = user
        instance.save()
    # Kolejny komentarz
        return instance
"#;
        
        let _expected = r#"
    # To jest komentarz
    instance.updated_by = user
    instance.save()
    # Kolejny komentarz
    return instance
"#;
 
        // Uwaga: input w teście ma bazowe wcięcie 4 spacje dla pierwszej linii,
        // a kod po niej ma 8 spacji. Funkcja powinna to spłaszczyć do poziomu komentarza.
        let normalized = IndentationNormalizer::fix_ghost_indentation(input);
        
        // Sprawdzamy czy linie kodu są wyrównane do komentarzy
        let lines: Vec<&str> = normalized.lines().collect();
        // Skip empty first line if present in raw string
        let start_idx = if lines[0].trim().is_empty() { 1 } else { 0 };
        
        let comment_indent = lines[start_idx].find('#').unwrap(); // indeks #
        let code_indent = lines[start_idx + 1].find('i').unwrap();    // indeks i
        
        assert_eq!(comment_indent, code_indent, "Kod powinien być wyrównany do komentarza");
    }
 
    #[test]
    fn test_reconstruct_python_indentation_fixes_explosion() {
        // Symulacja "Indentation Explosion": Nagłówek ma wcięcie 0, ale ciało ma 8 spacji (zamiast 4)
        let broken_llm_output = r#"
def get_weight_percentage(self, obj):
        if obj.vehicle:
            # Comment
            return 100
        return 0
"#;
 
        // Oczekiwany wynik: Ciało funkcji wymuszone na 4 spacje względem nagłówka
        let expected = r#"
def get_weight_percentage(self, obj):
    if obj.vehicle:
        # Comment
        return 100
    return 0
"#;
 
        let normalized = IndentationNormalizer::reconstruct_python_indentation(broken_llm_output);
        
        // Porównujemy trimowane wersje, aby zignorować wiodące/kończące nowe linie całego bloku
        assert_eq!(normalized.trim(), expected.trim());
    }
 
    #[test]
    fn test_reconstruct_python_indentation_with_decorator() {
        // Symulacja: Dekorator i funkcja wcięte na 4 spacje, ciało "odleciało" na 16 spacji
        let input = r#"
    @property
    def is_valid(self):
                return True
"#;
 
        // Oczekiwane: Zachowanie wcięcia nagłówka (4), naprawa ciała (4+4=8)
        let _expected = r#"
    @property
    def is_valid(self):
        return True
"#;
 
        let normalized = IndentationNormalizer::reconstruct_python_indentation(input);
        assert_eq!(normalized.trim(), _expected.trim());
    }
}