//! KROK 3: Transaction System & Safety Layer
//! 
//! Handles Atomic Transactions (All-or-Nothing execution) and Destruction Guards.
//! 
//! Features:
//! - In-memory simulation of changes before writing to disk
//! - Snapshotting and Rollback capability
//! - Heuristics to prevent accidental code deletion

use crate::apply_system::shared::types::{ChangeQueueItem, ChangeStatus};
use crate::apply_system::matchers::match_code;
use std::collections::HashMap;
use tokio::fs;
use regex::Regex;

/// Represents the state of a file during a transaction
struct FileState {
    /// Original content on disk (for rollback)
    original_content: String,
    /// Current content in memory (after applying previous patches in this batch)
    working_content: String,
    /// Has this file been modified in memory?
    is_modified: bool,
}

pub struct TransactionManager {
    /// Map of file path -> file state
    files: HashMap<String, FileState>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Execute a batch of changes atomically.
    /// 
    /// 1. Load all affected files into memory.
    /// 2. Apply changes sequentially in memory (Dry Run).
    /// 3. Validate safety (Destruction Guard).
    /// 4. If all good -> Write to disk.
    /// 5. If any error -> Discard changes (nothing was written yet).
    pub async fn execute_batch(
        &mut self, 
        changes: &mut [ChangeQueueItem]
    ) -> Result<BatchResult, String> {
        // Phase 1: Load Files & Snapshot
        for change in changes.iter() {
            if !self.files.contains_key(&change.file_path) {
                let content = match fs::read_to_string(&change.file_path).await {
                    Ok(c) => c,
                    Err(_) => {
                        // Handle new file creation case
                        if change.old_code.is_empty() {
                            String::new()
                        } else {
                            return Err(format!("Failed to read file for transaction: {}", change.file_path));
                        }
                    }
                };
                
                self.files.insert(change.file_path.clone(), FileState {
                    original_content: content.clone(),
                    working_content: content,
                    is_modified: false,
                });
            }
        }

        let mut applied_indices = Vec::new();

        // Phase 2: In-Memory Application & Matching
        for (idx, change) in changes.iter_mut().enumerate() {
            // Skip already processed/failed items
            if !change.can_apply() { continue; }

            let file_state = self.files.get_mut(&change.file_path).unwrap();

            // [GLUON IDEMPOTENCY CHECK] Detect already-applied changes before even matching.
            // If new_code is already in the file AND old_code is no longer present, the change was
            // applied in a PREVIOUS session. Re-applying it would corrupt the file (BUG B).
            // Safe resolution: mark as Applied (idempotent success) and skip.
            if !change.new_code.trim().is_empty() && !change.old_code.trim().is_empty() {
                let new_already_present = file_state.working_content.contains(change.new_code.trim());
                let old_still_present = file_state.working_content.contains(change.old_code.trim());

                if new_already_present && !old_still_present {
                    // Change was already applied — skip safely without touching the file.
                    eprintln!(
                        "[Gluon Idempotency] Change '{}': new_code already in file and old_code is gone. \
                        This change was applied in a previous session. Skipping to avoid duplication.",
                        change.id
                    );
                    change.status = ChangeStatus::Applied;
                    change.applied_timestamp = Some(std::time::SystemTime::now());
                    applied_indices.push(idx);
                    continue;
                }
            }

            // 2a. Match against WORKING content (not disk content!)
            // This allows applying multiple patches to the same file in one batch
            let match_result = match match_code(&file_state.working_content, &change.old_code, change.line_start, Some(&change.file_path)) {
                Ok(res) => res,
                Err(e) => {
                    change.status = ChangeStatus::Failed;
                    change.error_message = Some(format!("Matching failed during dry-run: {:?}", e));
                    return Err(format!("Transaction aborted: Match failed for change {}", change.id));
                }
            };

            // 2b. Validate Destruction Guard
            if let Err(safety_err) = DestructionGuard::validate(&file_state.working_content, &change.new_code, &match_result) {
                change.status = ChangeStatus::Failed;
                change.error_message = Some(safety_err.clone());
                return Err(format!("Transaction aborted (Safety): {}", safety_err));
            }

            // 2c. [NEW] Check removal threshold (Continue-style)
            if let Err(removal_err) = DestructionGuard::check_removal_threshold(&change.old_code, &change.new_code) {
                change.status = ChangeStatus::Failed;
                change.error_message = Some(removal_err.clone());
                return Err(format!("Transaction aborted (Excessive Removal): {}", removal_err));
            }

            // [GLUON FIX 2.2] Anti-Stutter Pre-Check (ABORT mode)
            // If new_code already exists OUTSIDE the matched region, this change would duplicate content.
            // This catches the case where the model retries an already-applied change (BUG A).
            if !change.new_code.trim().is_empty() {
                if let Some(existing_pos) = file_state.working_content.find(change.new_code.trim()) {
                    let existing_line = file_state.working_content[..existing_pos].lines().count() + 1;
                    let in_match_region = existing_line >= match_result.matched_line_start
                        && existing_line <= match_result.matched_line_end;
                    if !in_match_region {
                        change.status = ChangeStatus::Failed;
                        let err_msg = format!(
                            "Stutter Guard: replace_code already exists in file at line {} (outside match region {}–{}). \
                            This change was likely already applied in a previous session. Skipping to prevent duplication.",
                            existing_line,
                            match_result.matched_line_start,
                            match_result.matched_line_end
                        );
                        change.error_message = Some(err_msg.clone());
                        return Err(err_msg);
                    }
                }
            }

            // [GLUON SEARCH CODE VALIDATION] Check if search_code itself has invalid Python indentation.
            // If search_code is malformed, it might be copied from already-broken code in the file,
            // and the replacement will perpetuate or worsen the error. Abort before attempting match.
            if change.file_path.ends_with(".py") && !change.old_code.trim().is_empty() {
                use crate::apply_system::analysis::validation::AstValidator;
                if let Err(search_err) = AstValidator::validate_python_impossible_nesting(&change.old_code) {
                    change.status = ChangeStatus::Failed;
                    change.error_message = Some(format!("search_code is malformed: {}", search_err));
                    return Err(format!("Transaction aborted (Search Code Validation): search_code has invalid structure: {}", search_err));
                }
            }

            // [GLUON PYTHON INDENT GUARD] Pre-apply: validate replace_code has valid Python indentation.
            // Catches BUG 2: model generated `cd, created = ...\n    if created:` (if nested inside assignment).
            // An assignment/expression cannot create a block scope in Python — only `:` does.
            if change.file_path.ends_with(".py") && !change.new_code.trim().is_empty() {
                use crate::apply_system::analysis::validation::AstValidator;
                if let Err(nesting_err) = AstValidator::validate_python_impossible_nesting(&change.new_code) {
                    change.status = ChangeStatus::Failed;
                    change.error_message = Some(nesting_err.clone());
                    return Err(format!("Transaction aborted (Python Indent Guard): {}", nesting_err));
                }
            }

            // [GLUON JSX BRACKET GUARD] Pre-apply: validate replace_code bracket balance for JSX/JS/TS.
            // Catches BUG 3: model produced incomplete replacement ending mid-JSX-element (unclosed tag/callback).
            // An unbalanced replace_code would corrupt the file structure.
            {
                let is_jsx_file = change.file_path.ends_with(".jsx")
                    || change.file_path.ends_with(".tsx")
                    || change.file_path.ends_with(".js")
                    || change.file_path.ends_with(".ts");
                if is_jsx_file && !change.new_code.trim().is_empty() {
                    use crate::apply_system::analysis::validation::AstValidator;
                    if let Err(bracket_err) = AstValidator::validate_jsx_bracket_balance(&change.new_code) {
                        change.status = ChangeStatus::Failed;
                        change.error_message = Some(bracket_err.clone());
                        return Err(format!("Transaction aborted (JSX Bracket Guard): {}", bracket_err));
                    }
                }
            }

            // [GLUON V5 + V6] Context & Indentation Intelligence
            use crate::apply_system::matchers::utils::{smart_adjust_indentation, expand_context_backwards, expand_context_forward};

            // 1. Check for Context Overlap (Duplication Prevention) - BACKWARD
            // If new code repeats headers existing before the match, expand match upwards.
            let adjusted_start = expand_context_backwards(
                &file_state.working_content,
                match_result.matched_line_start,
                &change.new_code
            );

            // [GLUON V6] NEW: Forward Expansion (Prevents "code appending" bug)
            // If new code repeats footers existing after the match, expand match downwards.
            let adjusted_end = expand_context_forward(
                &file_state.working_content,
                match_result.matched_line_end,
                &change.new_code
            );

            if adjusted_start != match_result.matched_line_start {
                eprintln!("[Gluon SmartFix] Expanded match start from {} to {} to prevent duplication.",
                    match_result.matched_line_start, adjusted_start);
            }

            if adjusted_end != match_result.matched_line_end {
                eprintln!("[Gluon SmartFix] Expanded match end from {} to {} to prevent code appending.",
                    match_result.matched_line_end, adjusted_end);
            }
 
            // 2. Smart Indentation Adjustment
            // Adjust the indentation of new_code to match the (potentially new) destination anchor
            let adjusted_new_code = smart_adjust_indentation(
                &file_state.working_content,
                adjusted_start,
                &change.new_code
            );
 
            // 3. Apply to memory buffer (with vertical compression and adjusted boundaries)
            let new_content = apply_patch_string(
                &file_state.working_content,
                &adjusted_new_code,
                adjusted_start,
                adjusted_end
            )?;

            // [GLUON ORPHAN INDENT GUARD] Post-apply: check for orphaned over-indented code.
            // Catches BUG 1: replacement removed a structural block (if/for/with) but left the
            // body code at its original deeper indentation, creating Python IndentationError.
            // e.g.: replacement ends at indent=8, but next line in file is at indent=12 (was inside removed `if`).
            if change.file_path.ends_with(".py") {
                if let Some(orphan_err) = check_post_replacement_orphan(
                    &new_content,
                    adjusted_end,
                    &adjusted_new_code,
                ) {
                    change.status = ChangeStatus::Failed;
                    change.error_message = Some(orphan_err.clone());
                    return Err(format!("Transaction aborted (Orphan Indent Guard): {}", orphan_err));
                }
            }

            // [GLUON FIX 2.3] Anti-Stutter Post-Check (Duplication Guard)
            // Verify if the patch resulted in immediate duplication of the inserted block.
            // This happens when matching finds the start but misses the end, causing an append.
            if let Err(stutter_err) = DestructionGuard::check_result_redundancy(&new_content, &adjusted_new_code, adjusted_start) {
                change.status = ChangeStatus::Failed;
                change.error_message = Some(stutter_err.clone());
                return Err(format!("Transaction aborted (Stutter Guard): {}", stutter_err));
            }
 
            // [SYSTEM B] Post-Process Integrity Guard (Deep Simulation)
            // We perform a comparative analysis of the file health BEFORE and AFTER the change.
            use crate::apply_system::analysis::simulation::{SimulationGuard, IntegrityStatus};
            use crate::apply_system::validators::SyntaxValidator;
            let validator = SyntaxValidator;
 
            // 1. Strict Structure Validation (The "Syntax Firewall")
            // If the IndentationNormalizer failed and produced invalid Python code, this check MUST catch it.
            // We prioritize this over "comparative" analysis because syntax errors are fatal.
            if let Err(new_issues) = validator.validate_structure_integrity(&new_content, &change.file_path) {
                match validator.validate_structure_integrity(&file_state.working_content, &change.file_path) {
                    Ok(_) => {
                        // Original was valid, new is invalid -> ABORT immediately.
                        change.status = ChangeStatus::Failed;
                        let first_err = &new_issues[0];
                        let err_msg = format!(
                            "Transaction Aborted: Patch introduced critical syntax errors (Line {}): {}",
                            first_err.line_number.unwrap_or(0),
                            first_err.message
                        );
                        change.error_message = Some(err_msg.clone());
                        return Err(err_msg);
                    }
                    Err(orig_issues) => {
                        // [GLUON FIX 5] Both original and new have errors.
                        // Old behavior: silently allow (leniency bypass). This masked BUG 1.
                        // New behavior: check if any new errors are at DIFFERENT lines than original.
                        // If the patch MOVED or ADDED errors at new lines → ABORT (newly introduced damage).
                        use crate::apply_system::validators::syntax_validator::ValidationSeverity;
                        let orig_error_lines: std::collections::HashSet<usize> = orig_issues
                            .iter()
                            .filter(|i| i.severity == ValidationSeverity::Error)
                            .filter_map(|i| i.line_number)
                            .collect();

                        let new_errors_at_new_lines: Vec<_> = new_issues
                            .iter()
                            .filter(|i| i.severity == ValidationSeverity::Error)
                            .filter(|i| i.line_number.map_or(true, |ln| !orig_error_lines.contains(&ln)))
                            .collect();

                        if !new_errors_at_new_lines.is_empty() {
                            change.status = ChangeStatus::Failed;
                            let first_err = &new_errors_at_new_lines[0];
                            let err_msg = format!(
                                "Transaction Aborted: Patch introduced NEW syntax errors at line {} not present in original: {}. \
                                Original file had {} pre-existing error(s) — those are preserved as-is, \
                                but newly introduced errors at different lines are not allowed.",
                                first_err.line_number.unwrap_or(0),
                                first_err.message,
                                orig_issues.len()
                            );
                            change.error_message = Some(err_msg.clone());
                            return Err(err_msg);
                        }
                        // All new errors are at lines that were already broken in the original — proceed.
                        eprintln!("[Gluon Fix5] Original file had pre-existing syntax errors. Patch did not introduce errors at new lines — proceeding.");
                    }
                }
            }
            
            // 2. Comparative AST Analysis (Regression Testing)
            // Even if syntax is valid, did we introduce *more* semantic issues?
            match SimulationGuard::check_integrity(&file_state.working_content, &new_content, &change.file_path) {
                IntegrityStatus::Degraded { new_error_count, original_error_count, first_error_msg } => {
                    change.status = ChangeStatus::Failed;
                    let err_msg = format!(
                        "Transaction Guard Abort: Code quality degraded. Errors increased from {} to {}. Hint: {}", 
                        original_error_count, new_error_count, first_error_msg
                    );
                    change.error_message = Some(err_msg.clone());
                    return Err(err_msg);
                },
                _ => {} // Safe, Improved, or Neutral - Proceed
            }
 
            // 2. Global Bracket/Parenthesis Balance Check (Language Agnostic Failsafe)
            // This catches massive structure breaks that might confuse the AST parser or pass as "Neutral"
            // if the original file was also garbage. We enforce this strictly for sanity.
            let validator = SyntaxValidator;
            if let Err(issue) = validator.check_global_bracket_balance(&new_content) {
                 // Only fail if this is a NEW issue (compare with old content)
                 if validator.check_global_bracket_balance(&file_state.working_content).is_ok() {
                     change.status = ChangeStatus::Failed;
                     let err_msg = format!("Global Structure Collapse: {}", issue.message);
                     change.error_message = Some(err_msg.clone());
                     return Err(format!("Transaction aborted: {}", err_msg));
                 }
            }

            file_state.working_content = new_content;
            file_state.is_modified = true;
            
            // Update change metadata
            change.match_result = Some(match_result);
            applied_indices.push(idx);
        }

        // Phase 3: Commit to Disk
        for (path, state) in &self.files {
            if state.is_modified {
                if let Err(e) = fs::write(path, &state.working_content).await {
                    // Critical IO Error during commit - attempt rollback of previous files
                    // (Though in this architecture, we haven't written previous files yet, 
                    // loop order matters. If we wrote file A and fail on file B, we must revert A).
                    self.rollback_written_files().await; 
                    return Err(format!("IO Error during commit on {}: {}. Transaction rolled back.", path, e));
                }
            }
        }

        // Phase 4: Finalize Status
        for idx in applied_indices {
            changes[idx].status = ChangeStatus::Applied;
            changes[idx].applied_timestamp = Some(std::time::SystemTime::now());
        }

        Ok(BatchResult {
            files_modified: self.files.values().filter(|f| f.is_modified).count(),
        })
    }

