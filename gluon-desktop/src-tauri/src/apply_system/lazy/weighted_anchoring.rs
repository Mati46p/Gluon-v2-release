/// Weighted Anchoring System for Fuzzy Code Patching
///
/// Based on "Algorithmic Strategies for Fuzzy Code Patching" (Document IV, Section 3.3)
///
/// This module implements a sophisticated matching algorithm that:
/// 1. Analyzes frequency of lines in target file (uniqueness scoring)
/// 2. Identifies the most unique line in search block as "hard anchor"
/// 3. Expands fuzzy matching outwards from anchor with configurable tolerance
/// 4. Falls back to context-aware matching when no unique anchors exist
///
/// Algorithm workflow:
/// ```
/// 1. build_frequency_map(target_file) → Map<NormalizedLine, Frequency>
/// 2. calculate_uniqueness_score(line, frequency_map) → 0.0-1.0
/// 3. find_best_anchor(search_block, frequency_map) → WeightedAnchor
/// 4. fuzzy_expand_from_anchor(anchor, search_block, target_file) → MatchResult
/// ```

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Represents a line with its uniqueness score in the context of a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedAnchor {
    /// Index of the line within the search block (0-based)
    pub line_index: usize,

    /// The actual line content (normalized)
    pub line_content: String,

    /// Uniqueness score: 0.0 = very common, 1.0 = unique
    /// Calculated as: 1.0 / (frequency + 1.0)
    pub uniqueness_score: f64,

    /// Position in target file (if matched), None if not yet matched
    pub target_position: Option<usize>,

    /// Classification of anchor quality
    pub quality: AnchorQuality,
}

/// Classification of anchor quality based on structural analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnchorQuality {
    /// Structural keyword (function/class definition, import) - 0.9-1.0
    High,

    /// Unique identifier (specific variable/function name) - 0.6-0.9
    Medium,

    /// Generic pattern (}, return, //) - 0.0-0.3
    Low,

    /// Unknown/ambiguous - 0.3-0.6
    Unknown,
}

impl AnchorQuality {
    /// Get numeric weight for quality tier
    pub fn weight(&self) -> f64 {
        match self {
            AnchorQuality::High => 1.0,
            AnchorQuality::Medium => 0.75,
            AnchorQuality::Low => 0.2,
            AnchorQuality::Unknown => 0.5,
        }
    }
}

/// Configuration for weighted anchoring algorithm
#[derive(Debug, Clone)]
pub struct WeightedAnchoringConfig {
    /// Minimum uniqueness score to consider a line as anchor (default: 0.3)
    pub min_anchor_uniqueness: f64,

    /// Fuzzy matching threshold for expansion (default: 0.85)
    pub fuzzy_threshold: f64,

    /// Maximum number of lines to expand from anchor (default: 50)
    pub max_expansion_distance: usize,

    /// Enable structural analysis for anchor quality (default: true)
    pub enable_structural_analysis: bool,

    /// Whitespace normalization mode
    pub normalize_whitespace: bool,

    /// Maximum number of lines to skip (gap tolerance) when matching (default: 2)
    /// Allows matching even if the file contains extra lines (logs, comments) not in search block.
    pub max_gap_size: usize,
}

impl Default for WeightedAnchoringConfig {
    fn default() -> Self {
        Self {
            min_anchor_uniqueness: 0.3,
            fuzzy_threshold: 0.85,
            max_expansion_distance: 50,
            enable_structural_analysis: true,
            normalize_whitespace: true,
            max_gap_size: 2,
        }
    }
}

/// Build frequency map of normalized lines in target file
///
/// This is Step 1 of the Weighted Anchoring algorithm.
///
/// # Algorithm (from Document IV):
/// "Scan the target file and create a map of line rareness"
///
/// # Example:
/// ```
/// let file_lines = vec![
///     "function foo() {",
///     "    return 42;",     // appears 3 times
///     "}",                  // appears 10 times
///     "function bar() {",
///     "    return 42;",     // appears 3 times
///     "}",
/// ];
/// let freq_map = build_frequency_map(&file_lines, true);
/// // freq_map["return 42;"] = 3
/// // freq_map["}"] = 10
/// ```
pub fn build_frequency_map(
    target_file_lines: &[String],
    normalize_whitespace: bool,
) -> HashMap<String, usize> {
    let mut frequency_map = HashMap::new();

    for line in target_file_lines {
        let normalized = if normalize_whitespace {
            normalize_line(line)
        } else {
            line.clone()
        };

        // Skip empty lines
        if normalized.is_empty() {
            continue;
        }

        *frequency_map.entry(normalized).or_insert(0) += 1;
    }

    frequency_map
}

