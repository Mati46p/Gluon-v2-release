use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use uuid::Uuid;
use chrono;

/// Typ agenta
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentType {
    /// Zwykły agent (przetwarzanie 1-1)
    Normal,
    /// Agregator raportów (czeka na wszystkie dzieci)
    Report,
    /// Auto-Apply executor (NIE jest modelem AI, wykonuje zmiany w kodzie automatycznie)
    AutoApply,
    /// Terminal listener (NIE jest modelem AI, zbiera output z terminala VS Code)
    Terminal,
}

/// Wiadomość w historii agenta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role: "system", "user", "assistant"
    pub role: String,
    /// Treść wiadomości
    pub content: String,
    /// Timestamp utworzenia
    pub timestamp: i64,
}

/// Pojedynczy agent w grafie workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unikalny identyfikator agenta
    pub id: String,
    /// Nazwa/rola agenta (np. "Architekt", "Koder")
    pub name: String,
    /// Kod do parowania (np. "#ARCH")
    pub pairing_code: String,
    /// Status połączenia
    pub status: AgentStatus,
    /// ID sesji WebSocket (gdy połączony)
    pub socket_id: Option<String>,
    /// Opcjonalny wrapper dla wiadomości wychodzących
    pub output_wrapper: Option<String>,
    /// Typ agenta (Normal lub Report)
    #[serde(default = "default_agent_type")]
    pub agent_type: AgentType,
    /// Pozycja w grafie wizualnym (x, y) - dla UI
    #[serde(default)]
    pub position: Option<(f32, f32)>,

    // === SSOT Workflow Engine Fields ===
    /// Szablon system promptu (bez wstrzykniętej struktury JSON)
    #[serde(default)]
    pub system_prompt_template: Option<String>,
    /// Bieżące zadanie (gdy agent pracuje)
    #[serde(default)]
    pub current_task: Option<String>,
    /// Pełna historia konwersacji
    #[serde(default)]
    pub history: Vec<Message>,
    /// Ostatni sparsowany output JSON (dla routingu)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_output_json: Option<serde_json::Value>,
}

fn default_agent_type() -> AgentType {
    AgentType::Normal
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    /// Bezczynny - gotowy do pracy
    Idle,
    /// Pracuje (generowanie lub akcja w toku)
    Working,
    /// Zadanie zakończone sukcesem
    Success,
    /// Zadanie zakończone błędem
    Failed,
    /// Oczekuje na połączenie z oknem przeglądarki (dla Live Mode)
    PendingConnection,
    /// Oczekuje na decyzję użytkownika (Approve node)
    WaitingForUser,
    /// Legacy: Oczekuje na połączenie (będzie zastąpione przez PendingConnection)
    #[serde(alias = "Waiting")]
    Waiting,
    /// Legacy: Połączony i gotowy (będzie zastąpione przez Idle)
    #[serde(alias = "Connected")]
    Connected,
    /// Rozłączony (sesja zakończona)
    Disconnected,
}

/// Połączenie między agentami (krawędź grafu)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// ID agenta źródłowego
    pub from_agent_id: String,
    /// ID agenta docelowego
    pub to_agent_id: String,
    /// Opcjonalny template do transformacji wiadomości
    pub message_template: Option<String>,
}

/// Przechowuje wiadomości dla Report node'a
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportBuffer {
    /// Agent ID Report node'a
    pub report_agent_id: String,
    /// Zgromadzone odpowiedzi: (source_agent_id, source_agent_name, content)
    pub collected_responses: Vec<(String, String, String)>,
    /// Liczba oczekiwanych odpowiedzi (dzieci tego Report node'a)
    pub expected_count: usize,
}

/// Stan całego workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowGraph {
    /// Mapa agentów (id -> Agent)
    pub agents: HashMap<String, Agent>,
    /// Lista połączeń
    pub connections: Vec<Connection>,
    /// Czy automatyczne przekazywanie jest włączone
    pub auto_forward: bool,
    /// Bufory dla Report nodes (nie serializujemy tego - runtime state)
    #[serde(skip)]
    pub report_buffers: HashMap<String, ReportBuffer>,
}

