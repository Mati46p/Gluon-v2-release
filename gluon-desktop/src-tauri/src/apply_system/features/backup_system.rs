use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use std::io::{BufRead, BufReader};
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub filename: String,
    pub filepath: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub project_path: String,
    pub project_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Unchanged,
    Modified,
    New,
    Deleted,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFilePreview {
    pub path: String,
    pub backup_content: String,
    pub current_content: Option<String>,
    pub status: FileStatus,
}

pub fn scan_for_backups(project_paths: Vec<String>, global_download_path: Option<String>) -> Vec<BackupEntry> {
    let mut backups = Vec::new();
    let mut scanned_paths = std::collections::HashSet::new();

    let mut dirs_to_scan = Vec::new();
    for p in &project_paths {
        dirs_to_scan.push((p.clone(), false)); // (path, is_download_folder)
    }
    if let Some(p) = &global_download_path {
        dirs_to_scan.push((p.clone(), true));
    }

    for (dir, is_download) in dirs_to_scan {
        if scanned_paths.contains(&dir) { continue; }
        scanned_paths.insert(dir.clone());

        let path = Path::new(&dir);
        if !path.exists() { continue; }

        for entry in WalkDir::new(path).max_depth(1).into_iter().filter_map(|e| e.ok()) {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.contains("-context-") && fname.ends_with(".txt") {
                if let Ok(metadata) = entry.metadata() {
                    let created_at = if let Some(idx) = fname.find("-context-") {
                        let date_part = &fname[idx + 9 .. fname.len() - 4];
                        date_part.to_string()
                    } else {
                        "Unknown".to_string()
                    };

                    let project_name = if is_download {
                        Path::new(&dir)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Downloads")
                            .to_string()
                    } else {
                        Path::new(&dir)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string()
                    };

                    backups.push(BackupEntry {
                        filename: fname,
                        filepath: entry.path().to_string_lossy().to_string(),
                        created_at,
                        size_bytes: metadata.len(),
                        project_path: dir.clone(),
                        project_name,
                    });
                }
            }
        }
    }

    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    backups
}

pub fn parse_backup_file(backup_path: &str) -> Result<Vec<BackupFilePreview>, String> {
    let file = fs::File::open(backup_path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    
    let mut files = Vec::new();
    let mut current_root_path = String::new();
    let mut current_file_path = String::new();
    let mut current_content = String::new();
    let mut in_files_section = false;

    for line_res in reader.lines() {
        let line = line_res.map_err(|e| e.to_string())?;

        if line.trim() == "=== PROJECT FILES ===" {
            in_files_section = true;
            continue;
        }

        if !in_files_section { continue; }

        if line.starts_with("Path: ") {
            current_root_path = line.trim_start_matches("Path: ").trim().to_string();
            current_root_path = current_root_path.replace('\\', "/");
            continue;
        }

        if line.starts_with("// ") && !line.starts_with("//  ") && !current_root_path.is_empty() {
            let potential_path = line.trim_start_matches("// ").trim();
            if potential_path.contains('.') && !potential_path.contains(' ') {
                if !current_file_path.is_empty() {
                    let full_path = if Path::new(&current_file_path).is_absolute() {
                        current_file_path.clone()
                    } else {
                        format!("{}/{}", current_root_path, current_file_path)
                    };
                    files.push(process_parsed_file(&full_path, &current_content));
                }
                current_file_path = potential_path.to_string();
                current_content.clear();
                continue;
            }
        }

        if !current_file_path.is_empty() {
            current_content.push_str(&line);
            current_content.push('\n');
        }
    }

    if !current_file_path.is_empty() {
        let full_path = if Path::new(&current_file_path).is_absolute() {
            current_file_path.clone()
        } else {
            format!("{}/{}", current_root_path, current_file_path)
        };
        files.push(process_parsed_file(&full_path, &current_content));
    }

    Ok(files)
}

fn process_parsed_file(path: &str, content: &str) -> BackupFilePreview {
    let clean_content = content.trim_end();
    
    let (current_content, status) = match fs::read_to_string(path) {
        Ok(disk_content) => {
            let norm_disk = disk_content.replace("\r\n", "\n");
            let norm_backup = clean_content.replace("\r\n", "\n");
            
            if norm_disk == norm_backup {
                (Some(disk_content), FileStatus::Unchanged)
            } else {
                (Some(disk_content), FileStatus::Modified)
            }
        },
        Err(_) => (None, FileStatus::New)
    };

    BackupFilePreview {
        path: path.to_string(),
        backup_content: clean_content.to_string(),
        current_content,
        status,
    }
}

pub fn restore_files(files: Vec<BackupFilePreview>) -> Result<usize, String> {
    let mut restored_count = 0;

    for file in files {
        let path = Path::new(&file.path);
        
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        fs::write(path, &file.backup_content).map_err(|e| format!("Failed to write {}: {}", file.path, e))?;
        restored_count += 1;
    }

    Ok(restored_count)
}