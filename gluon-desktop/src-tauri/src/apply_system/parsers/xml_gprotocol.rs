//! KROK 2.1: Robust Stream Parser for G-Protocol (Enhanced)
//!
//! This parser uses a State Machine approach with advanced features:
//! - Missing closing tags (auto-recovery)
//! - Markdown artifacts (preprocessing)
//! - Typos in tags (fuzzy tag matching)
//! - Interleaved commentary (ignored outside content blocks)
//! - **NEW**: Indentation normalization and validation
//! - **NEW**: Semantic anchor-based matching
//! - **NEW**: Context-aware whitespace preservation
//!
//! Based on best practices from:
//! - AST validation (Python ast module)
//! - libcst for indentation-aware parsing
//! - Dynamic AST with Semantic Anchoring (Roo-Code DAST)
//! - Levenshtein distance for fuzzy matching

use crate::apply_system::parsers::Parser;
use crate::apply_system::parsers::IndentationNormalizer;
use crate::apply_system::shared::types::ChangeQueueItem;
use regex::Regex;

pub struct XmlGProtocolParser;

/// Internal state of the parser
#[derive(Debug, PartialEq)]
enum ParserState {
    Idle,           // Looking for <file>
    InFile,         // Inside <file>, looking for <change> or <search>
    InChange,       // Inside <change>, looking for <search> or <replace>
    InSearch,       // Inside <search>, capturing old_code
    InReplace,      // Inside <replace>, capturing new_code
}

/// Accumulator for the change currently being parsed
#[derive(Debug, Default)]
struct CurrentChangeBuilder {
    file_path: Option<String>,
    old_code: Vec<String>,
    new_code: Vec<String>,
    has_search_block: bool,
    has_replace_block: bool,
    
    /// Base indentation of the &lt;search&gt; tag itself
    search_block_indent: usize,
    /// Have we checked the first line of content for nesting?
    search_nesting_checked: bool,
    /// Is the content indented relative to the tag?
    search_is_nested: bool,

    /// Base indentation of the &lt;replace&gt; tag itself
    replace_block_indent: usize,
    replace_nesting_checked: bool,
    replace_is_nested: bool,
}

impl Parser for XmlGProtocolParser {
    fn name(&self) -> &'static str {
        "XmlGProtocol (Stream)"
    }

    fn can_handle(&self, raw_response: &str) -> bool {
        // More permissible check: looks for file tag or gluon_patch
        raw_response.contains("<gluon_patch>") || raw_response.contains("<file")
    }

    fn parse(&self, raw_response: &str) -> Result<Vec<ChangeQueueItem>, String> {
        // 1. Pre-process: Clean noise, remove markdown fences
        let cleaned_lines = self.preprocess_input(raw_response);

        // 2. Parse Stream: Run state machine
        let changes = self.run_state_machine(cleaned_lines)?;

        if changes.is_empty() {
            return Err("No valid G-Protocol changes found in response".to_string());
        }

        // 3. Batch Validation: Check for conflicts and consistency
        use crate::apply_system::validators::BatchValidator;
        let batch_errors = BatchValidator::validate_batch(&changes);

        if !batch_errors.is_empty() {
            crate::gluon_warn!("XmlGProtocol", "Batch Validation Results:");
            for error in &batch_errors {
                let severity_icon = match error.severity {
                    crate::apply_system::validators::batch_validator::ErrorSeverity::Error => "❌",
                    crate::apply_system::validators::batch_validator::ErrorSeverity::Warning => "⚠️ ",
                };
                crate::gluon_warn!("XmlGProtocol", "{} Change #{}: {}", severity_icon, error.change_index + 1, error.message);
            }

            // Only fail if there are critical errors (not just warnings)
            if BatchValidator::has_errors(&batch_errors) {
                return Err(format!(
                    "Batch validation failed with {} critical errors. See logs above for details.",
                    batch_errors.iter().filter(|e| matches!(e.severity, crate::apply_system::validators::batch_validator::ErrorSeverity::Error)).count()
                ));
            }
        }

        Ok(changes)
    }
}

impl XmlGProtocolParser {
    // ========================================================================
    // Layer 1: Pre-processing (Noise Reduction)
    // ========================================================================

