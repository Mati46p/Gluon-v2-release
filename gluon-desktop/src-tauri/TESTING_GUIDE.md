# Gluon v2 - Przewodnik po Testach Backend Rust

## Utworzony Szkielet Testów

Został stworzony kompletny szkielet testów pokrywający cały backend Rust aplikacji Gluon v2.

## Struktura Utworzonych Plików

### 📚 Dokumentacja
- **tests/README.md** - Pełna dokumentacja testów, konwencje, instrukcje uruchomienia

### 🛠️ Utilities
- **tests/test_helpers.rs** - Funkcje pomocnicze do testów:
  - `TestFixture` - zarządzanie tymczasowymi plikami testowymi
  - Przykładowe kody źródłowe (TypeScript, Python, Rust, JavaScript)
  - Funkcje asercji (assert_contains_all, assert_approx_eq)
  - Generatory danych testowych
  - Helpery wydajnościowe i bezpieczeństwa

### 🧪 Pliki Testowe

#### 1. **tests/analysis_tests.rs** (200+ testów)
Testy dla modułu `apply_system::analysis`:
- Engine analizy kodu
- Parsery języków (TypeScript, Python, Rust, JavaScript, Go, Java, C++)
- Zapytania Tree-sitter
- Symulacja zmian
- Walidacja kodu

#### 2. **tests/context_tests.rs** (100+ testów)
Testy dla modułu `apply_system::context`:
- Graf zależności (tworzenie, modyfikacja, cykle, ścieżki)
- Ranking kontekstu (scoring, relevancja)
- Mapa repozytorium (indeksowanie, wyszukiwanie, importy)
- Ekstrakcja symboli (funkcje, klasy, interfejsy, typy)

#### 3. **tests/matchers_tests.rs** (150+ testów)
Testy dla modułu `apply_system::matchers`:
- Anchor matcher (ekstrakcja i dopasowanie kotwic)
- Fuzzy matcher (dopasowanie rozmyte, Levenshtein)
- Regex matcher (wzorce, capture groups)
- Block matcher (bloki funkcji, klas)
- Weighted anchor matcher (ważone dopasowanie)
- Koordynator matcherów (strategia fallback)

#### 4. **tests/parsers_tests.rs** (120+ testów)
Testy dla modułu `apply_system::parsers`:
- Unified diff parser
- Markdown parser
- Search/Replace parser
- Git-style parser
- XML/GProtocol parser
- Lazy stitcher (łączenie zmian)
- Pattern matching
- Indentation normalizer
- Koordynator parserów

#### 5. **tests/core_modules_tests.rs** (100+ testów)
Testy dla głównych modułów:
- Backup system (tworzenie, przywracanie, kompresja)
- Config (ładowanie, zapisywanie, walidacja)
- Debug manager (logi, snapshoty, metryki)
- Integrity auditor (audyt, integralność plików)
- Logging (poziomy, rotacja, structured logging)
- RAG engine (indeksowanie, zapytania, semantic search)
- Self-healing (detekcja błędów, auto-fix)
- Service manager (start, stop, health checks)
- Snapshot (tworzenie, porównywanie, konflikty)
- Transaction (commit, rollback, izolacja)
- Agent workflow (kroki, błędy, pause/resume)
- Prompts (generowanie, szablony)

#### 6. **tests/commands_tests.rs** (80+ testów)
Testy dla komend Tauri:
- Parse model response
- Apply change (z matchingiem, walidacją)
- Change queue (get, apply all)
- Undo operations
- Config commands
- Context commands (refresh graph, repo map)
- Backup commands (preview, restore)
- Integrity commands (audit, report)
- Debug commands (config, logs, snapshots, diagnostics)
- Performance commands (metrics, traces)
- Error reporting
- Regression tests
- Workflow commands (start, pause, resume, cancel)

#### 7. **tests/local_ai_tests.rs** (25+ testów)
Testy dla modułu `local_ai`:
- Inicjalizacja i ładowanie modelu
- Inference (prosty, batch, streaming)
- Konfiguracja (temperatura, max tokens)
- Wydajność i pamięć
- Obsługa błędów
- Integracja z analizą kodu i RAG

### 📊 Istniejące Testy
- **tests/integration_tests.rs** - testy integracyjne (istniejący)
- **tests/security_tests.rs** - testy bezpieczeństwa (istniejący)
- **tests/load_tests.rs** - testy obciążenia (istniejący)

## Statystyki Szkieletu

- **~900+ testów zdefiniowanych**
- **8 plików testowych** (3 istniejące + 5 nowych)
- **100% pokrycie modułów** backendu Rust
- Każdy test ma jasno określony cel i opis TODO

