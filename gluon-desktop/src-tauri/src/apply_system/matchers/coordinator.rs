//! Coordinator for matching strategies.
//!
//! ## NEW HIERARCHY (Etap 1 - Weighted Anchoring):
//! 1. WeightedAnchorMatcher (PRIORITY 1 - Document IV Section 3.3)
//! 2. BlockMatcher (Surgical Precision)
//! 3. AnchorMatcher (Legacy - fallback)
//! 4. FuzzyMatcher (Legacy - fallback)
//! 5. RegexMatcher (Last resort)

use super::anchor_matcher::AnchorMatcher;
use super::block_matcher::BlockMatcher;
use super::fuzzy_matcher::FuzzyMatcher;
use super::regex_matcher::RegexMatcher;
use super::weighted_anchor_matcher::WeightedAnchorMatcher;
use super::Matcher;
use crate::apply_system::shared::types::{MatchResult, MatchError};

pub fn find_best_match(file_content: &str, search_block: &str, file_path: Option<&str>) -> Result<MatchResult, MatchError> {
    println!("\n\n");
    println!("╔════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                       🎯 NOWA ZMIANA / NEW CHANGE                             ║");
    println!("╚════════════════════════════════════════════════════════════════════════════════╝");
    crate::gluon_info!("MatchCoordinator", "Starting match process...");
    crate::gluon_info!("MatchCoordinator", "File: {:?}", file_path);
    crate::gluon_info!("MatchCoordinator", "File content: {} chars, {} lines", file_content.len(), file_content.lines().count());
    crate::gluon_info!("MatchCoordinator", "Search block: {} chars, {} lines", search_block.len(), search_block.lines().count());
    crate::gluon_info!("MatchCoordinator", "Search block preview (first 200 chars):\n{}", search_block.chars().take(200).collect::<String>());

    // 0. [NEW - PRIORITY 1] Try Weighted Anchor Matcher
    // Based on Document IV (Section 3.3: "Weighted Anchoring and Unique Line Indexing")
    // This solves the "repetitive code blindness" problem and handles AI hallucinations
    crate::gluon_info!("MatchCoordinator", "Trying WeightedAnchorMatcher...");
    let weighted_anchor_matcher = WeightedAnchorMatcher::new();
    if let Some(res) = weighted_anchor_matcher.find_match(file_content, search_block, file_path) {
        crate::gluon_info!("MatchCoordinator", "✅ WeightedAnchorMatcher SUCCESS! Confidence: {:.2}", res.confidence);
        println!("╔════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                    ✅ ZMIANA ZAKOŃCZONA SUKCESEM / SUCCESS                     ║");
        println!("║                         Matcher: WeightedAnchorMatcher                         ║");
        println!("╚════════════════════════════════════════════════════════════════════════════════╝\n\n");
        return Ok(res);
    } else {
        crate::gluon_warn!("MatchCoordinator", "❌ WeightedAnchorMatcher failed");
    }
    crate::gluon_warn!("MatchCoordinator", "WeightedAnchorMatcher failed, trying next...");

    // 1. [System A] Try Block Matcher (Surgical Precision)
    // If we can identify a complete function/class in the search block, find it in the file map.
    // This overrides fuzzy matching to prevent duplicates.
    crate::gluon_info!("MatchCoordinator", "Trying BlockMatcher...");
    let block_matcher = BlockMatcher;
    if let Some(res) = block_matcher.find_match(file_content, search_block, file_path) {
        crate::gluon_info!("MatchCoordinator", "BlockMatcher SUCCESS!");
        println!("╔════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                    ✅ ZMIANA ZAKOŃCZONA SUKCESEM / SUCCESS                     ║");
        println!("║                              Matcher: BlockMatcher                             ║");
        println!("╚════════════════════════════════════════════════════════════════════════════════╝\n\n");
        return Ok(res);
    }
    crate::gluon_warn!("MatchCoordinator", "BlockMatcher failed, trying next...");

    // 2. Try Anchor Matcher (High Confidence - Legacy)
    // Looks for unique signatures like function names, classes, long strings.
    crate::gluon_info!("MatchCoordinator", "Trying AnchorMatcher (legacy)...");
    let anchor_matcher = AnchorMatcher;
    if let Some(res) = anchor_matcher.find_match(file_content, search_block, file_path) {
        crate::gluon_info!("MatchCoordinator", "AnchorMatcher SUCCESS!");
        println!("╔════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                    ✅ ZMIANA ZAKOŃCZONA SUKCESEM / SUCCESS                     ║");
        println!("║                             Matcher: AnchorMatcher                             ║");
        println!("╚════════════════════════════════════════════════════════════════════════════════╝\n\n");
        return Ok(res);
    }
    crate::gluon_warn!("MatchCoordinator", "AnchorMatcher failed, trying next...");

    // 3. Try Fuzzy Matcher (Medium Confidence - Legacy)
    // Uses Levenshtein distance on normalized text to handle whitespace/comment diffs.
    crate::gluon_info!("MatchCoordinator", "Trying FuzzyMatcher (legacy) with 5s timeout...");
    
    // Clone data for the thread (necessary to move ownership)
    let file_content_own = file_content.to_string();
    let search_block_own = search_block.to_string();
    let file_path_own = file_path.map(|s| s.to_string());

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let fuzzy_matcher = FuzzyMatcher;
        // Use as_deref() to convert Option<String> back to Option<&str>
        let res = fuzzy_matcher.find_match(&file_content_own, &search_block_own, file_path_own.as_deref());
        let _ = tx.send(res);
    });

    // Wait up to 5 seconds
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Some(res)) => {
            crate::gluon_info!("MatchCoordinator", "FuzzyMatcher SUCCESS!");
            println!("╔════════════════════════════════════════════════════════════════════════════════╗");
            println!("║                    ✅ ZMIANA ZAKOŃCZONA SUKCESEM / SUCCESS                     ║");
            println!("║                             Matcher: FuzzyMatcher                              ║");
            println!("╚════════════════════════════════════════════════════════════════════════════════╝\n\n");
            return Ok(res);
        },
        Ok(None) => {
            crate::gluon_warn!("MatchCoordinator", "FuzzyMatcher failed, trying next...");
        },
        Err(_) => {
            crate::gluon_warn!("MatchCoordinator", "FuzzyMatcher TIMED OUT (>5s), skipping...");
            // The thread continues in background but we ignore it to unblock the process
        }
    }
    crate::gluon_warn!("MatchCoordinator", "FuzzyMatcher failed, trying next...");

    // 4. Try Regex Matcher (Fallback, Low Confidence)
    // Converts search block into a flexible regex (whitespace insensitive).
    crate::gluon_info!("MatchCoordinator", "Trying RegexMatcher (last resort)...");
    let regex_matcher = RegexMatcher;
    if let Some(res) = regex_matcher.find_match(file_content, search_block, file_path) {
        crate::gluon_info!("MatchCoordinator", "RegexMatcher SUCCESS!");
        println!("╔════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                    ✅ ZMIANA ZAKOŃCZONA SUKCESEM / SUCCESS                     ║");
        println!("║                             Matcher: RegexMatcher                              ║");
        println!("╚════════════════════════════════════════════════════════════════════════════════╝\n\n");
        return Ok(res);
    }
    crate::gluon_warn!("MatchCoordinator", "RegexMatcher failed.");

    // If all fail
    crate::gluon_error!("MatchCoordinator", "ALL MATCHERS FAILED!");
    println!("╔════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                         ❌ ZMIANA NIEUDANA / FAILED                            ║");
    println!("║                          All matchers failed to find match                     ║");
    println!("╚════════════════════════════════════════════════════════════════════════════════╝\n\n");
    Err(MatchError::AllMatchersFailed {
        anchor_error: "No unique anchors matched".to_string(),
        fuzzy_error: "Similarity below threshold".to_string(),
        regex_error: "Flexible pattern not found".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply_system::shared::types::MatchMethod;

    #[test]
    fn test_coordinator_precedence() {
        // Test that code is matched by WeightedAnchorMatcher (highest precedence)
        // WeightedAnchorMatcher is now priority 1 and tries first
        let file = "function uniqueName123() {}";
        let search = "function uniqueName123() {}";
        let res = find_best_match(file, search, Some("test.js")).unwrap();
        // WeightedAnchorMatcher should match this first (priority 1)
        assert_eq!(res.method_used, MatchMethod::WeightedAnchor);
    }
 
    #[test]
    fn test_coordinator_fuzzy_fallback() {
        // Test that typo-heavy code falls back to Fuzzy
        let file = "function test() { return 1; }";
        let search = "function test() { return 2; }"; // '2' makes it fail exact/anchor logic slightly if strict
        let _res = find_best_match(file, search, Some("test.js")).unwrap();
        // Note: Anchor matcher might pick this up if function name matches, 
        // but if we assume function name is common, Fuzzy handles the body difference.
        // In this specific mock, Anchor works on function names, so it might win.
        // Let's force fuzzy by removing anchors:
        let file_no_anchor = "var x = 10; var y = 20;";
        let search_no_anchor = "var x = 10; var y = 22;";
        
        let res = find_best_match(file_no_anchor, search_no_anchor, Some("test.js")).unwrap();
        assert_eq!(res.method_used, MatchMethod::FuzzyMatch);
    }
}