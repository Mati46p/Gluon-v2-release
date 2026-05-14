//! Snapshot Management Module
//!
//! Handles creation, storage, and restoration of file snapshots
//! for conflict detection and undo operations.
//!
//! Snapshots are stored in-memory (HashMap) for the current session only.

use crate::apply_system::types::SnapshotData;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Global snapshot manager
///
/// Stores one snapshot per file (previous iteration)
/// Cleared when app closes
pub struct SnapshotManager {
    snapshots: Arc<Mutex<HashMap<String, SnapshotData>>>,
}

impl SnapshotManager {
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a snapshot of the current file content
    ///
    /// Overwrites any existing snapshot for this file
    pub fn create_snapshot(&self, file_path: String, content: String) {
        let snapshot = SnapshotData::new(file_path.clone(), content);

        let mut snapshots = self.snapshots.lock().unwrap();
        snapshots.insert(file_path, snapshot);
    }

    /// Get snapshot for a file (if exists)
    pub fn get_snapshot(&self, file_path: &str) -> Option<SnapshotData> {
        let snapshots = self.snapshots.lock().unwrap();
        snapshots.get(file_path).cloned()
    }

    /// Check if file has changed since snapshot
    ///
    /// Returns:
    /// - None if no snapshot exists (first change)
    /// - Some(true) if content changed
    /// - Some(false) if content unchanged
    pub fn has_changed(&self, file_path: &str, current_content: &str) -> Option<bool> {
        let snapshot = self.get_snapshot(file_path)?;
        Some(snapshot.has_changed(current_content))
    }

    /// Remove snapshot for a file
    pub fn remove_snapshot(&self, file_path: &str) {
        let mut snapshots = self.snapshots.lock().unwrap();
        snapshots.remove(file_path);
    }

    /// Clear all snapshots
    pub fn clear_all(&self) {
        let mut snapshots = self.snapshots.lock().unwrap();
        snapshots.clear();
    }

    /// Get count of stored snapshots
    pub fn count(&self) -> usize {
        let snapshots = self.snapshots.lock().unwrap();
        snapshots.len()
    }
}

impl Default for SnapshotManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_snapshot() {
        let manager = SnapshotManager::new();

        manager.create_snapshot("test.ts".to_string(), "original code".to_string());

        let snapshot = manager.get_snapshot("test.ts");
        assert!(snapshot.is_some());

        let snap = snapshot.unwrap();
        assert_eq!(snap.content, "original code");
    }

    #[test]
    fn test_has_changed() {
        let manager = SnapshotManager::new();

        // No snapshot yet
        assert_eq!(manager.has_changed("test.ts", "any content"), None);

        // Create snapshot
        manager.create_snapshot("test.ts".to_string(), "original".to_string());

        // Same content
        assert_eq!(manager.has_changed("test.ts", "original"), Some(false));

        // Changed content
        assert_eq!(manager.has_changed("test.ts", "modified"), Some(true));
    }

    #[test]
    fn test_remove_snapshot() {
        let manager = SnapshotManager::new();

        manager.create_snapshot("test.ts".to_string(), "code".to_string());
        assert_eq!(manager.count(), 1);

        manager.remove_snapshot("test.ts");
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_clear_all() {
        let manager = SnapshotManager::new();

        manager.create_snapshot("test1.ts".to_string(), "code1".to_string());
        manager.create_snapshot("test2.ts".to_string(), "code2".to_string());
        assert_eq!(manager.count(), 2);

        manager.clear_all();
        assert_eq!(manager.count(), 0);
    }
}