/// Normalize a line for comparison
///
/// # Normalization rules:
/// - Trim leading/trailing whitespace
/// - Collapse multiple spaces to single space
/// - Lowercase for case-insensitive comparison (optional)
///
/// # Example:
/// ```
/// normalize_line("  const   foo  =  42;  ") => "const foo = 42;"
/// ```
pub fn normalize_line(line: &str) -> String {
    line.trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Calculate uniqueness score for a line
///
/// # Formula (from Document IV, Section 3.3):
/// ```
/// uniqueness_score = 1.0 / (frequency + 1.0)
/// ```
///
/// # Examples:
/// - Line appears 1 time → score = 1.0 / 2.0 = 0.5 (moderate)
/// - Line appears 0 times → score = 1.0 / 1.0 = 1.0 (unique!)
pub fn calculate_uniqueness_score(
    line: &str,
    frequency_map: &HashMap<String, usize>,
    normalize: bool,
) -> f64 {
    let normalized = if normalize {
        normalize_line(line)
    } else {
        line.to_string()
    };

    let frequency = frequency_map.get(&normalized).copied().unwrap_or(0);

    // [GLUON FIX] Anchor Existence Guard
    // If the line does not exist in the target file (frequency == 0), 
    // it CANNOT be used as an anchor. Assign score 0.0.
    if frequency == 0 {
        return 0.0;
    }

    // Document IV formula: 1.0 / (count + 1.0)
    1.0 / (frequency as f64 + 1.0)
}

/// Classify anchor quality based on structural patterns
///
/// # Heuristics (from Document IV, Section 5.3.1 - adapted):
///
/// ## High Quality (0.9-1.0):
/// - Function/method definitions: `fn`, `def`, `function`, `async fn`
/// - Class definitions: `class`, `struct`, `interface`, `trait`
/// - Import statements: `import`, `use`, `require`, `from`
/// - Unique identifiers with type signatures
///
/// ## Medium Quality (0.6-0.9):
/// - Variable declarations with specific names
/// - Method calls with unique names
/// - String literals with unique content
///
/// ## Low Quality (0.0-0.3):
/// - Generic closing braces: `}`, `};`, `]`
/// - Generic keywords: `return`, `break`, `continue`
/// - Comments without unique content: `//`, `/*`, `#`
///
/// ## Unknown (0.3-0.6):
/// - Everything else
pub fn classify_anchor_quality(line: &str) -> AnchorQuality {
    let normalized = normalize_line(line);
    let trimmed = normalized.trim();

    // [GLUON FIX 1.1] Critical: Empty lines are NOT anchors
    if trimmed.is_empty() {
        return AnchorQuality::Low;
    }

    // Low quality patterns (generic code)
    const GENERIC_PATTERNS: &[&str] = &[
        "}", "{", "};", "});", "]", "return", "break", "continue",
        "//", "/*", "*/", "#", "pass", "...", "end", "},"
    ];

    if GENERIC_PATTERNS.iter().any(|&p| trimmed == p || trimmed.starts_with(p)) {
        return AnchorQuality::Low;
    }

    // High quality patterns (structural definitions)
    const STRUCTURAL_KEYWORDS: &[&str] = &[
        "fn ", "def ", "function ", "async fn", "async def",
        "class ", "struct ", "interface ", "trait ", "enum ",
        "import ", "use ", "require(", "from ", "export ",
        "pub fn", "pub struct", "pub enum", "pub trait",
        "public class", "private class", "protected class",
    ];

    if STRUCTURAL_KEYWORDS.iter().any(|&kw| normalized.contains(kw)) {
        return AnchorQuality::High;
    }

    // Medium quality patterns (unique identifiers)
    // Heuristic: line contains alphanumeric identifier and assignment/call
    let has_assignment = normalized.contains('=') || normalized.contains(':');
    let has_call = normalized.contains('(') && normalized.contains(')');
    let has_identifier = normalized.chars().any(|c| c.is_alphanumeric());

    if has_identifier && (has_assignment || has_call) {
        return AnchorQuality::Medium;
    }

    // Default: unknown
    AnchorQuality::Unknown
}

/// Find the best anchor line in search block
///
/// # Algorithm (from Document IV, Section 3.3):
/// "Scan the LLM's 'Search' block to identify lines that are unique within the target file.
///  If a unique line is found (e.g., specific_function_call(x, y)), anchor the match to that line index."
///
/// # Steps:
/// 1. Calculate uniqueness score for each line in search block
/// 2. Classify structural quality of each line
/// 3. Combine scores: `final_score = uniqueness_score * quality_weight`
/// 4. Return line with highest combined score
///
/// # Returns:
/// - `Some(WeightedAnchor)` if anchor found with score >= min_uniqueness
/// - `None` if no suitable anchor exists
pub fn find_best_anchor(
    search_block_lines: &[String],
    frequency_map: &HashMap<String, usize>,
    config: &WeightedAnchoringConfig,
) -> Option<WeightedAnchor> {
    println!("\n┌──────────────────────────────────────────────────────────────────────────────┐");
    println!("│ [find_best_anchor] 🔍 Analyzing {} search lines for best anchor", search_block_lines.len());

    let mut best_anchor: Option<WeightedAnchor> = None;
    let mut best_score = 0.0;
    let mut candidates: Vec<(usize, f64, f64, f64, AnchorQuality, String)> = Vec::new();

    for (idx, line) in search_block_lines.iter().enumerate() {
        // Calculate uniqueness based on frequency
        let uniqueness = calculate_uniqueness_score(
            line,
            frequency_map,
            config.normalize_whitespace,
        );

        // Classify structural quality
        let quality = if config.enable_structural_analysis {
            classify_anchor_quality(line)
        } else {
            AnchorQuality::Unknown
        };

        // Combined score: uniqueness * quality_weight
        // This prioritizes structurally important lines even if they appear multiple times
        let combined_score = uniqueness * quality.weight();

        // Store candidate for logging
        candidates.push((
            idx,
            uniqueness,
            quality.weight(),
            combined_score,
            quality,
            line.chars().take(60).collect::<String>()
        ));

        // Skip lines below minimum threshold
        if uniqueness < config.min_anchor_uniqueness {
            continue;
        }

        // [GLUON FIX 1.1] Absolute Safety Filter
        // Never allow empty lines or very short generic tokens to become the Best Anchor.
        // Even if they are unique in the block, they are dangerous in the file.
        let trimmed_len = normalize_line(line).trim().len();
        if trimmed_len == 0 { continue; }
        if trimmed_len < 3 && quality == AnchorQuality::Low { continue; }

        if combined_score > best_score {
            best_score = combined_score;
            best_anchor = Some(WeightedAnchor {
                line_index: idx,
                line_content: normalize_line(line),
                uniqueness_score: uniqueness,
                target_position: None,
                quality,
            });
        }
    }

    // Log all candidates
    println!("[find_best_anchor] 📊 All anchor candidates:");
    for (idx, uniqueness, quality_weight, combined, quality, content) in &candidates {
        let marker = if Some(*idx) == best_anchor.as_ref().map(|a| a.line_index) { "👑" } else { "  " };
        println!("[find_best_anchor] {} [{}] uniqueness={:.2}, quality_weight={:.2} ({:?}), combined={:.2} | '{}'",
            marker, idx, uniqueness, quality_weight, quality, combined, content);
    }

    if let Some(ref anchor) = best_anchor {
        println!("│ ✅ Best anchor: line {} with score {:.2}", anchor.line_index, best_score);
    } else {
        println!("│ ❌ No suitable anchor found (all below min_uniqueness threshold {:.2})", config.min_anchor_uniqueness);
    }
    println!("└──────────────────────────────────────────────────────────────────────────────┘\n");

    best_anchor
}

/// Result of fuzzy expansion from anchor
#[derive(Debug, Clone)]
pub struct FuzzyExpansionResult {
    /// Start line in target file
    pub start_line: usize,

    /// End line in target file (exclusive)
    pub end_line: usize,

    /// Overall confidence score (0.0-1.0)
    pub confidence: f64,

    /// Anchor used for matching
    pub anchor: WeightedAnchor,

    /// Breakdown of confidence components
    pub confidence_breakdown: ConfidenceBreakdown,
}

/// Breakdown of confidence score components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceBreakdown {
    /// Similarity score from fuzzy matching (0.0-1.0)
    pub similarity: f64,

    /// Token-level similarity (whitespace-agnostic) (0.0-1.0)
    pub token_similarity: f64,

    /// Anchor quality contribution (0.0-1.0)
    pub anchor_quality: f64,
}

