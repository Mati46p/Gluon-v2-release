// Plik: gluon-desktop/src-tauri/src/apply_system/lazy/matcher.rs
//! AST-based Lazy Block Matching
//!
//! This module implements the Myers-diff-like algorithm for finding
//! which nodes in the original file should replace each lazy block.
//!
//! Adapted from Continue's deterministic.ts logic.

use tree_sitter::{Node, Tree};

/// Represents a replacement for a lazy block
#[derive(Debug, Clone)]
pub struct LazyReplacement {
    /// The lazy block node in the new file (comment node)
    pub lazy_block_node: NodeInfo,

    /// The nodes from the old file that should replace the lazy block
    pub replacement_nodes: Vec<NodeInfo>,
}

/// Information about a tree-sitter node
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub start_column: usize,
    pub end_column: usize,
    pub kind: String,
    pub text: String,
}

impl NodeInfo {
    pub fn from_node(node: Node, source: &str) -> Self {
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row,
            end_line: node.end_position().row,
            start_column: node.start_position().column,
            end_column: node.end_position().column,
            kind: node.kind().to_string(),
            text: node.utf8_text(source.as_bytes()).unwrap_or("").to_string(),
        }
    }
}

pub fn find_lazy_replacements(
    old_tree: &Tree,
    new_tree: &Tree,
    old_source: &str,
    new_source: &str,
) -> Vec<LazyReplacement> {
    let mut replacements = Vec::new();

    find_lazy_replacements_recursive(
        old_tree.root_node(),
        new_tree.root_node(),
        old_source,
        new_source,
        &mut replacements,
    );

    replacements
}

/// Recursive helper inspired by Continue's `findLazyBlockReplacements`
/// Key difference: Iterates primarily over structure to handle moves/insertions gracefully.
fn find_lazy_replacements_recursive(
    old_node: Node,
    new_node: Node,
    old_source: &str,
    new_source: &str,
    replacements: &mut Vec<LazyReplacement>,
) {
    // Base case: nodes are exactly the same
    if nodes_are_exact(old_node, new_node, old_source, new_source) {
        return;
    }

    // Base case: no lazy blocks in new node subtree
    if !contains_lazy_block(new_node, new_source) {
        return;
    }

    let mut left_children: Vec<Node> = old_node.named_children(&mut old_node.walk()).collect();
    let mut right_children: Vec<Node> = new_node.named_children(&mut new_node.walk()).collect();

    let mut is_lazy = false;
    let mut current_lazy_block: Option<NodeInfo> = None;
    let mut current_lazy_replacement_nodes = Vec::new();

    while !left_children.is_empty() && !right_children.is_empty() {
        let left_node = left_children[0];
        let right_node = right_children[0];

        // 1. Check if the current Right node is a Lazy Block
        if is_lazy_block(right_node, new_source) {
            // Enter "lazy mode"
            is_lazy = true;
            current_lazy_block = Some(NodeInfo::from_node(right_node, new_source));
            
            // Consume the lazy marker from Right
            right_children.remove(0);
            continue;
        }

        // 2. Look for the first match of Left (Old) in Right (New)
        // We look ahead in Right children to see if the current Left node appears later.
        let match_index = right_children
            .iter()
            .position(|&r_node| nodes_are_similar(left_node, r_node, old_source, new_source));

        match match_index {
            None => {
                // NO MATCH: The Left Node (Old) was removed or is covered by the lazy block.
                if is_lazy {
                    // If we are in lazy mode, we preserve this node (it's part of "... existing code ...")
                    current_lazy_replacement_nodes.push(NodeInfo::from_node(left_node, old_source));
                }
                // Consume the Left node (it's handled)
                left_children.remove(0);
            }
            Some(index) => {
                // MATCH FOUND at `index`!
                // This means `right_children[index]` corresponds to `left_node`.
                
                // All nodes in Right BEFORE `index` are NEW INSERTIONS.
                for _ in 0..index {
                    // We consume them so they stay in the new tree.
                    // Importantly: We do NOT advance Left here, because these new nodes 
                    // didn't match the current Left node.
                    right_children.remove(0);
                }

                // Now `right_children[0]` is the matching node.
                // Recurse into the match to resolve nested lazy blocks.
                find_lazy_replacements_recursive(
                    left_node,
                    right_children[0],
                    old_source,
                    new_source,
                    replacements,
                );

                // Consume both nodes (they are matched)
                left_children.remove(0);
                right_children.remove(0);

                // Exit "lazy mode" because we hit a solid anchor (explicit code in New)
                if is_lazy {
                    if let Some(lazy_block) = current_lazy_block.take() {
                        replacements.push(LazyReplacement {
                            lazy_block_node: lazy_block,
                            replacement_nodes: current_lazy_replacement_nodes.clone(),
                        });
                    }
                    is_lazy = false;
                    current_lazy_replacement_nodes.clear();
                }
            }
        }
    }

    // 3. Handle remaining Left nodes (Old code at the end)
    if is_lazy {
        if let Some(lazy_block) = current_lazy_block.take() {
            // Append all remaining Left nodes to the current lazy block
            current_lazy_replacement_nodes.extend(
                left_children.iter().map(|&node| NodeInfo::from_node(node, old_source))
            );
            replacements.push(LazyReplacement {
                lazy_block_node: lazy_block,
                replacement_nodes: current_lazy_replacement_nodes,
            });
        }
    }

    // 4. Handle remaining Right nodes (New code at the end)
    // Specifically, check for any extra lazy blocks that might be in the new code (e.g. empty ones)
    for right_node in right_children {
        if is_lazy_block(right_node, new_source) {
            replacements.push(LazyReplacement {
                lazy_block_node: NodeInfo::from_node(right_node, new_source),
                replacement_nodes: Vec::new(), // Empty replacement
            });
        }
    }
}

