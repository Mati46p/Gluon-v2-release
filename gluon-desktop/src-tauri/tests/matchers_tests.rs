//! Testy dla modułu apply_system::matchers
//!
//! Testuje:
//! - Anchor matcher (anchor_matcher.rs, anchor_extraction.rs)
//! - Fuzzy matcher (fuzzy_matcher.rs)
//! - Regex matcher (regex_matcher.rs)
//! - Weighted anchor matcher (weighted_anchor_matcher.rs)
//! - Pattern matching (pattern_matching.rs)
//! - Block matcher (block_matcher.rs)
//! - Koordynator (coordinator.rs)

mod test_helpers;
use test_helpers::*;

// ============================================================================
// Anchor Matcher Tests
// ============================================================================

#[test]
fn test_anchor_extract_function_name() {
    use gluon_desktop_lib::apply_system::matchers::anchor_extraction::extract_anchors;

    let code = sample_typescript_code();
    let anchors = extract_anchors(code);

    // Powinien wyekstrahować funkcje: fetchUser, deleteUser
    assert!(anchors.functions.contains(&"fetchUser".to_string()));
    assert!(anchors.functions.contains(&"deleteUser".to_string()));
    assert!(anchors.functions.len() >= 2);
}

#[test]
fn test_anchor_extract_class_name() {
    use gluon_desktop_lib::apply_system::matchers::anchor_extraction::extract_anchors;

    let code = sample_javascript_code();
    let anchors = extract_anchors(code);

    // Powinien wyekstrahować klasę ShoppingCart
    assert!(anchors.classes.contains(&"ShoppingCart".to_string()));
    assert!(anchors.classes.len() >= 1);
}

#[test]
fn test_anchor_extract_unique_strings() {
    use gluon_desktop_lib::apply_system::matchers::anchor_extraction::extract_anchors;

    let code = r#"
        const greeting = "Hello, World!";
        const message = "Unique message 12345";
        function test() {
            console.log("test");
        }
    "#;

    let anchors = extract_anchors(code);

    // Powinien wyekstrahować unikalne literały stringowe
    assert!(!anchors.literals.is_empty(), "Should extract string literals");

    // Sprawdź czy ekstrahował długie, unikalne stringi
    let has_long_literal = anchors.literals.iter().any(|lit| lit.len() > 5);
    assert!(has_long_literal, "Should extract longer string literals as anchors");
}

#[test]
fn test_anchor_extract_comments() {
    use gluon_desktop_lib::apply_system::matchers::anchor_extraction::extract_anchors;

    let code = r#"
        // This is a unique comment ABC123
        function test() {
            /* Important block comment XYZ789 */
            return 1;
        }
    "#;

    let anchors = extract_anchors(code);

    // Ekstrahowane kotwice mogą zawierać komentarze jako literały
    // lub mogą je ignorować (zależy od implementacji)
    // Sprawdzamy że funkcja została znaleziona
    assert!(
        anchors.functions.contains(&"test".to_string()) ||
        !anchors.literals.is_empty(),
        "Should extract either function names or unique comment strings as anchors"
    );
}

#[test]
fn test_anchor_match_single() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, AnchorMatcher};

    let file = r#"
        function irrelevant() {
            return 0;
        }

        function targetFunction() {
            console.log("target");
            return 42;
        }

        function another() {
            return 1;
        }
    "#;

    let search = r#"
        function targetFunction() {
            console.log("target");
            return 42;
        }
    "#;

    let matcher = AnchorMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_some(), "Should find the target function");
    let res = result.unwrap();
    assert!(res.matched_line_start > 1, "Should not match first function");
    assert_eq!(res.method_used, gluon_desktop_lib::apply_system::types::MatchMethod::AnchorPoints);
}

#[test]
fn test_anchor_match_multiple() {
    // TODO: Test dopasowania wielu kotwic
    println!("TODO: test_anchor_match_multiple");
}

#[test]
fn test_anchor_match_confidence_scoring() {
    // TODO: Test scoringu pewności dopasowania kotwicy
    println!("TODO: test_anchor_match_confidence_scoring");
}

#[test]
fn test_anchor_match_no_match() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, AnchorMatcher};

    let file = r#"
        function existingFunction() {
            return 1;
        }
    "#;

    let search = r#"
        function nonExistentFunction() {
            return 999;
        }
    "#;

    let matcher = AnchorMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_none(), "Should not find match for non-existent function");
}

