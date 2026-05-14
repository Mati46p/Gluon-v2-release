use super::Matcher;
use crate::apply_system::shared::types::{MatchResult, MatchMethod};
use crate::apply_system::analysis::{AnalysisEngine, SupportedLanguage};
use crate::apply_system::analysis::queries::QueryMatcher;
 
/// [GLUON SYSTEM A] Block Matcher (Tree-sitter Edition)
/// 
/// Uses Abstract Syntax Trees to surgically locate functions and classes.
/// This method replaces "Jąkanie" (Stuttering) by ignoring textual context
/// and matching purely on structural definitions (Name + Kind).
pub struct BlockMatcher;
 
impl Matcher for BlockMatcher {
    fn find_match(&self, file_content: &str, search_block: &str, file_path: Option<&str>) -> Option<MatchResult> {
        let path = file_path?;
        let language = SupportedLanguage::from_path(path)?;
 
        // =========================================================================================
        // [SYSTEM A] PRIMARY: AST SYMBOL MATCHING
        // Searches for definitions (Function/Class) with matching names and parents.
        // =========================================================================================
        
        // 1. Analyze the search block to find the "Dominant Definition"
        let search_result = AnalysisEngine::parse_with_heuristics(search_block, path).ok()?;
        // FIX: Use effective_code (wrapped) instead of raw search_block, because tree indices correspond to effective_code
        let search_signatures = QueryMatcher::extract_signatures(&search_result.effective_code, &search_result.tree, language);
 
        let target_sig = search_signatures.iter()
            .find(|s| s.kind == "function" || s.kind == "method" || s.kind == "class")?;
 
        // 2. Parse the target file
        let file_tree = AnalysisEngine::parse(file_content, path).ok()?;
        let file_signatures = QueryMatcher::extract_signatures(file_content, &file_tree, language);
 
        // 3. Find exact structural match
        let candidates: Vec<_> = file_signatures.iter()
            .filter(|s| {
                // Mandatory Name Match
                if s.name != target_sig.name { return false; }
                
                // Strict Kind Match
                if target_sig.kind == "class" && s.kind != "class" { return false; }
                
                // Context Guard (Parent Checking)
                let requires_parent = target_sig.kind == "method" || 
                                      (target_sig.parent_name.is_some() && target_sig.parent_name.as_deref() != Some("DummyContext"));
 
                if requires_parent {
                    if s.parent_name.is_none() { return false; } // Must have parent
                    
                    if let Some(search_parent) = &target_sig.parent_name {
                        if search_parent != "DummyContext" && s.parent_name.as_ref() != Some(search_parent) {
                            return false; // Parent name mismatch
                        }
                    }
                }
                true
            })
            .collect();
 
        // If exactly one perfect match, return it (Surgical Precision)
        if candidates.len() == 1 {
            let best_match = candidates[0];

            // [DEBUG] Log what Tree-sitter returned
            crate::gluon_info!("BlockMatcher", "Tree-sitter returned:");
            crate::gluon_info!("BlockMatcher", "start_row: {}, end_row: {}", best_match.start_row, best_match.end_row);
            let file_lines_vec: Vec<&str> = file_content.lines().collect();
            if best_match.start_row > 0 && best_match.start_row < file_lines_vec.len() {
                crate::gluon_info!("BlockMatcher", "line before start [{}]: {:?}", best_match.start_row - 1, file_lines_vec[best_match.start_row - 1]);
            }
            if best_match.start_row < file_lines_vec.len() {
                crate::gluon_info!("BlockMatcher", "start line [{}]: {:?}", best_match.start_row, file_lines_vec[best_match.start_row]);
            }

            // [GLUON V6 FIX] Smart End Detection Strategy
            // For LONG blocks (>30 lines), ALWAYS use Tree-sitter end (it's reliable for complete functions)
            // For SHORT blocks (<30 lines), use fuzzy matching (handles AI hallucinations)
            let file_lines: Vec<String> = file_content.lines().map(|s| s.to_string()).collect();
            let search_lines: Vec<String> = search_block.lines().map(|s| s.to_string()).collect();
            let start_line = best_match.start_row + 1;
            let tree_sitter_end = best_match.end_row + 1;

            let final_end_line = if search_lines.len() > 30 {
                // Long block: Trust Tree-sitter (it parsed the ACTUAL function in the file)
                crate::gluon_info!("BlockMatcher", "LONG block ({} lines) → Using Tree-sitter end: {}", search_lines.len(), tree_sitter_end);
                tree_sitter_end
            } else {
                // Short block: Use fuzzy matching to handle AI hallucinations
                let fuzzy_end = crate::apply_system::matchers::utils::find_fuzzy_block_end(
                    &file_lines,
                    start_line,
                    &search_lines,
                    0.85  // fuzzy threshold
                );
                crate::gluon_info!("BlockMatcher", "SHORT block ({} lines) → Using fuzzy end: {}", search_lines.len(), fuzzy_end);
                fuzzy_end
            };

            crate::gluon_info!("BlockMatcher", "Single match found for '{}'", target_sig.name);
            crate::gluon_info!("BlockMatcher", "Search block: {} lines", search_lines.len());
            crate::gluon_info!("BlockMatcher", "Tree-sitter end_row: {} (baseline)", tree_sitter_end);
            crate::gluon_info!("BlockMatcher", "Final selected end: {}", final_end_line);
            crate::gluon_info!("BlockMatcher", "Final range: {}-{} (exclusive end)", start_line, final_end_line);

            // [CRITICAL FIX] Do NOT add +1 here!
            // Auto-Apply uses these values as 0-indexed array indices for slicing.
            // matched_line_start is used in: lines[..matched_line_start] (keeps lines BEFORE this index)
            // So if decorator is at lines[640], we must return 640, not 641.
            return Some(MatchResult {
                matched_line_start: best_match.start_row,  // Keep as 0-indexed!
                matched_line_end: final_end_line - 1,      // Convert to 0-indexed (was 1-indexed)
                method_used: MatchMethod::BlockStructure,
                confidence: 1.0,
                details: Some(format!("System A: Surgical AST replacement of {} '{}'", target_sig.kind, target_sig.name)),
                confidence_breakdown: None,
            });
        }
 
        // =========================================================================================
        // [SYSTEM B] FALLBACK: STRUCTURAL SKELETON MATCHING
        // If name didn't match (e.g. typo) or ambiguous, check structure/signature similarity.
        // =========================================================================================
        
        if candidates.is_empty() {
            // Filter only candidates of the same KIND (e.g. only methods if we look for method)
            let structural_candidates: Vec<_> = file_signatures.iter()
                .filter(|s| {
                    if s.kind != target_sig.kind { return false; }
                    
                    // Parent Context must still match strictly (don't match method in wrong class)
                    if let Some(parent) = &target_sig.parent_name {
                        if parent != "DummyContext" && s.parent_name.as_ref() != Some(parent) {
                            return false;
                        }
                    }
                    true
                })
                .collect();
 
            // Check for Name Similarity (Typo Fixing)
            // e.g. "validate_contct" vs "validate_contact"
            let best_fuzzy = structural_candidates.iter()
                .map(|s| (s, crate::apply_system::matchers::utils::calculate_similarity(&s.name, &target_sig.name)))
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
 
            if let Some((match_sig, score)) = best_fuzzy {
                // Threshold 0.85 allows for small typos (1-2 chars) but prevents matching completely different functions
                if score > 0.85 {
                    // [CRITICAL FIX] Keep 0-indexed for Auto-Apply
                    return Some(MatchResult {
                        matched_line_start: match_sig.start_row,  // Keep as 0-indexed!
                        matched_line_end: match_sig.end_row,      // Keep as 0-indexed!
                        method_used: MatchMethod::BlockStructure,
                        confidence: 0.8, // Lower confidence due to fuzzy name
                        details: Some(format!("System B: Fuzzy AST match ('{}' ~ '{}', score: {:.2})", target_sig.name, match_sig.name, score)),
                        confidence_breakdown: None,
                    });
                }
            }
        }
 
        if candidates.len() > 1 {
            crate::gluon_warn!("BlockMatcher", "Ambiguous match! Found {} candidates for '{}'. Attempting disambiguation by content...", candidates.len(), target_sig.name);
            
            // [GLUON IMPROVEMENT] Disambiguation by Content Body
            // We compare the body of each candidate (from file) with the search block to find the most similar one.
            
            let file_lines: Vec<&str> = file_content.lines().collect();
            crate::gluon_info!("BlockMatcher", "Disambiguating {} candidates...", candidates.len());
            
            let best_candidate = candidates.iter()
                .enumerate()
                .map(|(idx, sig)| {
                    let start = sig.start_row;
                    let end = sig.end_row + 1;
                    
                    let body_text = if start < file_lines.len() {
                        file_lines[start..end.min(file_lines.len())].join("\n")
                    } else {
                        String::new()
                    };
                    
                    // Log progress for large disambiguations
                    if idx % 5 == 0 { crate::gluon_info!("BlockMatcher", "Checking candidate {}/{}", idx+1, candidates.len()); }
 
                    let score = crate::apply_system::matchers::utils::calculate_similarity(&body_text, search_block);
                    (sig, score)
                })
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
 
            if let Some((winner, score)) = best_candidate {
                // Threshold lowered to 0.4 for Jaccard/Fast similarity which can be lower
                if score > 0.4 {
                     crate::gluon_info!("BlockMatcher", "Disambiguated! Selected candidate at line {} (Similarity: {:.2})", winner.start_row + 1, score);

                     // [GLUON V6 FIX] Smart End Detection Strategy (same as single match)
                     let file_lines: Vec<String> = file_content.lines().map(|s| s.to_string()).collect();
                     let search_lines: Vec<String> = search_block.lines().map(|s| s.to_string()).collect();
                     let start_line = winner.start_row + 1;
                     let tree_sitter_end = winner.end_row + 1;

                     let final_end_line = if search_lines.len() > 30 {
                         // Long block: Trust Tree-sitter (it parsed the ACTUAL function in the file)
                         crate::gluon_info!("BlockMatcher", "LONG block ({} lines) → Using Tree-sitter end: {}", search_lines.len(), tree_sitter_end);
                         tree_sitter_end
                     } else {
                         // Short block: Use fuzzy matching to handle AI hallucinations
                         let fuzzy_end = crate::apply_system::matchers::utils::find_fuzzy_block_end(
                             &file_lines,
                             start_line,
                             &search_lines,
                             0.85  // fuzzy threshold
                         );
                         crate::gluon_info!("BlockMatcher", "SHORT block ({} lines) → Using fuzzy end: {}", search_lines.len(), fuzzy_end);
                         fuzzy_end
                     };

                     crate::gluon_info!("BlockMatcher", "Search block: {} lines", search_lines.len());
                     crate::gluon_info!("BlockMatcher", "Tree-sitter end_row: {} (baseline)", tree_sitter_end);
                     crate::gluon_info!("BlockMatcher", "Final selected end: {}", final_end_line);
                     crate::gluon_info!("BlockMatcher", "Final range: {}-{} (exclusive end)", start_line, final_end_line);
 
                     // [CRITICAL FIX] Same as above - keep 0-indexed for Auto-Apply slicing
                     return Some(MatchResult {
                        matched_line_start: winner.start_row,      // Keep as 0-indexed!
                        matched_line_end: final_end_line - 1,      // Convert to 0-indexed
                        method_used: MatchMethod::BlockStructure,
                        confidence: 0.95,
                        details: Some(format!("System A: Surgical AST match (Disambiguated, score {:.2})", score)),
                        confidence_breakdown: None,
                    });
                }
            }
        }
 
        None
    }
}