// --- Helpers ---

fn nodes_are_exact(old_node: Node, new_node: Node, old_source: &str, new_source: &str) -> bool {
    old_node.utf8_text(old_source.as_bytes()).unwrap_or("")
        == new_node.utf8_text(new_source.as_bytes()).unwrap_or("")
}

fn is_lazy_block(node: Node, source: &str) -> bool {
    if !node.kind().contains("comment") {
        return false;
    }
    let text = node.utf8_text(source.as_bytes()).unwrap_or("");
    // Check for standard lazy patterns
    text.contains("...") && (text.contains("existing code") || text.contains("rest of"))
}

fn contains_lazy_block(node: Node, source: &str) -> bool {
    if is_lazy_block(node, source) {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if contains_lazy_block(child, source) {
            return true;
        }
    }
    false
}

fn nodes_are_similar(old_node: Node, new_node: Node, old_source: &str, new_source: &str) -> bool {
    if old_node.kind() != new_node.kind() {
        return false;
    }

    // 1. Identical Name Check (Fastest & Most Reliable)
    if let (Some(old_name), Some(new_name)) = (
        old_node.child_by_field_name("name"),
        new_node.child_by_field_name("name"),
    ) {
        let old_name_text = old_name.utf8_text(old_source.as_bytes()).unwrap_or("").trim();
        let new_name_text = new_name.utf8_text(new_source.as_bytes()).unwrap_or("").trim();
        if !old_name_text.is_empty() && old_name_text == new_name_text {
            return true;
        }
        return false;
    }

    // 2. Fallback: Identifier lookup for definitions (e.g. JS functions without explicit 'name' field in some grammars)
    fn get_def_name(s: &str) -> Option<&str> {
        s.split_whitespace().nth(1).map(|n| n.split('(').next().unwrap_or(n))
    }

    if old_node.kind().contains("definition") || old_node.kind().contains("declaration") {
        let old_text = old_node.utf8_text(old_source.as_bytes()).unwrap_or("");
        let new_text = new_node.utf8_text(new_source.as_bytes()).unwrap_or("");
        
        if let (Some(n1), Some(n2)) = (get_def_name(old_text), get_def_name(new_text)) {
            if n1 == n2 { return true; }
        }
    }

    // 3. Exact Content Match (for small nodes)
    let old_text = old_node.utf8_text(old_source.as_bytes()).unwrap_or("").trim();
    let new_text = new_node.utf8_text(new_source.as_bytes()).unwrap_or("").trim();
    if old_text == new_text {
        return true;
    }

    // 4. Fuzzy First Line Match
    let old_first = old_text.lines().next().unwrap_or("").trim();
    let new_first = new_text.lines().next().unwrap_or("").trim();
    
    strings_within_lev_threshold(old_first, new_first, 0.2)
}

fn strings_within_lev_threshold(a: &str, b: &str, threshold: f64) -> bool {
    if a == b { return true; }
    let dist = levenshtein_distance(a, b);
    let max_len = a.len().max(b.len()) as f64;
    if max_len == 0.0 { return true; }
    (dist as f64) / max_len <= threshold
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let len1 = v1.len();
    let len2 = v2.len();
    if len1 > len2 { return levenshtein_distance(s2, s1); }
    let mut cache: Vec<usize> = (0..=len1).collect();
    let mut dist_diag;
    let mut dist_left;
    for j in 1..=len2 {
        dist_diag = cache[0];
        cache[0] = j;
        for i in 1..=len1 {
            dist_left = cache[i];
            let cost = if v1[i - 1] == v2[j - 1] { 0 } else { 1 };
            cache[i] = std::cmp::min(std::cmp::min(cache[i] + 1, cache[i - 1] + 1), dist_diag + cost);
            dist_diag = dist_left;
        }
    }
    cache[len1]
}