    /// Rollback mechanism (used if IO error occurs mid-commit)
    async fn rollback_written_files(&self) {
        eprintln!("[Transaction] 🚨 CRITICAL: Starting atomic rollback of all modified files...");
        
        for (path, state) in &self.files {
            // We rollback ALL files in the transaction scope, not just the ones that were modified,
            // to guarantee state consistency with the snapshot.
            if state.is_modified {
                eprintln!("[Transaction] ↺ Restoring original content for: {}", path);
                // Restore original content
                match fs::write(path, &state.original_content).await {
                    Ok(_) => eprintln!("[Transaction]    ✅ Restored: {}", path),
                    Err(e) => eprintln!("[Transaction]    ❌ FAILED to restore {}: {}. Manual intervention required!", path, e),
                }
            }
        }
        eprintln!("[Transaction] Rollback sequence completed.");
    }
}

pub struct BatchResult {
    pub files_modified: usize,
}

// ============================================================================
// Destruction Guard Logic
// ============================================================================

struct DestructionGuard;

impl DestructionGuard {
    /// Validates if a change is safe to apply.
    /// Checks for:
    /// 1. Lazy Coding (Zombie Code)
    /// 2. Massive deletion (Code ratio)
    /// 3. Loss of structural keywords
    /// 4. [GLUON HARDENING] Symbol Continuity (Zombie Node)
    /// 5. [NEW] AST Integrity Verification
    /// 6. [NEW] Continue-style removal threshold (>30%)
    fn validate(
        full_content: &str,
        new_fragment: &str,
        match_res: &crate::apply_system::MatchResult
    ) -> Result<(), String> {
        // 0. Failsafe: Lazy Coding Detection
        if let Err(e) = crate::apply_system::parsers::detect_lazy_coding(new_fragment) {
            return Err(format!("Destruction Guard: {}", e));
        }
 
        let lines: Vec<&str> = full_content.lines().collect();
        
        // Calculate old fragment size
        let start_idx = match_res.matched_line_start.saturating_sub(1);
        let end_idx = match_res.matched_line_end.min(lines.len());
        
        // 4. [GLUON HARDENING] Symbol Continuity Check (Zombie Node)
        // Check if we are deleting a function definition without replacing it.
        // We only check this if we are replacing a substantial block.
        if start_idx < end_idx {
            let old_fragment = lines[start_idx..end_idx].join("\n");
            
            // Quick heuristic regex to find function names
            // (Full AST parse is too expensive here, we use regex for speed in Guard)
            // Look for "def name" or "fn name"
            // [GLUON FIX] Use imported Regex directly to avoid unused import warning
            let def_re = Regex::new(r"(?m)^\s*(?:def|fn|class)\s+([a-zA-Z0-9_]+)").unwrap();
            
            for cap in def_re.captures_iter(&old_fragment) {
                if let Some(name) = cap.get(1) {
                    let symbol = name.as_str();
                    // If the symbol existed in OLD, it SHOULD exist in NEW, unless explicitly deleted.
                    // Explicit delete usually means new_fragment is empty or very small.
                    // If new_fragment is large but missing the symbol, it's likely an "orphaned body" bug.
                    
                    if !new_fragment.contains(symbol) && new_fragment.len() > 50 {
                        // Exception: rename? We assume rename is rare in "apply".
                        // Warning only, as it might be intentional.
                        // But for "def validate" disappearing, this catches it.
                        // We check if the NEW code has *any* definition.
                        if !def_re.is_match(new_fragment) {
                             return Err(format!(
                                "Destruction Guard: Definition of '{}' was removed, but a code block remains. \
                                Did you accidentally remove the function header? (Zombie Node Protection)", 
                                symbol
                            ));
                        }
                    }
                }
            }
        }
        let start_idx = match_res.matched_line_start.saturating_sub(1);
        let end_idx = match_res.matched_line_end.min(lines.len());
        
        if start_idx >= end_idx {
            // Insertion or empty replacement - usually safe
            return Ok(());
        }

        let old_fragment = lines[start_idx..end_idx].join("\n");
        
        // 1. Ratio Check (Token-Based)
        // Instead of checking raw bytes (which fail if code is simply reformatted/indented),
        // we check the density of semantic tokens.
        use crate::apply_system::matchers::utils::normalize_token_stream;
        
        let old_tokens = normalize_token_stream(&old_fragment);
        let new_tokens = normalize_token_stream(new_fragment);
        
        let old_len = old_tokens.len();
        let new_len = new_tokens.len();
        
        // Threshold: If we delete >90% of tokens in a large block (>50 tokens), raise alarm.
        if old_len > 50 && (new_len as f32 / old_len as f32) < 0.1 {
            // Exception: Check if new code contains explicit TODOs or placeholders
            if !new_fragment.contains("TODO") && !new_fragment.contains("FIXME") && !new_fragment.trim().is_empty() {
                return Err(format!(
                    "Destruction Guard: Attempt to replace {} tokens with {} tokens (<10%). Looks like accidental deletion of logic.",
                    old_len, new_len
                ));
            }
        }

        // 2. Structural Keyword Safety
        // If old code had "class" or "function" and new code doesn't, warn.
        let keywords = vec!["class ", "struct ", "function ", "fn ", "impl ", "interface "];
        
        for kw in keywords {
            if old_fragment.contains(kw) && !new_fragment.contains(kw) {
                // It's allowed to delete functions, but it's suspicious if it happens inside a generic <replace>
                // We allow it if the new code is empty (explicit delete), but warn if it's just "different" code
                if !new_fragment.trim().is_empty() {
                    // Make sure we didn't just rename it
                    // Simple heuristic: loose check
                    // return Err(format!("Destruction Guard: Original code contained '{}', but replacement does not. Verify structure.", kw));
                }
            }
        }

        Ok(())
    }