impl WorkflowGraph {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            connections: Vec::new(),
            auto_forward: false,
            report_buffers: HashMap::new(),
        }
    }

    /// Generuje unikalny kod parowania
    fn generate_pairing_code() -> String {
        format!("#{}", Uuid::new_v4().to_string()[..6].to_uppercase())
    }

    /// Dodaje nowego agenta do grafu
    pub fn add_agent(&mut self, name: String, output_wrapper: Option<String>, agent_type: AgentType, position: Option<(f32, f32)>) -> Agent {
        let id = Uuid::new_v4().to_string();
        let pairing_code = Self::generate_pairing_code();

        let agent = Agent {
            id: id.clone(),
            name,
            pairing_code,
            status: AgentStatus::PendingConnection, // Nowy default zgodny z SSOT spec
            socket_id: None,
            output_wrapper,
            agent_type,
            position,
            system_prompt_template: None,
            current_task: None,
            history: Vec::new(),
            last_output_json: None,
        };

        self.agents.insert(id.clone(), agent.clone());
        agent
    }

    /// Usuwa agenta i wszystkie jego połączenia
    pub fn remove_agent(&mut self, agent_id: &str) {
        self.agents.remove(agent_id);
        self.connections.retain(|c| c.from_agent_id != agent_id && c.to_agent_id != agent_id);
    }

    /// Aktualizuje istniejącego agenta
    pub fn update_agent(
        &mut self,
        agent_id: &str,
        name: Option<String>,
        output_wrapper: Option<Option<String>>,
        system_prompt: Option<String>,
    ) -> Result<Agent, String> {
        let agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

        if let Some(new_name) = name {
            agent.name = new_name;
        }

        if let Some(new_wrapper) = output_wrapper {
            agent.output_wrapper = new_wrapper;
        }

        if let Some(new_prompt) = system_prompt {
            agent.system_prompt_template = Some(new_prompt);
        }

        Ok(agent.clone())
    }

    /// Dodaje połączenie między agentami
    pub fn add_connection(&mut self, from_id: String, to_id: String, template: Option<String>) -> Result<(), String> {
        if !self.agents.contains_key(&from_id) {
            return Err(format!("Source agent {} not found", from_id));
        }
        if !self.agents.contains_key(&to_id) {
            return Err(format!("Target agent {} not found", to_id));
        }

        // Sprawdź czy połączenie już nie istnieje
        if self.connections.iter().any(|c| c.from_agent_id == from_id && c.to_agent_id == to_id) {
            return Err("Connection already exists".to_string());
        }

        self.connections.push(Connection {
            from_agent_id: from_id,
            to_agent_id: to_id,
            message_template: template,
        });

        Ok(())
    }

    /// Usuwa połączenie
    pub fn remove_connection(&mut self, from_id: &str, to_id: &str) {
        self.connections.retain(|c| !(c.from_agent_id == from_id && c.to_agent_id == to_id));
    }

    /// Rejestruje agent z socket_id (handshake)
    pub fn register_agent(&mut self, pairing_code: &str, socket_id: String) -> Result<Agent, String> {
        let agent = self.agents.values_mut()
            .find(|a| a.pairing_code == pairing_code)
            .ok_or_else(|| format!("No agent found with pairing code: {}", pairing_code))?;

        agent.socket_id = Some(socket_id);
        agent.status = AgentStatus::Idle; // Connected -> Idle (SSOT update)

        Ok(agent.clone())
    }

    /// Odłącza agenta (WebSocket closed)
    pub fn disconnect_agent(&mut self, socket_id: &str) {
        if let Some(agent) = self.agents.values_mut().find(|a| a.socket_id.as_deref() == Some(socket_id)) {
            agent.status = AgentStatus::Disconnected;
            agent.socket_id = None;
        }
    }

    /// Znajduje agenta po socket_id
    pub fn find_agent_by_socket(&self, socket_id: &str) -> Option<&Agent> {
        self.agents.values().find(|a| a.socket_id.as_deref() == Some(socket_id))
    }

    /// Znajduje docelowych agentów dla wiadomości (routing)
    pub fn get_targets(&self, source_agent_id: &str) -> Vec<&Agent> {
        println!("[Workflow Routing] Finding targets for source: {}", source_agent_id);
        
        self.connections
            .iter()
            .filter(|c| c.from_agent_id == source_agent_id)
            .filter_map(|c| {
                let agent = self.agents.get(&c.to_agent_id)?;
                
                // Heurystyka naprawcza: Jeśli nazwa zawiera "Raport", "Report" lub "Agregator", traktuj jako Report Node
                // nawet jeśli w bazie ma stary typ Normal.
                let name_lower = agent.name.to_lowercase();
                let is_implicit_report = name_lower.contains("raport") || 
                                       name_lower.contains("report") || 
                                       name_lower.contains("agregator") ||
                                       name_lower.contains("kolektor");

                // [FIX] Allow Virtual Nodes (Report, AutoApply, Terminal) to be targets even if not connected via WS
                // Also allow agents in Idle, Working, Success states (active agents)
                let is_valid = matches!(agent.status, AgentStatus::Idle | AgentStatus::Working | AgentStatus::Success | AgentStatus::Connected) ||
                               agent.agent_type == AgentType::Report ||
                               agent.agent_type == AgentType::AutoApply ||
                               agent.agent_type == AgentType::Terminal ||
                               is_implicit_report;

                if is_valid {
                    Some(agent)
                } else {
                    println!("[Workflow Routing] Skipped target '{}' (Status: {:?}, Type: {:?}) - Not connected and not identified as Virtual Node", 
                        agent.name, agent.status, agent.agent_type);
                    None
                }
            })
            .collect()
    }

    /// Pobiera połączenie (do odczytania templatu)
    pub fn get_connection(&self, from_id: &str, to_id: &str) -> Option<&Connection> {
        self.connections.iter().find(|c| c.from_agent_id == from_id && c.to_agent_id == to_id)
    }

    /// Liczy ile agentów wysyła do danego agenta (ile dzieci ma Report node)
    pub fn count_incoming_connections(&self, agent_id: &str) -> usize {
        self.connections.iter().filter(|c| c.to_agent_id == agent_id).count()
    }

    /// Dodaje wiadomość do bufora Report node'a
    /// Zwraca Some(aggregated_message) jeśli wszystkie dzieci już odpowiedziały
    pub fn add_to_report_buffer(
        &mut self,
        report_agent_id: &str,
        source_agent_id: &str,
        content: String,
    ) -> Option<String> {
        println!("\n========= [AGGREGATOR DIAGNOSTICS START] =========");

        // 1. Sprawdź czy target istnieje i jest Raportem
        let agent = match self.agents.get(report_agent_id) {
            Some(a) => a,
            None => {
                println!("❌ ERROR: Report Agent ID {} not found in agents map!", report_agent_id);
                return None;
            }
        };

        println!("1. Target Agent: '{}' (Type: {:?})", agent.name, agent.agent_type);

        if agent.agent_type != AgentType::Report {
            println!("❌ ERROR: Target is NOT a Report node. Aborting buffering.");
            return None;
        }

        // 2. Analiza połączeń (Kluczowy moment)
        println!("2. Analyzing ALL connections in graph:");
        let mut expected_sources_ids = Vec::new();
        let mut expected_sources_names = Vec::new();

        for (idx, conn) in self.connections.iter().enumerate() {
            let is_incoming = conn.to_agent_id == report_agent_id;
            let is_outgoing = conn.from_agent_id == report_agent_id;

            let direction_mark = if is_incoming { ">>> [INCOMING]" } else if is_outgoing { "<<< [OUTGOING]" } else { "    [OTHER]" };

            if is_incoming || is_outgoing {
                let from_name = self.agents.get(&conn.from_agent_id).map(|a| a.name.as_str()).unwrap_or("?");
                let to_name = self.agents.get(&conn.to_agent_id).map(|a| a.name.as_str()).unwrap_or("?");
                println!("   Conn #{}: {} {} -> {}", idx, direction_mark, from_name, to_name);
            }

            if is_incoming {
                expected_sources_ids.push(conn.from_agent_id.clone());
                if let Some(src_agent) = self.agents.get(&conn.from_agent_id) {
                    expected_sources_names.push(src_agent.name.clone());
                } else {
                    expected_sources_names.push(format!("UNKNOWN_ID({})", conn.from_agent_id));
                }
            }
        }

        let expected_count = expected_sources_ids.len();
        println!("3. Calculated Expectation: {} inputs from: {:?}", expected_count, expected_sources_names);

        // 3. Pobranie/Inicjalizacja Bufora
        let buffer = self.report_buffers.entry(report_agent_id.to_string()).or_insert_with(|| {
            println!("   (Creating new buffer for this agent)");
            ReportBuffer {
                report_agent_id: report_agent_id.to_string(),
                collected_responses: Vec::new(),
                expected_count,
            }
        });

        // Zawsze aktualizuj expected_count na wypadek zmian w grafie
        buffer.expected_count = expected_count;

        // 4. Przetwarzanie przychodzącej wiadomości
        let source_name = self.agents.get(source_agent_id)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        println!("4. Processing Message from: '{}'", source_name);

        // Sprawdź duplikaty
        if !buffer.collected_responses.iter().any(|(id, _, _)| id == source_agent_id) {
            buffer.collected_responses.push((
                source_agent_id.to_string(),
                source_name.clone(),
                content,
            ));
            println!("   ✅ Added to buffer.");
        } else {
            println!("   ⚠️ Duplicate message from this source ignored.");
        }

        // 5. Decyzja
        println!("5. Buffer Status: {} / {} collected.", buffer.collected_responses.len(), buffer.expected_count);

        let received_names: Vec<String> = buffer.collected_responses.iter().map(|(_, n, _)| n.clone()).collect();
        println!("   Received so far: {:?}", received_names);

        let result = if buffer.collected_responses.len() >= buffer.expected_count {
            println!("   🚀 THRESHOLD REACHED! Aggregating results...");
            let responses_to_aggregate = buffer.collected_responses.clone();
            self.report_buffers.remove(report_agent_id);
            Some(self.aggregate_report_messages(&responses_to_aggregate))
        } else {
            println!("   ⏳ WAITING for more inputs...");
            None
        };

        println!("========= [AGGREGATOR DIAGNOSTICS END] =========\n");
        result
    }

    /// Agreguje wiadomości od dzieci w jeden raport (konkatenacja)
    fn aggregate_report_messages(&self, responses: &[(String, String, String)]) -> String {
        let mut result = String::from("=== AGGREGATED REPORT ===\n\n");

        for (idx, (_id, name, content)) in responses.iter().enumerate() {
            result.push_str(&format!("--- Agent: {} ---\n", name));
            result.push_str(content);
            if idx < responses.len() - 1 {
                result.push_str("\n\n");
            }
        }

        result.push_str("\n\n=== END OF REPORT ===");
        result
    }

    /// Resetuje bufor dla Report node'a (używane gdy workflow się resetuje)
    pub fn reset_report_buffer(&mut self, report_agent_id: &str) {
        self.report_buffers.remove(report_agent_id);
    }

    /// Resetuje wszystkie bufory Report
    pub fn reset_all_report_buffers(&mut self) {
        self.report_buffers.clear();
    }

    /// Ustawia zadanie dla agenta i zmienia status na Working
    pub fn set_agent_task(&mut self, agent_id: &str, task: String) -> Result<(), String> {
        let agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

        agent.current_task = Some(task);
        agent.status = AgentStatus::Working;

        Ok(())
    }

    /// Oznacza zadanie agenta jako zakończone z sukcesem
    pub fn mark_agent_success(&mut self, agent_id: &str) -> Result<(), String> {
        let agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

        agent.current_task = None;
        agent.status = AgentStatus::Success;

        Ok(())
    }

    /// Oznacza zadanie agenta jako nieudane
    pub fn mark_agent_failed(&mut self, agent_id: &str) -> Result<(), String> {
        let agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

        agent.current_task = None;
        agent.status = AgentStatus::Failed;

        Ok(())
    }

    /// Resetuje agenta do stanu Idle (gotowy do kolejnego zadania)
    pub fn reset_agent_to_idle(&mut self, agent_id: &str) -> Result<(), String> {
        let agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

        agent.current_task = None;
        agent.status = AgentStatus::Idle;

        Ok(())
    }

    /// Dodaje wiadomość do historii agenta
    pub fn add_message_to_history(&mut self, agent_id: &str, role: String, content: String) -> Result<(), String> {
        let agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

        let message = Message {
            role,
            content,
            timestamp: chrono::Utc::now().timestamp(),
        };

        agent.history.push(message);

        Ok(())
    }

    /// Czyści historię agenta
    pub fn clear_agent_history(&mut self, agent_id: &str) -> Result<(), String> {
        let agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

        agent.history.clear();

        Ok(())
    }

    /// Saves workflow graph to JSON file
    pub fn save(&self, storage_path: &PathBuf) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize workflow: {}", e))?;

        std::fs::write(storage_path, json)
            .map_err(|e| format!("Failed to write workflow file: {}", e))?;

        Ok(())
    }

    /// Loads workflow graph from JSON file
    pub fn load(storage_path: &PathBuf) -> Result<Self, String> {
        if !storage_path.exists() {
            return Ok(WorkflowGraph::new());
        }

        let content = std::fs::read_to_string(storage_path)
            .map_err(|e| format!("Failed to read workflow file: {}", e))?;

        let mut graph: WorkflowGraph = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse workflow file: {}", e))?;

        // Reset all agents to "PendingConnection" status on load (they're not connected yet)
        for agent in graph.agents.values_mut() {
            agent.status = AgentStatus::PendingConnection;
            agent.socket_id = None;
        }

        Ok(graph)
    }
}

