//! Git integration for temporal consistency
//!
//! Tracks commit hashes to version vectors and enable time-travel queries

use git2::{Repository, Oid};
use std::path::Path;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

pub struct GitTracker {
    repo: Option<Repository>,  // None if not a git repo
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    pub message: String,
}

impl GitTracker {
    /// Initialize Git tracker for a project
    ///
    /// Returns Ok with tracker even if not a git repo (graceful degradation)
    pub fn new(project_path: &Path) -> Result<Self, String> {
        match Repository::discover(project_path) {
            Ok(repo) => {
                println!("[GitTracker] Initialized for repo: {:?}", repo.path());
                Ok(Self { repo: Some(repo) })
            }
            Err(e) => {
                println!("[GitTracker] No git repo found at {:?}: {}. Operating in non-git mode.", project_path, e);
                Ok(Self { repo: None })
            }
        }
    }

    /// Get current HEAD commit hash
    ///
    /// Returns "nogit" if not a git repository
    pub fn current_commit(&self) -> Result<String, String> {
        match &self.repo {
            Some(repo) => {
                let head = repo.head().map_err(|e| format!("Failed to get HEAD: {}", e))?;
                let commit = head.peel_to_commit().map_err(|e| format!("Failed to peel commit: {}", e))?;
                Ok(commit.id().to_string())
            }
            None => Ok("nogit".to_string()),
        }
    }

    /// Get detailed commit information
    pub fn commit_info(&self, commit_hash: &str) -> Result<CommitInfo, String> {
        match &self.repo {
            Some(repo) => {
                // Special case: "nogit" placeholder
                if commit_hash == "nogit" {
                    return Ok(CommitInfo {
                        hash: "nogit".to_string(),
                        author: "unknown".to_string(),
                        timestamp: Utc::now(),
                        message: "No git repository".to_string(),
                    });
                }

                let oid = Oid::from_str(commit_hash)
                    .map_err(|e| format!("Invalid commit hash: {}", e))?;
                let commit = repo.find_commit(oid)
                    .map_err(|e| format!("Commit not found: {}", e))?;

                let timestamp = DateTime::from_timestamp(commit.time().seconds(), 0)
                    .unwrap_or_else(Utc::now);

                Ok(CommitInfo {
                    hash: commit_hash.to_string(),
                    author: commit.author().name().unwrap_or("unknown").to_string(),
                    timestamp,
                    message: commit.message().unwrap_or("").to_string(),
                })
            }
            None => Ok(CommitInfo {
                hash: "nogit".to_string(),
                author: "unknown".to_string(),
                timestamp: Utc::now(),
                message: "No git repository".to_string(),
            }),
        }
    }

    /// Check if a file has changed since a specific commit
    ///
    /// Returns true if changed, false if unchanged or if not a git repo
    pub fn file_changed(&self, file_path: &Path, last_commit_hash: &str) -> Result<bool, String> {
        match &self.repo {
            Some(repo) => {
                let current = self.current_commit()?;

                // Same commit = no changes
                if current == last_commit_hash {
                    return Ok(false);
                }

                // TODO: Implement precise diff checking using git2::Diff
                // For now, conservatively assume changed if commits differ
                Ok(true)
            }
            None => Ok(false),  // Non-git repos always report no change
        }
    }

    /// Get current branch name
    pub fn current_branch(&self) -> Result<String, String> {
        match &self.repo {
            Some(repo) => {
                let head = repo.head().map_err(|e| format!("Failed to get HEAD: {}", e))?;
                let branch_name = head.shorthand().unwrap_or("detached");
                Ok(branch_name.to_string())
            }
            None => Ok("nobranch".to_string()),
        }
    }

    /// Check if repository is in a git repo
    pub fn is_git_repo(&self) -> bool {
        self.repo.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_git_tracker_non_git_dir() {
        let tracker = GitTracker::new(Path::new("/tmp")).unwrap();
        assert!(!tracker.is_git_repo());

        let commit = tracker.current_commit().unwrap();
        assert_eq!(commit, "nogit");
    }

    #[test]
    fn test_generate_commit_info_nogit() {
        let tracker = GitTracker::new(Path::new("/tmp")).unwrap();
        let info = tracker.commit_info("nogit").unwrap();
        assert_eq!(info.hash, "nogit");
        assert_eq!(info.author, "unknown");
    }
}
