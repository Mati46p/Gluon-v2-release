# Gluon Backend Tests Documentation

Kompletny szkielet testów dla backendu Rust aplikacji Gluon v2.

## Struktura Testów

### 1. Testy Integracyjne (`tests/`)
- **integration_tests.rs** - podstawowe testy end-to-end
- **security_tests.rs** - testy bezpieczeństwa
- **load_tests.rs** - testy wydajności i obciążenia
- **performance_tests.rs** - testy wydajności (nowy)
- **api_tests.rs** - testy API Tauri (nowy)

### 2. Testy Jednostkowe (wewnątrz modułów)
Każdy moduł posiada sekcję `#[cfg(test)]` z testami jednostkowymi.

#### Apply System - Analysis (`src/apply_system/analysis/`)
- `engine.rs` - testy silnika analizy kodu
- `languages.rs` - testy parsowania języków (Python, JS, TS, Rust, Go, Java, C++)
- `queries.rs` - testy zapytań Tree-sitter
- `simulation.rs` - testy symulacji zmian
- `validation.rs` - testy walidacji kodu

#### Apply System - Context (`src/apply_system/context/`)
- `graph.rs` - testy grafu zależności
- `ranker.rs` - testy rankingu kontekstu
- `repo_map.rs` - testy mapy repozytorium
- `symbol_extractor.rs` - testy ekstrakcji symboli

#### Apply System - Matchers (`src/apply_system/matchers/`)
- `anchor_matcher.rs` - testy dopasowania kotwic
- `anchor_extraction.rs` - testy ekstrakcji kotwic
- `fuzzy_matcher.rs` - testy dopasowania rozmytego
- `regex_matcher.rs` - testy dopasowania regex
- `weighted_anchor_matcher.rs` - testy ważonego dopasowania
- `coordinator.rs` - testy koordynatora matcherów

#### Apply System - Parsers (`src/apply_system/parsers/`)
- `lazy_stitcher.rs` - testy łączenia zmian
- `git_style_search_replace.rs` - testy git-style parsowania
- `markdown.rs` - testy parsowania markdown
- `unified_diff.rs` - testy parsowania unified diff
- `xml_gprotocol.rs` - testy parsowania XML
- `search_replace.rs` - testy search/replace
- `pattern_matching.rs` - testy dopasowania wzorców
- `indentation_normalizer.rs` - testy normalizacji wcięć
- `coordinator.rs` - testy koordynatora parserów
- `regression_tests.rs` - testy regresji (istniejące)

#### Apply System - Validators (`src/apply_system/validators/`)
- `syntax_validator.rs` - testy walidacji składni
- `batch_validator.rs` - testy walidacji wsadowej

#### Apply System - Lazy (`src/apply_system/lazy/`)
- `detector.rs` - testy detekcji lazy changes
- `engine.rs` - testy silnika lazy
- `matcher.rs` - testy matchera lazy
- `reconstructor.rs` - testy rekonstrukcji
- `safety.rs` - testy bezpieczeństwa
- `structural_matcher.rs` - testy strukturalnego matchera
- `weighted_anchoring.rs` - testy ważonych kotwic
- `integration_tests.rs` - testy integracyjne (istniejące)

#### Apply System - Główne moduły
- `agent_workflow.rs` - testy workflow agenta
- `backup_system.rs` - testy systemu backupu
- `config.rs` - testy konfiguracji
- `debug_manager.rs` - testy zarządzania debugiem
- `integrity_auditor.rs` - testy audytu integralności
- `logging.rs` - testy logowania
- `prompts.rs` - testy promptów
- `rag_engine.rs` - testy RAG engine
- `self_healing.rs` - testy samo-naprawy
- `service_manager.rs` - testy zarządzania serwisami
- `snapshot.rs` - testy snapshotów
- `transaction.rs` - testy transakcji
- `tauri_commands.rs` - testy komend Tauri
- `workflow_commands.rs` - testy komend workflow

