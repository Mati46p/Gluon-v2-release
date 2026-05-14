use gluon_desktop_lib::apply_system::analysis::{AnalysisEngine, queries::QueryMatcher};
use gluon_desktop_lib::apply_system::analysis::languages::SupportedLanguage;

fn main() {
    // Test 1: With indentation (as in SEARCH block)
    let search_code = r#"    @action(detail=True, methods=['get'])
    def stats(self, request, pk=None):
        pass
"#;

    println!("=== TEST 1: SEARCH block (with indentation) ===");
    println!("{}", search_code);
    test_parse(search_code, "search.py");

    // Test 2: Real file context
    let file_code = r#"    return Response(parameters_list)

    @action(detail=True, methods=['get'])
    def stats(self, request, pk=None):
        """
        Zwraca statystyki dla konkretnej kolejki.
        Implementacja Strategii: 3A (Filters = Live) oraz 2C (Hybrid Cache).
        """
        import hashlib

        queue = self.get_object()
        pass
"#;

    println!("\n=== TEST 2: Real file ===");
    println!("{}", file_code);
    test_parse(file_code, "real.py");
}

fn test_parse(code: &str, name: &str) {
    println!("\n--- Parsing {} ---", name);

    match AnalysisEngine::parse(code, "test.py") {
        Ok(tree) => {
            let sigs = QueryMatcher::extract_signatures(code, &tree, SupportedLanguage::Python);

            println!("\nFound {} signatures:", sigs.len());
            for (i, sig) in sigs.iter().enumerate() {
                println!("\n[{}] Signature:", i);
                println!("  Name: {}", sig.name);
                println!("  Kind: {}", sig.kind);
                println!("  start_row: {} (0-indexed)", sig.start_row);
                println!("  end_row: {} (0-indexed)", sig.end_row);

                let lines: Vec<&str> = code.lines().collect();
                if sig.start_row < lines.len() {
                    println!("  Line at start_row [{}]: {:?}", sig.start_row, lines[sig.start_row]);
                }
                if sig.start_row > 0 && sig.start_row - 1 < lines.len() {
                    println!("  Line BEFORE start_row [{}]: {:?}", sig.start_row - 1, lines[sig.start_row - 1]);
                }
            }
        }
        Err(e) => {
            eprintln!("Parse error: {}", e);
        }
    }
}
