use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;
use std::fs;
use std::path::Path;

const EMBED_API: &str = "http://127.0.0.1:8081/v1/embeddings";

#[derive(Serialize)]
struct EmbeddingRequest {
    input: String,
    model: String,
}

#[derive(Deserialize)]
struct EmbeddingResponseData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingResponseData>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub content: String,
    pub score: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VectorStore {
    // Klucz -> (Wektor, Treść Chunka)
    index: HashMap<String, (Vec<f32>, String)>,
}

impl VectorStore {
    pub fn new() -> Self {
        Self { index: HashMap::new() }
    }

    /// Returns the keys in the vector store index
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.index.keys()
    }

    /// Get the number of embeddings in the store
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Insert an embedding into the store
    pub fn insert(&mut self, key: String, embedding: Vec<f32>, content: String) {
        self.index.insert(key, (embedding, content));
    }

    /// Search for similar embeddings (returns key, score, content)
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<(String, f32, String)> {
        let mut scores: Vec<(String, f32, String)> = Vec::new();

        for (key, (vec, content)) in &self.index {
            let score = cosine_similarity(query_embedding, vec);
            scores.push((key.clone(), score, content.clone()));
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scores.into_iter().take(top_k).collect()
    }

    /// Get all chunks with their embeddings (for saving to database)
    pub fn get_all_chunks(&self) -> Vec<(String, Vec<f32>, String)> {
        self.index.iter()
            .map(|(key, (embedding, content))| (key.clone(), embedding.clone(), content.clone()))
            .collect()
    }

    pub fn save_to_disk(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string(&self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())?;
        println!("[RAG Persistence] Index saved to {:?}", path);
        Ok(())
    }

    pub fn load_from_disk(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let store: Self = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        println!("[RAG Persistence] Index loaded from {:?} ({} entries)", path, store.index.len());
        Ok(store)
    }

    // Krok A: Indeksowanie pliku (z Semantic Chunking)
    pub async fn index_file(&mut self, path: String, content: String) -> Result<(), String> {
        // [FIX] Timeout 120s, wyłączone proxy (częsty problem na localhost)
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .no_proxy() 
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        // Użyj chunkera
        let chunks = crate::local_ai::semantic_chunking::chunk_file(&path, &content);
        let total_chunks = chunks.len();

        // Log liczby chunków
        println!("[RAG]     → {} chunks to process", total_chunks);

        for (i, chunk) in chunks.iter().enumerate() {
            // Log postępu co 5 chunków lub dla małych plików
            if total_chunks <= 5 || i % 5 == 0 || i == total_chunks - 1 {
                println!("[RAG]     → Processing chunk {}/{}", i + 1, total_chunks);
            }
            let unique_key = format!("{}::{}", chunk.file_path, chunk.start_line);
            
            // Dodajemy nagłówek do treści dla lepszego kontekstu embeddingu
            let context_content = format!("File: {}\nLine: {}\n\n{}", chunk.file_path, chunk.start_line, chunk.content);
            let prompt = format!("search_document: {}", context_content);

            let mut attempts = 0;
            // [FIX] Zwiększona odporność na restarty serwera (do ~60 sekund oczekiwania)
            const MAX_RETRIES: u32 = 20;
            let mut last_error = String::new();

            // Retry Loop with Smart Backoff
            loop {
                attempts += 1;
                let res_result = client.post(EMBED_API)
                    .json(&EmbeddingRequest {
                        input: prompt.clone(),
                        model: "nomic-embed-text-v2-moe.Q8_0.gguf".to_string()
                    })
                    .send()
                    .await;

                match res_result {
                    Ok(res) => {
                        if !res.status().is_success() {
                            let status = res.status();
                            // Ignoruj błędy 5xx (serwer zajęty/ładuje się)
                            let text = res.text().await.unwrap_or_default();
                            last_error = format!("API Error {} for {}: {}", status, unique_key, text);

                            // Jeśli 400 (Bad Request), to wina zapytania - nie ma sensu ponawiać
                            if status.as_u16() == 400 {
                                break; 
                            }
                        } else {
                            // Sukces HTTP
                            let body_result = res.json::<EmbeddingResponse>().await;
                            match body_result {
                                Ok(body) => {
                                    if let Some(data) = body.data.first() {
                                        self.index.insert(unique_key.clone(), (data.embedding.clone(), chunk.content.clone()));
                                        last_error.clear();
                                    } else {
                                        println!("[RAG Index] Warning: No embedding data returned for chunk: {}", unique_key);
                                    }
                                    break; // Sukces - wyjście
                                },
                                Err(e) => {
                                    last_error = format!("Invalid JSON for {}: {}", unique_key, e);
                                }
                            }
                        }
                    },
                    Err(e) => {
                        // Błąd połączenia (serwer leży lub wstaje)
                        last_error = format!("Connection failed for {}: {}. Is AI (Port 8081) ready?", unique_key, e);
                    }
                }

                if attempts >= MAX_RETRIES {
                    println!("[RAG Index] ❌ Failed after {} attempts: {}", attempts, last_error);
                    return Err(last_error);
                }

                if !last_error.is_empty() {
                    // Backoff: 500ms, 1000ms, 1500ms... 
                    // Co 5 prób logujemy informację, że czekamy na serwer
                    let delay = 500 * attempts as u64;
                    if attempts % 5 == 0 {
                        println!("[RAG Index] ⏳ Waiting for AI Service to be ready... (Attempt {}/{})", attempts, MAX_RETRIES);
                    }
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        }
        Ok(())
    }

    // New search method that returns structured results
    pub async fn search_structured(&self, query: &str, top_k: usize) -> Result<Vec<SearchResult>, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .no_proxy()
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let prompt = format!("search_query: {}", query);

        println!("[RAG Search] Querying: '{}' (Index size: {})", query, self.index.len());

        if self.index.is_empty() {
            return Ok(Vec::new());
        }

        let res = client.post(EMBED_API)
            .json(&EmbeddingRequest {
                input: prompt,
                model: "nomic-embed-text-v2-moe.Q8_0.gguf".to_string()
            })
            .send()
            .await
            .map_err(|e| format!("Embedding API request failed: {}", e))?
            .json::<EmbeddingResponse>()
            .await
            .map_err(|e| format!("Failed to parse embedding response: {}", e))?;

        let query_vec = &res.data[0].embedding;
        let results_raw = self.search(query_vec, top_k);

        // Convert to structured results
        let results = results_raw.into_iter()
            .map(|(key, score, content)| {
                // key format: "file_path::line_number"
                let file_path = key.split("::").next().unwrap_or(&key).to_string();
                SearchResult {
                    file_path,
                    content,
                    score,
                }
            })
            .collect();

        Ok(results)
    }

    // Krok B: Szukanie (Zwraca treść chunków) - Legacy method
    pub async fn search_by_query(&self, query: String, top_k: usize) -> Result<Vec<String>, String> {
        let client = Client::new();
        let prompt = format!("search_query: {}", query);
        
        println!("[RAG Search] Querying: '{}' (Index size: {})", query, self.index.len());

        if self.index.is_empty() {
             return Ok(Vec::new());
        }

        let res = client.post(EMBED_API)
            .json(&EmbeddingRequest {
                input: prompt,
                model: "nomic-embed-text-v2-moe.Q8_0.gguf".to_string()
            })
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<EmbeddingResponse>()
            .await
            .map_err(|e| e.to_string())?;

        let query_vec = &res.data[0].embedding;
        let mut scores: Vec<(String, f32, String)> = Vec::new(); // Key, Score, Content

        for (key, (vec, content)) in &self.index {
            let score = cosine_similarity(&query_vec, vec);
            scores.push((key.clone(), score, content.clone()));
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Zwracamy sformatowane fragmenty: "File: ... \n Content"
        let results = scores.into_iter()
            .take(top_k)
            .map(|(key, _, content)| {
                // key to np. "src/main.rs::10"
                let parts: Vec<&str> = key.split("::").collect();
                let file_path = parts[0];
                format!("// File: {}\n{}", file_path, content)
            })
            .collect();

        Ok(results)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot_product / (norm_a * norm_b)
}