/// Manager workflow - thread-safe singleton
pub struct AgentWorkflowManager {
    graph: Arc<Mutex<WorkflowGraph>>,
}

impl AgentWorkflowManager {
    pub fn new() -> Self {
        Self {
            graph: Arc::new(Mutex::new(WorkflowGraph::new())),
        }
    }

    pub fn get_graph(&self) -> Arc<Mutex<WorkflowGraph>> {
        self.graph.clone()
    }

    /// Loads workflow from storage (call this after app initialization)
    pub fn load_from_storage(&self, storage_path: &PathBuf) -> Result<(), String> {
        let loaded_graph = WorkflowGraph::load(storage_path)?;
        let mut graph = self.graph.lock().unwrap();
        *graph = loaded_graph;
        Ok(())
    }

    /// Saves workflow to storage
    pub fn save_to_storage(&self, storage_path: &PathBuf) -> Result<(), String> {
        let graph = self.graph.lock().unwrap();
        graph.save(storage_path)
    }

    /// Przetwarza wiadomość od agenta i zwraca routing targets
    pub fn route_message(&self, agent_id: &str, content: &str) -> Result<Vec<MessageTarget>, String> {
        let graph = self.graph.lock().unwrap();

        // Znajdź agenta źródłowego po ID
        let source_agent = graph.agents.get(agent_id)
            .ok_or_else(|| format!("Source agent not found: {}", agent_id))?;

        // Znajdź docelowych agentów
        let targets = graph.get_targets(&source_agent.id);

        if targets.is_empty() {
            return Err("No connected targets found".to_string());
        }

        // Buduj listę MessageTarget
        let mut result = Vec::new();
        for target in targets {
            let connection = graph.get_connection(&source_agent.id, &target.id);

            // Formatuj wiadomość
            let formatted_content = self.format_message(
                content,
                &source_agent,
                connection,
            );

            // Zastosuj tę samą heurystykę co w get_targets, aby main.rs wiedział jak obsłużyć
            let name_lower = target.name.to_lowercase();
            let effective_type = if target.agent_type == AgentType::Report || 
                                  name_lower.contains("raport") || 
                                  name_lower.contains("report") || 
                                  name_lower.contains("agregator") {
                AgentType::Report
            } else {
                target.agent_type.clone()
            };

            result.push(MessageTarget {
                agent_id: target.id.clone(),
                agent_name: target.name.clone(),
                content: formatted_content,
                agent_type: effective_type, 
            });
        }

        Ok(result)
    }

