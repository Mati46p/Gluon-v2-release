use std::process::{Command, Child, Stdio};
use std::sync::Mutex;
use std::io::{BufRead, BufReader};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Emitter};
use tokio::time::sleep;
use sqlx::SqlitePool;

pub struct LocalAiService {
    embed_process: Mutex<Option<Child>>,
}

impl LocalAiService {
    pub fn new() -> Self {
        Self {
            embed_process: Mutex::new(None),
        }
    }

    pub fn is_running(&self) -> bool {
        let embed_lock = self.embed_process.lock().unwrap();
        embed_lock.is_some()
    }

    pub async fn start_services(&self, app_handle: &AppHandle, pool: &SqlitePool) -> Result<(), String> {
        if self.is_running() {
            let _ = app_handle.emit("ai-loading-progress", serde_json::json!({
                "stage": "already_running",
                "message": "AI services are already running",
                "progress": 100
            }));
            return Ok(());
        }

        // Emit initial progress
        let _ = app_handle.emit("ai-loading-progress", serde_json::json!({
            "stage": "initializing",
            "message": "Initializing AI services...",
            "progress": 0
        }));

        // [SAFETY] KROK 0: Brutalne czyszczenie sierot przed startem
        if cfg!(windows) {
            let _ = app_handle.emit("ai-loading-progress", serde_json::json!({
                "stage": "cleanup",
                "message": "Cleaning up previous processes...",
                "progress": 5
            }));
            println!("[Gluon AI] 🧹 Pre-flight cleanup: Killing lingering KoboldCPP processes...");
            let _ = Command::new("taskkill")
                .args(&["/F", "/IM", "koboldcpp-oldpc.exe", "/T"])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .output();

            let _ = Command::new("taskkill")
                .args(&["/F", "/IM", "koboldcpp.exe", "/T"])
                .creation_flags(0x08000000)
                .output();

            // Daj chwilę systemowi na zwolnienie portów
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        // ZMIANA: Używamy KoboldCPP (wersja dla starszych PC bez AVX2)
        let bin_name = if cfg!(windows) { "koboldcpp-oldpc.exe" } else { "koboldcpp" };

        // Helper to find a directory in multiple locations
        let find_dir = |dir_name: &str| -> Option<std::path::PathBuf> {
            let current_dir = std::env::current_dir().ok()?;
            let exe_path = std::env::current_exe().ok()?;
            let exe_dir = exe_path.parent()?;

            // List of candidate paths to check
            let candidates = vec![
                // Production: resource directory
                app_handle.path().resource_dir().ok().map(|p| p.join(dir_name)),
                // Production: next to executable
                Some(exe_dir.join(dir_name)),
                // Dev: from project root (gluon-desktop/src-tauri/bins)
                Some(current_dir.join("gluon-desktop").join("src-tauri").join(dir_name)),
                // Dev: from src-tauri directory
                Some(current_dir.join(dir_name)),
                // Dev: parent of current directory (if running from subdirectory)
                current_dir.parent().map(|p| p.join(dir_name)),
            ];

            for candidate in candidates.into_iter().flatten() {
                if candidate.exists() && candidate.is_dir() {
                    println!("[Gluon AI] Found '{}' at: {:?}", dir_name, candidate);
                    return Some(candidate);
                }
            }
            None
        };

        // Find bins and models directories
        let _ = app_handle.emit("ai-loading-progress", serde_json::json!({
            "stage": "finding_files",
            "message": "Locating AI models and binaries...",
            "progress": 10
        }));
        let bins_dir = find_dir("bins")
            .ok_or_else(|| "Could not locate 'bins' directory. Please ensure bins/ exists with llama-server executable.".to_string())?;
        let models_dir = find_dir("models")
            .ok_or_else(|| "Could not locate 'models' directory. Please ensure models/ exists with required GGUF files.".to_string())?;

        let bin_path = bins_dir.join(bin_name);

        // Read selected embedding model from database settings (default: nomic-embed-text-v2-moe.Q8_0.gguf)
        let selected_model: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'embedding_model'")
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Failed to read embedding_model setting: {}", e))?
            .unwrap_or_else(|| "nomic-embed-text-v2-moe.Q8_0.gguf".to_string());

        let model_embed = models_dir.join(&selected_model);

        println!("[Gluon RAG] Resolving paths:");
        println!("  Bin: {:?}", bin_path);
        println!("  Selected Embed Model: {:?} ({})", model_embed, selected_model);

        if !bin_path.exists() {
            return Err(format!("Llama binary not found at: {:?}", bin_path));
        }
        if !model_embed.exists() {
            return Err(format!("Embed model not found at: {:?}", model_embed));
        }

        // Helper to spawn and capture logs with GPU/CPU fallback
        let spawn_server = |port: &str, model: &std::path::PathBuf, is_embed: bool, use_gpu: bool| -> Result<Child, String> {
            // Konfiguracja argumentów dla KoboldCPP
            // Używamy formatu Vec<String> bo KoboldCPP ma specyficzną składnię flag
            let mut args = vec![
                "--model".to_string(), 
                model.to_str().unwrap().to_string(),
                "--port".to_string(), 
                port.to_string(),
                "--host".to_string(), // Bezpieczeństwo - tylko localhost
                "127.0.0.1".to_string(),
                "--skiplauncher".to_string(), // Pomiń okno GUI, domyślnie nie otwiera przeglądarki
            ];

            if use_gpu {
                // R9 390 (Hawaii) - Vulkan (14 T/s) vs OpenCL (4 T/s)
                println!("[Gluon AI] Attempting GPU acceleration (Vulkan) on port {}...", port);
                
                // [TUNING] Config pod R9 390 (8GB VRAM) - Benchmark OK:
                // 1. Context 4096: Bezpieczny limit dla 8GB
                // 2. --quantkv 2: Kompresja pamięci (kluczowe!)
                // 3. --usevulkan: Znacznie szybszy backend dla tej karty
                // 4. --flashattention: Przyspiesza Qwena
                let ctx_size = if port == "8082" { "4096" } else { "2048" };

                args.extend_from_slice(&[
                    "--gpulayers".to_string(), "99".to_string(),
                    "--usevulkan".to_string(), "0".to_string(),   // <--- Zmiana na Vulkan (GPU 0)
                    "--contextsize".to_string(), ctx_size.to_string(),
                    "--quantkv".to_string(), "2".to_string(),     // <--- Kompresja cache
                    "--flashattention".to_string(),               // <--- Optymalizacja
                ]);
            } else {
                println!("[Gluon AI] Using CPU-only mode on port {}...", port);
                args.extend_from_slice(&[
                    "--gpulayers".to_string(), "0".to_string(),
                    "--threads".to_string(), "4".to_string(),
                    "--noblas".to_string(), // [FIX] Wymuś czyste CPU, ignoruj OpenCL (naprawia błąd inicjalizacji Nomic)
                ]);
            }

            if is_embed {
                // Zamiast --model, używamy --embeddingsmodel dla modelu embeddings
                // Musimy usunąć wcześniej dodane --model i dodać --embeddingsmodel
                // Znajdujemy indeks --model w args i zmieniamy go
                if let Some(pos) = args.iter().position(|x| x == "--model") {
                    args[pos] = "--embeddingsmodel".to_string();
                }

                // Włącz akcelerację GPU dla embeddingów (jeśli dostępne)
                if use_gpu {
                    args.push("--embeddingsgpu".to_string());
                }

                // Dla embeddingów wyłączamy GUI i inne zbędne rzeczy
                args.push("--quiet".to_string());
            }

            println!("[Gluon AI] Spawning KoboldCPP on port {}...", port);
            println!("[Gluon AI] Command: {:?} {:?}", bin_path, args);

            let mut cmd = Command::new(&bin_path);
            
            // KoboldCPP oczekuje argumentów jako oddzielne stringi
            for arg in args {
                cmd.arg(arg);
            }

            // Capture output for debugging
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            // Na Windows ukrywamy okno konsoli Kobolda, bo logi przechwytujemy
            #[cfg(windows)]
            use std::os::windows::process::CommandExt;
            #[cfg(windows)]
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

            let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn on port {}: {}", port, e))?;

            // === IMMEDIATE HEALTH CHECK ===
            std::thread::sleep(Duration::from_millis(2000)); // Dajemy mu 2 sekundy na start
            match child.try_wait() {
                Ok(Some(status)) => {
                    let exit_code = status.code().unwrap_or(-1);
                    let err_msg = format!(
                        "KoboldCPP Server on port {} DIED IMMEDIATELY with code {}. \
                        Check logs for details.",
                        port, exit_code
                    );
                    eprintln!("[Gluon AI] ❌ {}", err_msg);
                    return Err(err_msg);
                },
                Ok(None) => {
                    println!("[Gluon AI] ✅ Process appears healthy (KoboldCPP running).");
                },
                Err(e) => eprintln!("[Gluon AI] Error checking process status: {}", e),
            }
            // ==============================

            let pid = child.id();
            println!("[Gluon AI] Process spawned with PID: {}", pid);

            // Stream logs to console
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();
            let p_label = format!("AI-{}", port);
            let p_label_err = p_label.clone();

            thread::spawn(move || {
                println!("[Gluon AI] Starting stdout reader for {}", p_label);
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(l) = line {
                        println!("[{}] {}", p_label, l);
                    } else {
                        eprintln!("[{}] Error reading stdout line", p_label);
                    }
                }
                println!("[Gluon AI] Stdout reader ended for {}", p_label);
            });

            thread::spawn(move || {
                println!("[Gluon AI] Starting stderr reader for {}", p_label_err);
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(l) = line {
                        eprintln!("[{}] {}", p_label_err, l);
                    } else {
                        eprintln!("[{}] Error reading stderr line", p_label_err);
                    }
                }
                println!("[Gluon AI] Stderr reader ended for {}", p_label_err);
            });

            Ok(child)
        };

