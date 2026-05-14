//! Debounced Save for Workflow Graph
//!
//! Buffers changes in RAM and saves to disk only after 2 seconds of inactivity.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use std::path::PathBuf;

/// Debounced save manager
pub struct DebouncedSaver {
    /// Last modification timestamp
    last_modified: Arc<Mutex<Option<Instant>>>,
    /// Save delay duration (default: 2 seconds)
    delay: Duration,
    /// Flag indicating if save task is running
    save_task_running: Arc<Mutex<bool>>,
}

impl DebouncedSaver {
    pub fn new() -> Self {
        Self {
            last_modified: Arc::new(Mutex::new(None)),
            delay: Duration::from_secs(2),
            save_task_running: Arc::new(Mutex::new(false)),
        }
    }

    pub fn new_with_delay(delay_secs: u64) -> Self {
        Self {
            last_modified: Arc::new(Mutex::new(None)),
            delay: Duration::from_secs(delay_secs),
            save_task_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Signals that a modification has occurred
    pub fn mark_modified(&self) {
        let mut last = self.last_modified.lock().unwrap();
        *last = Some(Instant::now());
    }

    /// Starts the debounce save task (call this once on app init)
    pub fn start_auto_save<F>(
        &self,
        save_fn: F,
    ) where
        F: Fn() -> Result<(), String> + Send + 'static,
    {
        let last_modified = self.last_modified.clone();
        let delay = self.delay;
        let task_running = self.save_task_running.clone();

        // Check if task is already running
        {
            let mut running = task_running.lock().unwrap();
            if *running {
                println!("[DebouncedSave] Task already running, skipping start");
                return;
            }
            *running = true;
        }

        tokio::spawn(async move {
            loop {
                sleep(Duration::from_millis(500)).await; // Check every 500ms

                let should_save = {
                    let last = last_modified.lock().unwrap();
                    if let Some(last_time) = *last {
                        last_time.elapsed() >= delay
                    } else {
                        false
                    }
                };

                if should_save {
                    println!("[DebouncedSave] 💾 Triggering save after {} seconds of inactivity", delay.as_secs());

                    match save_fn() {
                        Ok(_) => {
                            println!("[DebouncedSave] ✅ Save successful");
                            // Clear the modification flag after successful save
                            let mut last = last_modified.lock().unwrap();
                            *last = None;
                        }
                        Err(e) => {
                            eprintln!("[DebouncedSave] ❌ Save failed: {}", e);
                        }
                    }
                }
            }
        });
    }

    /// Forces an immediate save (bypasses debounce)
    pub fn force_save<F>(&self, save_fn: F) -> Result<(), String>
    where
        F: Fn() -> Result<(), String>,
    {
        println!("[DebouncedSave] 🚀 Force save triggered");

        let result = save_fn();

        if result.is_ok() {
            // Clear modification flag after successful save
            let mut last = self.last_modified.lock().unwrap();
            *last = None;
        }

        result
    }
}

impl Default for DebouncedSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_debounced_save() {
        let saver = DebouncedSaver::new_with_delay(1); // 1 second for testing
        let save_count = Arc::new(AtomicUsize::new(0));
        let save_count_clone = save_count.clone();

        // Start auto-save task
        saver.start_auto_save(move || {
            save_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        // Mark as modified
        saver.mark_modified();

        // Wait 0.5 seconds - should NOT save yet
        sleep(Duration::from_millis(500)).await;
        assert_eq!(save_count.load(Ordering::SeqCst), 0);

        // Wait another 1 second - should save now
        sleep(Duration::from_secs(1)).await;
        assert_eq!(save_count.load(Ordering::SeqCst), 1);

        // Multiple modifications within delay window - should save only once
        for _ in 0..5 {
            saver.mark_modified();
            sleep(Duration::from_millis(200)).await;
        }

        // Wait for debounce
        sleep(Duration::from_secs(2)).await;
        assert_eq!(save_count.load(Ordering::SeqCst), 2); // Only one additional save
    }

    #[test]
    fn test_force_save() {
        let saver = DebouncedSaver::new();
        let save_count = Arc::new(AtomicUsize::new(0));
        let save_count_clone = save_count.clone();

        saver.mark_modified();

        // Force save immediately
        saver.force_save(move || {
            save_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }).unwrap();

        assert_eq!(save_count.load(Ordering::SeqCst), 1);
    }
}