    /// Formatuje wiadomość z uwzględnieniem wrapperów i template'ów
    fn format_message(&self, content: &str, source: &Agent, connection: Option<&Connection>) -> String {
        let mut result = content.to_string();

        // 1. Aplikuj output_wrapper agenta źródłowego (jeśli istnieje)
        if let Some(wrapper) = &source.output_wrapper {
            result = wrapper.replace("{content}", &result);
        }

        // 2. Aplikuj message_template z połączenia (jeśli istnieje)
        if let Some(conn) = connection {
            if let Some(template) = &conn.message_template {
                result = template.replace("{content}", &result);
            }
        }

        result
    }
}

/// Cel przekazania wiadomości
#[derive(Debug, Clone, Serialize)]
pub struct MessageTarget {
    pub agent_id: String,
    pub agent_name: String,
    pub content: String,
    pub agent_type: AgentType, // ZMIANA: Dodano typ agenta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_creation() {
        let mut graph = WorkflowGraph::new();

        // Dodaj dwóch agentów
        let agent_a = graph.add_agent("Architekt".to_string(), None, AgentType::Normal, None);
        let agent_b = graph.add_agent("Koder".to_string(), None, AgentType::Normal, None);

        assert_eq!(graph.agents.len(), 2);
        assert_eq!(agent_a.status, AgentStatus::PendingConnection);

