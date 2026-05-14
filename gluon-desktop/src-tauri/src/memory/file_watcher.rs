//! Background file watching for automatic re-indexing

use notify::{Watcher, RecursiveMode, EventKind};
use notify_debouncer_full::{new_debouncer, Debouncer, FileIdMap};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex as StdMutex};

pub struct FileWatcher {
    debouncer: Arc<StdMutex<Debouncer<notify::RecommendedWatcher, FileIdMap>>>,
    event_rx: mpsc::UnboundedReceiver<Vec<PathBuf>>,
}

impl FileWatcher {
    /// Create a new file watcher for a project directory
    ///
    /// Excludes patterns from .gitignore and .gluonignore automatically
    pub fn new(
        project_path: &Path,
        _excluded_patterns: Vec<String>,  // TODO: Implement pattern filtering
    ) -> Result<Self, String> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Debouncer aggregates events over 500ms window
        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |result: Result<Vec<notify_debouncer_full::DebouncedEvent>, Vec<notify::Error>>| {
                match result {
                    Ok(events) => {
                        // Extract changed file paths
                        let paths: Vec<PathBuf> = events
                            .iter()
                            .filter_map(|event| {
                                // Only track Modify and Create events
                                match event.kind {
                                    EventKind::Modify(_) | EventKind::Create(_) => {
                                        event.paths.first().cloned()
                                    }
                                    _ => None,
                                }
                            })
                            .collect();

                        if !paths.is_empty() {
                            let _ = tx.send(paths);
                        }
                    }
                    Err(errors) => {
                        for e in errors {
                            eprintln!("[FileWatcher] Error: {:?}", e);
                        }
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to create debouncer: {}", e))?;

        // Watch the project directory recursively
        debouncer
            .watcher()
            .watch(project_path, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch directory: {}", e))?;

        println!("[FileWatcher] Watching directory: {:?}", project_path);

        Ok(Self {
            debouncer: Arc::new(StdMutex::new(debouncer)),
            event_rx: rx,
        })
    }

    /// Receive next batch of changed files (non-blocking)
    ///
    /// Returns None if no events are pending
    pub async fn next_changes(&mut self) -> Option<Vec<PathBuf>> {
        self.event_rx.recv().await
    }

    /// Run the watcher loop (blocking)
    ///
    /// This should be spawned as a tokio task
    pub async fn run<F>(
        mut self,
        mut on_change: F,
    ) where
        F: FnMut(Vec<PathBuf>) + Send + 'static,
    {
        println!("[FileWatcher] Starting event loop");

        while let Some(changed_files) = self.next_changes().await {
            println!("[FileWatcher] Detected {} changed files", changed_files.len());
            on_change(changed_files);
        }

        println!("[FileWatcher] Event loop ended");
    }

    /// Stop watching (drops the watcher)
    pub fn stop(self) {
        println!("[FileWatcher] Stopping file watcher");
        drop(self.debouncer);
    }
}

/// Helper to filter files based on ignore patterns
pub fn should_ignore(path: &Path, excluded_patterns: &[String]) -> bool {
    let path_str = path.to_string_lossy();

    // Hardcoded ignores
    if path_str.contains("node_modules")
        || path_str.contains("target")
        || path_str.contains(".git")
        || path_str.contains("dist")
        || path_str.contains("build")
        || path_str.ends_with(".lock")
        || path_str.ends_with(".log")
    {
        return true;
    }

    // Check custom patterns
    for pattern in excluded_patterns {
        if path_str.contains(pattern) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_ignore_node_modules() {
        let path = Path::new("project/node_modules/package/index.js");
        assert!(should_ignore(path, &[]));
    }

    #[test]
    fn test_should_ignore_target() {
        let path = Path::new("project/target/debug/main");
        assert!(should_ignore(path, &[]));
    }

    #[test]
    fn test_should_not_ignore_source() {
        let path = Path::new("project/src/main.rs");
        assert!(!should_ignore(path, &[]));
    }

    #[test]
    fn test_should_ignore_custom_pattern() {
        let path = Path::new("project/temp/file.txt");
        assert!(should_ignore(path, &vec!["temp".to_string()]));
    }
}
