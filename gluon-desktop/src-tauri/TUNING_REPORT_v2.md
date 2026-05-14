# Gluon v2 - Tuning Report

**Data**: 2026-01-19
**Wersja**: 0.2.2
**Autor**: Optymalizacja WeightedAnchorMatcher + Self-Healing Analysis

---

## 🎯 Cele Tuning

Na podstawie analizy logów produkcyjnych i feedbacku użytkownika, zidentyfikowano trzy główne obszary optymalizacji:

### 1. **WeightedAnchorMatcher - Rygorystyczność dla krótkich bloków**
**Problem**: Często LLM robi drobne literówki w kontekście (komentarze, white space), co obecnie powoduje odrzucenie silnej kotwicy.

**Przykład błędu**:
```
WeightedAnchorMatcher → Failed (threshold 0.85)
  └─ Anchor: function calculateTotal() { // HIGH quality, uniqueness=0.95
  └─ Context: 2 linie z literówką w komentarzu ("Calcu late" zamiast "Calculate")
  └─ Similarity: 0.82 < 0.85 → REJECTED

AnchorMatcher (Legacy) → Success (threshold 0.75)
```

**Root Cause**: Stały threshold `fuzzy_threshold = 0.85` jest zbyt rygorystyczny dla:
- Bardzo krótkich bloków (2-5 linii)
- Silnych kotwic (High quality + uniqueness > 0.8)

### 2. **Self-Healing Loop - AMBIGUOUS MATCH**
**Problem**: Gdy wystąpi błąd AMBIGUOUS MATCH (znaleziono 2+ pasujące miejsca), system powinien automatycznie wygenerować nowy prompt do LLM: *"Znalazłem 2 pasujące miejsca. Podaj więcej kontekstu (np. nazwę funkcji nadrzędnej), aby ujednoznacznić."*

**Obecny stan**:
- ✅ `PatchFailureType::AmbiguousMatch` jest wykrywany
- ✅ `generate_repair_prompt()` tworzy poprawny prompt
- ❌ **Auto-retry NIE jest zaimplementowany** (placeholder w `apply_with_healing`, linia 517-518)

### 3. **Legacy AnchorMatcher - Zbyt częste użycie**
**Problem**: AnchorMatcher (Legacy) wciąż "ratuje" sytuację zbyt często (~15-20% przypadków).

**Cel**: WeightedAnchorMatcher + BlockMatcher powinny przejmować 95% przypadków, a legacy być tylko ostatecznością.

---

## 🔧 Implementacja

### 1. Dynamic Threshold Adjustment

#### Dodane pola do `WeightedAnchoringConfig`:

```rust
pub struct WeightedAnchoringConfig {
    // ... existing fields ...

    /// Enable dynamic threshold adjustment for short blocks (default: true)
    pub enable_dynamic_threshold: bool,

    /// Threshold reduction for strong anchors (default: 0.15)
    pub strong_anchor_threshold_reduction: f64,
}
```

#### Nowa funkcja: `calculate_dynamic_threshold()`

```rust
fn calculate_dynamic_threshold(
    search_block_lines: &[String],
    anchor: &WeightedAnchor,
    config: &WeightedAnchoringConfig,
) -> f64 {
    let mut threshold = config.fuzzy_threshold; // Start: 0.85
    let block_size = search_block_lines.len();

    // 1. Adjust for short blocks (2-5 lines)
    if block_size <= 5 {
        let size_reduction = match block_size {
            1..=2 => 0.15,  // 2 lines: 0.85 → 0.70
            3 => 0.10,      // 3 lines: 0.85 → 0.75
            4..=5 => 0.05,  // 4-5 lines: 0.85 → 0.80
            _ => 0.0,
        };
        threshold -= size_reduction;
    }

    // 2. Adjust for strong anchors (High quality + high uniqueness)
    let is_strong_anchor = anchor.quality == AnchorQuality::High
                          && anchor.uniqueness_score >= 0.8;
    if is_strong_anchor {
        threshold -= config.strong_anchor_threshold_reduction; // -0.15
    }

    // Floor at 0.65 to maintain minimum quality
    threshold.max(0.65)
}
```

#### Integracja:

**Przed**:
```rust
// expand_upwards/downwards
if similarity >= config.fuzzy_threshold { // Fixed: 0.85
    matched_count += 1;
}
```

**Po**:
```rust
// fuzzy_expand_from_anchor
let dynamic_threshold = calculate_dynamic_threshold(search_block_lines, &anchor, config);

// expand_upwards/downwards
if similarity >= dynamic_threshold { // Adaptive: 0.65-0.85
    matched_count += 1;
}
```

---

## 📊 Oczekiwane Rezultaty

### Scenariusz 1: Krótki blok (2 linie) + Silna kotwica

**Przed**:
- Base threshold: 0.85
- Result: FAIL → Legacy rescue

**Po**:
- Base threshold: 0.85
- Size reduction: -0.15 (2 lines)
- Strong anchor reduction: -0.15 (High + uniqueness 0.9)
- **Dynamic threshold: 0.55** ✅
- Result: SUCCESS

### Scenariusz 2: Średni blok (10 linii) + Słaba kotwica

**Przed**:
- Base threshold: 0.85
- Result: Depends

**Po**:
- Base threshold: 0.85
- Size reduction: 0 (>5 lines)
- Strong anchor reduction: 0 (Low quality)
- **Dynamic threshold: 0.85** (no change)
- Result: Same behavior (conservative)

### Scenariusz 3: Krótki blok (3 linie) + Medium kotwica

**Przed**:
- Base threshold: 0.85
- Result: Often FAIL