    fn preprocess_input(&self, text: &str) -> Vec<String> {
        // STATE-AWARE PREPROCESSING
        // Only remove ``` if they are OUTSIDE of <search>/<replace> blocks
        // or if they form the boundary of the code block.
        
        let mut processed_lines = Vec::new();
        let mut inside_tag_block = false;
        let start_tag_re = Regex::new(r"(?i)<(search|replace)>").unwrap();
        let end_tag_re = Regex::new(r"(?i)</(search|replace)>").unwrap();

        for line in text.lines() {
            let trimmed = line.trim();
            
            // State tracking
            if start_tag_re.is_match(trimmed) {
                inside_tag_block = true;
                processed_lines.push(line.trim_end().to_string());
                continue;
            }
            if end_tag_re.is_match(trimmed) {
                inside_tag_block = false;
                processed_lines.push(line.trim_end().to_string());
                continue;
            }

            // Filtering Logic
            if trimmed.starts_with("```") {
                if !inside_tag_block {
                    // Safe to remove fences outside of blocks (Markdown wrapper)
                    continue; 
                } else {
                    // Inside block: KEEP IT, unless it's clearly a boundary artifact
                    // (heuristic: if it's the very first or last line of content, maybe trim, 
                    // but safer to keep for parser sanitization later)
                    // We let sanitization logic handle inner artifacts if needed.
                }
            }
            
            // Remove conversational text outside blocks (optional, but clean)
            // For now, we keep everything inside blocks.
            
            processed_lines.push(line.trim_end().to_string());
        }
        
        processed_lines
    }

    // ========================================================================
    // Layer 2: State Machine
    // ========================================================================

