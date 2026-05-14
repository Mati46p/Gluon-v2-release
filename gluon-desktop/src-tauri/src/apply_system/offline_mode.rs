/**
 * Change Export and Manual Application Support
 *
 * Provides functionality to export changes to various formats
 * for manual review and application
 */
use crate::apply_system::ChangeQueueItem;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Export format for changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeExport {
    pub changes: Vec<ChangeQueueItem>,
    pub exported_at: String,
    pub format_version: u32,
}

/// Export changes to JSON file for manual application
pub fn export_changes_to_file(
    changes: &[ChangeQueueItem],
    output_path: PathBuf,
) -> Result<(), String> {
    let export = ChangeExport {
        changes: changes.to_vec(),
        exported_at: chrono::Local::now().to_rfc3339(),
        format_version: 1,
    };

    let json = serde_json::to_string_pretty(&export)
        .map_err(|e| format!("Failed to serialize changes: {}", e))?;

    fs::write(&output_path, json).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

/// Generate unified diff patch for manual application
pub fn generate_diff_patch(changes: &[ChangeQueueItem]) -> String {
    let mut patch = String::new();

    patch.push_str("# Gluon Apply System - Manual Patch\n");
    patch.push_str(&format!(
        "# Generated: {}\n",
        chrono::Local::now().to_rfc3339()
    ));
    patch.push_str(&format!("# Total changes: {}\n\n", changes.len()));

    for (idx, change) in changes.iter().enumerate() {
        patch.push_str(&format!(
            "## Change {}/{}: {}\n",
            idx + 1,
            changes.len(),
            change.file_path
        ));
        patch.push_str(&format!(
            "## Lines: {}-{}\n\n",
            change.line_start, change.line_end
        ));

        // Unified diff format
        patch.push_str(&format!("--- a/{}\n", change.file_path));
        patch.push_str(&format!("+++ b/{}\n", change.file_path));
        patch.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            change.line_start,
            change.line_end - change.line_start + 1,
            change.line_start,
            change.new_code.lines().count()
        ));

        // Old code (removed)
        for line in change.old_code.lines() {
            patch.push_str(&format!("-{}\n", line));
        }

        // New code (added)
        for line in change.new_code.lines() {
            patch.push_str(&format!("+{}\n", line));
        }

        patch.push_str("\n");
    }

    patch
}

/// Generate manual application instructions
pub fn generate_manual_instructions(changes: &[ChangeQueueItem]) -> String {
    let mut instructions = String::new();

    instructions.push_str("# Manual Application Instructions\n\n");
    instructions.push_str("Apply these changes manually to your files:\n\n");

    for (idx, change) in changes.iter().enumerate() {
        instructions.push_str(&format!("{}. File: {}\n", idx + 1, change.file_path));
        instructions.push_str(&format!(
            "   Lines: {}-{}\n",
            change.line_start, change.line_end
        ));
        instructions.push_str("   Action: Replace the old code with the new code below\n\n");

        instructions.push_str("   OLD CODE (remove this):\n");
        for line in change.old_code.lines() {
            instructions.push_str(&format!("   | {}\n", line));
        }

        instructions.push_str("\n   NEW CODE (add this):\n");
        for line in change.new_code.lines() {
            instructions.push_str(&format!("   | {}\n", line));
        }

        instructions.push_str("\n   ---\n\n");
    }

    instructions.push_str("\nAlternatively:\n");
    instructions.push_str("1. Export changes to JSON using the Export button\n");
    instructions.push_str("2. Use the Apply System to automatically apply changes\n");

    instructions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply_system::ChangeStatus;
    use std::time::SystemTime;

    fn create_test_change() -> ChangeQueueItem {
        ChangeQueueItem {
            id: "test-1".to_string(),
            file_path: "src/main.rs".to_string(),
            line_start: 10,
            line_end: 12,
            old_code: "fn old() {\n    println!(\"old\");\n}".to_string(),
            new_code: "fn new() {\n    println!(\"new\");\n}".to_string(),
            matching_data: Default::default(),
            status: ChangeStatus::Pending,
            error_message: None,
            applied_timestamp: None,
            match_result: None,
            created_at: SystemTime::now(),
        }
    }

    #[test]
    fn test_generate_diff_patch() {
        let changes = vec![create_test_change()];
        let patch = generate_diff_patch(&changes);

        assert!(patch.contains("--- a/src/main.rs"));
        assert!(patch.contains("+++ b/src/main.rs"));
        assert!(patch.contains("-fn old()"));
        assert!(patch.contains("+fn new()"));
    }

    #[test]
    fn test_generate_manual_instructions() {
        let changes = vec![create_test_change()];
        let instructions = generate_manual_instructions(&changes);

        assert!(instructions.contains("File: src/main.rs"));
        assert!(instructions.contains("Lines: 10-12"));
        assert!(instructions.contains("OLD CODE"));
        assert!(instructions.contains("NEW CODE"));
    }
}