        // Dodaj połączenie
        graph.add_connection(agent_a.id.clone(), agent_b.id.clone(), None).unwrap();
        assert_eq!(graph.connections.len(), 1);
    }

    #[test]
    fn test_agent_registration() {
        let mut graph = WorkflowGraph::new();
        let agent = graph.add_agent("Tester".to_string(), None, AgentType::Normal, None);
        let code = agent.pairing_code.clone();

        // Zarejestruj agenta
        let registered = graph.register_agent(&code, "socket123".to_string()).unwrap();
        assert_eq!(registered.status, AgentStatus::Idle);
        assert_eq!(registered.socket_id, Some("socket123".to_string()));
    }

    #[test]
    fn test_message_routing() {
        let mut graph = WorkflowGraph::new();

        let agent_a = graph.add_agent("A".to_string(), None, AgentType::Normal, None);
        let agent_b = graph.add_agent("B".to_string(), None, AgentType::Normal, None);

        // Połącz A -> B
        graph.add_connection(agent_a.id.clone(), agent_b.id.clone(), None).unwrap();

        // Zarejestruj oba
        graph.register_agent(&agent_a.pairing_code, "sock_a".to_string()).unwrap();
        graph.register_agent(&agent_b.pairing_code, "sock_b".to_string()).unwrap();

        // Routing test
        let targets = graph.get_targets(&agent_a.id);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name, "B");
    }

    #[test]
    fn test_output_wrapper() {
        let manager = AgentWorkflowManager::new();
        let mut graph = manager.graph.lock().unwrap();

        // Agent z wrapperem
        let agent_a = graph.add_agent("A".to_string(), Some("[SYSTEM]: {content}".to_string()), AgentType::Normal, None);
        let agent_b = graph.add_agent("B".to_string(), None, AgentType::Normal, None);
        let agent_a_id = agent_a.id.clone();

        graph.add_connection(agent_a.id.clone(), agent_b.id.clone(), None).unwrap();
        graph.register_agent(&agent_a.pairing_code, "sock_a".to_string()).unwrap();
        graph.register_agent(&agent_b.pairing_code, "sock_b".to_string()).unwrap();

        drop(graph); // Unlock

        // Route message using agent_id
        let targets = manager.route_message(&agent_a_id, "Hello").unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].content, "[SYSTEM]: Hello");
    }

    #[test]
    fn test_report_aggregation() {
        let mut graph = WorkflowGraph::new();

        // Stwórz 2 agentów normalnych i 1 Report node
        let agent_a = graph.add_agent("Agent A".to_string(), None, AgentType::Normal, None);
        let agent_b = graph.add_agent("Agent B".to_string(), None, AgentType::Normal, None);
        let report = graph.add_agent("Report".to_string(), None, AgentType::Report, None);

        // Połącz A -> Report, B -> Report
        graph.add_connection(agent_a.id.clone(), report.id.clone(), None).unwrap();
        graph.add_connection(agent_b.id.clone(), report.id.clone(), None).unwrap();

        // Dodaj pierwszą wiadomość od A
        let result1 = graph.add_to_report_buffer(&report.id, &agent_a.id, "Response from A".to_string());
        assert!(result1.is_none()); // Jeszcze nie wszystkie dzieci

        // Dodaj drugą wiadomość od B
        let result2 = graph.add_to_report_buffer(&report.id, &agent_b.id, "Response from B".to_string());
        assert!(result2.is_some()); // Teraz mamy wszystkie!

        let aggregated = result2.unwrap();
        assert!(aggregated.contains("Agent A"));
        assert!(aggregated.contains("Agent B"));
        assert!(aggregated.contains("Response from A"));
        assert!(aggregated.contains("Response from B"));
    }
}