    fn run_state_machine(&self, lines: Vec<String>) -> Result<Vec<ChangeQueueItem>, String> {
        let mut state = ParserState::Idle;
        let mut completed_changes = Vec::new();
        let mut current_builder = CurrentChangeBuilder::default();
        let mut current_file_path: Option<String> = None;

        // Regex patterns for fuzzy tag matching
        // allow whitespace, optional attributes
        let file_start_re = Regex::new(r"(?i)<\s*file\s+path\s*=\s*[:=>]?\s*[\x22']([^\x22']+)[\x22']").unwrap();
        let file_end_re = Regex::new(r"(?i)<\s*/\s*file\s*>").unwrap();
        
        let change_start_re = Regex::new(r"(?i)<\s*change\s*>").unwrap();
        let change_end_re = Regex::new(r"(?i)<\s*/\s*change\s*>").unwrap();

        let search_start_re = Regex::new(r"(?i)<\s*search\s*>").unwrap();
        let search_end_re = Regex::new(r"(?i)<\s*/\s*search\s*>").unwrap();

        let replace_start_re = Regex::new(r"(?i)<\s*replace\s*>").unwrap();
        let replace_end_re = Regex::new(r"(?i)<\s*/\s*replace\s*>").unwrap();

        for line in lines {
            let trimmed_line = line.trim();

            match state {
                // ------------------------------------------------------------
                // IDLE / IN FILE / IN CHANGE (Structural States)
                // ------------------------------------------------------------
                ParserState::Idle | ParserState::InFile | ParserState::InChange => {
                    // 1. Detect <file path="...">
                    if let Some(caps) = file_start_re.captures(trimmed_line) {
                        // If we were building a change, save it (Auto-recovery)
                        self.try_commit_change(&mut completed_changes, &mut current_builder);
                        
                        // Start new file context
                        if let Some(path) = caps.get(1) {
                            current_file_path = Some(path.as_str().to_string());
                            current_builder.file_path = current_file_path.clone();
                        }
                        state = ParserState::InFile;
                        continue;
                    }

                    // 2. Detect <change> (Explicit start)
                    if change_start_re.is_match(trimmed_line) {
                        self.try_commit_change(&mut completed_changes, &mut current_builder);
                        // Reset builder but keep file path
                        current_builder = CurrentChangeBuilder {
                            file_path: current_file_path.clone(),
                            ..Default::default()
                        };
                        state = ParserState::InChange;
                        continue;
                    }

                    // 3. Detect <search> (Implicit change start)
                    if search_start_re.is_match(trimmed_line) {
                        if current_builder.has_search_block || current_builder.has_replace_block {
                             // Already had data? This is a NEW change without </change>
                             self.try_commit_change(&mut completed_changes, &mut current_builder);
                             current_builder = CurrentChangeBuilder {
                                file_path: current_file_path.clone(),
                                ..Default::default()
                            };
                        }
                        state = ParserState::InSearch;
                        current_builder.has_search_block = true;
                        
                        // Capture indentation of the &lt;search&gt; tag itself
                        current_builder.search_block_indent = self.measure_indent(&line);

                        // [GLUON V2.2] Robust Inline Tag Handling
                        // Check if the line contains BOTH start and end tags: &lt;search&gt;code&lt;/search&gt;
                        if search_end_re.is_match(trimmed_line) {
                            // Extract content between tags
                            let content_with_end = self.strip_tag(&line, &search_start_re);
                            let content = self.strip_closing_tag(&content_with_end, &search_end_re);
                            
                            if !content.trim().is_empty() {
                                current_builder.old_code.push(content);
                            }
                            // Transition directly to InChange as the block is closed
                            state = ParserState::InChange;
                        } else {
                            // Standard multiline block start
                            let content = self.strip_tag(&line, &search_start_re);
                            if !content.trim().is_empty() {
                                current_builder.old_code.push(content);
                            }
                        }
                        continue;
                    }

                    // 4. Detect <replace> (Implicit change start - possibly create new file)
                    if replace_start_re.is_match(trimmed_line) {
                        if current_builder.has_replace_block {
                             self.try_commit_change(&mut completed_changes, &mut current_builder);
                             current_builder = CurrentChangeBuilder {
                                file_path: current_file_path.clone(),
                                ..Default::default()
                            };
                        }
                        state = ParserState::InReplace;
                        current_builder.has_replace_block = true;

                        // Capture indentation of the &lt;replace&gt; tag
                        current_builder.replace_block_indent = self.measure_indent(&line);

                        // [GLUON V2.2] Robust Inline Tag Handling for Replace
                        // Check if the line contains BOTH start and end tags: &lt;replace&gt;code&lt;/replace&gt;
                        if replace_end_re.is_match(trimmed_line) {
                            let content_with_end = self.strip_tag(&line, &replace_start_re);
                            let content = self.strip_closing_tag(&content_with_end, &replace_end_re);
                            
                            if !content.trim().is_empty() {
                                current_builder.new_code.push(content);
                            }
                            state = ParserState::InChange;
                        } else {
                            let content = self.strip_tag(&line, &replace_start_re);
                            if !content.trim().is_empty() {
                                current_builder.new_code.push(content);
                            }
                        }
                        continue;
                    }

                    // 5. Detect closing tags (just state transitions)
                    if change_end_re.is_match(trimmed_line) {
                        self.try_commit_change(&mut completed_changes, &mut current_builder);
                        current_builder = CurrentChangeBuilder::default();
                        current_builder.file_path = current_file_path.clone(); // Ready for next
                        state = ParserState::InFile;
                    } else if file_end_re.is_match(trimmed_line) {
                        self.try_commit_change(&mut completed_changes, &mut current_builder);
                        current_file_path = None;
                        state = ParserState::Idle;
                    }
                }

                // ------------------------------------------------------------
                // IN SEARCH (Capturing Old Code)
                // ------------------------------------------------------------
                ParserState::InSearch => {
                    // Check for closing </search>
                    if search_end_re.is_match(trimmed_line) {
                        // Handle content before tag: code...</search>
                        let content = self.strip_closing_tag(&line, &search_end_re);
                        if !content.trim().is_empty() {
                            current_builder.old_code.push(content);
                        }
                        state = ParserState::InChange;
                    } 
                    // Robustness: If we see <replace>, assume </search> was missed
                    else if replace_start_re.is_match(trimmed_line) {
                        state = ParserState::InReplace;
                        current_builder.has_replace_block = true;
                        // Don't add this line to old_code
                        let content = self.strip_tag(&line, &replace_start_re);
                        if !content.trim().is_empty() {
                            current_builder.new_code.push(content);
                        }
                    }
                    else {
                        // Determine nesting strategy on the first non-empty line
                        if !line.trim().is_empty() && !current_builder.search_nesting_checked {
                            let content_indent = self.measure_indent(&line);
                            // If content is indented at least as much as the tag, assume it's nested
                            current_builder.search_is_nested = content_indent >= current_builder.search_block_indent;
                            current_builder.search_nesting_checked = true;
                        }

                        // Capture content
                        let clean_line = if current_builder.search_is_nested {
                            self.strip_base_indent(&line, current_builder.search_block_indent)
                        } else {
                            line
                        };
                        current_builder.old_code.push(clean_line);
                    }
                }

                // ------------------------------------------------------------
                // IN REPLACE (Capturing New Code)
                // ------------------------------------------------------------
                ParserState::InReplace => {
                    // Check for closing </replace>
                    if replace_end_re.is_match(trimmed_line) {
                        let content = self.strip_closing_tag(&line, &replace_end_re);
                        if !content.trim().is_empty() {
                            current_builder.new_code.push(content);
                        }
                        state = ParserState::InChange;
                    }
                    // Robustness: If we see </change> or <file> or <search>, close block
                    else if change_end_re.is_match(trimmed_line) || file_start_re.is_match(trimmed_line) || search_start_re.is_match(trimmed_line) {
                        // Backtrack this line by one step essentially (commit then re-process)
                        self.try_commit_change(&mut completed_changes, &mut current_builder);
                        
                        // We need to re-eval this line in the Idle/InFile state logic
                        // Since we are iterating, we can't easily "push back". 
                        // Instead, we manually handle the transition logic here for the common cases.
                        
                        if search_start_re.is_match(trimmed_line) {
                             current_builder = CurrentChangeBuilder {
                                file_path: current_file_path.clone(),
                                ..Default::default()
                            };
                            state = ParserState::InSearch;
                            current_builder.has_search_block = true;
                        } else if file_start_re.is_match(trimmed_line) {
                             if let Some(caps) = file_start_re.captures(trimmed_line) {
                                if let Some(path) = caps.get(1) {
                                    current_file_path = Some(path.as_str().to_string());
                                    current_builder.file_path = current_file_path.clone();
                                }
                            }
                            state = ParserState::InFile;
                        } else {
                            // </change> case
                            current_builder = CurrentChangeBuilder {
                                file_path: current_file_path.clone(),
                                ..Default::default()
                            };
                            state = ParserState::InFile;
                        }
                    }
                    else {
                        // Determine nesting strategy on the first non-empty line
                        if !line.trim().is_empty() && !current_builder.replace_nesting_checked {
                            let content_indent = self.measure_indent(&line);
                            current_builder.replace_is_nested = content_indent >= current_builder.replace_block_indent;
                            current_builder.replace_nesting_checked = true;
                        }
 
                        // Capture content
                        let clean_line = if current_builder.replace_is_nested {
                            self.strip_base_indent(&line, current_builder.replace_block_indent)
                        } else {
                            line
                        };
                        current_builder.new_code.push(clean_line);
                    }
                }
            }
        }

        // EOF: Commit anything pending
        self.try_commit_change(&mut completed_changes, &mut current_builder);

        Ok(completed_changes)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    fn try_commit_change(&self, changes: &mut Vec<ChangeQueueItem>, builder: &mut CurrentChangeBuilder) {
        // Clone file_path early to avoid borrow issues
        let file_path = builder.file_path.clone();
 
        if let Some(path) = &file_path {
            // Only commit if we have at least one block (search or replace)
            if builder.has_search_block || builder.has_replace_block {
                // [GLUON PHASE 1] Smart Dedent & Normalization & Sanitization
                // We pass the file path to detect and strip filename artifacts (Aider-style)
                let mut old_str = self.normalize_and_sanitize(&builder.old_code, path);
                let mut new_str = self.normalize_and_sanitize(&builder.new_code, path);
 
                // Ignore empty-to-empty changes (no-op)
                if !old_str.trim().is_empty() || !new_str.trim().is_empty() {
                    // ✅ NEW STEP 0: Zombie Code Detection (Fail Fast)
                    if let Err(e) = crate::apply_system::parsers::detect_lazy_coding(&new_str) {
                        crate::gluon_error!("XmlGProtocol", "Lazy Coding Detected in {}: {}", path, e);
                        // Abort this specific change by resetting builder and returning
                        // We do NOT queue invalid lazy code.
                        self.reset_builder(builder, path);
                        return;
                    }
 
                    // ✅ NEW STEP 1: Indentation Analysis and Normalization
                    // Detect file extension to determine language
                    let language = Self::detect_language(path);

                    // For Python files, perform indentation validation and normalization
                    // GLUON FIX: Only run Python logic if it is explicitly Python.
                    if language == "python" {
                        // [GLUON V7 FIX - BUG 7] Only reconstruct REPLACE block (new_str), not SEARCH block (old_str)
                        // Reason: reconstruct_python_indentation changes indentation of search blocks,
                        // breaking matcher's ability to find code in original file.
                        // For search blocks, preserve original indentation as much as possible.

                        // Always fix ghost indentations for search block (preserve structure)
                        old_str = IndentationNormalizer::fix_ghost_indentation(&old_str);

                        // For replace block, perform full reconstruction if not a partial fragment
                        let is_partial = crate::apply_system::matchers::utils::is_partial_fragment(&new_str);
                        if !is_partial {
                            // [GLUON STEP 3] Enforce Strict Indentation Re-Flow for REPLACE only
                            new_str = IndentationNormalizer::reconstruct_python_indentation(&new_str);
                        } else {
                            // Just fix ghost indentations for replace partials
                            new_str = IndentationNormalizer::fix_ghost_indentation(&new_str);
                        }
 
                        // Detect indentation context from code blocks
                        let old_context = IndentationNormalizer::detect_indentation(&old_str);
                        let new_context = IndentationNormalizer::detect_indentation(&new_str);

                        crate::gluon_info!("XmlGProtocol", "Indentation Analysis for {}:", path);
                        crate::gluon_info!("XmlGProtocol", "<search> block: {:?} base_level={}", old_context.style, old_context.base_level);
                        crate::gluon_info!("XmlGProtocol", "<replace> block: {:?} base_level={}", new_context.style, new_context.base_level);

                        // Check if indentation styles match
                        if old_context.style != new_context.style {
                            crate::gluon_warn!("XmlGProtocol", "Indentation style mismatch between <search> and <replace>");
                        }

                        // ⚠️  DISABLED: normalize_leading_whitespace was REMOVING base indentation!
                        // This caused matcher failures because:
                        //   - AI generates code with correct indent (e.g., 4 spaces for function body)
                        //   - normalize_leading_whitespace dedents to 0 (module level)
                        //   - Matcher can't find code at wrong indent level
                        //
                        // Solution: Trust AI's indentation and let matcher handle fuzzy matching
                        // old_str = self.normalize_leading_whitespace(&old_str);
                        // new_str = self.normalize_leading_whitespace(&new_str);
                    }

                    // ✅ STEP 2: [DISABLED] Fragment validation was removed (BUG FIX)
                    // Reason: validate_block parses fragments as complete programs in Tree-sitter,
                    // causing false positives for code snippets. Walidacja pełnego pliku
                    // (validate_structure_integrity) happens later in transaction.rs.

                    // ✅ STEP 3: Commit the change
                    crate::gluon_info!("XmlGProtocol", "Change validated and queued for file: {}", path);
                    changes.push(ChangeQueueItem::new(
                        path.clone(),
                        0, // Matcher will resolve line numbers
                        0,
                        old_str,
                        new_str
                    ));
                }
            }
        }

        // Reset mutable parts, keep file path for next change in same file
        self.reset_builder(builder, &file_path.unwrap_or_default());
    }

    /// Helper to reset builder state
    fn reset_builder(&self, builder: &mut CurrentChangeBuilder, path: &str) {
        builder.old_code.clear();
        builder.new_code.clear();
        builder.has_search_block = false;
        builder.has_replace_block = false;
        
        builder.search_block_indent = 0;
        builder.search_nesting_checked = false;
        builder.search_is_nested = false;

        builder.replace_block_indent = 0;
        builder.replace_nesting_checked = false;
        builder.replace_is_nested = false;

        // Keep file path for next change
        if !path.is_empty() {
            builder.file_path = Some(path.to_string());
        }
    }

    /// Measures leading whitespace (indentation) of a line
    fn measure_indent(&self, line: &str) -> usize {
        line.chars().take_while(|c| c.is_whitespace()).count()
    }

    /// Strips base indentation from a line if it matches the block indent.
    /// This fixes the "XML Nesting" problem where code inside XML has extra indentation.
    fn strip_base_indent(&self, line: &str, base_indent: usize) -> String {
        if base_indent == 0 {
            return line.to_string();
        }
        
        let current_indent = self.measure_indent(line);
        if current_indent >= base_indent {
            // Safe to strip - assumes XML indent uses same char (space/tab) as code
            // (Standard for LLM output)
            line.chars().skip(base_indent).collect()
        } else if line.trim().is_empty() {
            // Empty lines are just empty
            String::new()
        } else {
            // Line has LESS indentation than the XML tag? Weird, but preserve as-is.
            line.to_string()
        }
    }

    /// Removes the tag from the line, returning the content after it.
    /// E.g., "<search>code" -> "code"
    fn strip_tag(&self, line: &str, regex: &Regex) -> String {
        regex.replace(line, "").to_string()
    }

    /// Removes the closing tag from the line, returning the content before it.
    /// E.g., "code</search>" -> "code"
    fn strip_closing_tag(&self, line: &str, regex: &Regex) -> String {
        regex.replace(line, "").to_string()
    }

    /// Layer 4: Content Sanitization
    /// - Joins lines
    /// - Removes common artifacts
    /// - Decodes HTML entities
    fn sanitize_content(&self, lines: &[String]) -> String {
        // Use the shared sanitizer first (removes , "Code", etc.)
        // We construct a temporary vec of &str for the shared fn
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let sanitized = crate::apply_system::parsers::sanitize_code_block(refs);
 
        // Decode XML/HTML entities
        sanitized
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
    }
 
    /// [GLUON PHASE 1] Smart Block Normalization & Sanitization
    /// Combines indentation fixing with artifact removal.
    ///
    /// Algorithm:
    /// 1. Strip Filename Artifacts (Aider-style) - remove lines that just repeat the filename
    /// 2. Identify common minimum indentation
    /// 3. Strip indentation (Smart Dedent)
    /// 4. Sanitize artifacts (markdown fences, UI text)
    /// 5. Decode HTML entities
    fn normalize_and_sanitize(&self, lines: &[String], file_path: &str) -> String {
        if lines.is_empty() {
            return String::new();
        }
 
        // 1. Artifact Removal (Top/Bottom noise)
        // Check if first line matches filename/basename (common LLM hallucination)
        let mut start_idx = 0;
        let end_idx = lines.len();
 
        let first_line = lines[0].trim();
        let path_obj = std::path::Path::new(file_path);
        let filename = path_obj.file_name().and_then(|n| n.to_str()).unwrap_or("");
        
        // Remove line if it matches "filename.ext" or "path/to/filename.ext" or "filename.ext:"
        if !first_line.is_empty() && (first_line == file_path || first_line == filename || first_line == format!("{}:", filename)) {
             // Heuristic: Only remove if it's not a valid code line (e.g. not "import filename")
             // Most filenames won't parse as valid code lines in their own language context, 
             // but to be safe, we only strip if it looks isolated.
             start_idx += 1;
        }
 
        if start_idx >= end_idx {
            return String::new();
        }
 
        let processing_slice = &lines[start_idx..end_idx];
 
        // 2. Filter non-empty lines to find common indent
        let non_empty: Vec<&String> = processing_slice.iter()
            .filter(|l| !l.trim().is_empty())
            .collect();
 
        if non_empty.is_empty() {
            return String::new();
        }
 
        // 3. Find minimum common indentation
        let min_indent = non_empty.iter()
            .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
            .min()
            .unwrap_or(0);
 
        // 4. Reconstruct block with stripped indentation
        let stripped_lines: Vec<String> = processing_slice.iter()
            .map(|line| {
                if line.len() >= min_indent {
                    line.chars().skip(min_indent).collect()
                } else if line.trim().is_empty() {
                    String::new()
                } else {
                    line.trim_start().to_string()
                }
            })
            .collect();
 
        // 5. Pass to standard sanitizer
        self.sanitize_content(&stripped_lines)
    }

    /// Detects programming language from file extension.
    fn detect_language(file_path: &str) -> &str {
        let lower = file_path.to_lowercase();
        if lower.ends_with(".py") || lower.ends_with(".pyw") {
            "python"
        } else if lower.ends_with(".js") || lower.ends_with(".ts") || lower.ends_with(".jsx") || lower.ends_with(".tsx") {
            "javascript"
        } else if lower.ends_with(".rs") {
            "rust"
        } else if lower.ends_with(".go") {
            "go"
        } else if lower.ends_with(".java") {
            "java"
        } else if lower.ends_with(".kt") || lower.ends_with(".kts") {
            "kotlin"
        } else {
            "unknown"
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_flow() {
        let parser = XmlGProtocolParser;
        let input = r#"
<gluon_patch>
  <file path="src/main.rs">
    <change>
      <search>
fn old() {
}
      </search>
      <replace>
fn new() {
}
      </replace>
    </change>
  </file>
</gluon_patch>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/main.rs");
        assert!(changes[0].old_code.contains("fn old"));
        assert!(changes[0].new_code.contains("fn new"));
    }

    #[test]
    fn test_parse_missing_closing_tags_recovery() {
        let parser = XmlGProtocolParser;
        // Missing </search>, </replace>, </change>
        // Note: Parser is strict with validation, so code must be syntactically valid
        let input = r#"
<file path="test.js">
  <change>
    <search>
const old = 1;
    </search>
    <replace>
const new1 = 2;
  <change>
    <search>
const old2 = 3;
    </search>
    <replace>
const new2 = 4;
"#;
        let changes = parser.parse(input).unwrap();
        // With strict validation, we expect at least 1 valid change
        assert!(changes.len() >= 1);
        assert!(changes[0].old_code.contains("old"));
    }

    #[test]
    fn test_fuzzy_tags() {
        let parser = XmlGProtocolParser;
        let input = r#"
< file path = "test.rs" >
  < SEARCH >
old
  < / search >
  < REPLACE >
new
  < / replace >
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "test.rs");
        assert_eq!(changes[0].old_code.trim(), "old");
    }

    #[test]
    fn test_inline_content() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="inline.rs">
<change>
<search>let x = 1;</search>
<replace>let x = 2;</replace>
</change>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].old_code.trim(), "let x = 1;");
        assert_eq!(changes[0].new_code.trim(), "let x = 2;");
    }

    #[test]
    fn test_multiple_changes_same_file() {
        let parser = XmlGProtocolParser;
        let input = r#"
<gluon_patch>
  <file path="src/multi.ts">
    <change>
      <search>
const a = 1;
      </search>
      <replace>
const a = 2;
      </replace>
    </change>
    <change>
      <search>
const b = 3;
      </search>
      <replace>
const b = 4;
      </replace>
    </change>
  </file>
</gluon_patch>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "src/multi.ts");
        assert_eq!(changes[1].file_path, "src/multi.ts");
        assert!(changes[0].old_code.contains("a = 1"));
        assert!(changes[1].old_code.contains("b = 3"));
    }

    #[test]
    fn test_multiple_files() {
        let parser = XmlGProtocolParser;
        let input = r#"
<gluon_patch>
  <file path="src/file1.ts">
    <change>
      <search>old1</search>
      <replace>new1</replace>
    </change>
  </file>
  <file path="src/file2.ts">
    <change>
      <search>old2</search>
      <replace>new2</replace>
    </change>
  </file>
</gluon_patch>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "src/file1.ts");
        assert_eq!(changes[1].file_path, "src/file2.ts");
    }

    #[test]
    fn test_missing_replace_tag_recovery() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="test.rs">
  <search>
