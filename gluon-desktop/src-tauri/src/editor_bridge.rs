use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};

struct EditorConnection {
    sender: mpsc::UnboundedSender<String>,
    roots: Vec<String>,
    last_activity: u64, // Unix timestamp in milliseconds
}

#[derive(Clone, Serialize, Debug)]
pub struct EditorEditRequest {
    pub id: String,
    pub file_path: String,
    pub new_content: String, // Pełna nowa zawartość pliku

    // Extended fields for undo/redo tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_id: Option<String>,  // UUID for tracking individual changes

    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,   // Common ID for changes from one window

    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_content: Option<String>, // Content before change (for undo)

    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_start: Option<usize>,   // Starting line of the change
}

#[derive(Deserialize, Debug)]
pub struct EditorEditResponse {
    pub id: String,
    pub success: bool,
    pub error: Option<String>,
}

// Struktury dla powiadomień wizualnych (Flash Effect)
#[derive(Debug, Serialize, Clone)]
pub struct ChangeRange {
    pub start_line: usize, // 0-based
    pub end_line: usize,   // 0-based
}

#[derive(Debug, Serialize, Clone)]
pub struct FileChangeNotification {
    pub path: String,
    pub ranges: Vec<ChangeRange>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EditorMessage {
    ShowChanges { files: Vec<FileChangeNotification> },
    ApplyEdit(EditorEditRequest),
}

pub struct EditorBridge {
    // Lista aktywnych połączeń zamiast jednego
    connections: Arc<Mutex<Vec<EditorConnection>>>,
    // Mapa oczekujących odpowiedzi: RequestID -> Kanał zwrotny
    pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<Result<(), String>>>>>,
}

impl EditorBridge {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(Vec::new())),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Rejestruje nowe połączenie z VS Code
    pub fn register_connection(&self, sender: mpsc::UnboundedSender<String>, roots: Vec<String>) {
        let mut conns = self.connections.lock().unwrap();
        // Remove closed channels
        conns.retain(|c| !c.sender.is_closed());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        conns.push(EditorConnection {
            sender,
            roots,
            last_activity: now
        });
        println!("[EditorBridge] VS Code connected (Total: {}).", conns.len());
    }

    pub fn disconnect(&self) {
        let mut conns = self.connections.lock().unwrap();
        conns.retain(|c| !c.sender.is_closed());
        println!("[EditorBridge] VS Code disconnected (Active: {}).", conns.len());
    }

    pub fn is_connected(&self) -> bool {
        !self.connections.lock().unwrap().is_empty()
    }

    /// Aktualizuje timestamp aktywności dla workspace zawierającego podany plik
    fn update_activity_for_file(&self, file_path: &str) {
        let mut conns = self.connections.lock().unwrap();
        let normalized_target = Self::normalize_path_string(file_path);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        for conn in conns.iter_mut() {
            for root in &conn.roots {
                let normalized_root = Self::normalize_path_string(root);
                let root_prefix = if normalized_root.ends_with('/') {
                    normalized_root.clone()
                } else {
                    format!("{}/", normalized_root)
                };

                if normalized_target == normalized_root || normalized_target.starts_with(&root_prefix) {
                    conn.last_activity = now;
                    // println!("[EditorBridge] Updated activity for workspace: {}", root); // Disabled - too verbose
                    return;
                }
            }
        }
    }

    /// Aktualizuje timestamp aktywności dla workspace z podaną listą roots
    /// Używane przez heartbeat z VS Code gdy okno otrzymuje focus
    pub fn update_activity_for_roots(&self, roots: Vec<String>) {
        let mut conns = self.connections.lock().unwrap();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        for conn in conns.iter_mut() {
            // Sprawdź czy jakikolwiek root w connection pasuje do podanych roots
            for conn_root in &conn.roots {
                let normalized_conn_root = Self::normalize_path_string(conn_root);
                for target_root in &roots {
                    let normalized_target = Self::normalize_path_string(target_root);
                    if normalized_conn_root == normalized_target {
                        conn.last_activity = now;
                        // println!("[EditorBridge] 💓 Updated activity for workspace: {}", conn_root); // Disabled - too verbose
                        return;
                    }
                }
            }
        }
    }

    /// Zwraca wszystkie aktywne workspace roots z połączonych VS Code instancji
    /// Roots są zwracane w kolejności od ostatnio używanego (wg last_activity)
    pub fn get_all_roots(&self) -> Vec<String> {
        let conns = self.connections.lock().unwrap();

        // Sortuj połączenia wg last_activity (malejąco - najnowszy pierwszy)
        let mut sorted_conns: Vec<&EditorConnection> = conns.iter().collect();
        sorted_conns.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        let mut all_roots = Vec::new();
        for conn in sorted_conns {
            all_roots.extend(conn.roots.clone());
        }

        all_roots
    }

