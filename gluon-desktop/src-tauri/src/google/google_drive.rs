use serde::{Deserialize, Serialize};
use tauri::State;

use crate::google::google_auth::GoogleAuthState;

// Google Drive API endpoints
const DRIVE_FILES_LIST_URL: &str = "https://www.googleapis.com/drive/v3/files";
const DRIVE_FILES_EXPORT_URL: &str = "https://www.googleapis.com/drive/v3/files";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(default)]
    pub web_view_link: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DriveFilesListResponse {
    files: Vec<DriveFile>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

/// List files from Google Drive
#[tauri::command]
pub async fn list_drive_files(
    folder_id: Option<String>,
    search_query: Option<String>,
    state: State<'_, GoogleAuthState>,
) -> Result<Vec<DriveFile>, String> {
    // Get access token
    let access_token = crate::google::google_auth::get_google_access_token(state)
        .await
        .map_err(|e| format!("Failed to get access token: {}", e))?;

    let client = reqwest::Client::new();

    // Build query
    let mut query = vec![
        ("pageSize", "100".to_string()),
        ("orderBy", "modifiedTime desc".to_string()), // Sortuj od najnowszych
        ("fields", "files(id,name,mimeType,webViewLink)".to_string()),
    ];

    // Base filter: Mime types + trash status
    let mut q_parts = vec![
        "(mimeType='application/vnd.google-apps.document' or mimeType='text/plain' or mimeType='text/markdown')".to_string(),
        "trashed = false".to_string()
    ];

    // Filter by folder if provided
    if let Some(fid) = folder_id {
        q_parts.push(format!("'{}' in parents", fid));
    }

    // Filter by name (Server-side search)
    if let Some(term) = search_query {
        if !term.trim().is_empty() {
            // Escape single quotes just in case
            let safe_term = term.replace("'", "\\'");
            q_parts.push(format!("name contains '{}'", safe_term));
        }
    }

    let q = q_parts.join(" and ");
    query.push(("q", q));

    let response = client
        .get(DRIVE_FILES_LIST_URL)
        .bearer_auth(&access_token)
        .query(&query)
        .send()
        .await
        .map_err(|e| format!("Failed to list files: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Drive API error {}: {}", status, body));
    }

    let list_response: DriveFilesListResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(list_response.files)
}

/// Download file content as text/markdown
#[tauri::command]
pub async fn download_file_content(
    file_id: String,
    state: State<'_, GoogleAuthState>,
) -> Result<String, String> {
    // Get access token
    let access_token = crate::google::google_auth::get_google_access_token(state)
        .await
        .map_err(|e| format!("Failed to get access token: {}", e))?;

    let client = reqwest::Client::new();

    // First, get file metadata to check mime type
    let metadata_url = format!("{}/{}", DRIVE_FILES_EXPORT_URL, file_id);
    let metadata_response = client
        .get(&metadata_url)
        .bearer_auth(&access_token)
        .query(&[("fields", "id,name,mimeType")]) // Fix: Request all mandatory fields for DriveFile struct
        .send()
        .await
        .map_err(|e| format!("Failed to get file metadata: {}", e))?;

    let metadata: DriveFile = metadata_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;

    // Download content based on mime type
    let content = if metadata.mime_type == "application/vnd.google-apps.document" {
        // Export Google Doc as plain text
        let export_url = format!("{}/{}/export", DRIVE_FILES_EXPORT_URL, file_id);
        let response = client
            .get(&export_url)
            .bearer_auth(&access_token)
            .query(&[("mimeType", "text/plain")])
            .send()
            .await
            .map_err(|e| format!("Failed to export document: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Export API error {}: {}", status, body));
        }

        response
            .text()
            .await
            .map_err(|e| format!("Failed to read content: {}", e))?
    } else {
        // Download regular file
        let download_url = format!("{}{}?alt=media", DRIVE_FILES_EXPORT_URL, file_id);
        let response = client
            .get(&download_url)
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| format!("Failed to download file: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Download API error {}: {}", status, body));
        }

        response
            .text()
            .await
            .map_err(|e| format!("Failed to read content: {}", e))?
    };

    Ok(content)
}

/// Get file info by ID
#[tauri::command]
pub async fn get_drive_file_info(
    file_id: String,
    state: State<'_, GoogleAuthState>,
) -> Result<DriveFile, String> {
    let access_token = crate::google::google_auth::get_google_access_token(state)
        .await
        .map_err(|e| format!("Failed to get access token: {}", e))?;

    let client = reqwest::Client::new();
    let url = format!("{}/{}", DRIVE_FILES_LIST_URL, file_id);

    let response = client
        .get(&url)
        .bearer_auth(&access_token)
        .query(&[("fields", "id,name,mimeType,webViewLink")])
        .send()
        .await
        .map_err(|e| format!("Failed to get file info: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Drive API error {}: {}", status, body));
    }

    let file: DriveFile = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(file)
}