        // 2. Embed Server (Nomic) - WYMUSZAMY GPU (true w argumencie use_gpu)
        // Skoro Qwen został wyłączony, cała karta R9 390 jest dostępna dla RAG.
        // GPU drastycznie przyspieszy obliczanie wektorów dla każdego chunka.
        let _ = app_handle.emit("ai-loading-progress", serde_json::json!({
            "stage": "loading_embed_model",
            "message": "Loading Embedding AI (Nomic) on GPU...",
            "detail": "Initializing vector embeddings (Vulkan)",
            "progress": 50
        }));
        println!("[Gluon RAG] Starting Embed Server on GPU (R9 390)...");
        let mut embed = spawn_server("8081", &model_embed, true, true)?;
        
        std::thread::sleep(Duration::from_millis(500));
        match embed.try_wait() {
            Ok(Some(status)) => {
                return Err(format!("Embed server (port 8081) failed to start. Exit status: {}", status));
            }
            Ok(None) => println!("[Gluon RAG] Embed server process is running"),
            Err(e) => eprintln!("[Gluon RAG] Error checking embed server status: {}", e),
        }
        *self.embed_process.lock().unwrap() = Some(embed);

        // Async wait helper
        async fn wait_for_health_async(port: &str, name: &str, app: &AppHandle, progress: u8) -> Result<(), String> {
            let _ = app.emit("ai-loading-progress", serde_json::json!({
                "stage": "health_check",
                "message": format!("Waiting for {} to be ready...", name),
                "detail": format!("Port {}", port),
                "progress": progress
            }));
            println!("[Gluon RAG] Waiting for {} (Port {}) to be ready...", name, port);
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
            
            let start = Instant::now();
            let mut attempt = 0;

            // Zwiększamy timeout do 120 sekund
            while start.elapsed() < Duration::from_secs(120) {
                attempt += 1;
                if attempt % 10 == 1 {
                    println!("[Gluon RAG] Health check attempt {} for {} (elapsed: {:.1}s)",
                        attempt, name, start.elapsed().as_secs_f32());
                }

                // ZMIANA: KoboldCPP nie ma endpointu /health, używamy /api/extra/version lub /api/v1/model
                let check_url = format!("http://127.0.0.1:{}/api/extra/version", port);

                match client.get(&check_url).send().await {
                    Ok(resp) => {
                        let status = resp.status();
                        println!("[Gluon RAG] {} responded with status: {}", name, status);
                        if status.is_success() {
                            println!("[Gluon RAG] ✅ {} is READY!", name);
                            return Ok(());
                        }
                    },
                    Err(e) => {
                        if attempt % 10 == 1 {
                            println!("[Gluon RAG] Health check failed for {}: {}", name, e);
                        }
                    }
                }
                sleep(Duration::from_millis(500)).await;
            }
            Err(format!("Timeout waiting for {} on port {} after {} attempts", name, port, attempt))
        }