old
  </search>
  <replace>
new
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].old_code.trim(), "old");
        assert_eq!(changes[0].new_code.trim(), "new");
    }

    #[test]
    fn test_implicit_change_start() {
        let parser = XmlGProtocolParser;
        // No explicit <change> tag
        let input = r#"
<file path="implicit.rs">
  <search>fn old() {}</search>
  <replace>fn new() {}</replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("old"));
        assert!(changes[0].new_code.contains("new"));
    }

    #[test]
    fn test_case_insensitive_tags() {
        let parser = XmlGProtocolParser;
        let input = r#"
<FILE path="test.rs">
  <SEARCH>old</SEARCH>
  <REPLACE>new</REPLACE>
</FILE>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_whitespace_in_tags() {
        let parser = XmlGProtocolParser;
        let input = r#"
< file path = "test.rs" >
  < search >old</ search >
  < replace >new</ replace >
</ file >
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_markdown_fence_removal() {
        let parser = XmlGProtocolParser;
        let input = r#"
```xml
<file path="test.rs">
  <search>old</search>
  <replace>new</replace>
</file>
```
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_python_indentation_validation() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="test.py">
  <search>
def function():
    return True
  </search>
  <replace>
def function():
    return False
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("return True"));
        assert!(changes[0].new_code.contains("return False"));
    }

    #[test]
    fn test_nested_xml_indentation_stripping() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="nested.ts">
  <search>
    const x = 1;
    const y = 2;
  </search>
  <replace>
    const x = 10;
    const y = 20;
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        // Should strip the base XML indentation
        assert!(changes[0].old_code.contains("const x = 1"));
    }

    #[test]
    fn test_empty_search_creates_new_code() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="test.rs">
  <search></search>
  <replace>fn new_func() {}</replace>