    fn get_sender_for_file(&self, file_path: &str) -> Option<mpsc::UnboundedSender<String>> {
        let conns = self.connections.lock().unwrap();
        
        // 1. Normalizacja ścieżki pliku (dla Windows i unifikacji separatorów)
        let normalized_target = Self::normalize_path_string(file_path);

        // Zbieramy kandydatów: (długość_root, sender)
        let mut candidates: Vec<(usize, mpsc::UnboundedSender<String>)> = Vec::new();

        for (i, conn) in conns.iter().enumerate() {
            for root in &conn.roots {
                let normalized_root = Self::normalize_path_string(root);
                
                // Sprawdzamy czy znormalizowana ścieżka pliku zaczyna się od znormalizowanego roota
                // Dodajemy "/" na końcu roota, aby uniknąć dopasowania np. "/project-api" do "/project"
                let root_prefix = if normalized_root.ends_with('/') { 
                    normalized_root.clone() 
                } else { 
                    format!("{}/", normalized_root) 
                };

                // Obsługa pliku w roocie (bez slash) lub w podkatalogu
                if normalized_target == normalized_root || normalized_target.starts_with(&root_prefix) {
                    println!("[EditorBridge] Match found: Window #{} owns '{}'", i, root);
                    candidates.push((normalized_root.len(), conn.sender.clone()));
                }
            }
        }

        // 2. Wybieramy kandydata z NAJDŁUŻSZĄ ścieżką root (najbardziej specyficzne okno)
        // To naprawia problem monorepo (otwarte okno root i okno podkatalogu)
        candidates.sort_by(|a, b| b.0.cmp(&a.0)); // Sort malejąco po długości

        if let Some((_, sender)) = candidates.first() {
            return Some(sender.clone());
        }

        println!("[EditorBridge] ⚠️ No specific window found for: {}. Using fallback (last active).", file_path);

        // 3. Fallback: Ostatnie aktywne okno (jeśli plik jest spoza otwartych workspace'ów)
        if let Some(last) = conns.last() {
            return Some(last.sender.clone());
        }

        None
    }

    /// Pomocnicza funkcja do normalizacji ścieżek (Windows/Unix agnostic)
    fn normalize_path_string(path: &str) -> String {
        let p = path.replace('\\', "/");
        if cfg!(windows) {
            p.to_lowercase()
        } else {
            p
        }
    }

    /// Wysyła żądanie edycji do VS Code i czeka na potwierdzenie (legacy)
    pub async fn request_edit(&self, file_path: String, new_content: String) -> Result<(), String> {
        self.request_edit_extended(
            file_path,
            new_content,
            None,  // changeId
            None,  // batchId
            None,  // oldContent
            None,  // lineStart
        ).await
    }

    /// Wysyła żądanie edycji do VS Code z rozszerzonymi polami dla undo/redo
    pub async fn request_edit_extended(
        &self,
        file_path: String,
        new_content: String,
        change_id: Option<String>,
        batch_id: Option<String>,
        old_content: Option<String>,
        line_start: Option<usize>,
    ) -> Result<(), String> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (resp_tx, resp_rx) = oneshot::channel();

        // 1. Zarejestruj oczekiwanie na odpowiedź
        {
            let mut pending = self.pending_requests.lock().unwrap();
            pending.insert(request_id.clone(), resp_tx);
        }

        // 2. Przygotuj wiadomość JSON z dodatkowymi polami
        let mut payload = serde_json::json!({
            "type": "apply_edit",
            "id": request_id,
            "filePath": file_path,
            "content": new_content
        });

        // Dodaj opcjonalne pola dla undo/redo tracking
        if let Some(cid) = change_id {
            payload["changeId"] = serde_json::json!(cid);
        }
        if let Some(bid) = batch_id {
            payload["batchId"] = serde_json::json!(bid);
        }
        if let Some(old) = old_content {
            payload["oldContent"] = serde_json::json!(old);
        }
        if let Some(ls) = line_start {
            payload["lineStart"] = serde_json::json!(ls);
        }

        // 3. Wyślij przez WebSocket (Routing do odpowiedniego okna)
        let sender = self.get_sender_for_file(&file_path)
            .ok_or_else(|| "No matching VS Code window found for this file".to_string())?;