impl ConfidenceBreakdown {
    /// Calculate weighted average confidence
    ///
    /// # Adaptive Weights Strategy:
    /// When token_similarity is very high (>= 0.95), it indicates that the code structure
    /// matches well, even if exact line similarity is lower (e.g. due to markdown artifacts,
    /// comments, or whitespace differences). In such cases, we give more weight to token_similarity.
    ///
    /// ## High Token Similarity (>= 0.95):
    /// - Similarity: 20% (less important when tokens match)
    /// - Token similarity: 60% (primary signal - structure matches)
    /// - Anchor quality: 20% (context confirmation)
    ///
    /// ## Normal Case:
    /// - Similarity: 40% (exact match important)
    /// - Token similarity: 35% (structure important)
    /// - Anchor quality: 25% (context important)
    pub fn calculate_confidence(&self) -> f64 {
        if self.token_similarity >= 0.95 {
            // High token similarity - prioritize structural match
            0.20 * self.similarity + 0.60 * self.token_similarity + 0.20 * self.anchor_quality
        } else {
            // Normal weighted calculation
            0.40 * self.similarity + 0.35 * self.token_similarity + 0.25 * self.anchor_quality
        }
    }
}

/// Fuzzy expand from anchor to find full match
///
/// # Algorithm (from Document IV, Section 3.3):
/// "Grow the match outwards from the anchor, verifying that the surrounding lines
///  match loosely (fuzzy)."
///
/// # Strategy:
/// 1. Find anchor line in target file
/// 2. Expand upwards: match lines above anchor with fuzzy tolerance
/// 3. Expand downwards: match lines below anchor with fuzzy tolerance
/// 4. Calculate confidence based on overall similarity
///
/// # Parameters:
/// - `anchor`: The anchor line to start from
/// - `search_block_lines`: Full search block
/// - `target_file_lines`: Full target file
/// - `config`: Fuzzy matching configuration
///
/// # Returns:
/// - `Some(FuzzyExpansionResult)` if match found with sufficient confidence
/// - `None` if no match found or confidence below threshold
pub fn fuzzy_expand_from_anchor(
    mut anchor: WeightedAnchor,
    search_block_lines: &[String],
    target_file_lines: &[String],
    config: &WeightedAnchoringConfig,
) -> Option<FuzzyExpansionResult> {
    println!("\n================================================================================");
    println!("[fuzzy_expand_from_anchor] 🎯 ROZPOCZĘCIE NOWEJ ZMIANY");
    println!("[fuzzy_expand_from_anchor] 🎯 Starting expansion from anchor at search line {}", anchor.line_index);
    println!("================================================================================\n");

    // Step 1: Find anchor in target file
    let anchor_target_pos = find_anchor_in_target(&anchor, target_file_lines, config)?;
    anchor.target_position = Some(anchor_target_pos);
    println!("[fuzzy_expand_from_anchor] 📍 Anchor found in target at line {}", anchor_target_pos);

    // Step 2: Expand upwards from anchor
    let lines_before_anchor = anchor.line_index;
    println!("[fuzzy_expand_from_anchor] ⬆️ Expanding upwards: need to match {} lines before anchor", lines_before_anchor);
    let start_line = match expand_upwards(
        anchor_target_pos,
        lines_before_anchor,
        search_block_lines,
        target_file_lines,
        config,
    ) {
        Some(line) => {
            println!("[fuzzy_expand_from_anchor] ✅ Upward expansion succeeded: start_line={}", line);
            line
        }
        None => {
            println!("[fuzzy_expand_from_anchor] ❌ Upward expansion failed!");
            return None;
        }
    };

    // Step 3: Expand downwards from anchor
    let lines_after_anchor = search_block_lines.len().saturating_sub(anchor.line_index + 1);
    println!("[fuzzy_expand_from_anchor] ⬇️ Expanding downwards: need to match {} lines after anchor", lines_after_anchor);
    let end_line = match expand_downwards(
        anchor_target_pos,
        lines_after_anchor,
        search_block_lines,
        target_file_lines,
        config,
    ) {
        Some(line) => {
            println!("[fuzzy_expand_from_anchor] ✅ Downward expansion succeeded: end_line={}", line);
            line
        }
        None => {
            println!("[fuzzy_expand_from_anchor] ❌ Downward expansion failed!");
            return None;
        }
    };

    println!("[fuzzy_expand_from_anchor] 📏 Final range: lines {}-{} ({} lines total)", start_line, end_line, end_line - start_line);

    // Step 4: Calculate confidence
    // Use the 0-based indices for slicing
    let matched_block = &target_file_lines[start_line..end_line];
    let confidence_breakdown = calculate_confidence_breakdown(
        search_block_lines,
        matched_block,
        &anchor,
        config,
    );

    let confidence = confidence_breakdown.calculate_confidence();
    println!("[fuzzy_expand_from_anchor] 🎲 Confidence: {:.2} (threshold: {:.2})", confidence, config.fuzzy_threshold);
    println!("[fuzzy_expand_from_anchor] 📊 Breakdown: similarity={:.2}, token_sim={:.2}, anchor_quality={:.2}",
        confidence_breakdown.similarity, confidence_breakdown.token_similarity, confidence_breakdown.anchor_quality);

    // CRITICAL FIX: Adaptive threshold based on match quality
    let effective_threshold = if confidence_breakdown.similarity >= 0.99 && confidence_breakdown.token_similarity >= 0.99 {
        // Near-perfect match - very confident
        let relaxed_threshold = 0.80;
        println!("[fuzzy_expand_from_anchor] ✨ Near-perfect match detected (similarity={:.2}, token_sim={:.2}), using relaxed threshold {:.2}",
            confidence_breakdown.similarity, confidence_breakdown.token_similarity, relaxed_threshold);
        relaxed_threshold
    } else if confidence_breakdown.token_similarity >= 0.95 {
        // High token similarity means structure matches well (code is correct)
        // Even if some lines differ (markdown artifacts, comments), the match is valid
        let relaxed_threshold = 0.70;
        println!("[fuzzy_expand_from_anchor] ✨ High token similarity detected ({:.2}), using relaxed threshold {:.2}",
            confidence_breakdown.token_similarity, relaxed_threshold);
        relaxed_threshold
    } else if confidence_breakdown.similarity >= 0.999 && confidence_breakdown.token_similarity >= 0.88 {
        // Perfect line similarity with good token coverage - reliable match (BUG FIX)
        let relaxed_threshold = 0.75;
        println!("[fuzzy_expand_from_anchor] ✨ Perfect similarity + good token match, relaxed threshold {:.2}",
            relaxed_threshold);
        relaxed_threshold
    } else {
        config.fuzzy_threshold
    };

    // Check if confidence meets threshold
    if confidence < effective_threshold {
        println!("[fuzzy_expand_from_anchor] ❌ Confidence {:.2} < threshold {:.2} - returning None", confidence, effective_threshold);
        println!("\n================================================================================");
        println!("[fuzzy_expand_from_anchor] ❌ ZMIANA NIEUDANA");
        println!("================================================================================\n");
        return None;
    }

    println!("[fuzzy_expand_from_anchor] ✅ Expansion successful!");
    println!("\n================================================================================");
    println!("[fuzzy_expand_from_anchor] ✅ ZMIANA ZAKOŃCZONA SUKCESEM");
    println!("================================================================================\n");
    
    // [GLUON FIX] Convert indices to MatchResult format (1-based start, 0-based exclusive end)
    // start_line is 0-based index. matched_line_start needs to be 1-based.
    // end_line is 0-based exclusive index. matched_line_end needs to be 0-based exclusive (same).
    Some(FuzzyExpansionResult {
        start_line: start_line + 1, // Convert to 1-based
        end_line: end_line,         // Keep as 0-based exclusive (matches MatchResult expectation)
        confidence,
        anchor,
        confidence_breakdown,
    })
}

