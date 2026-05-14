use super::graph::ContextGraph;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Generuje "Inteligentny Wycinek" (Smart Slice) repozytorium.
///
/// Algorytm:
/// 1. Ignoruje pliki już załączone (oszczędność tokenów).
/// 2. Analizuje załączone pliki w poszukiwaniu importów (silne sygnały) i słów kluczowych (słabe sygnały).
/// 3. Skanuje resztę projektu, szukając pasujących symboli (funkcji, klas).
/// 4. Wymusza spójność hierarchiczną (jeśli pokażemy metodę, pokażemy też jej klasę).
pub fn rank_files(
    graph: &ContextGraph,
    attached_files: &[String],
    max_tokens: usize
) -> String {
    // KROK 1: Normalizacja ścieżek załączonych plików (aby unikać problemów Windows/Linux)
    let attached_set: HashSet<String> = attached_files
        .iter()
        .map(|f| f.replace('\\', "/"))
        .collect();

    // KROK 2: Ekstrakcja Sygnałów (Czego szukamy w innych plikach?)
    
    // A. Silne sygnały: Jawne importy wyciągnięte przez Tree-sittera w ContextGraph
    let mut explicit_dependencies: HashSet<String> = HashSet::new();
    for attached in &attached_set {
        if let Some(imports) = graph.get_imports(attached) {
            for imported_path in imports {
                explicit_dependencies.insert(imported_path.replace('\\', "/"));
            }
        }
    }

    // B. Słabe sygnały: Analiza tekstu (Tokenizacja) w poszukiwaniu nazw funkcji/klas
    let usage_tokens = extract_usage_signals(attached_files);
    
    // KROK 3: Ranking i Selekcja Symboli
    // Mapa: Ścieżka Pliku -> Zbiór indeksów symboli do pokazania
    let mut file_relevant_indices: HashMap<String, HashSet<usize>> = HashMap::new();
    let mut file_scores: HashMap<String, f64> = HashMap::new();

    for (path, symbols) in &graph.file_symbols {
        // Pomiń plik, jeśli jest już załączony w całości
        if attached_set.contains(path) { continue; }

        let mut matched_indices = HashSet::new();
        let mut score = 0.1; // Bazowy, niski ranking

        // Boost za bycie jawnym importem
        let is_dependency = explicit_dependencies.iter().any(|dep| path.contains(dep));
        if is_dependency { score += 50.0; }

        // Skanowanie symboli w pliku
        for (idx, symbol) in symbols.iter().enumerate() {
            // Jeśli nazwa symbolu występuje w usage_tokens, to znaczy, że jest używany
            if usage_tokens.contains(&symbol.name) {
                matched_indices.insert(idx);
                score += 10.0;
            }
        }

        // Fallback: Jeśli plik jest importowany, ale tokeny nie pasują (np. wildcard import *),
        // pokaż główne definicje (Klasy/Funkcje top-level).
        if matched_indices.is_empty() && is_dependency {
            for (idx, symbol) in symbols.iter().enumerate() {
                if symbol.parent.is_none() { 
                    matched_indices.insert(idx);
                }
            }
        }

        // KROK 4: Parent Backfilling (Kluczowa logika hierarchii)
        // Jeśli zdecydowaliśmy się pokazać symbol, który ma rodzica (np. metodę klasy),
        // musimy znaleźć i dodać tego rodzica do listy, aby zachować strukturę.
        if !matched_indices.is_empty() {
            let mut parents_to_add = HashSet::new();
            for &idx in &matched_indices {
                let sym = &symbols[idx];
                if let Some(parent_name) = &sym.parent {
                    // Szukamy symbolu, który definiuje tego rodzica (np. "class User")
                    if let Some(parent_idx) = symbols.iter().position(|s| s.name == *parent_name) {
                        parents_to_add.insert(parent_idx);
                    }
                }
            }
            matched_indices.extend(parents_to_add);

            file_relevant_indices.insert(path.clone(), matched_indices);
            file_scores.insert(path.clone(), score);
        }
    }

    // KROK 5: Sortowanie plików według ważności (score)
    let mut ranked_files: Vec<(String, f64)> = file_scores.into_iter().collect();
    ranked_files.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // KROK 6: Renderowanie Mapy (Wycinki)
    let max_lines = max_tokens / 10;
    let mut current_lines = 0;
    let mut output = String::from("Repo Context Map (Smart Slice):\n");

    for (path, _) in ranked_files {
        if current_lines >= max_lines { break; }
        
        if let Some(indices_set) = file_relevant_indices.get(&path) {
            if let Some(all_symbols) = graph.get_symbols(&path) {
                // Renderujemy ścieżkę w stylu drzewa
                output.push_str(&format!("📄 {}\n", path));
                current_lines += 1;

                let mut sorted_indices: Vec<usize> = indices_set.iter().cloned().collect();
                sorted_indices.sort_unstable();

                let total_indices = sorted_indices.len();
                for (i, &idx) in sorted_indices.iter().enumerate() {
                    if current_lines >= max_lines { break; }
                    let sym = &all_symbols[idx];
                    
                    let is_last = i == total_indices - 1;
                    let connector = if is_last { "└── " } else { "├── " };
                    
                    // Jeśli ma rodzica, dodaj dodatkowe wcięcie
                    let depth_prefix = if sym.parent.is_some() { "│   " } else { "" };
                    
                    output.push_str(&format!("  {}{}{}\n", depth_prefix, connector, sym.signature));
                    current_lines += 1;
                }
                
                // Opcjonalnie: pusty wiersz oddzielający pliki dla czytelności
                output.push('\n');
            }
        }
    }

    if output.len() == "Repo Context Map (Smart Slice):\n".len() {
        return "Repo Context Map: (No direct dependencies detected outside attached files)".to_string();
    }

    output
}