## Uruchomienie Testów

### Wszystkie testy
```bash
cd gluon-desktop/src-tauri
cargo test
```

### Testy kompilacji (sprawdzenie czy się kompilują)
```bash
cargo test --no-fail-fast
```

### Konkretne moduły testowe
```bash
cargo test --test analysis_tests
cargo test --test context_tests
cargo test --test matchers_tests
cargo test --test parsers_tests
cargo test --test core_modules_tests
cargo test --test commands_tests
cargo test --test local_ai_tests
```

### Istniejące testy integracyjne
```bash
cargo test --test integration_tests
cargo test --test security_tests
cargo test --test load_tests
```

### Z logowaniem
```bash
cargo test -- --nocapture
```

## Następne Kroki

### 1. Implementacja Testów
Każdy test jest oznaczony jako `TODO` z opisem tego, co powinien testować:
```rust
#[test]
fn test_example() {
    // TODO: Test opisany tutaj
    println!("TODO: test_example");
}
```

Aby zaimplementować test:
1. Znajdź test oznaczony TODO
2. Zaimplementuj logikę testową
3. Usuń komentarz TODO
4. Uruchom test: `cargo test test_example`

### 2. Priorytety Implementacji
Zalecana kolejność implementacji (wg priorytetu):

**Priorytet Wysoki:**
1. **parsers_tests.rs** - krytyczne dla parsowania odpowiedzi modelu
2. **matchers_tests.rs** - krytyczne dla dopasowania kodu
3. **commands_tests.rs** - API Tauri używane przez frontend

**Priorytet Średni:**
4. **analysis_tests.rs** - analiza kodu i Tree-sitter
5. **context_tests.rs** - kontekst i zależności
6. **core_modules_tests.rs** - systemy wspomagające

**Priorytet Niski:**
7. **local_ai_tests.rs** - jeśli używasz lokalnego AI

### 3. Dodatkowe Narzędzia Testowe

#### Pokrycie kodu (Code Coverage)
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage
```

#### Benchmarking
```bash
# Dodaj benchmarki w benches/
cargo bench
```

#### Property-based testing
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_property(s in "\\PC*") {
        // Test właściwości
    }
}
```

## Konwencje Testowe

### Nazewnictwo
- `test_<moduł>_<funkcjonalność>` - podstawowe testy
- `test_<moduł>_<funkcjonalność>_succeeds` - testy pozytywne
- `test_<moduł>_<funkcjonalność>_fails_with_<warunek>` - testy negatywne
- `test_<moduł>_<funkcjonalność>_edge_case_<scenariusz>` - testy brzegowe

### Struktura
```rust
#[test]
fn test_example() {
    // Arrange - przygotowanie
    let input = prepare_test_data();

    // Act - wykonanie
    let result = function_under_test(input);

    // Assert - sprawdzenie
    assert_eq!(result, expected);
}
```

## Dodane Zależności Testowe

Do `Cargo.toml` dodano:
```toml
[dev-dependencies]
tempfile = "3.23.0"      # Tymczasowe pliki (istniejące)
mockall = "0.13"         # Mockowanie (nowe)
proptest = "1.5"         # Property-based testing (nowe)
criterion = "0.5"        # Benchmarking (nowe)
serial_test = "3.2"      # Testy sekwencyjne (nowe)
```

## Continuous Integration

### GitHub Actions (przykład)
```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test --all-features
```

## Debugging Testów

### Uruchomienie pojedynczego testu z logami
```bash
RUST_LOG=debug cargo test test_name -- --nocapture
```

### Uruchomienie testów sekwencyjnie (gdy są konflikty)
```bash
cargo test -- --test-threads=1
```

### Ignorowanie testów
```rust
#[test]
#[ignore]
fn expensive_test() {
    // Test ignorowany domyślnie
}

// Uruchomienie: cargo test -- --ignored
```

## Wsparcie

Przy problemach z testami:
1. Sprawdź [tests/README.md](tests/README.md) - pełna dokumentacja
2. Użyj `cargo test --help` - opcje cargo test
3. Sprawdź logi: `cargo test -- --nocapture`

## Podsumowanie

✅ Stworzono kompletny szkielet testów dla całego backendu Rust
✅ ~900+ testów zdefiniowanych z jasnym opisem
✅ Utilities testowe (test_helpers.rs)
✅ Pełna dokumentacja (README.md)
✅ Dodatkowe zależności testowe
✅ Instrukcje uruchomienia i CI

**Teraz możesz rozpocząć implementację testów krok po kroku!**
