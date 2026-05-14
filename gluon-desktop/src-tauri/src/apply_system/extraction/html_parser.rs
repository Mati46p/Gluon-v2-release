// Plik: gluon-desktop/src-tauri/src/apply_system/extraction/html_parser.rs
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use html_escape::decode_html_entities;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtractedCodeBlock {
    pub language: Option<String>,
    pub content: String,
    pub raw_html_snippet: String, // Do debugowania
}

pub struct HtmlParser;

impl HtmlParser {
    pub fn extract(raw_html: &str, provider: &str) -> Vec<ExtractedCodeBlock> {
        let fragment = Html::parse_fragment(raw_html);
        let mut blocks = Vec::new();

        // 1. Wybierz selektor bazowy dla bloków kodu w zależności od providera
        let block_selector_str = match provider {
            "claude" => "pre, .code-block-wrapper", // Claude często używa <pre> lub wrapperów
            "chatgpt" => "pre code",
            "gemini" => "ms-code-block pre, pre",   // [NAPRAWA] Obsługa Gemini
            _ => "pre",
        };

        let selector = Selector::parse(block_selector_str)
            .expect("Invalid CSS selector defined in parser");

        for element in fragment.select(&selector) {
            // 2. Wyciągnij język (jeśli dostępny w klasie, np. class="language-rust")
            let language = element.value().classes()
                .find(|c| c.starts_with("language-"))
                .map(|c| c.replace("language-", ""));

            // 3. Chirurgiczne czyszczenie treści (Sanityzacja)
            let clean_content = Self::sanitize_node_content(&element);

            if !clean_content.trim().is_empty() {
                blocks.push(ExtractedCodeBlock {
                    language,
                    content: clean_content,
                    raw_html_snippet: element.html(),
                });
            }
        }

        blocks
    }

    /// Funkcja rekurencyjnie pobiera tekst, ale ignoruje elementy "śmieciowe"
    fn sanitize_node_content(element: &scraper::ElementRef) -> String {
        let mut text = String::new();

        // Lista klas/tagów do zignorowania (przyciski copy, numery linii, UI Google'a)
        let ignored_selectors = [
            "button", 
            ".copy-button", 
            ".line-numbers", 
            ".flex.items-center", // Nagłówki w Claude
            ".sr-only", // Screen reader text
            "mat-icon", // Ikony Material Design w Google AI Studio
            ".actions-container", // Kontener akcji w Google
            ".mat-expansion-panel-header" // Nagłówek panelu (np. "Python")
        ];

        // Sprawdź czy sam element nie jest śmieciem
        for ignored in ignored_selectors.iter() {
            if element.value().name() == *ignored 
               || element.value().classes().any(|c| ("." .to_string() + c) == *ignored) {
                return String::new();
            }
        }

        // Iteruj po dzieciach
        if element.has_children() {
            for child in element.children() {
                if let Some(el_ref) = scraper::ElementRef::wrap(child) {
                    text.push_str(&Self::sanitize_node_content(&el_ref));
                } else if let Some(text_node) = child.value().as_text() {
                    text.push_str(text_node);
                }
            }
        } else {
            // Jeśli to liść, pobierz tekst
            text.push_str(&element.text().collect::<Vec<_>>().join(""));
        }

        // 4. Dekodowanie encji (np. &lt; -> <) i normalizacja
        let decoded = decode_html_entities(&text).to_string();
        
        // 5. Fix na "Smart Quotes" (częsty problem w przeglądarkach)
        decoded
            .replace('\u{00A0}', " ") // Non-breaking space
            .replace("“", "\"")
            .replace("”", "\"")
            .replace("‘", "'")
            .replace("’", "'")
    }
}