/// Find anchor line position in target file
///
/// CRITICAL FIX: This function now finds ALL matching anchors and selects
/// the best one based on context similarity, not just the first match.
/// This solves the "repetitive code blindness" problem where identical lines
/// appear multiple times in a file.
fn find_anchor_in_target(
    anchor: &WeightedAnchor,
    target_file_lines: &[String],
    config: &WeightedAnchoringConfig,
) -> Option<usize> {
    println!("[find_anchor_in_target] 🔍 Searching for anchor: '{}'", anchor.line_content.chars().take(50).collect::<String>());

    let normalized_anchor = if config.normalize_whitespace {
        normalize_line(&anchor.line_content)
    } else {
        anchor.line_content.clone()
    };

    // Collect ALL exact matches (not just the first one!)
    let mut exact_matches: Vec<usize> = Vec::new();

    for (idx, line) in target_file_lines.iter().enumerate() {
        let normalized_line = if config.normalize_whitespace {
            normalize_line(line)
        } else {
            line.clone()
        };

        if normalized_line == normalized_anchor {
            exact_matches.push(idx);
            println!("[find_anchor_in_target] 📍 Exact match at line {}: '{}'", idx, line.chars().take(50).collect::<String>());
        }
    }

    // If we found exact matches, select the best one
    if !exact_matches.is_empty() {
        println!("[find_anchor_in_target] ✅ Found {} exact matches: {:?}", exact_matches.len(), exact_matches);

        // If only one match, return it
        if exact_matches.len() == 1 {
            println!("[find_anchor_in_target] 🎯 Single match at line {}", exact_matches[0]);
            return Some(exact_matches[0]);
        }

        // Multiple matches - THIS is the repetitive code problem!
        // We need to select the best match based on context.

        // [GLUON FIX] Critical Bug Fix: Choose FIRST match instead of LAST
        // Choosing LAST causes duplication snowball effect - if code was already
        // duplicated once, it will keep matching the duplicate and creating more copies.
        // FIRST match is safer as it targets the original code location.
        let best_match = exact_matches.iter()
            .min_by_key(|&&idx| idx)  // Choose the FIRST occurrence
            .copied();

        if let Some(line_num) = best_match {
            println!("[find_anchor_in_target] 🎯 Multiple matches - choosing FIRST at line {}", line_num);
        }

        return best_match;
    }

    // Fallback: fuzzy match with high threshold (0.95)
    println!("[find_anchor_in_target] 🔄 No exact matches, trying fuzzy...");
    let mut fuzzy_matches: Vec<(usize, f64)> = Vec::new();

    for (idx, line) in target_file_lines.iter().enumerate() {
        let similarity = calculate_line_similarity(&anchor.line_content, line);
        if similarity >= 0.95 {
            fuzzy_matches.push((idx, similarity));
            println!("[find_anchor_in_target] 📍 Fuzzy match at line {} with similarity {:.2}", idx, similarity);
        }
    }

    // Select best fuzzy match (highest similarity)
    fuzzy_matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    if let Some((idx, sim)) = fuzzy_matches.first() {
        println!("[find_anchor_in_target] 🎯 Best fuzzy match at line {} (similarity={:.2})", idx, sim);
        return Some(*idx);
    }

    println!("[find_anchor_in_target] ❌ No matches found");
    None
}