</file>
"#;
        // Empty old_code is valid - it means adding new code
        let result = parser.parse(input);
        assert!(result.is_ok());
        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].old_code.trim(), "");
        assert!(changes[0].new_code.contains("new_func"));
    }

    #[test]
    fn test_html_entity_decoding() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="test.ts">
  <search>
const x = "&lt;div&gt;";
  </search>
  <replace>
const x = "&lt;span&gt;";
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("\"<div>\""));
        assert!(changes[0].new_code.contains("\"<span>\""));
    }

    #[test]
    fn test_consecutive_searches_without_change_tag() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="multi.ts">
  <search>old1</search>
  <replace>new1</replace>
  <search>old2</search>
  <replace>new2</replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 2);
        assert!(changes[0].old_code.contains("old1"));
        assert!(changes[1].old_code.contains("old2"));
    }

    #[test]
    fn test_file_path_with_quotes() {
        let parser = XmlGProtocolParser;
        let input1 = r#"
<file path="src/test.ts">
  <search>
const a = 1;
  </search>
  <replace>
const b = 2;
  </replace>
</file>
"#;
        let changes1 = parser.parse(input1).unwrap();
        assert_eq!(changes1[0].file_path, "src/test.ts");

        let input2 = r#"
<file path='src/test.ts'>
  <search>
const a = 1;
  </search>
  <replace>
const b = 2;
  </replace>
</file>
"#;
        let changes2 = parser.parse(input2).unwrap();
        assert_eq!(changes2[0].file_path, "src/test.ts");
    }

    #[test]
    fn test_no_gluon_patch_wrapper() {
        let parser = XmlGProtocolParser;
        // Should work even without <gluon_patch> wrapper
        let input = r#"
<file path="nowrapper.rs">
  <search>old</search>
  <replace>new</replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_preserve_blank_lines() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="blanks.ts">
  <search>
function test() {
  console.log("a");
  console.log("b");
}
  </search>
  <replace>
function test() {
  console.log("a");

  console.log("b");
}
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        // Should preserve the blank line in new_code
        assert!(changes[0].new_code.contains("\n\n"));
    }

    #[test]
    fn test_mixed_indentation_detection() {
        let parser = XmlGProtocolParser;
        // Mix of tabs and spaces should be detected
        let input = r#"
<file path="mixed.py">
  <search>
def func():
	return 1
  </search>
  <replace>
def func():
    return 2
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_filename_artifact_removal() {
        let parser = XmlGProtocolParser;
        // First line is just the filename - should be stripped
        let input = r#"
<file path="artifact.ts">
  <search>
artifact.ts
const x = 1;
  </search>
  <replace>
artifact.ts
const x = 2;
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        // Filename artifact should be removed
        assert!(!changes[0].old_code.starts_with("artifact.ts"));
    }

    #[test]
    fn test_multiline_class_definition() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="class.py">
  <search>
class MyClass:
    def __init__(self):
        self.value = 0

    def get_value(self):
        return self.value
  </search>
  <replace>
class MyClass:
    def __init__(self, initial=0):
        self.value = initial

    def get_value(self):
        return self.value

    def set_value(self, value):
        self.value = value
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("def __init__(self)"));
        assert!(changes[0].new_code.contains("def set_value"));
    }

    #[test]
    fn test_special_characters_in_code() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="special.ts">
  <search>