#### Local AI (`src/local_ai/`)
- Testy modułu lokalnego AI

#### Editor Bridge (`src/editor_bridge.rs`)
- Testy mostu do edytora

### 3. Utilities Testowe
- **test_helpers.rs** - funkcje pomocnicze do testów
- **fixtures/** - dane testowe i fixturey

## Uruchamianie Testów

### Wszystkie testy
```bash
cargo test
```

### Testy jednostkowe
```bash
cargo test --lib
```

### Testy integracyjne
```bash
cargo test --test integration_tests
cargo test --test security_tests
cargo test --test load_tests
```

### Testy z logowaniem
```bash
cargo test -- --nocapture
```

### Testy równoległe/sekwencyjne
```bash
cargo test -- --test-threads=1  # sekwencyjnie
cargo test -- --test-threads=4  # 4 wątki
```

### Testy konkretnego modułu
```bash
cargo test --lib apply_system::analysis
cargo test --lib apply_system::matchers
```

## Pokrycie Testami

Aby zmierzyć pokrycie testami, użyj `tarpaulin`:

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage
```

## Konwencje Testowe

### Nazewnictwo
- Testy jednostkowe: `test_<funkcjonalność>`
- Testy pozytywne: `test_<funkcjonalność>_succeeds`
- Testy negatywne: `test_<funkcjonalność>_fails_with_<warunek>`
- Testy brzegowe: `test_<funkcjonalność>_edge_case_<scenariusz>`

### Struktura testu
```rust
#[test]
fn test_example() {
    // Arrange - przygotowanie danych
    let input = "test data";

    // Act - wywołanie funkcji
    let result = function_under_test(input);

    // Assert - sprawdzenie wyniku
    assert_eq!(result, expected);
}
```

### Używanie tempfile dla testów I/O
```rust
use tempfile::TempDir;

#[test]
fn test_with_temp_files() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    // ... test code ...
}
```

## Zależności Testowe

Dodatkowe zależności w `[dev-dependencies]`:
- `tempfile` - tymczasowe pliki i katalogi
- `mockall` - mockowanie (TODO: dodać)
- `proptest` - property-based testing (TODO: dodać)
- `criterion` - benchmarking (TODO: dodać)

## TODO

### Priorytet Wysoki
- [ ] Rozszerzyć testy jednostkowe dla wszystkich modułów
- [ ] Dodać testy API dla wszystkich komend Tauri
- [ ] Stworzyć testy wydajnościowe dla krytycznych ścieżek
- [ ] Dodać integration tests dla pełnego flow użytkownika

### Priorytet Średni
- [ ] Dodać property-based testing (proptest)
- [ ] Stworzyć benchmarki (criterion)
- [ ] Dodać mockowanie (mockall)
- [ ] Testy fuzz testing dla parserów

### Priorytet Niski
- [ ] Testy snapshot testing
- [ ] Testy A/B dla różnych algorytmów matchingu
- [ ] Continuous integration z coverage reporting

## Metryki Testów

Cel: **80%+ pokrycie kodu testami**

Aktualny status:
- [ ] Analysis: 0% → target 80%
- [ ] Context: 0% → target 80%
- [ ] Matchers: 20% → target 80%
- [ ] Parsers: 30% → target 80%
- [ ] Validators: 0% → target 80%
- [ ] Lazy: 40% → target 80%
- [ ] Main modules: 10% → target 80%
- [ ] Local AI: 0% → target 80%
- [ ] Tauri Commands: 0% → target 80%

## Raportowanie Błędów w Testach

Gdy test wykryje błąd:
1. Upewnij się, że test jest odtwarzalny
2. Zapisz minimalny przykład odtwarzający błąd
3. Zgłoś issue z tagiem `bug` i `testing`
4. Dodaj test regresyjny zapobiegający powrotowi błędu