    /// Verify AST integrity before and after change
    ///
    /// This implements Continue's AST validation strategy:
    /// - Parse both old and new content with tree-sitter
    /// - Check for ERROR nodes
    /// - If new version introduces errors that old didn't have, reject
    ///
    /// Returns Ok if safe, Err with warning if risky
    #[allow(dead_code)]
    fn verify_ast_integrity(
        old_content: &str,
        new_content: &str,
        file_path: &str,
    ) -> Result<(), String> {
        use crate::apply_system::analysis::AnalysisEngine;

        // Parse old content
        let old_result = AnalysisEngine::parse_with_heuristics(old_content, file_path);
        let new_result = AnalysisEngine::parse_with_heuristics(new_content, file_path);

        // If we can't parse for this language, skip validation (not supported)
        if old_result.is_err() && new_result.is_err() {
            return Ok(());
        }

        let old_parsed = old_result.map_err(|e| format!("Failed to parse old content: {}", e))?;
        let new_parsed = new_result.map_err(|e| format!("Failed to parse new content: {}", e))?;

        // Check for ERROR nodes
        let old_has_errors = has_syntax_errors(&old_parsed.tree);
        let new_has_errors = has_syntax_errors(&new_parsed.tree);

        // If old had no errors but new has errors -> REJECT
        if !old_has_errors && new_has_errors {
            return Err(
                "AST Verification Failed: New code introduces syntax errors that didn't exist before. \
                This change is likely broken.".to_string()
            );
        }

        // If both have errors, issue warning but allow (might be fixing partial errors)
        if old_has_errors && new_has_errors {
            eprintln!("⚠️  AST Verification Warning: Both old and new code have syntax errors. Proceeding with caution.");
        }

        Ok(())
    }