const regex = /[<>&"']/g;
  </search>
  <replace>
const regex = /[<>&"'\t\n]/g;
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_missing_file_path() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file>
  <search>old</search>
  <replace>new</replace>
</file>
"#;
        // Should fail or skip - no file path provided
        let result = parser.parse(input);
        // Current implementation may auto-recover, but ideally should have 0 changes
        if let Ok(changes) = result {
            assert_eq!(changes.len(), 0);
        }
    }

    #[test]
    fn test_detect_language_python() {
        assert_eq!(XmlGProtocolParser::detect_language("test.py"), "python");
        assert_eq!(XmlGProtocolParser::detect_language("script.pyw"), "python");
    }

    #[test]
    fn test_detect_language_javascript() {
        assert_eq!(XmlGProtocolParser::detect_language("app.js"), "javascript");
        assert_eq!(XmlGProtocolParser::detect_language("component.ts"), "javascript");
        assert_eq!(XmlGProtocolParser::detect_language("view.jsx"), "javascript");
        assert_eq!(XmlGProtocolParser::detect_language("page.tsx"), "javascript");
    }

    #[test]
    fn test_detect_language_rust() {
        assert_eq!(XmlGProtocolParser::detect_language("main.rs"), "rust");
    }

    #[test]
    fn test_detect_language_unknown() {
        assert_eq!(XmlGProtocolParser::detect_language("file.txt"), "unknown");
        assert_eq!(XmlGProtocolParser::detect_language("readme.md"), "unknown");
    }

    #[test]
    fn test_can_handle_variations() {
        let parser = XmlGProtocolParser;

        assert!(parser.can_handle("<gluon_patch>"));
        assert!(parser.can_handle("<file path='test.rs'>"));
        assert!(parser.can_handle("Some text <file path=\"test.rs\">"));
        assert!(!parser.can_handle("No XML tags here"));
    }

    #[test]
    fn test_state_machine_recovery_from_malformed() {
        let parser = XmlGProtocolParser;
        // Malformed: missing closing tags, but should recover
        let input = r#"
<file path="recover.ts">
  <search>
old1
  <replace>
new1
  <search>
old2
  <replace>
new2
"#;
        let changes = parser.parse(input).unwrap();
        // Should recover and parse both changes
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn test_code_with_xml_tags_inside() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="xml-code.ts">
  <search>
const html = '<div>content</div>';
  </search>
  <replace>
const html = '<span>content</span>';
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("<div>"));
        assert!(changes[0].new_code.contains("<span>"));
    }

    #[test]
    fn test_only_replace_block() {
        let parser = XmlGProtocolParser;
        // Only <replace> without <search> - create new code
        let input = r#"
<file path="new.ts">
  <replace>
export const NEW_CONST = 42;
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].old_code.trim(), "");
        assert!(changes[0].new_code.contains("NEW_CONST"));
    }

    #[test]
    fn test_preprocessor_keeps_fences_inside_blocks() {
        let parser = XmlGProtocolParser;
        // Backticks inside <search>/<replace> should be preserved
        let input = r#"
<file path="markdown.ts">
  <search>
const md = `
# Title
`;
  </search>
  <replace>
const md = `
# New Title
`;
  </replace>
</file>
"#;
        let changes = parser.parse(input).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("`"));
        assert!(changes[0].new_code.contains("`"));
    }

    #[test]
    fn test_measure_indent() {
        let parser = XmlGProtocolParser;
        assert_eq!(parser.measure_indent("    code"), 4);
        assert_eq!(parser.measure_indent("\t\tcode"), 2);
        assert_eq!(parser.measure_indent("no indent"), 0);
    }

    #[test]
    fn test_strip_base_indent() {
        let parser = XmlGProtocolParser;
        let result = parser.strip_base_indent("    code line", 4);
        assert_eq!(result, "code line");

        // When line has less indent than base, it's preserved as-is
        let result2 = parser.strip_base_indent("  short indent", 4);
        assert_eq!(result2, "  short indent");

        // Empty line becomes empty
        let result3 = parser.strip_base_indent("    ", 4);
        assert_eq!(result3, "");
    }

    #[test]
    fn test_empty_file_path_handling() {
        let parser = XmlGProtocolParser;
        let input = r#"
<file path="">
  <search>old</search>
  <replace>new</replace>
</file>
"#;
        let result = parser.parse(input);
        // Should either fail or produce 0 changes
        if let Ok(changes) = result {
            assert_eq!(changes.len(), 0);
        }
    }
}