//! Test Helpers and Utilities
//!
//! Funkcje pomocnicze używane w testach jednostkowych i integracyjnych

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// ============================================================================
// File System Helpers
// ============================================================================

/// Tworzy tymczasowy katalog z plikami testowymi
pub struct TestFixture {
    pub temp_dir: TempDir,
    pub files: Vec<PathBuf>,
}

impl TestFixture {
    /// Tworzy nową fixture z pustym katalogiem
    pub fn new() -> Self {
        Self {
            temp_dir: TempDir::new().unwrap(),
            files: Vec::new(),
        }
    }

    /// Tworzy plik w fixture z podaną zawartością
    pub fn create_file(&mut self, name: &str, content: &str) -> PathBuf {
        let file_path = self.temp_dir.path().join(name);

        // Stwórz katalog nadrzędny jeśli nie istnieje
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        fs::write(&file_path, content).unwrap();
        self.files.push(file_path.clone());
        file_path
    }

    /// Tworzy wiele plików z jednego słownika
    pub fn create_files(&mut self, files: Vec<(&str, &str)>) -> Vec<PathBuf> {
        files
            .into_iter()
            .map(|(name, content)| self.create_file(name, content))
            .collect()
    }

    /// Zwraca ścieżkę do katalogu fixture
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Czyta zawartość pliku w fixture
    pub fn read_file(&self, name: &str) -> String {
        let file_path = self.temp_dir.path().join(name);
        fs::read_to_string(file_path).unwrap()
    }

    /// Sprawdza czy plik istnieje w fixture
    pub fn file_exists(&self, name: &str) -> bool {
        self.temp_dir.path().join(name).exists()
    }
}

impl Default for TestFixture {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Code Sample Helpers
// ============================================================================

/// Przykładowy kod TypeScript do testów
pub fn sample_typescript_code() -> &'static str {
    r#"
import { User } from './types';

export function fetchUser(id: string): Promise<User> {
    console.log("Fetching user");
    return fetch(`/api/users/${id}`)
        .then(response => response.json());
}

export function deleteUser(id: string): Promise<void> {
    return fetch(`/api/users/${id}`, { method: 'DELETE' })
        .then(() => console.log("User deleted"));
}
"#
}

/// Przykładowy kod Python do testów
pub fn sample_python_code() -> &'static str {
    r#"
def calculate_total(items):
    """Calculate total price of items."""
    total = 0
    for item in items:
        total += item.price
    return total

def apply_discount(total, discount_percent):
    """Apply discount to total."""
    discount = total * (discount_percent / 100)
    return total - discount
"#
}

/// Przykładowy kod Rust do testów
pub fn sample_rust_code() -> &'static str {
    r#"
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
}

impl User {
    pub fn new(id: String, name: String, email: String) -> Self {
        Self { id, name, email }
    }

    pub fn is_valid(&self) -> bool {
        !self.email.is_empty() && self.email.contains('@')
    }
}
"#
}

/// Przykładowy kod JavaScript do testów
pub fn sample_javascript_code() -> &'static str {
    r#"
function validateEmail(email) {
    const regex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return regex.test(email);
}

class ShoppingCart {
    constructor() {
        this.items = [];
    }

    addItem(item) {
        this.items.push(item);
    }

    getTotal() {
        return this.items.reduce((sum, item) => sum + item.price, 0);
    }
}
"#
}

// ============================================================================
// Diff/Change Helpers
// ============================================================================

/// Tworzy przykładowy unified diff
pub fn sample_unified_diff() -> &'static str {
    r#"
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,5 +10,6 @@ fn main() {
-    let old_value = 1;
+    let new_value = 2;
+    println!("Value: {}", new_value);
"#
}

/// Tworzy przykładowy markdown change
pub fn sample_markdown_change() -> &'static str {
    r#"
File: `src/utils.ts`

Before:
```typescript
export function add(a: number, b: number): number {
    return a + b;
}
```

After:
```typescript
export function add(a: number, b: number): number {
    // Add two numbers with validation
    if (isNaN(a) || isNaN(b)) {
        throw new Error("Invalid numbers");
    }
    return a + b;
}
```
"#
}

/// Tworzy przykładowy search/replace change
pub fn sample_search_replace() -> &'static str {
    r#"
<<<< SEARCH
function oldFunction() {
    return "old";
}
====
function newFunction() {
    return "new";
}
>>>> REPLACE
"#
}

// ============================================================================
// Assertion Helpers
// ============================================================================

/// Sprawdza czy string zawiera wszystkie podane substringi
pub fn assert_contains_all(haystack: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            haystack.contains(needle),
            "Expected to find '{}' in string",
            needle
        );
    }
}

/// Sprawdza czy string nie zawiera żadnego z podanych substringów
pub fn assert_contains_none(haystack: &str, needles: &[&str]) {
    for needle in needles {
        assert!(
            !haystack.contains(needle),
            "Did not expect to find '{}' in string",
            needle
        );
    }
}