    /// Check for excessive code removal (Continue's shouldRejectDiff)
    ///
    /// If >30% of lines are removals, reject the change
    fn check_removal_threshold(
        old_content: &str,
        new_content: &str,
    ) -> Result<(), String> {
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();

        let old_count = old_lines.len();
        let new_count = new_lines.len();

        if old_count == 0 {
            return Ok(());
        }

        // Calculate removal percentage
        let removal_count = old_count.saturating_sub(new_count);
        let removal_percentage = (removal_count as f64) / (old_count as f64);

        // Continue uses 30% threshold (REMOVAL_PERCENTAGE_THRESHOLD = 0.3)
        if removal_percentage > 0.3 {
            return Err(format!(
                "Excessive code removal detected: {:.1}% of lines removed ({} → {}). \
                This looks like accidental deletion.",
                removal_percentage * 100.0,
                old_count,
                new_count
            ));
        }

        Ok(())
    }

    /// [GLUON FIX 2.3] Anti-Stutter Check
    /// Checks if the inserted code appears twice in immediate succession in the result.
    /// This detects "Append instead of Replace" bugs.
    fn check_result_redundancy(
        result_content: &str,
        inserted_fragment: &str,
        insertion_line_start: usize // 1-based
    ) -> Result<(), String> {
        let trimmed_fragment = inserted_fragment.trim();
        if trimmed_fragment.len() < 20 { return Ok(()); } // Skip short snippets (brackets, etc)

        // Extract a window around the insertion point in the RESULT file
        let lines: Vec<&str> = result_content.lines().collect();
        let start_idx = insertion_line_start.saturating_sub(1);
        let fragment_lines = inserted_fragment.lines().count();
        
        // Window: [Insertion] + [Next N Lines]
        // If [Next N Lines] == [Insertion], we stuttred.
        let check_start = start_idx + fragment_lines;
        let check_end = (check_start + fragment_lines).min(lines.len());
        
        if check_start >= check_end { return Ok(()); }

        let following_block = lines[check_start..check_end].join("\n");
        
        // Use loose comparison (ignore indent)
        let f_norm: String = following_block.chars().filter(|c| !c.is_whitespace()).collect();
        let i_norm: String = trimmed_fragment.chars().filter(|c| !c.is_whitespace()).collect();

        if !f_norm.is_empty() && f_norm == i_norm {
            return Err(format!(
                "Stutter detected: The inserted code block appears immediately again after the insertion point. \
                This indicates the original code was not removed correctly (Append-instead-of-Replace)."
            ));
        }

        Ok(())
    }
}

