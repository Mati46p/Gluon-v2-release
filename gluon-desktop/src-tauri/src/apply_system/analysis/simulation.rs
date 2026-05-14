//! Post-Transaction Integrity Check
//!
//! Validates the state of the file AFTER applying changes in memory.
//! Ensures we don't introduce new syntax errors into the file.

use super::engine::AnalysisEngine;
use super::validation::AstValidator;
use super::languages::SupportedLanguage;

#[derive(Debug)]
pub enum IntegrityStatus {
    Safe,                   // No new errors introduced
    Degraded {              // New errors appeared
        new_error_count: usize,
        original_error_count: usize,
        first_error_msg: String,
    },
    Improved,               // Errors actually decreased (fixing bugs!)
    Neutral,                // Errors count stayed same (maybe replaced one bug with another, or unrelated)
}

pub struct SimulationGuard;

impl SimulationGuard {
    /// Compares the AST health before and after a change.
    ///
    /// # Arguments
    /// * `original_content` - File content before change
    /// * `new_content` - File content after applying change (in memory)
    /// * `file_path` - Used for language detection
    ///
    /// # Returns
    /// IntegrityStatus indicating if the change is safe.
    pub fn check_integrity(original_content: &str, new_content: &str, file_path: &str) -> IntegrityStatus {
        let language = match SupportedLanguage::from_path(file_path) {
            Some(l) => l,
            None => return IntegrityStatus::Safe, // Can't validate unsupported languages
        };

        // Parse original
        let original_tree_res = AnalysisEngine::parse(original_content, file_path);
        let original_errors = if let Ok(tree) = original_tree_res {
            AstValidator::validate(original_content, &tree, language).len()
        } else {
            0 // Assume original was OK if parser failed unexpectedly, or treat as 0 to be safe
        };

        // Parse new
        let new_tree_res = AnalysisEngine::parse(new_content, file_path);
        let (new_errors, first_msg) = if let Ok(tree) = new_tree_res {
            let errors = AstValidator::validate(new_content, &tree, language);
            let msg = errors.first().map(|e| e.message.clone()).unwrap_or_default();
            (errors.len(), msg)
        } else {
            return IntegrityStatus::Degraded {
                new_error_count: 1,
                original_error_count: original_errors,
                first_error_msg: "Parser failed to process new content".to_string(),
            };
        };

        // Compare
        if new_errors > original_errors {
            IntegrityStatus::Degraded {
                new_error_count: new_errors,
                original_error_count: original_errors,
                first_error_msg: first_msg,
            }
        } else if new_errors < original_errors {
            IntegrityStatus::Improved
        } else if new_errors == 0 {
            IntegrityStatus::Safe
        } else {
            IntegrityStatus::Neutral // Errors exist but count didn't increase
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integrity_check_python() {
        let original = "def foo(): pass";
        // Introduce syntax error (missing colon)
        let broken = "def foo() pass"; 
        
        let status = SimulationGuard::check_integrity(original, broken, "test.py");
        
        match status {
            IntegrityStatus::Degraded { .. } => (), // Expected
            _ => panic!("Should detect degradation"),
        }
    }

    #[test]
    fn test_integrity_safe_update() {
        let original = "def foo(): pass";
        let updated = "def foo(): return 1";
        
        let status = SimulationGuard::check_integrity(original, updated, "test.py");
        
        match status {
            IntegrityStatus::Safe | IntegrityStatus::Improved => (),
            _ => panic!("Should be safe"),
        }
    }
}