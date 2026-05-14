//! System Health Simulation Module
//!
//! Ten moduł służy jako plik testowy dla systemu Gluon.
//! Jego celem jest dostarczenie pliku o długości ponad 200 linii kodu
//! w celu weryfikacji działania:
//! 1. Przewijania w edytorze (Virtual Scrolling)
//! 2. Parsowania dużych bloków diff
//! 3. Testowania mechanizmu Apply System na większych plikach.
//!
//! Symuluje on monitorowanie zasobów systemowych (CPU, RAM, Dysk).
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt;
// ============================================================================
// Constants and Configuration
// ============================================================================
const MAX_HISTORY_SIZE: usize = 1000;
const WARNING_THRESHOLD_CPU: f32 = 80.0;
const CRITICAL_THRESHOLD_CPU: f32 = 95.0;
const WARNING_THRESHOLD_RAM: f32 = 85.0;
const REFRESH_RATE_MS: u64 = 1000;
/// Definicja statusu komponentu systemu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentStatus {
Healthy,
Warning,
Critical,
Offline,
Maintenance,
}
impl fmt::Display for ComponentStatus {
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
let s = match self {
ComponentStatus::Healthy => "HEALTHY",
ComponentStatus::Warning => "WARNING",
ComponentStatus::Critical => "CRITICAL",
ComponentStatus::Offline => "OFFLINE",
ComponentStatus::Maintenance => "MAINTENANCE",
};
write!(f, "{}", s)
}
}
// ============================================================================
// Data Structures
// ============================================================================
/// Reprezentuje pojedynczy punkt danych w czasie
#[derive(Debug, Clone)]
pub struct MetricPoint {
pub timestamp: u64,
pub value: f32,
pub label: String,
}
/// Statystyki użycia procesora
#[derive(Debug, Clone)]
pub struct CpuStats {
pub core_count: usize,
pub usage_per_core: Vec<f32>,
pub average_usage: f32,
pub temperature: f32,
pub process_count: usize,
}
/// Statystyki użycia pamięci
#[derive(Debug, Clone)]
pub struct MemoryStats {
pub total_bytes: u64,
pub used_bytes: u64,
pub free_bytes: u64,
pub swap_total: u64,
pub swap_used: u64,
}
/// Główna struktura monitora
pub struct SystemMonitor {
pub system_name: String,
pub uptime_seconds: u64,
pub cpu: CpuStats,
pub memory: MemoryStats,
pub disks: HashMap<String, u64>, // Mount point -> Free space
pub history: Vec<MetricPoint>,
pub status: ComponentStatus,
}
// ============================================================================
// Implementations
// ============================================================================
impl Default for CpuStats {
fn default() -> Self {
Self {
core_count: 4,
usage_per_core: vec![0.0; 4],
average_usage: 0.0,
temperature: 40.0,
process_count: 0,
}
}
}
impl SystemMonitor {
/// Tworzy nową instancję monitora z domyślnymi wartościami
pub fn new(name: &str) -> Self {
Self {
system_name: name.to_string(),
uptime_seconds: 0,
cpu: CpuStats::default(),
memory: MemoryStats {
total_bytes: 16 * 1024 * 1024 * 1024, // 16 GB
used_bytes: 0,
free_bytes: 16 * 1024 * 1024 * 1024,
swap_total: 4 * 1024 * 1024 * 1024,
swap_used: 0,
},
disks: HashMap::new(),
history: Vec::with_capacity(MAX_HISTORY_SIZE),
status: ComponentStatus::Healthy,
}
}
 code 
/// Symuluje aktualizację stanu systemu
/// W rzeczywistej aplikacji tutaj byłyby wywołania systemowe (np. sysinfo)
pub fn update(&mut self) {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    self.uptime_seconds += 1;
    // Symulacja zmian CPU (pseudolosowe zmiany)
    // Używamy prostego algorytmu opartego na czasie, aby uniknąć zewnętrznych crates
    let noise = (current_time % 100) as f32 / 100.0; 
    self.cpu.average_usage = (self.cpu.average_usage + noise * 10.0) % 100.0;
    for usage in &mut self.cpu.usage_per_core {
        *usage = (*usage + noise * 5.0) % 100.0;
    }
    // Symulacja temperatury w zależności od obciążenia
    self.cpu.temperature = 35.0 + (self.cpu.average_usage * 0.5);
    // Aktualizacja pamięci (symulacja wycieku pamięci dla testu)
    let mem_leak = 1024 * 1024; // 1 MB per tick
    if self.memory.used_bytes + mem_leak < self.memory.total_bytes {
        self.memory.used_bytes += mem_leak;
        self.memory.free_bytes = self.memory.total_bytes - self.memory.used_bytes;
    } else {
        // Reset simulation if full
        self.memory.used_bytes = self.memory.total_bytes / 4;
    }
    // Zapisz historię
    self.log_metric("cpu_avg", self.cpu.average_usage);
    // Przelicz status systemu
    self.recalculate_status();
}
/// Dodaje punkt metryczny do historii
fn log_metric(&mut self, label: &str, value: f32) {
    if self.history.len() >= MAX_HISTORY_SIZE {
        self.history.remove(0);
    }
    self.history.push(MetricPoint {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        value,
        label: label.to_string(),
    });
}
/// Określa ogólny stan zdrowia systemu na podstawie metryk
fn recalculate_status(&mut self) {
    if self.cpu.average_usage > CRITICAL_THRESHOLD_CPU {
        self.status = ComponentStatus::Critical;
        return;
    }
    let ram_usage_percent = (self.memory.used_bytes as f32 / self.memory.total_bytes as f32) * 100.0;
    if ram_usage_percent > WARNING_THRESHOLD_RAM || self.cpu.average_usage > WARNING_THRESHOLD_CPU {
        self.status = ComponentStatus::Warning;
        return;
    }
    self.status = ComponentStatus::Healthy;
}
/// Generuje raport tekstowy o stanie systemu
pub fn generate_report(&self) -> String {
    let mut report = String::new();
    report.push_str("======================================\n");
    report.push_str(&format!(" SYSTEM REPORT: {}\n", self.system_name));
    report.push_str("======================================\n");
    report.push_str(&format!(" Status: {}\n", self.status));
    report.push_str(&format!(" Uptime: {}s\n", self.uptime_seconds));
    report.push_str("--------------------------------------\n");
    report.push_str(" CPU:\n");
    report.push_str(&format!("   Average: {:.2}%\n", self.cpu.average_usage));
    report.push_str(&format!("   Temp:    {:.1}°C\n", self.cpu.temperature));
    report.push_str("   Cores:\n");
    for (i, core) in self.cpu.usage_per_core.iter().enumerate() {
        report.push_str(&format!("     Core {}: {:.1}%\n", i, core));
    }
    report.push_str("--------------------------------------\n");
    report.push_str(" MEMORY:\n");
    report.push_str(&format!("   Total: {}\n", format_bytes(self.memory.total_bytes)));
    report.push_str(&format!("   Used:  {}\n", format_bytes(self.memory.used_bytes)));
    report.push_str(&format!("   Free:  {}\n", format_bytes(self.memory.free_bytes)));
    report.push_str("======================================\n");
    report
}
}
// ============================================================================
// Utilities
// ============================================================================
/// Formatuje bajty do czytelnej postaci (KB, MB, GB)
fn format_bytes(bytes: u64) -> String {
const KB: u64 = 1024;
const MB: u64 = KB * 1024;
const GB: u64 = MB * 1024;
 code 
if bytes >= GB {
    format!("{:.2} GB", bytes as f64 / GB as f64)
} else if bytes >= MB {
    format!("{:.2} MB", bytes as f64 / MB as f64)
} else if bytes >= KB {
    format!("{:.2} KB", bytes as f64 / KB as f64)
} else {
    format!("{} B", bytes)
}
}
/// Prosty logger do testowania wyjścia
pub struct MockLogger {
logs: Vec<String>,
}
impl MockLogger {
pub fn new() -> Self {
Self { logs: Vec::new() }
}
 code 
pub fn info(&mut self, msg: &str) {
    let log = format!("[INFO] {}", msg);
    println!("{}", log);
    self.logs.push(log);
}
pub fn warn(&mut self, msg: &str) {
    let log = format!("[WARN] {}", msg);
    eprintln!("{}", log);
    self.logs.push(log);
}
pub fn error(&mut self, msg: &str) {
    let log = format!("[ERROR] {}", msg);
    eprintln!("{}", log);
    self.logs.push(log);
}
pub fn dump_to_file(&self) -> String {
    self.logs.join("\n")
}
}
// ============================================================================
// Tests & Usage Simulation
// ============================================================================
#[cfg(test)]
mod tests {
use super::*;
 code 
#[test]
fn test_monitor_initialization() {
    let monitor = SystemMonitor::new("TestNode");
    assert_eq!(monitor.status, ComponentStatus::Healthy);
    assert_eq!(monitor.cpu.core_count, 4);
}
#[test]
fn test_status_update() {
    let mut monitor = SystemMonitor::new("TestNode");
    // Force critical state
    monitor.cpu.average_usage = 99.0;
    monitor.recalculate_status();
    assert_eq!(monitor.status, ComponentStatus::Critical);
    // Force warning state
    monitor.cpu.average_usage = 82.0;
    monitor.recalculate_status();
    assert_eq!(monitor.status, ComponentStatus::Warning);
    // Force healthy state
    monitor.cpu.average_usage = 10.0;
    monitor.recalculate_status();
    assert_eq!(monitor.status, ComponentStatus::Healthy);
}
#[test]
fn test_byte_formatting() {
    assert_eq!(format_bytes(1024), "1.00 KB");
    assert_eq!(format_bytes(1024 * 1024 * 2 + 512 * 1024), "2.50 MB");
}
}
/// Funkcja uruchamiająca krótką symulację (do wywołania z main.rs jeśli potrzebne)
pub fn run_simulation_demo() {
let mut monitor = SystemMonitor::new("Production-Server-01");
let mut logger = MockLogger::new();
 code 
logger.info("Starting simulation...");
for i in 0..10 {
    monitor.update();
    let report_line = format!("Tick {}: CPU {:.1}%, RAM used: {}", 
        i, 
        monitor.cpu.average_usage, 
        format_bytes(monitor.memory.used_bytes)
    );
    if monitor.status == ComponentStatus::Healthy {
        logger.info(&report_line);
    } else {
        logger.warn(&format!("{} - Status: {}", report_line, monitor.status));
    }
    // W prawdziwym kodzie byłoby tu thread::sleep
}
logger.info("Simulation finished.");
println!("{}", monitor.generate_report());
}