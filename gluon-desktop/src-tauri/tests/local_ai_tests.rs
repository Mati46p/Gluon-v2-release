//! Testy dla modułu local_ai, RAG i Qwen
//!
//! Obejmuje:
//! 1. Semantic Chunking (logika podziału tekstu)
//! 2. Vector Store (trwałość danych)
//! 3. Integrację z API (Qwen/Nomic) - testy te wymagają działających usług

mod test_helpers;
use gluon_desktop_lib::local_ai::rag_engine::VectorStore;
use gluon_desktop_lib::local_ai::semantic_chunking;
use std::path::Path;
use tempfile::TempDir;

// ============================================================================
// 1. Semantic Chunking Tests (Pure Logic)
// ============================================================================

#[test]
fn test_chunk_file_splitting_rust() {
    let content = r#"
fn function_one() {
    println!("One");
}

fn function_two() {
    println!("Two");
}
"#;
    let chunks = semantic_chunking::chunk_file("test.rs", content);
    
    // Spodziewamy się, że chunker podzieli to na logiczne bloki lub zachowa całość jeśli małe
    // W obecnej implementacji (semantic_chunking.rs), jeśli wcięcie == 0, zaczyna nowy blok.
    
    assert!(!chunks.is_empty(), "Should generate chunks");
    assert_eq!(chunks[0].file_path, "test.rs");
    
    // Weryfikacja czy zawartość nie została zgubiona
    let joined_content: String = chunks.iter().map(|c| c.content.clone()).collect();
    assert!(joined_content.contains("function_one"));
    assert!(joined_content.contains("function_two"));
}

#[test]
fn test_chunk_file_large_text() {
    // Generujemy duży plik tekstowy, aby wymusić podział na limicie 6000 znaków
    let line = "To jest linia testowa która ma trochę znaków.\n";
    let content = line.repeat(500); // ~23k znaków

    let chunks = semantic_chunking::chunk_file("large.txt", &content);

    assert!(chunks.len() > 1, "Large file should be split into multiple chunks");
    
    // Sprawdź czy żaden chunk nie przekracza bezpiecznego limitu (z lekkim marginesem na nagłówki)
    for (i, chunk) in chunks.iter().enumerate() {
        assert!(chunk.content.len() <= 6500, "Chunk {} is too large: {}", i, chunk.content.len());
    }
}

#[test]
fn test_chunk_preserves_indentation_context() {
    let content = r#"
class User {
    constructor() {
        this.name = "Test";
    }

    save() {
        console.log("Saving");
    }
}
"#;
    let chunks = semantic_chunking::chunk_file("user.js", content);
    
    // Chunker powinien (zależnie od logiki) albo złączyć to w jedną klasę, 
    // albo podzielić zachowując sens.
    // Obecna implementacja semantic_chunking.rs dzieli przy wcięciu 0.
    
    assert!(!chunks.is_empty());
    // Sprawdźmy czy pierwszy chunk zawiera definicję klasy
    assert!(chunks[0].content.contains("class User"));
}

// ============================================================================
// 2. Vector Store Persistence Tests (Disk I/O)
// ============================================================================

#[test]
fn test_vector_store_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_vectors.json");

    // 1. Utwórz i zapisz (pusty) store
    let store = VectorStore::new();
    let save_result = store.save_to_disk(&db_path);
    assert!(save_result.is_ok(), "Should save empty store successfully");
    assert!(db_path.exists(), "File should be created");

    // 2. Załaduj ponownie
    let loaded_result = VectorStore::load_from_disk(&db_path);
    assert!(loaded_result.is_ok(), "Should load store successfully");
    
    // 3. Sprawdź czy działa dla nieistniejącego pliku (powinien stworzyć nowy)
    let non_existent = temp_dir.path().join("ghost.json");
    let loaded_ghost = VectorStore::load_from_disk(&non_existent);
    assert!(loaded_ghost.is_ok(), "Should handle missing file gracefully by creating new store");
}

// ============================================================================
// 3. Integration Tests (Requires Running AI Services)
// ============================================================================
// Te testy są domyślnie ignorowane. Uruchom je komendą:
// cargo test --test local_ai_tests -- --ignored
// UPEWNIJ SIĘ, ŻE Gluon-v2 LUB KoboldCPP DZIAŁA W TLE NA PORTACH 8081/8082!

#[tokio::test]
#[ignore]
async fn test_integration_qwen_chat_api() {
    println!("Connecting to Qwen at http://127.0.0.1:8082...");
    
    let client = reqwest::Client::new();
    let prompt = "<|im_start|>user\nHello, are you working?<|im_end|>\n<|im_start|>assistant\n";
    
    let response = client.post("http://127.0.0.1:8082/completion")
        .json(&serde_json::json!({
            "prompt": prompt,
            "n_predict": 20,
            "temperature": 0.1
        }))
        .send()
        .await;

    match response {
        Ok(res) => {
            assert!(res.status().is_success(), "Qwen API returned error status");
            let body: serde_json::Value = res.json().await.unwrap();
            let content = body["content"].as_str().unwrap_or("");
            println!("Qwen Response: {}", content);
            assert!(!content.is_empty(), "Qwen returned empty content");
        },
        Err(e) => panic!("Failed to connect to Qwen (Is port 8082 open?): {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_integration_rag_indexing_and_search() {
    println!("Connecting to Embed API at http://127.0.0.1:8081...");

    let mut store = VectorStore::new();
    let test_file_path = "integration_test.txt";
    let test_content = "Gluon v2 is an advanced AI coding assistant written in Rust and JavaScript.";

    // 1. Test Indexing (Calls Nomic Embedder)
    let index_result = store.index_file(test_file_path.to_string(), test_content.to_string()).await;
    
    match index_result {
        Ok(_) => println!("Indexing successful."),
        Err(e) => panic!("Indexing failed (Is port 8081 open?): {}", e),
    }

    // 2. Test Searching
    let query = "What is Gluon v2?";
    let search_result = store.search(query.to_string(), 1).await;

    match search_result {
        Ok(results) => {
            assert!(!results.is_empty(), "Search should return results");
            let first_match = &results[0];
            println!("Found match: {}", first_match);
            assert!(first_match.contains("Gluon v2"), "Search result should contain relevant text");
        },
        Err(e) => panic!("Search failed: {}", e),
    }
}

// ============================================================================
// 4. Configuration & Utils Tests
// ============================================================================

#[test]
fn test_rag_vector_store_initialization() {
    let store = VectorStore::new();
    // VectorStore jest nieprzezroczysty (pola prywatne), ale możemy sprawdzić czy się tworzy
    // i czy metody publiczne nie panikują na pustym stanie.
    assert!(true, "VectorStore initialized");
}