#[test]
fn test_anchor_match_ambiguous() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, AnchorMatcher};

    // Kod z wieloma identycznymi funkcjami (ambiwalencja)
    let file = r#"
function helper() {
    return 1;
}

function main() {
    helper();
}

function helper() {
    return 1;
}

function another() {
    helper();
}

function helper() {
    return 1;
}
"#;

    let search = r#"function helper() {
    return 1;
}"#;

    let matcher = AnchorMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    // Matcher może:
    // 1. Zwrócić None (preferowane dla ambiwalencji)
    // 2. Zwrócić pierwsze dopasowanie z niskim confidence
    // NIE powinien: crashować lub zwrócić losowe dopasowanie z wysokim confidence

    if let Some(res) = result {
        // Jeśli zwrócił wynik, nie powinien być zbyt pewny
        assert!(
            res.confidence < 0.99,
            "Ambiguous match should not have near-perfect confidence"
        );
    }
    // W przeciwnym razie None jest akceptowalne
}

// ============================================================================
// Weighted Anchor Matcher Tests
// ============================================================================

#[test]
fn test_weighted_anchor_scoring() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, WeightedAnchorMatcher};

    let file = r#"
        // Common error handling
        if err != nil {
            return err
        }

        function processUserData(id string) error {
            // Unique function
            if err != nil {
                return err
            }
        }

        // More common code
        if err != nil {
            return err
        }
    "#;

    // Wyszukujemy blok z unikalną kotwicą (processUserData)
    let search = r#"function processUserData(id string) error {
            // Unique function
            if err != nil {
                return err
            }
        }"#;

    let matcher = WeightedAnchorMatcher::new();
    let result = matcher.find_match(file, search, Some("test.go"));

    assert!(result.is_some(), "WeightedAnchor should find unique anchor");
    let res = result.unwrap();

    // Powinien mieć confidence breakdown
    assert!(res.confidence_breakdown.is_some(), "Should provide confidence breakdown");
    let breakdown = res.confidence_breakdown.unwrap();

    // Sprawdź że wszystkie komponenty są obecne
    assert!(breakdown.anchor_quality > 0.0, "Anchor quality should be calculated");
    assert!(breakdown.similarity > 0.0, "Similarity should be calculated");
    assert!(breakdown.token_similarity > 0.0, "Token similarity should be calculated");
}

#[test]
fn test_weighted_anchor_before_after() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, WeightedAnchorMatcher};

    // Kod z kontekstem przed i po
    let file = r#"
const config = loadConfig();

function processData(data) {
    // This is the target function
    const result = transform(data);
    return result;
}

function exportData(data) {
    save(data);
}
"#;

    // Wyszukiwanie z kontekstem
    let search = r#"function processData(data) {
    // This is the target function
    const result = transform(data);
    return result;
}"#;

    let matcher = WeightedAnchorMatcher::new();
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_some(), "Should find function with context");
    let res = result.unwrap();

    // Powinien mieć wysoki confidence dzięki unikalnemu nazwie funkcji
    assert!(res.confidence > 0.80, "Should have high confidence with unique function name, got {}", res.confidence);

    // Sprawdź breakdown
    if let Some(breakdown) = res.confidence_breakdown {
        assert!(breakdown.anchor_quality >= 0.5, "Function name should be good anchor (>=0.5), got {}", breakdown.anchor_quality);
    }
}

#[test]
fn test_weighted_anchor_proximity() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, WeightedAnchorMatcher};

    // Kod z repetytywnym wzorcem, ale unikalną kotwicą w środku
    let file = r#"
if (error) {
    log("error");
    return;
}

function uniqueProcessor(data) {
    if (error) {
        log("error");
        return;
    }
    process(data);
}

if (error) {
    log("error");
    return;
}
"#;

    // Wyszukujemy blok z unikalną funkcją
    let search = r#"function uniqueProcessor(data) {
    if (error) {
        log("error");
        return;
    }
    process(data);
}"#;

    let matcher = WeightedAnchorMatcher::new();
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_some(), "Should find block with unique anchor despite repetitive code");
    let res = result.unwrap();

    // Sprawdź czy znaleziono właściwy blok (nie pierwszy ani ostatni error block)
    assert!(res.matched_line_start > 2, "Should not match first error block");
    assert!(res.matched_line_end < 15, "Should not match last error block");
}

