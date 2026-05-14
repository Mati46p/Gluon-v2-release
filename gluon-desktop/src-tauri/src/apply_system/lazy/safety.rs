use super::structural_matcher::StructuralMatcher; // Używamy helperów z matchera

#[derive(Debug)]
pub struct SafetyReport {
    pub is_safe: bool,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

impl SafetyReport {
    pub fn safe() -> Self {
        Self { is_safe: true, warnings: Vec::new(), error: None }
    }
    
    pub fn reject(reason: String) -> Self {
        Self { is_safe: false, warnings: Vec::new(), error: Some(reason) }
    }
}

pub struct SafetyGuard;

impl SafetyGuard {
    /// Główna funkcja walidująca propozycję zmiany
    pub fn check_edit(
        original_content: &str,
        new_content: &str,
        file_extension: &str,
    ) -> SafetyReport {
        let mut report = SafetyReport::safe();

        // 1. Sprawdzenie "Lazy Comments" (Szybki filtr tekstowy)
        // Jeśli nowy kod zawiera podejrzane komentarze zastępujące treść
        let lazy_markers = [
            "// ... existing code", 
            "// ... rest of code", 
            "// ... code ...", 
            "# ... existing code",
            "/* ... existing code */"
        ];
        
        for marker in lazy_markers {
            if new_content.contains(marker) {
                // To jest "miękkie" ostrzeżenie, chyba że plik drastycznie zmalał
                report.warnings.push(format!("Detected lazy marker: '{}'", marker));
            }
        }

        // 2. Analiza AST (Drzewo składniowe)
        // Musimy sprawdzić, czy nie usuwamy zbyt wielu węzłów logicznych (nie spacji/komentarzy)
        if let Some(metrics) = Self::calculate_ast_diff(original_content, new_content, file_extension) {
            // Reguła: Jeśli usunęliśmy > 40% węzłów, to podejrzane
            if metrics.node_reduction_ratio > 0.4 {
                let msg = format!(
                    "Large logic deletion detected! Node count dropped by {:.1}% ({} -> {}). This often indicates a 'lazy' response.",
                    metrics.node_reduction_ratio * 100.0,
                    metrics.original_nodes,
                    metrics.new_nodes
                );
                
                // Jeśli mamy też lazy marker, to odrzucamy zmianę całkowicie
                if !report.warnings.is_empty() {
                    return SafetyReport::reject(msg);
                } else {
                    report.warnings.push(msg);
                }
            }

            // Reguła: Błąd składniowy w nowym pliku
            if metrics.has_syntax_errors {
                report.warnings.push("New code contains syntax errors.".to_string());
                // Możemy tu dodać logikę: return SafetyReport::reject(...) jeśli chcemy być surowi
            }
        }

        // 3. Sprawdzenie "Empty Body"
        // Czy nie zamieniliśmy funkcji z ciałem na pustą funkcję?
        if original_content.len() > 50 && new_content.len() < 10 {
             report.warnings.push("Suspiciously small content replacement.".to_string());
        }

        report
    }

    fn calculate_ast_diff(_original: &str, _new_code: &str, _ext: &str) -> Option<AstMetrics> {
        // Tu musielibyśmy zainicjować parser podobnie jak w structural_matcher.
        // Dla uproszczenia (żeby nie duplikować kodu inicjalizacji języków), 
        // możemy użyć tymczasowo prostego licznika lub przenieść mapę parserów do statica.
        
        // Na potrzeby tego etapu, użyjmy StructuralMatchera, który już ma parsery,
        // ale ponieważ SafetyGuard jest statyczny, zrobimy tu lokalną instancję 
        // (to jest lekkie, bo gramatyki są wkompilowane).
        
        // [FIX] Usunięto 'mut' i dodano '_', bo zmienna jest nieużywana w tym placeholderze
        let _matcher = StructuralMatcher::new();
        
        // Używamy publicznej metody (którą musimy dodać do structural_matcher.rs) 
        // lub po prostu parsowania jeśli parser jest dostępny.
        
        // ... Logika parsowania (zostanie dodana w kroku integracji) ...
        
        // PLACEHOLDER (Zaimplementujemy pełne liczenie węzłów w engine.rs, 
        // bo tam mamy dostęp do instancji parsera)
        None 
    }
}

pub struct AstMetrics {
    pub original_nodes: usize,
    pub new_nodes: usize,
    pub node_reduction_ratio: f32,
    pub has_syntax_errors: bool,
}