/// Expand upwards from anchor position
///
/// # Parameters:
/// - `anchor_pos`: Position of anchor in TARGET file
/// - `lines_to_match`: Number of lines BEFORE anchor in SEARCH block (i.e., anchor.line_index)
/// - `search_block_lines`: Full search block
/// - `target_file_lines`: Full target file
/// - `config`: Fuzzy matching config
///
/// Algorithm:
/// Starting from anchor, go UP and match lines from search block with target file.
fn expand_upwards(
    anchor_pos: usize,
    lines_to_match: usize,
    search_block_lines: &[String],
    target_file_lines: &[String],
    config: &WeightedAnchoringConfig,
) -> Option<usize> {
    println!("┌─── [expand_upwards] 🔼 START ───");
    println!("│ anchor_pos={}, lines_to_match={}", anchor_pos, lines_to_match);

    if lines_to_match == 0 {
        // [GLUON FIX] Return 0-based index
        // If no lines to match upwards, the start is the anchor itself.
        return Some(anchor_pos);
    }

    // The line immediately before the anchor in the search block
    let mut current_search_idx = lines_to_match.saturating_sub(1);
    // The line immediately before the anchor in the target file
    let mut current_target_idx = anchor_pos.saturating_sub(1);
    
    // Explicitly track the last confirmed match position in the target file
    // Initialize with anchor_pos (technically one line below match area, but safe fallback)
    let mut last_matched_target_idx = anchor_pos;

    // We need to match 'lines_to_match' lines from search block
    let mut matched_count = 0;
    
    while matched_count < lines_to_match {
        if current_search_idx >= search_block_lines.len() || current_target_idx >= target_file_lines.len() {
            break;
        }

        let search_line = &search_block_lines[current_search_idx];
        
        // [GLUON FIX] Lazy Gap Jumping (Upwards) - Enhanced
        if is_lazy_marker_line(search_line) {
            println!("[expand_upwards] 💤 Lazy marker detected at search line {}. Scanning backwards...", current_search_idx);

            // 1. Advance search cursor past the marker (backward in search block)
            if current_search_idx == 0 { break; }
            current_search_idx -= 1;
            matched_count += 1;

            // 2. Find the PREVIOUS SUBSTANTIAL line in search block (skip empty lines)
            // This is critical because matching an empty line against a gap is ambiguous.
            while current_search_idx < search_block_lines.len() && search_block_lines[current_search_idx].trim().is_empty() {
                if current_search_idx == 0 { break; }
                current_search_idx -= 1;
                // Don't increment matched_count for skipped empty lines in search,
                // they are part of the "gap" concept.
            }

            if current_search_idx >= search_block_lines.len() {
                // Marker was the last significant thing. Assume match from BOF.
                // We expand target to the very beginning.
                last_matched_target_idx = 0;
                break;
            }

            let next_strong_anchor = &search_block_lines[current_search_idx];
            let mut found_jump = false;

            // 3. Scan file backwards to find that strong anchor
            // We start from current_target_idx - 1 (must be at least 1 line gap)
            for jump_idx in (0..current_target_idx).rev() {
                let target_line = &target_file_lines[jump_idx];

                // Use strict similarity for re-anchoring after a gap
                if calculate_line_similarity(next_strong_anchor, target_line) >= config.fuzzy_threshold {
                    println!("[expand_upwards] 🚀 Jumped gap! Re-anchored at target line {} using '{}'", jump_idx, next_strong_anchor.trim());

                    // Update state to point to this match
                    current_target_idx = jump_idx;
                    last_matched_target_idx = jump_idx;

                    // We successfully matched this line pair, so advance and count
                    if current_search_idx == 0 { break; }
                    current_search_idx -= 1;
                    if current_target_idx == 0 {
                        current_target_idx = 0;
                    } else {
                        current_target_idx = current_target_idx - 1;
                    }
                    matched_count += 1;
                    found_jump = true;
                    break;
                }
            }

            if found_jump {
                if matched_count >= lines_to_match { break; }
                continue;
            } else {
                println!("[expand_upwards] ❌ Could not find pre-gap anchor: '{}'. Gap jump failed.", next_strong_anchor.trim());
                break;
            }
        }

        let target_line = &target_file_lines[current_target_idx];
        let similarity = calculate_line_similarity(search_line, target_line);

        println!("[expand_upwards] 🔍 search[{}] vs target[{}] → sim={:.2}", 
            current_search_idx, current_target_idx, similarity);

        if similarity >= config.fuzzy_threshold {
            println!("[expand_upwards] ✅ Match!");
            matched_count += 1;
            last_matched_target_idx = current_target_idx;

            if current_search_idx == 0 { break; } 
            current_search_idx -= 1;
            
            if current_target_idx == 0 { break; } 
            current_target_idx -= 1;
        } else {
            // Mismatch - try Gap Tolerance
            let mut found_gap_match = false;
            
            for gap in 1..=config.max_gap_size {
                if current_target_idx < gap { break; }
                
                let candidate_target_idx = current_target_idx - gap;
                let gap_target_line = &target_file_lines[candidate_target_idx];
                let gap_similarity = calculate_line_similarity(search_line, gap_target_line);
                
                if gap_similarity >= config.fuzzy_threshold {
                    println!("[expand_upwards] 🕳️ Gap Skip! Matched search[{}] with target[{}] (gap={})", 
                        current_search_idx, candidate_target_idx, gap);
                    
                    found_gap_match = true;
                    matched_count += 1;
                    last_matched_target_idx = candidate_target_idx;
                    
                    if current_search_idx == 0 { break; }
                    current_search_idx -= 1;
                    
                    if candidate_target_idx == 0 { 
                        current_target_idx = 0; 
                    } else {
                        current_target_idx = candidate_target_idx - 1;
                    }
                    break;
                }
            }

            if found_gap_match {
                if matched_count >= lines_to_match { break; }
                continue;
            }

            println!("[expand_upwards] ❌ Mismatch and no gap recovery. Stopping.");
            break; 
        }
    }

    // [GLUON FIX] Return 0-based index for correct slicing
    // We return the index of the first matched line.
    println!("│ 📏 Final: matched_count={}/{}, returning start_idx={}",
        matched_count, lines_to_match, last_matched_target_idx);
    println!("└─── [expand_upwards] END ───\n");

    Some(last_matched_target_idx)
}