/// Check if a tree-sitter tree has syntax errors (ERROR nodes)
#[allow(dead_code)]
fn has_syntax_errors(tree: &tree_sitter::Tree) -> bool {
    has_error_node_recursive(tree.root_node())
}

/// Recursively check for ERROR nodes in tree-sitter AST
#[allow(dead_code)]
fn has_error_node_recursive(node: tree_sitter::Node) -> bool {
    if node.is_error() || node.is_missing() {
        return true;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_error_node_recursive(child) {
            return true;
        }
    }

    false
}

// ============================================================================
// Helper: Python Orphan Indent Detection
// ============================================================================

/// [GLUON ORPHAN GUARD] Checks whether the line immediately following the replaced region
/// has deeper indentation than the last non-empty line of the replacement.
///
/// This detects BUG 1: model replaces a block (e.g. `queryset = annotate(...)`) that was
/// originally inside an `if normalized_phone_query:` block. After replacement the `if` is gone
/// but the code that was INSIDE it (phone_conditions lines at +4 indent) is left orphaned —
/// it has deeper indentation than the replacement tail with no structural opener before it.
///
/// Returns Some(error_message) if orphaned code is detected, None if all is fine.
fn check_post_replacement_orphan(
    content: &str,
    replacement_end_line: usize, // 1-based, exclusive end of replaced region
    replacement_code: &str,
) -> Option<String> {
    let get_indent = |line: &str| -> usize {
        let mut indent = 0usize;
        for c in line.chars() {
            match c {
                ' ' => indent += 1,
                '\t' => indent += 4,
                _ => break,
            }
        }
        indent
    };

    let opens_block = |line: &str| -> bool {
        let trimmed = line.trim_end();
        trimmed.ends_with(':')
            || trimmed.ends_with('(')
            || trimmed.ends_with('[')
            || trimmed.ends_with('{')
            || trimmed.ends_with('\\')
            || trimmed.ends_with(',')
    };

    // Find the last non-empty, non-comment line of the replacement
    let tail_line = replacement_code
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .last();

    let (tail_indent, tail_opens_block) = match tail_line {
        Some(line) => (get_indent(line), opens_block(line)),
        None => return None, // Empty replacement — nothing to check
    };

    // If replacement tail opens a block, deeper indent after it is expected
    if tail_opens_block {
        return None;
    }

    // Find the first non-empty line AFTER the replaced region in the result
    let content_lines: Vec<&str> = content.lines().collect();
    // replacement_end_line is 1-based end (exclusive), so index = replacement_end_line
    let check_from_idx = replacement_end_line; // 0-based index

    let post_line = content_lines
        .get(check_from_idx..)
        .and_then(|lines| lines.iter().find(|l| !l.trim().is_empty()))
        .copied();

    if let Some(post) = post_line {
        let post_indent = get_indent(post);
        // If next line is MORE indented than the replacement tail with no opener → orphan
        if post_indent > tail_indent {
            return Some(format!(
                "Orphaned indented code after replacement: \
                replacement ends at indent={} but the next file line '{}' has indent={}. \
                The replacement likely removed a structural block (if/for/with) while leaving \
                its body code orphaned at deeper indentation. \
                Extend search_code to include the surrounding block header and its full body.",
                tail_indent,
                post.trim(),
                post_indent
            ));
        }
    }

    None
}