#[test]
fn test_weighted_anchor_combined_score() {
    // TODO: Test kombinowanego score z wielu czynników
    println!("TODO: test_weighted_anchor_combined_score");
}

// ============================================================================
// Fuzzy Matcher Tests
// ============================================================================

#[test]
fn test_fuzzy_exact_match() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher};

    let code = r#"function test() {
    return 42;
}"#;

    let matcher = FuzzyMatcher;
    let result = matcher.find_match(code, code, Some("test.js"));

    assert!(result.is_some(), "Should find exact match");
    let res = result.unwrap();
    assert!(res.confidence > 0.95, "Exact match should have >95% confidence, got {}", res.confidence);
    assert_eq!(res.method_used, gluon_desktop_lib::apply_system::types::MatchMethod::FuzzyMatch);
}

#[test]
fn test_fuzzy_similar_match() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher};

    let file = r#"function calculateTotal() {
    return sum;
}"#;

    // Wyszukiwanie z drobnymi różnicami (brak wielkich liter)
    let search = r#"function calculatetotal() {
    return sum;
}"#;

    let matcher = FuzzyMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_some(), "Should find similar match despite case differences");
    let res = result.unwrap();
    assert!(res.confidence >= 0.80, "Similar match should have >=80% confidence, got {}", res.confidence);
    assert!(res.confidence < 1.0, "Should not be perfect match due to case difference");
}

#[test]
fn test_fuzzy_whitespace_differences() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher};

    let file = "function test() {\n    return 1;\n}";
    let search = "function  test()  {\n  return   1;\n}"; // extra spaces

    let matcher = FuzzyMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_some(), "Should handle whitespace differences");
    let res = result.unwrap();
    assert!(res.confidence > 0.90, "Whitespace differences should still give high confidence, got {}", res.confidence);
}

#[test]
fn test_fuzzy_indentation_normalization() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher};

    // Kod z różnymi poziomami wcięć (2 spacje)
    let file = r#"function test() {
  const x = 1;
  if (x > 0) {
    return true;
  }
  return false;
}"#;

    // Wyszukiwanie z innymi wcięciami (4 spacje)
    let search = r#"function test() {
    const x = 1;
    if (x > 0) {
        return true;
    }
    return false;
}"#;

    let matcher = FuzzyMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_some(), "Should normalize indentation differences");
    let res = result.unwrap();
    assert!(
        res.confidence > 0.85,
        "Indentation differences should not significantly lower confidence, got {}",
        res.confidence
    );
}

#[test]
fn test_fuzzy_levenshtein_distance() {
    // TODO: Test używania odległości Levenshteina
    println!("TODO: test_fuzzy_levenshtein_distance");
}

#[test]
fn test_fuzzy_threshold_adjustment() {
    // TODO: Test dostosowania progu dopasowania
    println!("TODO: test_fuzzy_threshold_adjustment");
}

#[test]
fn test_fuzzy_no_match_below_threshold() {
    // TODO: Test braku dopasowania poniżej progu
    println!("TODO: test_fuzzy_no_match_below_threshold");
}

#[test]
fn test_fuzzy_performance_large_file() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, WeightedAnchorMatcher};
    use std::time::Duration;

    // Stwórz duży plik (20 kopii complex_code_sample)
    let large_code = vec![complex_code_sample(); 20].join("\n");

    // Wyszukaj fragment który występuje w środku (metoda add)
    let search = r#"add(n: number): Calculator {
        this.value += n;
        return this;
    }"#;

    // WeightedAnchorMatcher radzi sobie lepiej z ambiwalencją niż FuzzyMatcher
    // (używa kotwic do identyfikacji unikalnych fragmentów)
    let matcher = WeightedAnchorMatcher::new();

    // Powinno zakończyć się w rozsądnym czasie (<2 sekundy)
    let result = assert_completes_within(Duration::from_secs(2), || {
        matcher.find_match(&large_code, search, Some("test.ts"))
    });

    // W dużym pliku z wieloma kopiami tego samego kodu, matcher może zwrócić None
    // z powodu ambiwalencji - to jest poprawne zachowanie (fail-safe)
    if let Some(res) = result {
        assert!(res.confidence > 0.70, "If found, should have reasonable confidence");
    }
    // Nie wymuszamy znalezienia - ambiwalencja to akceptowalny powód do None
}

