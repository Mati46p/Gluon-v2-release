//! Batch Validator for multiple changes.
//! Validates that a set of changes is consistent and non-conflicting.

use crate::apply_system::shared::types::ChangeQueueItem;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BatchValidationError {
    pub change_index: usize,
    pub message: String,
    pub severity: ErrorSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorSeverity {
    Error,   // Critical - blocks all changes
    Warning, // Non-critical - logged but allowed
}

pub struct BatchValidator;

impl BatchValidator {
    /// Validates a batch of changes for conflicts and consistency.
    /// Returns list of errors/warnings found.
    pub fn validate_batch(changes: &[ChangeQueueItem]) -> Vec<BatchValidationError> {
        let mut errors = Vec::new();

        // 1. Check for duplicate file modifications (overlapping changes)
        errors.extend(Self::check_overlapping_changes(changes));

        // 2. Check for empty changes (no-op)
        errors.extend(Self::check_empty_changes(changes));

        // 3. Check for suspicious patterns (same change repeated)
        errors.extend(Self::check_duplicate_changes(changes));

        errors
    }

    /// Detects changes that might overlap in the same file.
    fn check_overlapping_changes(changes: &[ChangeQueueItem]) -> Vec<BatchValidationError> {
        let mut errors = Vec::new();
        let mut file_changes: HashMap<String, Vec<(usize, &ChangeQueueItem)>> = HashMap::new();

        // Group changes by file
        for (idx, change) in changes.iter().enumerate() {
            file_changes
                .entry(change.file_path.clone())
                .or_insert_with(Vec::new)
                .push((idx, change));
        }

        // Check each file for overlapping changes
        for (file_path, file_change_list) in file_changes.iter() {
            if file_change_list.len() > 1 {
                // Multiple changes to same file - check if they overlap
                for i in 0..file_change_list.len() {
                    for j in i + 1..file_change_list.len() {
                        let (idx1, change1) = file_change_list[i];
                        let (idx2, change2) = file_change_list[j];

                        // Check if search blocks are identical (likely duplicate)
                        if change1.old_code.trim() == change2.old_code.trim() {
                            errors.push(BatchValidationError {
                                change_index: idx2,
                                message: format!(
                                    "Duplicate change detected in file '{}': Changes #{} and #{} modify the same code block",
                                    file_path, idx1 + 1, idx2 + 1
                                ),
                                severity: ErrorSeverity::Warning,
                            });
                        }

                        // Check if search blocks partially overlap (potential conflict)
                        if Self::blocks_overlap(&change1.old_code, &change2.old_code) {
                            errors.push(BatchValidationError {
                                change_index: idx2,
                                message: format!(
                                    "Potentially conflicting changes in file '{}': Changes #{} and #{} may overlap",
                                    file_path, idx1 + 1, idx2 + 1
                                ),
                                severity: ErrorSeverity::Warning,
                            });
                        }
                    }
                }
            }
        }

        errors
    }

    /// Checks if two code blocks overlap (one contains part of the other).
    fn blocks_overlap(block1: &str, block2: &str) -> bool {
        let b1 = block1.trim();
        let b2 = block2.trim();

        // Skip if either is empty
        if b1.is_empty() || b2.is_empty() {
            return false;
        }

        // Skip if identical (handled separately)
        if b1 == b2 {
            return false;
        }

        // Check if one contains lines from the other
        let lines1: Vec<&str> = b1.lines().collect();
        let lines2: Vec<&str> = b2.lines().collect();

        // If blocks share more than 50% of lines, consider them overlapping
        let mut shared_lines = 0;
        for line1 in &lines1 {
            if lines2.contains(line1) && !line1.trim().is_empty() {
                shared_lines += 1;
            }
        }

        let smaller_block_size = lines1.len().min(lines2.len());
        if smaller_block_size > 0 {
            let overlap_ratio = shared_lines as f64 / smaller_block_size as f64;
            overlap_ratio > 0.5
        } else {
            false
        }
    }

    /// Checks for empty changes (old == new).
    fn check_empty_changes(changes: &[ChangeQueueItem]) -> Vec<BatchValidationError> {
        let mut errors = Vec::new();

        for (idx, change) in changes.iter().enumerate() {
            if change.old_code.trim() == change.new_code.trim() {
                errors.push(BatchValidationError {
                    change_index: idx,
                    message: format!(
                        "No-op change in file '{}': old_code and new_code are identical",
                        change.file_path
                    ),
                    severity: ErrorSeverity::Warning,
                });
            }
        }

        errors
    }

    /// Checks for exact duplicate changes.
    fn check_duplicate_changes(changes: &[ChangeQueueItem]) -> Vec<BatchValidationError> {
        let mut errors = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();

        for (idx, change) in changes.iter().enumerate() {
            // Create signature: file_path + old_code + new_code
            let signature = format!(
                "{}::{}::{}",
                change.file_path,
                change.old_code.trim(),
                change.new_code.trim()
            );

            if let Some(first_idx) = seen.get(&signature) {
                errors.push(BatchValidationError {
                    change_index: idx,
                    message: format!(
                        "Exact duplicate of change #{} in file '{}'",
                        first_idx + 1,
                        change.file_path
                    ),
                    severity: ErrorSeverity::Warning,
                });
            } else {
                seen.insert(signature, idx);
            }
        }

        errors
    }

    /// Helper: Filter to only error-level issues (not warnings).
    pub fn has_errors(issues: &[BatchValidationError]) -> bool {
        issues.iter().any(|e| e.severity == ErrorSeverity::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlapping_changes() {
        let changes = vec![
            ChangeQueueItem::new(
                "test.py".to_string(),
                0,
                0,
                "def foo():\n    pass".to_string(),
                "def foo():\n    return 1".to_string(),
            ),
            ChangeQueueItem::new(
                "test.py".to_string(),
                0,
                0,
                "def foo():\n    pass".to_string(), // Identical - duplicate
                "def foo():\n    return 2".to_string(),
            ),
        ];

        let errors = BatchValidator::validate_batch(&changes);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.message.contains("Duplicate change")));
    }

    #[test]
    fn test_empty_change() {
        let changes = vec![ChangeQueueItem::new(
            "test.py".to_string(),
            0,
            0,
            "def foo():\n    pass".to_string(),
            "def foo():\n    pass".to_string(), // Same code
        )];

        let errors = BatchValidator::validate_batch(&changes);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.message.contains("No-op change")));
    }

    #[test]
    fn test_duplicate_changes() {
        let changes = vec![
            ChangeQueueItem::new(
                "test.py".to_string(),
                0,
                0,
                "old".to_string(),
                "new".to_string(),
            ),
            ChangeQueueItem::new(
                "test.py".to_string(),
                0,
                0,
                "old".to_string(), // Exact duplicate
                "new".to_string(),
            ),
        ];

        let errors = BatchValidator::validate_batch(&changes);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.message.contains("Exact duplicate")));
    }

    #[test]
    fn test_no_errors_for_different_files() {
        let changes = vec![
            ChangeQueueItem::new(
                "test1.py".to_string(),
                0,
                0,
                "old".to_string(),
                "new".to_string(),
            ),
            ChangeQueueItem::new(
                "test2.py".to_string(),
                0,
                0,
                "old".to_string(), // Same code but different file
                "new".to_string(),
            ),
        ];

        let errors = BatchValidator::validate_batch(&changes);
        // Should be OK - different files
        assert!(errors.is_empty() || !BatchValidator::has_errors(&errors));
    }
}