/// Sprawdza czy dwie wartości są w granicach tolerancji (dla f64)
pub fn assert_approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() < tolerance,
        "Expected {} to be approximately {} (tolerance: {})",
        actual,
        expected,
        tolerance
    );
}

// ============================================================================
// Mock Data Generators
// ============================================================================

/// Generuje losowy string o określonej długości
pub fn generate_random_string(length: usize) -> String {
    use std::iter;
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        .chars()
        .collect();

    iter::repeat_with(|| chars[0])
        .take(length)
        .collect()
}

/// Generuje przykładowe dane użytkownika
pub fn mock_user_data() -> serde_json::Value {
    serde_json::json!({
        "id": "user_123",
        "name": "Test User",
        "email": "test@example.com",
        "created_at": "2024-01-01T00:00:00Z"
    })
}

/// Generuje listę przykładowych zmian
pub fn mock_changes_list(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| format!("change_{}", i))
        .collect()
}

// ============================================================================
// Performance Testing Helpers
// ============================================================================

/// Mierzy czas wykonania funkcji
pub fn measure_execution_time<F, R>(f: F) -> (R, std::time::Duration)
where
    F: FnOnce() -> R,
{
    let start = std::time::Instant::now();
    let result = f();
    let duration = start.elapsed();
    (result, duration)
}

/// Sprawdza czy operacja zakończyła się w określonym czasie
pub fn assert_completes_within<F, R>(max_duration: std::time::Duration, f: F) -> R
where
    F: FnOnce() -> R,
{
    let (result, duration) = measure_execution_time(f);
    assert!(
        duration <= max_duration,
        "Operation took {:?}, expected <= {:?}",
        duration,
        max_duration
    );
    result
}

// ============================================================================
// Tree-sitter Test Helpers
// ============================================================================

/// Tworzy przykładowy node dla testów tree-sitter
pub fn sample_tree_sitter_query() -> &'static str {
    r#"
(function_declaration
  name: (identifier) @function.name
  parameters: (formal_parameters) @function.params)
"#
}

/// Przykładowy kod z różnymi konstrukcjami dla testów parsowania
pub fn complex_code_sample() -> &'static str {
    r#"
// Comments should be handled
class Calculator {
    constructor(private value: number = 0) {}

    add(n: number): Calculator {
        this.value += n;
        return this;
    }

    subtract(n: number): Calculator {
        this.value -= n;
        return this;
    }

    multiply(n: number): Calculator {
        this.value *= n;
        return this;
    }

    /* Multi-line
       comment test */
    divide(n: number): Calculator {
        if (n === 0) {
            throw new Error("Division by zero");
        }
        this.value /= n;
        return this;
    }

    getResult(): number {
        return this.value;
    }
}

export { Calculator };
"#
}

// ============================================================================
// Security Test Helpers
// ============================================================================

/// Lista niebezpiecznych ścieżek do testów security
pub fn dangerous_paths() -> Vec<&'static str> {
    vec![
        "../../../etc/passwd",
        "..\\..\\..\\Windows\\System32",
        ".env",
        ".env.local",
        ".git/config",
        "node_modules/package.json",
        "../../.ssh/id_rsa",
    ]
}

/// Lista wrażliwych wzorców do testów
pub fn sensitive_patterns() -> Vec<&'static str> {
    vec![
        "password",
        "secret",
        "api_key",
        "private_key",
        "token",
        "credential",
    ]
}

// ============================================================================
// Concurrency Test Helpers
// ============================================================================

/// Wykonuje operację wielokrotnie w wielu wątkach
pub fn run_concurrent<F>(thread_count: usize, iterations: usize, f: F)
where
    F: Fn(usize) + Send + Sync + 'static + Clone,
{
    use std::sync::Arc;
    use std::thread;

    let f = Arc::new(f);
    let mut handles = vec![];

    for thread_id in 0..thread_count {
        let f = Arc::clone(&f);
        let handle = thread::spawn(move || {
            for i in 0..iterations {
                f(thread_id * iterations + i);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

// ============================================================================
// Tests for test_helpers themselves
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixture_creates_files() {
        let mut fixture = TestFixture::new();
        let path = fixture.create_file("test.txt", "content");
        assert!(path.exists());
        assert_eq!(fixture.read_file("test.txt"), "content");
    }

    #[test]
    fn test_assert_contains_all() {
        let text = "hello world from rust";
        assert_contains_all(text, &["hello", "world", "rust"]);
    }

    #[test]
    fn test_measure_execution_time() {
        let (result, duration) = measure_execution_time(|| {
            std::thread::sleep(std::time::Duration::from_millis(10));
            42
        });
        assert_eq!(result, 42);
        assert!(duration.as_millis() >= 10);
    }

    #[test]
    fn test_mock_user_data() {
        let user = mock_user_data();
        assert_eq!(user["id"], "user_123");
        assert_eq!(user["email"], "test@example.com");
    }

    #[test]
    fn test_dangerous_paths_detection() {
        let paths = dangerous_paths();
        assert!(paths.iter().any(|p| p.contains("..")));
        assert!(paths.iter().any(|p| p.contains(".env")));
    }
}