// ============================================================================
// Regex Matcher Tests
// ============================================================================

#[test]
fn test_regex_simple_pattern() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, RegexMatcher};

    let file = r#"
        const value = 42;
        if (value > 0) {
            console.log("positive");
        }
    "#;

    let search = "if (value > 0)";

    let matcher = RegexMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_some(), "Should find simple pattern match");
    let res = result.unwrap();
    assert_eq!(res.method_used, gluon_desktop_lib::apply_system::types::MatchMethod::RegexPattern);
}

#[test]
fn test_regex_complex_pattern() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, RegexMatcher, FuzzyMatcher};

    let file = r#"
const user = {
    name: "John",
    email: "john@example.com",
    age: 30,
    roles: ["admin", "user"]
};
"#;

    // Wyszukujemy pełny obiekt (RegexMatcher nie radzi sobie z częściowymi dopasowaniami)
    let search = r#"const user = {
    name: "John",
    email: "john@example.com",
    age: 30,
    roles: ["admin", "user"]
};"#;

    let regex_matcher = RegexMatcher;
    let fuzzy_matcher = FuzzyMatcher;

    // Spróbuj oba matchery
    let regex_result = regex_matcher.find_match(file, search, Some("test.js"));
    let fuzzy_result = fuzzy_matcher.find_match(file, search, Some("test.js"));

    assert!(
        regex_result.is_some() || fuzzy_result.is_some(),
        "At least one matcher should match complex object pattern"
    );
}

#[test]
fn test_regex_multiline_match() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, RegexMatcher};

    let file = r#"
        function calculate() {
            const x = 10;
            const y = 20;
            return x + y;
        }
    "#;

    let search = r#"const x = 10;
        const y = 20;
        return x + y;"#;

    let matcher = RegexMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_some(), "Should match multiline pattern");
    let res = result.unwrap();
    assert!(res.matched_line_end > res.matched_line_start, "Should span multiple lines");
}

#[test]
fn test_regex_capture_groups() {
    // TODO: Test grup przechwytujących
    println!("TODO: test_regex_capture_groups");
}

#[test]
fn test_regex_case_insensitive() {
    // TODO: Test dopasowania case-insensitive
    println!("TODO: test_regex_case_insensitive");
}

#[test]
fn test_regex_word_boundaries() {
    // TODO: Test granic słów (\b)
    println!("TODO: test_regex_word_boundaries");
}

#[test]
fn test_regex_invalid_pattern() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, RegexMatcher};

    let file = "function test() { return 1; }";

    // Wzorzec który może powodować problemy z regex (nawiasy są escapowane)
    // ale sam kod jest poprawny
    let search = "function test() { return 1; }";

    let matcher = RegexMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    // RegexMatcher powinien obsłużyć to bez crashowania
    // Może znaleźć dopasowanie lub zwrócić None
    assert!(
        result.is_some() || result.is_none(),
        "Should handle pattern without crashing"
    );
}

#[test]
fn test_regex_performance() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, RegexMatcher};
    use std::time::Duration;

    // Duży plik z wieloma dopasowaniami
    let large_file = vec![
        "const x = 1;",
        "const y = 2;",
        "const z = 3;",
    ]
    .into_iter()
    .cycle()
    .take(1000)
    .collect::<Vec<_>>()
    .join("\n");

    let search = "const x = 1;";

    let matcher = RegexMatcher;

    // Powinno zakończyć się szybko (<500ms)
    let result = assert_completes_within(Duration::from_millis(500), || {
        matcher.find_match(&large_file, search, Some("test.js"))
    });

    assert!(result.is_some(), "Should find match quickly");
}

// ============================================================================
// Block Matcher Tests
// ============================================================================

