//! Helper module to extract unique identifiers (anchors) from code blocks.
//! Supports extraction from: JS, TS, Rust, Python, Go, Java, C#.

use regex::Regex;
use crate::apply_system::shared::types::{MatchingData, AnchorPoints};
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub struct Anchors {
    pub functions: Vec<String>,
    pub classes: Vec<String>,
    pub literals: Vec<String>,
    pub imports: Vec<String>,
}

pub fn extract_anchors(code: &str) -> Anchors {
    let mut anchors = Anchors {
        functions: Vec::new(),
        classes: Vec::new(),
        literals: Vec::new(),
        imports: Vec::new(),
    };

    // 1. Function Definitions
    // JS/TS: function foo, async function foo, const foo =
    // Rust: fn foo
    // Python: def foo
    // Go: func foo
    let fn_re = Regex::new(r"(?m)(?:function|fn|def|func)\s+([a-zA-Z0-9_]+)\s*\(").unwrap();
    for cap in fn_re.captures_iter(code) {
        if let Some(m) = cap.get(1) {
            anchors.functions.push(m.as_str().to_string());
        }
    }
    
    // JS/TS Arrow functions: const foo = (...) =>
    let arrow_fn_re = Regex::new(r"(?m)(?:const|let|var)\s+([a-zA-Z0-9_]+)\s*=\s*(?:async\s*)?\(.*?\)\s*=>").unwrap();
    for cap in arrow_fn_re.captures_iter(code) {
        if let Some(m) = cap.get(1) {
            anchors.functions.push(m.as_str().to_string());
        }
    }

    // 2. Class/Struct Definitions
    let class_re = Regex::new(r"(?m)(?:class|struct|impl|interface|enum|trait)\s+([a-zA-Z0-9_]+)").unwrap();
    for cap in class_re.captures_iter(code) {
        if let Some(m) = cap.get(1) {
            anchors.classes.push(m.as_str().to_string());
        }
    }

    // 3. Unique String Literals (> 15 chars)
    // Helps anchor if code structure is broken but strings remain
    // Support both single and double quotes, handle escaped quotes
    let str_re = Regex::new(r#""([^"\\]{15,}(?:\\.[^"\\]*)*)"|'([^'\\]{15,}(?:\\.[^'\\]*)*)'"#).unwrap();
    for cap in str_re.captures_iter(code) {
        // Check both capture groups (double and single quotes)
        if let Some(m) = cap.get(1).or_else(|| cap.get(2)) {
            anchors.literals.push(m.as_str().to_string());
        }
    }

    // 4. Import/Use Statements
    // import ... from, use ..., from ... import
    // Allow optional whitespace at start for indented imports
    let import_re = Regex::new(r"(?m)^\s*(?:import|use|from)\s+.*$").unwrap();
    for cap in import_re.captures_iter(code) {
        if let Some(m) = cap.get(0) {
            let s = m.as_str().trim();
            if s.len() > 10 {
                anchors.imports.push(s.to_string());
            }
        }
    }

        // 5. Variable Assignments (Strong Anchors)
        // Catches: const config =, formats_to_try =, let x =
        // Useful for blocks lacking function definitions
        let assign_re = Regex::new(r"(?m)^\s*(?:const|let|var|static)?\s*([a-zA-Z0-9_]+)\s*=\s*").unwrap();
        for cap in assign_re.captures_iter(code) {
            if let Some(m) = cap.get(1) {
                let name = m.as_str();
                // Filter out common short names to keep anchors high quality
                if name.len() >= 4 && name != "self" && name != "this" {
                    anchors.literals.push(name.to_string());
                }
            }
        }

        anchors
    }

/// Extracts matching data structure required by the Apply System logic in main.rs
/// This function bridges the gap between raw extraction and the MatchingData struct.
pub fn extract_matching_data(old_code: &str, _new_code: &str) -> MatchingData {
    let anchors_raw = extract_anchors(old_code);
    
    // Map internal Anchors struct to public AnchorPoints struct
    let anchor_points = AnchorPoints {
        function_name: anchors_raw.functions.first().cloned(),
        class_name: anchors_raw.classes.first().cloned(),
        unique_comments: Vec::new(), // Comments extraction logic can be added here if needed
        export_statements: anchors_raw.imports, 
        other_identifiers: anchors_raw.literals,
    };

    // Calculate hash for ExactHash matching strategy
    let mut hasher = Sha256::new();
    hasher.update(old_code.as_bytes());
    let code_hash = format!("{:x}", hasher.finalize());

    MatchingData {
        anchors: anchor_points,
        code_hash,
        context_before: Vec::new(),
        context_after: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_js_anchors() {
        let code = r#"
            import { useState } from 'react';
            function calculateTotal() {}
            const formatName = (name) => {};
            class UserProfile {}
            const msg = "This is a very long unique error message";
        "#;
        let anchors = extract_anchors(code);
        assert!(anchors.functions.contains(&"calculateTotal".to_string()));
        assert!(anchors.functions.contains(&"formatName".to_string()));
        assert!(anchors.classes.contains(&"UserProfile".to_string()));
        assert!(anchors.literals.contains(&"This is a very long unique error message".to_string()));
    }

    #[test]
    fn test_extract_rust_anchors() {
        let code = r#"
            use std::collections::HashMap;
            fn main() {}
            struct Config {}
            impl Config {}
        "#;
        let anchors = extract_anchors(code);
        assert!(anchors.functions.contains(&"main".to_string()));
        assert!(anchors.classes.contains(&"Config".to_string()));
        assert!(!anchors.imports.is_empty());
    }
}