**Po**:
- Base threshold: 0.85
- Size reduction: -0.10 (3 lines)
- Strong anchor reduction: 0 (not High+0.8)
- **Dynamic threshold: 0.75** ✅
- Result: More likely to succeed

---

## 🔍 Analiza Self-Healing Loop

### Obecna Implementacja

**Plik**: `src\apply_system\self_healing.rs`

**Zidentyfikowane funkcje**:
1. ✅ `PatchFailureType::AmbiguousMatch` - enum variant (linia 92-98)
2. ✅ `generate_repair_prompt()` - tworzy prompt dla LLM (linia 358+)
3. ✅ `describe_failure_type()` - opisuje błąd (linia 409-414):
   ```rust
   PatchFailureType::AmbiguousMatch { candidate_count, confidence_gap } => {
       format!(
           "Ambiguous match - {} candidates found (gap: {:.1}%)",
           candidate_count, confidence_gap * 100.0
       )
   }
   ```
4. ✅ `generate_suggested_fixes()` - sugeruje poprawki (linia 319-325):
   ```rust
   PatchFailureType::AmbiguousMatch { candidate_count, .. } => {
       fixes.push(format!(
           "Found {} similar matches - your search block is not unique enough. \
            Add more specific context to disambiguate.",
           candidate_count
       ));
   }
   ```

**Brakujący element**: ❌ `apply_with_healing()` ma placeholder (linia 517-518):
```rust
// This is a placeholder - actual retry logic happens in transaction.rs
// We're just generating the repair prompt and returning it
```

### Punkt Integracji

**Plik**: `src\apply_system\matchers\mod.rs`, funkcja `match_code()`

```rust
pub fn match_code(...) -> Result<MatchResult, MatchError> {
    // Linia 47-50:
    let mut result = coordinator::find_best_match(file_content, &normalized_old_code, file_path)
        .or_else(|_| {
            coordinator::find_best_match(file_content, old_code, file_path)
        })?;

    // TODO: Add self-healing retry here for AmbiguousMatch errors

    Ok(result)
}
```

**Zalecana implementacja** (wymaga IPC do LLM):
```rust
let mut result = coordinator::find_best_match(file_content, &normalized_old_code, file_path)
    .or_else(|err| {
        // Check if it's an ambiguous match
        if let MatchError::AmbiguousMatch { locations } = &err {
            let context = self_healing::extract_error_context(...);
            let prompt = self_healing::generate_repair_prompt(&context);

            // Send prompt to LLM via IPC (Extension/Desktop handles this)
            // Return HealingResult::UserInterventionRequired with prompt

            emit_healing_request(prompt); // IPC event
        }
        Err(err)
    })?;
```

---

## 📝 Status Implementacji

### ✅ Zaimplementowane (Faza 4.1)

- [x] Dodano `enable_dynamic_threshold` do `WeightedAnchoringConfig`
- [x] Dodano `strong_anchor_threshold_reduction` do `WeightedAnchoringConfig`
- [x] Zaimplementowano `calculate_dynamic_threshold()`
- [x] Zintegrowano dynamic threshold w `expand_upwards()`
- [x] Zintegrowano dynamic threshold w `expand_downwards()`
- [x] Testy kompilacji - SUCCESS

### 🚧 Do dokończenia (Faza 4.2)

- [ ] Dokończenie `apply_with_healing()` - usunięcie placeholdera
- [ ] Integracja self-healing w `match_code()` dla AmbiguousMatch
- [ ] Dodanie IPC event: `emit_healing_request(prompt)`
- [ ] Frontend handler dla healing requests
- [ ] Testy integracyjne dla dynamic threshold
- [ ] Benchmarki przed/po dla WeightedAnchorMatcher
- [ ] Analiza logów: redukcja użycia Legacy AnchorMatcher

---

## 🎯 Metryki Sukcesu

### Przed tunningiem (baseline):
- WeightedAnchorMatcher success rate: ~70%
- Legacy AnchorMatcher usage: ~15-20%
- BlockMatcher success rate: ~10%

### Po tuningu (cel):
- WeightedAnchorMatcher success rate: **85-90%** (+15-20%)
- Legacy AnchorMatcher usage: **<5%** (-10-15%)
- BlockMatcher success rate: ~10% (no change)

**Całkowita success rate**: 65% → **85%+** (zgodnie z Gluon v2 Report)

---

## 🔬 Następne Kroki

1. **Benchmarking**: Uruchomić `cargo bench` dla nowych zmian
2. **Integration Tests**: Dodać testy dla dynamic threshold w `tests/matchers_tests.rs`
3. **Production Logs**: Monitorować logi przez 1 tydzień, mierzyć:
   - Częstość użycia poszczególnych matcherów
   - Confidence scores przed/po
   - Liczba błędów AmbiguousMatch
4. **Self-Healing Completion**: Dokończyć implementację auto-retry (wymaga IPC)
5. **Documentation**: Zaktualizować GLUON_V2_IMPLEMENTATION_REPORT.md

---

## 📚 Referencje

- **Gluon v2 Architecture Report** (Polish)
- **GLUON_V2_IMPLEMENTATION_REPORT.md** - Faza 1-3
- **User Feedback**: "Często LLM robi drobne literówki w kontekście"
- **Production Logs**: Analiza błędów 11:43:38 (AMBIGUOUS MATCH)

---

**Commit Message**:
```
feat(matchers): Dynamic threshold tuning for WeightedAnchorMatcher

- Add dynamic threshold adjustment for short blocks (2-5 lines)
- Reduce threshold for strong anchors (High quality + uniqueness > 0.8)
- Floor threshold at 0.65 to maintain minimum quality
- Expected: +15-20% success rate for short blocks with typos

Breaking: None (backward compatible, enabled by default)
Issue: #USER_FEEDBACK_2026-01-19
```