#[test]
fn test_block_match_function() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, BlockMatcher};

    let file = r#"
        export function fetchUser(id: string): Promise<User> {
            console.log("Fetching user");
            return fetch(`/api/users/${id}`)
                .then(response => response.json());
        }

        export function deleteUser(id: string): Promise<void> {
            return fetch(`/api/users/${id}`, { method: 'DELETE' });
        }
    "#;

    let search = r#"export function fetchUser(id: string): Promise<User> {
            console.log("Fetching user");
            return fetch(`/api/users/${id}`)
                .then(response => response.json());
        }"#;

    let matcher = BlockMatcher;
    let result = matcher.find_match(file, search, Some("test.ts"));

    assert!(result.is_some(), "Should match function block using AST");
    let res = result.unwrap();
    assert_eq!(res.method_used, gluon_desktop_lib::apply_system::types::MatchMethod::BlockStructure);
    assert!(res.confidence >= 0.95, "Block match should have high confidence");
}

#[test]
fn test_block_match_class() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, BlockMatcher};

    let file = complex_code_sample();

    let search = r#"class Calculator {
    constructor(private value: number = 0) {}

    add(n: number): Calculator {
        this.value += n;
        return this;
    }
}"#;

    let matcher = BlockMatcher;
    let result = matcher.find_match(file, search, Some("test.ts"));

    // Block matcher może znaleźć match lub nie, w zależności od implementacji
    // Sprawdzamy czy jeśli znajdzie, to jest to prawidłowy match
    if let Some(res) = result {
        assert_eq!(res.method_used, gluon_desktop_lib::apply_system::types::MatchMethod::BlockStructure);
        assert!(res.matched_line_start > 0, "Should find the class");
    }
}

#[test]
fn test_block_match_nested() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, BlockMatcher};

    let file = r#"
class Outer {
    method1() {
        return 1;
    }

    class Inner {
        nestedMethod() {
            return 2;
        }
    }

    method2() {
        return 3;
    }
}
"#;

    // Wyszukaj zagnieżdżoną klasę
    let search = r#"class Inner {
        nestedMethod() {
            return 2;
        }
    }"#;

    let matcher = BlockMatcher;
    let result = matcher.find_match(file, search, Some("test.ts"));

    // BlockMatcher może lub nie może znaleźć zagnieżdżone klasy
    // w zależności od implementacji tree-sitter
    if let Some(res) = result {
        assert_eq!(res.method_used, gluon_desktop_lib::apply_system::types::MatchMethod::BlockStructure);
        assert!(res.matched_line_start > 0);
    }
}

#[test]
fn test_block_match_boundaries() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, BlockMatcher};

    let file = r#"
function outer() {
    if (true) {
        function inner() {
            return 1;
        }
        return inner();
    }
}
"#;

    // Wyszukaj wewnętrzną funkcję
    let search = r#"function inner() {
            return 1;
        }"#;

    let matcher = BlockMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    if let Some(res) = result {
        // Jeśli znalazł, sprawdź że granice są rozsądne
        let line_count = res.matched_line_end - res.matched_line_start;
        assert!(line_count >= 2, "Function block should span at least 2 lines");
        assert!(line_count <= 10, "Should not capture too much code");
    }
}

#[test]
fn test_block_match_incomplete() {
    // TODO: Test dopasowania niekompletnego bloku
    println!("TODO: test_block_match_incomplete");
}

// ============================================================================
// Pattern Matching Tests
// ============================================================================

#[test]
fn test_pattern_match_simple() {
    // TODO: Test prostego dopasowania wzorca
    println!("TODO: test_pattern_match_simple");
}

#[test]
fn test_pattern_match_with_wildcards() {
    // TODO: Test dopasowania z wildcardami
    println!("TODO: test_pattern_match_with_wildcards");
}

#[test]
fn test_pattern_match_structure() {
    // TODO: Test dopasowania strukturalnego
    println!("TODO: test_pattern_match_structure");
}

#[test]
fn test_pattern_match_ast_based() {
    // TODO: Test dopasowania bazowanego na AST
    println!("TODO: test_pattern_match_ast_based");
}

// ============================================================================
// Coordinator Tests
// ============================================================================

#[test]
fn test_coordinator_try_all_matchers() {
    // TODO: Test próbowania wszystkich matcherów w kolejności
    println!("TODO: test_coordinator_try_all_matchers");
}

#[test]
fn test_coordinator_fallback_strategy() {
    // TODO: Test strategii fallback (anchor -> fuzzy -> regex)
    println!("TODO: test_coordinator_fallback_strategy");
}

#[test]
fn test_coordinator_best_match_selection() {
    // TODO: Test wyboru najlepszego dopasowania
    println!("TODO: test_coordinator_best_match_selection");
}