/// Generuje mapę semantyczną TYLKO dla wskazanych plików (Targeted Map).
/// Używane przez SemanticMap do podglądu struktury konkretnych plików bez ich pobierania.
pub fn map_target_files(
    graph: &ContextGraph,
    target_files: &[String]
) -> String {
    let mut output = String::from("Repo Context Map (Target Selection):\n");

    // Normalizacja ścieżek
    let target_set: HashSet<String> = target_files
        .iter()
        .map(|f| f.replace('\\', "/"))
        .collect();

    let mut found_any = false;

    // Sortujemy klucze aby wynik był deterministyczny
    let mut sorted_paths: Vec<&String> = graph.file_symbols.keys().collect();
    sorted_paths.sort();

    for path in sorted_paths {
        // Sprawdzamy czy plik jest na liście celów (obsługa ścieżek relatywnych/absolutnych)
        // Porównujemy znormalizowane końcówki ścieżek
        let normalized_path = path.replace('\\', "/");
        let is_target = target_set.iter().any(|t| normalized_path.ends_with(t) || t.ends_with(&normalized_path));

        if !is_target { continue; }

        if let Some(symbols) = graph.get_symbols(path) {
            output.push_str(&format!("📄 {}\n", path));
            found_any = true;

            for (i, sym) in symbols.iter().enumerate() {
                let is_last = i == symbols.len() - 1;
                let connector = if is_last { "└── " } else { "├── " };

                // Jeśli ma rodzica, dodaj dodatkowe wcięcie dla czytelności
                let depth_prefix = if sym.parent.is_some() { "│   " } else { "" };

                output.push_str(&format!("  {}{}{}\n", depth_prefix, connector, sym.signature));
            }
            output.push('\n');
        }
    }

    if !found_any {
        return format!("Repo Context Map: No symbols found for requested files. (Graph contains {} files)", graph.file_symbols.len());
    }

    output
}

/// Pomocnicza funkcja skanująca załączone pliki w poszukiwaniu używanych identyfikatorów.
/// Używa agresywnej "czarnej listy", aby ignorować słowa kluczowe języków programowania.
fn extract_usage_signals(file_paths: &[String]) -> HashSet<String> {
    let mut tokens = HashSet::new();
    
    // Stoplist: Słowa kluczowe (Rust, JS/TS, Python, Java, Go, C++)
    // Ignorujemy je, aby nie zaśmiecać mapy fałszywymi trafieniami.
    let stops: HashSet<&str> = [
        "if", "else", "return", "for", "while", "const", "let", "var", "function", "class", 
        "import", "from", "export", "async", "await", "pub", "fn", "struct", "impl", "use", 
        "mod", "crate", "self", "super", "true", "false", "null", "undefined", "void", "int", 
        "string", "bool", "new", "try", "catch", "interface", "type", "public", "private", 
        "protected", "static", "readonly", "override", "extends", "implements", "package",
        "defer", "go", "map", "chan", "default", "switch", "case", "break", "continue"
    ].iter().cloned().collect();

    for path_str in file_paths {
        let path = Path::new(path_str);
        if let Ok(content) = std::fs::read_to_string(path) {
            // Prosty tokenizator: dzielimy po wszystkim co nie jest literą/cyfrą/_
            let identifiers: Vec<&str> = content
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|s| !s.is_empty())
                .collect();

            for token in identifiers {
                // Filtrujemy:
                // - Krótkie tokeny (< 3 znaki) - zazwyczaj szum lub zmienne lokalne (i, x, y)
                // - Zaczynające się od cyfry (to nie są identyfikatory)
                // - Słowa ze stoplisty
                if token.len() > 2 
                   && !token.chars().next().unwrap().is_numeric() 
                   && !stops.contains(token) 
                {
                    tokens.insert(token.to_string());
                }
            }
        }
    }
    
    tokens
}