        let send_result = sender.send(payload.to_string());

        if let Err(_) = send_result {
            // Sprzątanie w przypadku błędu wysłania
            self.pending_requests.lock().unwrap().remove(&request_id);
            return Err("Failed to send message to VS Code".to_string());
        }

        // Zaktualizuj timestamp aktywności dla tego workspace
        self.update_activity_for_file(&file_path);

        // 4. Czekaj na odpowiedź (z timeoutem)
        match tokio::time::timeout(std::time::Duration::from_secs(5), resp_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("Response channel closed unexpectedly".to_string()),
            Err(_) => {
                self.pending_requests.lock().unwrap().remove(&request_id);
                Err("Timeout waiting for VS Code response".to_string())
            }
        }
    }

    /// Wywoływane, gdy przyjdzie odpowiedź od VS Code przez WebSocket
    pub fn handle_response(&self, response: EditorEditResponse) {
        let mut pending = self.pending_requests.lock().unwrap();
        if let Some(tx) = pending.remove(&response.id) {
            let result = if response.success {
                Ok(())
            } else {
                Err(response.error.unwrap_or_else(|| "Unknown editor error".to_string()))
            };
            let _ = tx.send(result);
        }
    }

    /// Wysyła powiadomienie o zmianach do VS Code (Flash Effect)
    /// Jest to operacja "fire-and-forget" - nie czekamy na odpowiedź.
    pub fn notify_changes(&self, files: Vec<FileChangeNotification>) {
        let conns = self.connections.lock().unwrap();
        if conns.is_empty() {
            println!("[EditorBridge] Cannot notify changes - VS Code NOT connected.");
            return;
        }

        // Group files by connection index to send batches to correct windows
        let mut files_per_conn: HashMap<usize, Vec<FileChangeNotification>> = HashMap::new();

        for file in files {
            let normalized_target = Self::normalize_path_string(&file.path);
            let mut best_conn_idx: Option<usize> = None;
            let mut max_len = 0;

            // Znajdź najlepsze okno dla tego pliku
            for (idx, conn) in conns.iter().enumerate() {
                for root in &conn.roots {
                    let normalized_root = Self::normalize_path_string(root);
                    let root_prefix = if normalized_root.ends_with('/') { 
                        normalized_root.clone() 
                    } else { 
                        format!("{}/", normalized_root) 
                    };

                    if normalized_target == normalized_root || normalized_target.starts_with(&root_prefix) {
                        if normalized_root.len() > max_len {
                            max_len = normalized_root.len();
                            best_conn_idx = Some(idx);
                        }
                    }
                }
            }

            if let Some(idx) = best_conn_idx {
                files_per_conn.entry(idx).or_default().push(file.clone());
            } else {
                // Fallback: wyślij do ostatniego
                files_per_conn.entry(conns.len() - 1).or_default().push(file);
            }
        }

        // Collect files to update activity after sending
        let mut files_to_update = Vec::new();

        // Send grouped messages
        for (conn_idx, file_batch) in files_per_conn {
            if let Some(conn) = conns.get(conn_idx) {
                let msg = EditorMessage::ShowChanges { files: file_batch.clone() };
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = conn.sender.send(json);

                    // Collect first file from batch to update activity
                    if let Some(first_file) = file_batch.first() {
                        files_to_update.push(first_file.path.clone());
                    }
                }
            }
        }

        drop(conns); // Release lock

        // Update activity timestamps for all workspaces that received notifications
        for file_path in files_to_update {
            self.update_activity_for_file(&file_path);
        }
    }

    /// Sends undo request to VS Code for specific change
    pub fn request_undo(&self, change_id: String, file_path: &str) {
        let payload = serde_json::json!({
            "type": "undo_change",
            "changeId": change_id
        });

        if let Some(sender) = self.get_sender_for_file(file_path) {
            let _ = sender.send(payload.to_string());
            println!("[EditorBridge] Sent undo request for change: {}", change_id);
        } else {
            println!("[EditorBridge] Cannot send undo request - no VS Code window found");
        }
    }

    /// Sends redo request to VS Code for specific change
    pub fn request_redo(&self, change_id: String, file_path: &str) {
        let payload = serde_json::json!({
            "type": "redo_change",
            "changeId": change_id
        });

        if let Some(sender) = self.get_sender_for_file(file_path) {
            let _ = sender.send(payload.to_string());
            println!("[EditorBridge] Sent redo request for change: {}", change_id);
        } else {
            println!("[EditorBridge] Cannot send redo request - no VS Code window found");
        }
    }
}