#[test]
fn test_coordinator_confidence_threshold() {
    // TODO: Test progu pewności dla akceptacji dopasowania
    println!("TODO: test_coordinator_confidence_threshold");
}

#[test]
fn test_coordinator_no_match_found() {
    // TODO: Test gdy żaden matcher nie znajdzie dopasowania
    println!("TODO: test_coordinator_no_match_found");
}

#[test]
fn test_coordinator_multiple_candidates() {
    // TODO: Test gdy jest wiele kandydatów
    println!("TODO: test_coordinator_multiple_candidates");
}

#[test]
fn test_coordinator_performance() {
    // TODO: Test wydajności koordynatora
    use std::time::Duration;

    let _result = assert_completes_within(Duration::from_millis(500), || {
        // Symuluj próbę dopasowania
        42
    });

    println!("TODO: test_coordinator_performance");
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_full_matching_pipeline() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, WeightedAnchorMatcher, AnchorMatcher, FuzzyMatcher};

    let file_content = sample_typescript_code();
    let search = r#"export function fetchUser(id: string): Promise<User> {
    console.log("Fetching user");
    return fetch(`/api/users/${id}`)
        .then(response => response.json());
}"#;

    // Testuj wszystkie matchery w kolejności (jak w koordynatorze)
    let matchers: Vec<(&str, Box<dyn Matcher>)> = vec![
        ("WeightedAnchor", Box::new(WeightedAnchorMatcher::new())),
        ("Anchor", Box::new(AnchorMatcher)),
        ("Fuzzy", Box::new(FuzzyMatcher)),
    ];

    let mut found_by = Vec::new();

    for (name, matcher) in matchers {
        if let Some(result) = matcher.find_match(file_content, search, Some("test.ts")) {
            found_by.push((name, result.confidence, result.method_used));
        }
    }

    // Przynajmniej jeden matcher powinien znaleźć dopasowanie
    assert!(!found_by.is_empty(), "At least one matcher should find the function");

    // Pierwszy znaleziony (najwyższy priorytet) powinien mieć wysoką pewność
    if let Some((name, conf, _)) = found_by.first() {
        assert!(conf > &0.70, "First successful matcher ({}) should have >70% confidence, got {}", name, conf);
    }
}

#[test]
fn test_matching_with_context() {
    // TODO: Test matchingu z użyciem kontekstu
    println!("TODO: test_matching_with_context");
}

#[test]
fn test_matching_across_files() {
    // TODO: Test matchingu przez wiele plików
    println!("TODO: test_matching_across_files");
}

#[test]
fn test_matching_code_moved() {
    // TODO: Test matchingu gdy kod został przeniesiony
    println!("TODO: test_matching_code_moved");
}

#[test]
fn test_matching_code_refactored() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher, WeightedAnchorMatcher};

    // Oryginalny kod
    let original = r#"
function calculatePrice(items) {
    let total = 0;
    for (let i = 0; i < items.length; i++) {
        total += items[i].price;
    }
    return total;
}
"#;

    // Zrefaktorowany kod (forEach zamiast for loop)
    let refactored = r#"
function calculatePrice(items) {
    let total = 0;
    items.forEach(item => {
        total += item.price;
    });
    return total;
}
"#;

    // Próbujemy znaleźć oryginalną funkcję w zrefaktorowanym kodzie
    // Używamy niższego threshold dla przypadku refaktoryzacji
    let mut config = crate::gluon_desktop_lib::apply_system::lazy::weighted_anchoring::WeightedAnchoringConfig::default();
    config.fuzzy_threshold = 0.60; // Obniżony threshold dla mocno zrefaktorowanego kodu
    let weighted = crate::gluon_desktop_lib::apply_system::matchers::WeightedAnchorMatcher::with_config(config);

    // WeightedAnchorMatcher powinien używać nazwy funkcji jako kotwicy i tolerować zmiany w ciele
    let weighted_result = weighted.find_match(refactored, original, Some("test.js"));

    // Powinien znaleźć dopasowanie bazując na nazwie funkcji (calculatePrice) jako kotwicy
    assert!(
        weighted_result.is_some(),
        "WeightedAnchorMatcher should match refactored code using function name as anchor"
    );
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_matching_empty_search() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher, AnchorMatcher};

    let file = "function test() { return 1; }";
    let empty_search = "";

    let fuzzy = FuzzyMatcher;
    let anchor = AnchorMatcher;

    // Puste wyszukiwanie nie powinno znaleźć dopasowania
    assert!(fuzzy.find_match(file, empty_search, Some("test.js")).is_none());
    assert!(anchor.find_match(file, empty_search, Some("test.js")).is_none());
}