/// Expand downwards from anchor position
///
/// # Parameters:
/// - `anchor_pos`: Position of anchor in TARGET file
/// - `lines_to_match`: Number of lines AFTER anchor in SEARCH block
/// - `search_block_lines`: Full search block
/// - `target_file_lines`: Full target file
/// - `config`: Fuzzy matching config
///
/// Algorithm:
/// Starting from anchor, go DOWN and match lines from search block with target file.
/// If anchor is at position K in search block, lines after it are [K+1..end].
fn expand_downwards(
    anchor_pos: usize,
    lines_to_match: usize,
    search_block_lines: &[String],
    target_file_lines: &[String],
    config: &WeightedAnchoringConfig,
) -> Option<usize> {
    println!("┌─── [expand_downwards] 🔽 START ───");
    println!("│ anchor_pos={}, lines_to_match={}", anchor_pos, lines_to_match);

    if lines_to_match == 0 {
        // Return 0-based exclusive end (which is anchor_pos + 1)
        return Some(anchor_pos + 1);
    }

    // Anchor index in search block
    let anchor_idx_in_search = search_block_lines.len().saturating_sub(lines_to_match + 1);
    
    // Start checking from the line immediately AFTER the anchor
    let mut current_search_idx = anchor_idx_in_search + 1;
    let mut current_target_idx = anchor_pos + 1;
    
    // Track the last successfully matched line index
    let mut last_matched_target_idx = anchor_pos;

    let mut matched_count = 0;
    
    while matched_count < lines_to_match {
        if current_search_idx >= search_block_lines.len() || current_target_idx >= target_file_lines.len() {
            break;
        }

        let search_line = &search_block_lines[current_search_idx];
        
        // [GLUON FIX] Lazy Gap Jumping (Downwards)
        if is_lazy_marker_line(search_line) {
            println!("[expand_downwards] 💤 Lazy marker detected at search line {}. Scanning forwards...", current_search_idx);
            
            current_search_idx += 1;
            matched_count += 1;

            if current_search_idx >= search_block_lines.len() {
                // Implicitly matches until end.
                // We assume end of file for now, or just stop here.
                // Better strategy: stop matching and return current position as end.
                break;
            }

            let next_search_line = &search_block_lines[current_search_idx];
            let mut found_jump = false;

            for jump_idx in current_target_idx..target_file_lines.len() {
                let target_line = &target_file_lines[jump_idx];
                if calculate_line_similarity(next_search_line, target_line) >= config.fuzzy_threshold {
                    println!("[expand_downwards] 🚀 Jumped gap! Found match at target line {}", jump_idx);
                    current_target_idx = jump_idx;
                    // last_matched is not updated yet, will be updated in next loop iteration
                    found_jump = true;
                    break;
                }
            }

            if found_jump {
                if matched_count >= lines_to_match { break; }
                continue;
            } else {
                println!("[expand_downwards] ❌ Could not find post-gap anchor: '{}'. Gap jump failed.", next_search_line.trim());
                break;
            }
        }

        let target_line = &target_file_lines[current_target_idx];
        let similarity = calculate_line_similarity(search_line, target_line);

        println!("[expand_downwards] 🔍 search[{}] vs target[{}] → sim={:.2}", 
            current_search_idx, current_target_idx, similarity);

        if similarity >= config.fuzzy_threshold {
            println!("[expand_downwards] ✅ Match!");
            matched_count += 1;
            last_matched_target_idx = current_target_idx;
            
            current_search_idx += 1;
            current_target_idx += 1;
        } else {
            // Mismatch - try Gap Tolerance
            let mut found_gap_match = false;
            
            for gap in 1..=config.max_gap_size {
                let candidate_target_idx = current_target_idx + gap;
                if candidate_target_idx >= target_file_lines.len() { break; }
                
                let gap_target_line = &target_file_lines[candidate_target_idx];
                let gap_similarity = calculate_line_similarity(search_line, gap_target_line);
                
                if gap_similarity >= config.fuzzy_threshold {
                    println!("[expand_downwards] 🕳️ Gap Skip! Matched search[{}] with target[{}] (gap={})", 
                        current_search_idx, candidate_target_idx, gap);
                    
                    found_gap_match = true;
                    matched_count += 1;
                    last_matched_target_idx = candidate_target_idx;
                    
                    current_search_idx += 1;
                    current_target_idx = candidate_target_idx + 1;
                    break;
                }
            }
            
            if found_gap_match {
                continue;
            }
            
            println!("[expand_downwards] ❌ Mismatch and no gap recovery. Stopping.");
            break;
        }
    }

    // [GLUON FIX] Return EXCLUSIVE end index (0-based)
    // last_matched_target_idx is the index of the last line INCLUDED in the match.
    // Exclusive end is last_matched + 1.
    let exclusive_end = last_matched_target_idx + 1;

    println!("│ 📏 Final: matched_count={}/{}, returning end_line={}",
        matched_count, lines_to_match, exclusive_end);
    println!("└─── [expand_downwards] END ───\n");

    Some(exclusive_end)
}