// ============================================================================
// Helper: String Patching
// ============================================================================

fn apply_patch_string(
    content: &str, 
    new_fragment: &str, 
    start_line: usize, 
    end_line: usize
) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    
    // 0-based indices
    let start_idx = if start_line > 0 { start_line - 1 } else { 0 };
    let end_idx = end_line; 
 
    if start_idx > lines.len() {
        return Err("Match start line out of bounds".to_string());
    }
 
    let safe_end_idx = std::cmp::min(end_idx, lines.len());
 
    // [GLUON V5] Vertical Compression "Seam" Logic
    // Instead of simple vector extension, we build the result intelligently 
    // to avoid "Newline Sprawl" (excessive blank lines at the join points).
 
    let mut result_lines: Vec<String> = lines[..start_idx].iter().map(|s| s.to_string()).collect();

    // 2. Add NEW content
    if !new_fragment.is_empty() {
        // [GLUON V8 FIX] Critical Bug Fix: Removed redundant IndentationNormalizer here!
        // `new_fragment` was ALREADY fully indented by `smart_adjust_indentation` exactly 20 lines ago!
        // Running normalize_to_context again caused "Indentation Explosion" (doubling the delta)
        // leading to Pylance "Unexpected indentation" errors in Python.
        let fragment_lines: Vec<String> = new_fragment.lines().map(|s| s.to_string()).collect();
        
        // Vertical Compression: Top Seam
        // Ensures we don't have double blank lines between preserved code and new code
        if let Some(last_pre) = result_lines.last() {
            if let Some(first_new) = fragment_lines.first() {
                let last_empty = last_pre.trim().is_empty();
                let first_empty = first_new.trim().is_empty();

                if last_empty && first_empty {
                    // Case: Code\n\nNew -> Code\nNew
                    result_lines.pop();
                }
            }
        }

        result_lines.extend(fragment_lines);
    }
    
    // 3. Add lines AFTER the change
    let mut actual_end_idx = safe_end_idx;

    // [GLUON V6] Multi-Line Boundary Validation (2-5 lines with 70% threshold)
    // Inspired by Weighted Anchoring (Document IV, Section 3.3) and user feedback.
    // Instead of checking only 1 line, we check 2-5 lines with fuzzy similarity.
    if !new_fragment.is_empty() && actual_end_idx < lines.len() {
        use crate::apply_system::matchers::utils::calculate_similarity;

        let new_lines_vec: Vec<&str> = new_fragment.lines().collect();
        let num_lines_to_check = std::cmp::min(5, new_lines_vec.len());

        // Check last 2-5 lines of new code against next 2-5 lines in file
        for check_len in 2..=num_lines_to_check {
            if new_lines_vec.len() < check_len {
                continue; // Not enough lines in new code
            }

            // Get last N lines of new code (in correct order)
            let last_new_lines: Vec<&str> = new_lines_vec
                .iter()
                .rev()
                .take(check_len)
                .rev()
                .copied()
                .collect();

            if actual_end_idx + check_len <= lines.len() {
                let next_file_lines = &lines[actual_end_idx..actual_end_idx + check_len];

                let new_block = last_new_lines.join("\n");
                let file_block = next_file_lines.join("\n");

                let similarity = calculate_similarity(&new_block, &file_block);

                // [GLUON V7 FIX] Prevent aggressive eating of structural closures (JSX/React bug)
                let is_structural_only = new_block.chars().all(|c| c.is_whitespace() || c == '}' || c == ']' || c == ')' || c == ';' || c == '<' || c == '>' || c == '/');
                let required_similarity = if is_structural_only { 1.0 } else { 0.85 }; // Higher threshold for code, absolute for brackets

                if similarity >= required_similarity {
                    // Check exact indentation for structural closures to prevent scope eating
                    let indent_match = !is_structural_only || last_new_lines.iter().zip(next_file_lines.iter()).all(|(n, f)| {
                        n.chars().take_while(|c| c.is_whitespace()).count() == f.chars().take_while(|c| c.is_whitespace()).count()
                    });

                    if indent_match {
                        eprintln!(
                            "[Gluon V6.1] Multi-line boundary match detected: {} lines with {:.0}% similarity. Expanding end to prevent duplication.",
                            check_len,
                            similarity * 100.0
                        );
                        actual_end_idx += check_len;
                        break; // Found match, stop checking longer sequences
                    }
                }
            }
        }

        // Fallback: Single-line check (legacy behavior for edge cases)
        if actual_end_idx == safe_end_idx {
            if let Some(last_new_line) = new_fragment.lines().last() {
                if actual_end_idx < lines.len() {
                    let next_file_line = lines[actual_end_idx];
                    if last_new_line.trim() == next_file_line.trim() && !last_new_line.trim().is_empty() {
                        eprintln!("[Gluon V6] Single-line boundary match (legacy fallback).");
                        actual_end_idx += 1;
                    }
                }
            }
        }
    }
 
    // Vertical Compression: Bottom Seam
    if actual_end_idx < lines.len() {
        if let Some(last_new) = result_lines.last() {
            let next_file_line = lines[actual_end_idx];
            if last_new.trim().is_empty() && next_file_line.trim().is_empty() {
                // Case: New\n + \nFile -> New\nFile
                actual_end_idx += 1;
            }
        }
    }
 
    if actual_end_idx < lines.len() {
        result_lines.extend(lines[actual_end_idx..].iter().map(|s| s.to_string()));
    }
 
    // [GLUON V5.1] Line Ending Preservation
    // Detect if the original content uses CRLF (\r\n) or LF (\n)
    // and join the result using the same terminator.
    let line_ending = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let final_content = result_lines.join(line_ending);

    // [GLUON V7] Method 2: AST-Aware Structural Validation
    if !crate::apply_system::analysis::validation::AstValidator::validate_structure_integrity(content, &final_content) {
        // Fallback w przypadku porażki AST - często ratuje sytuację dla Pythona
        let safe_recovery = crate::apply_system::parsers::IndentationNormalizer::fix_ghost_indentation(&final_content);
        if !crate::apply_system::analysis::validation::AstValidator::validate_structure_integrity(content, &safe_recovery) {
            return Err("BŁĄD KRYTYCZNY (Failsafe AST): Wygenerowany przez model kod nie kompiluje się lub niszczy strukturę pliku.".to_string());
        } else {
            crate::gluon_info!("TransactionManager", "Uratowano strukturę pliku poprzez naprawę wcięć duchów.");
            return Ok(safe_recovery);
        }
    }

    Ok(final_content)
}