        if let Err(e) = wait_for_health_async("8081", "Embed AI (Nomic)", app_handle, 85).await {
            eprintln!("[Gluon RAG] ❌ {}", e);
            self.stop_services();
            return Err(e);
        }

        let _ = app_handle.emit("ai-loading-progress", serde_json::json!({
            "stage": "complete",
            "message": "All AI services are online and ready!",
            "progress": 100
        }));
        println!("[Gluon AI] 🚀 All Local Intelligence Services are Online & Ready.");
        Ok(())
    }

    pub fn stop_services(&self) {
        // Metoda 1: Próba łagodna przez uchwyt procesu
        if let Some(mut child) = self.embed_process.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
            println!("[Gluon RAG] Embed Service stopped via handle.");
        }

        // Metoda 2: Zrzut napalmu (Taskkill) dla pewności
        // Wykonujemy to zawsze przy stopie, aby ubić procesy, które mogły zgubić uchwyt
        if cfg!(windows) {
            println!("[Gluon AI] 🧹 Force cleanup: Ensuring all KoboldCPP processes are dead...");
            let _ = Command::new("taskkill")
                .args(&["/F", "/IM", "koboldcpp-oldpc.exe", "/T"])
                .creation_flags(0x08000000)
                .output();
            
            let _ = Command::new("taskkill")
                .args(&["/F", "/IM", "koboldcpp.exe", "/T"])
                .creation_flags(0x08000000)
                .output();
        }
    }
}

// FIX: Implementacja Drop gwarantuje zabicie procesów, gdy obiekt LocalAiService jest usuwany z pamięci
impl Drop for LocalAiService {
    fn drop(&mut self) {
        println!("[Gluon AI] ServiceManager is dropping - killing child processes...");
        self.stop_services();
    }
}

// Windows-specific trait extension for creation_flags
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(not(windows))]
trait CommandExt {
    fn creation_flags(&mut self, _flags: u32) -> &mut Self { self }
}

#[cfg(not(windows))]
impl CommandExt for Command {}