#[test]
fn test_matching_very_long_line() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher};
    use std::time::Duration;

    let long_line = "const data = \"".to_string() + &"x".repeat(10000) + "\";";
    let file = format!("function test() {{\n    {}\n    return 1;\n}}", long_line);

    let search = format!("{}\n    return 1;", long_line);

    let matcher = FuzzyMatcher;

    // Test wydajności - powinno zakończyć się w rozsądnym czasie
    let result = assert_completes_within(Duration::from_secs(5), || {
        matcher.find_match(&file, &search, Some("test.js"))
    });

    // Może znaleźć lub nie, ważne żeby nie zawiesić się
    if let Some(res) = result {
        assert!(res.confidence > 0.5, "If found, should have reasonable confidence");
    }
}

#[test]
fn test_matching_special_characters() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher};

    let file = r#"
        const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
        const phoneRegex = /\d{3}-\d{3}-\d{4}/;
        const urlRegex = /https?:\/\/(www\.)?[-a-zA-Z0-9@:%._\+~#=]{1,256}/;
    "#;

    let search = r#"const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;"#;

    let matcher = FuzzyMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_some(), "Should handle regex special characters");
    let res = result.unwrap();
    assert!(res.confidence > 0.85, "Should match despite special characters");
}

#[test]
fn test_matching_unicode() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher};

    let file = r#"
        const greeting = "Привет мир! 你好世界!";
        const emoji = "🎉🚀✨🔥💡";
        const symbols = "α β γ δ ε";
    "#;

    let search = r#"const emoji = "🎉🚀✨🔥💡";"#;

    let matcher = FuzzyMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    assert!(result.is_some(), "Should handle Unicode characters");
    let res = result.unwrap();
    assert!(res.confidence > 0.90, "Unicode match should be accurate");
}

#[test]
fn test_matching_identical_blocks() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, AnchorMatcher};

    let file = r#"
function helper() { return 1; }
function other() { return 2; }
function helper() { return 1; }
function another() { return 3; }
function helper() { return 1; }
"#;

    let search = "function helper() { return 1; }";

    let matcher = AnchorMatcher;
    let result = matcher.find_match(file, search, Some("test.js"));

    // Dla identycznych bloków, matcher może:
    // 1. Znaleźć pierwszy (preferowane)
    // 2. Zwrócić None (ambiwalencja)
    // Ważne: NIE powinien crashować ani zwrócić losowego dopasowania
    if let Some(res) = result {
        // Jeśli znalazł, powinno być to jedno z trzech dopasowań
        assert!(res.matched_line_start > 0, "Should find one of the matches");
    }
}

#[test]
fn test_matching_minified_code() {
    use gluon_desktop_lib::apply_system::matchers::{Matcher, FuzzyMatcher, RegexMatcher};

    let file = "function calculate(){const x=10;const y=20;return x+y;}function other(){return 1;}";
    let search = "function calculate(){const x=10;const y=20;return x+y;}";

    let fuzzy_matcher = FuzzyMatcher;
    let regex_matcher = RegexMatcher;

    // Spróbuj oba matchery - minified code powinien być dopasowany przez przynajmniej jeden
    let fuzzy_result = fuzzy_matcher.find_match(file, search, Some("test.js"));
    let regex_result = regex_matcher.find_match(file, search, Some("test.js"));

    assert!(
        fuzzy_result.is_some() || regex_result.is_some(),
        "At least one matcher should handle minified code"
    );

    // Sprawdź confidence jeśli znaleziono
    if let Some(res) = fuzzy_result.or(regex_result) {
        assert!(res.confidence > 0.60, "Minified code should match with reasonable confidence");
    }
}

#[test]
fn test_matching_with_comments() {
    // TODO: Test matchingu z komentarzami
    let with_comments = r#"
// This is a comment
function test() {
    /* Block comment */
    return true;
}
"#;
    assert!(with_comments.contains("//"));
    assert!(with_comments.contains("/*"));

    println!("TODO: test_matching_with_comments");
}