/// Calculate line similarity using Levenshtein distance
///
/// # Algorithm:
/// Uses normalized Levenshtein distance: `1.0 - (distance / max_length)`
///
/// # Returns:
/// Similarity score 0.0-1.0 (1.0 = identical)
fn calculate_line_similarity(line1: &str, line2: &str) -> f64 {
    let normalized1 = normalize_line(line1);
    let normalized2 = normalize_line(line2);

    if normalized1 == normalized2 {
        return 1.0;
    }

    let distance = levenshtein_distance(&normalized1, &normalized2);
    let max_len = normalized1.len().max(normalized2.len());

    if max_len == 0 {
        return 1.0;
    }

    1.0 - (distance as f64 / max_len as f64)
}

/// Calculate Levenshtein distance between two strings
///
/// # Algorithm:
/// Classic dynamic programming approach - O(n*m) time, O(min(n,m)) space
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut prev_row: Vec<usize> = (0..=len2).collect();
    let mut curr_row = vec![0; len2 + 1];

    for (i, c1) in s1.chars().enumerate() {
        curr_row[0] = i + 1;

        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            curr_row[j + 1] = (curr_row[j] + 1)
                .min(prev_row[j + 1] + 1)
                .min(prev_row[j] + cost);
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[len2]
}

/// Calculate confidence breakdown for matched block
///
/// # Components (from Document IV, Etap 4 consultation):
/// - Similarity: Levenshtein-based exact matching
/// - Token similarity: Whitespace-agnostic structural matching
/// - Anchor quality: Quality weight of the anchor used
fn calculate_confidence_breakdown(
    search_block: &[String],
    matched_block: &[String],
    anchor: &WeightedAnchor,
    _config: &WeightedAnchoringConfig,
) -> ConfidenceBreakdown {
    // 1. Calculate line-by-line similarity
    let mut total_similarity = 0.0;
    let line_count = search_block.len().min(matched_block.len());

    for i in 0..line_count {
        total_similarity += calculate_line_similarity(&search_block[i], &matched_block[i]);
    }

    let similarity = if line_count > 0 {
        total_similarity / line_count as f64
    } else {
        0.0
    };

    // 2. Calculate token-level similarity (whitespace-agnostic)
    let search_tokens: Vec<String> = search_block
        .iter()
        .flat_map(|line| line.split_whitespace().map(|s| s.to_string()))
        .collect();

    let matched_tokens: Vec<String> = matched_block
        .iter()
        .flat_map(|line| line.split_whitespace().map(|s| s.to_string()))
        .collect();

    let token_similarity = calculate_token_similarity(&search_tokens, &matched_tokens);

    // 3. Anchor quality contribution
    let anchor_quality = anchor.uniqueness_score * anchor.quality.weight();

    ConfidenceBreakdown {
        similarity,
        token_similarity,
        anchor_quality,
    }
}

/// Detects lazy/truncation markers in SEARCH blocks (G-Protocol V2)
///
/// These markers indicate that the AI abbreviated a long function by including
/// only the first 5 + last 5 lines, omitting the middle with a comment.
///
/// This is ALLOWED and EXPECTED in SEARCH blocks for token optimization.
/// The matcher will jump over these markers to continue matching after the gap.
fn is_lazy_marker_line(line: &str) -> bool {
    let trimmed = line.trim();
    let lower = trimmed.to_lowercase();

    // Common ellipsis patterns
    if trimmed.starts_with("# ...") ||
       trimmed.starts_with("// ...") ||
       trimmed.starts_with("/* ...") ||
       trimmed.starts_with("<!-- ...") ||
       trimmed == "..." ||
       trimmed == "…" {
        return true;
    }

    // Truncation markers for long functions (G-Protocol V2 standard)
    // English variants
    if (trimmed.starts_with("//") && lower.contains("rest of function")) ||
       (trimmed.starts_with("#") && lower.contains("rest of function")) ||
       (trimmed.starts_with("//") && lower.contains("rest of code")) ||
       (trimmed.starts_with("#") && lower.contains("rest of code")) ||
       (trimmed.starts_with("//") && lower.contains("rest of file")) ||
       (trimmed.starts_with("#") && lower.contains("rest of file")) {
        return true;
    }

    // Polish variants
    if (trimmed.starts_with("//") && lower.contains("reszta funkcji")) ||
       (trimmed.starts_with("#") && lower.contains("reszta funkcji")) ||
       (trimmed.starts_with("//") && lower.contains("reszta kodu")) ||
       (trimmed.starts_with("#") && lower.contains("reszta kodu")) ||
       (trimmed.starts_with("//") && lower.contains("reszta pliku")) ||
       (trimmed.starts_with("#") && lower.contains("reszta pliku")) {
        return true;
    }

    false
}

fn calculate_token_similarity(tokens1: &[String], tokens2: &[String]) -> f64 {
    if tokens1.is_empty() && tokens2.is_empty() {
        return 1.0;
    }

    if tokens1.is_empty() || tokens2.is_empty() {
        return 0.0;
    }

    // Count matching tokens (case-sensitive)
    let set1: std::collections::HashSet<_> = tokens1.iter().collect();
    let set2: std::collections::HashSet<_> = tokens2.iter().collect();

    let intersection = set1.intersection(&set2).count();
    let union = set1.union(&set2).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_line() {
        assert_eq!(normalize_line("  const   foo  =  42;  "), "const foo = 42;");
        assert_eq!(normalize_line("function    bar()   {"), "function bar() {");
        assert_eq!(normalize_line(""), "");
    }

    #[test]
    fn test_build_frequency_map() {
        let lines = vec![
            "function foo() {".to_string(),
            "    return 42;".to_string(),
            "}".to_string(),
            "function bar() {".to_string(),
            "    return 42;".to_string(),
            "}".to_string(),
        ];

        let freq_map = build_frequency_map(&lines, true);

        assert_eq!(freq_map.get("return 42;"), Some(&2));
        assert_eq!(freq_map.get("}"), Some(&2));
        assert_eq!(freq_map.get("function foo() {"), Some(&1));
    }

    #[test]
    fn test_calculate_uniqueness_score() {
        let mut freq_map = HashMap::new();
        freq_map.insert("return 42;".to_string(), 3);
        freq_map.insert("}".to_string(), 10);
        freq_map.insert("function foo() {".to_string(), 1);

        // Unique line (not in map) - returns 0.0 because it doesn't exist in target
        // This is the "Anchor Existence Guard" - can't use a line as anchor if it's not in the file
        assert!((calculate_uniqueness_score("const x = 1;", &freq_map, true) - 0.0).abs() < 0.01);

        // Common line (appears 10 times)
        assert!((calculate_uniqueness_score("}", &freq_map, true) - 1.0/11.0).abs() < 0.01);

        // Moderately unique (appears 1 time)
        assert!((calculate_uniqueness_score("function foo() {", &freq_map, true) - 1.0/2.0).abs() < 0.01);
    }

    #[test]
    fn test_classify_anchor_quality() {
        assert_eq!(classify_anchor_quality("fn process_data() {"), AnchorQuality::High);
        assert_eq!(classify_anchor_quality("function foo() {"), AnchorQuality::High);
        assert_eq!(classify_anchor_quality("class MyClass:"), AnchorQuality::High);
        assert_eq!(classify_anchor_quality("import React from 'react';"), AnchorQuality::High);

        assert_eq!(classify_anchor_quality("const result = calculate();"), AnchorQuality::Medium);
        assert_eq!(classify_anchor_quality("user.save()"), AnchorQuality::Medium);

        assert_eq!(classify_anchor_quality("}"), AnchorQuality::Low);
        assert_eq!(classify_anchor_quality("return"), AnchorQuality::Low);
        assert_eq!(classify_anchor_quality("//"), AnchorQuality::Low);
    }

    #[test]
    fn test_find_best_anchor() {
        let search_lines = vec![
            "function processData() {".to_string(),
            "    const result = calculate();".to_string(),
            "    return result;".to_string(),
            "}".to_string(),
        ];

        let target_lines = vec![
            "// Some code".to_string(),
            "function processData() {".to_string(),
            "    const result = calculate();".to_string(),
            "    return result;".to_string(),
            "}".to_string(),
            "// More code".to_string(),
            "function other() {".to_string(),
            "    return result;".to_string(), // "return result;" appears twice
            "}".to_string(),
        ];

        let freq_map = build_frequency_map(&target_lines, true);
        let config = WeightedAnchoringConfig::default();

        let anchor = find_best_anchor(&search_lines, &freq_map, &config).unwrap();

        // Should anchor on "function processData() {" - unique + high quality
        assert_eq!(anchor.line_index, 0);
        assert_eq!(anchor.quality, AnchorQuality::High);
        assert!(anchor.uniqueness_score > 0.4); // Appears only once
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
    }

    #[test]
    fn test_calculate_line_similarity() {
        assert!((calculate_line_similarity("const foo = 42;", "const foo = 42;") - 1.0).abs() < 0.01);
        // After normalization: "const foo=42;" → "const foo=42;" and "const  foo  =  42;" → "const foo = 42;"
        // These are different, so similarity will be high but not 1.0
        assert!(calculate_line_similarity("const foo=42;", "const  foo  =  42;") > 0.85); // similar but not identical
        assert!(calculate_line_similarity("const foo = 42;", "const bar = 99;") < 0.8);
    }
}