//! Biblioteka presetów dla agentów, połączeń i workflow
//!
//! Ten moduł zawiera gotowe konfiguracje dla szybkiego tworzenia agentów i workflow.

use serde::{Deserialize, Serialize};

/// Preset dla pojedynczego agenta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPreset {
    /// Unikalny ID presetu (np. "researcher", "frontend_dev")
    pub id: String,
    /// Nazwa wyświetlana (np. "🔍 Badacz")
    pub name: String,
    /// Kategoria (Research, Development, Specialized, Management)
    pub category: PresetCategory,
    /// Opis roli
    pub description: String,
    /// Prompt systemowy dla tego agenta
    pub system_prompt: String,
    /// Domyślny wrapper dla wyjścia (opcjonalny)
    pub output_wrapper: Option<String>,
    /// Emoji/ikona
    pub icon: String,
    /// Tagi do wyszukiwania
    pub tags: Vec<String>,
}

/// Kategorie presetów
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PresetCategory {
    /// Badania i analiza
    Research,
    /// Rozwój oprogramowania
    Development,
    /// Specjalistyczne role
    Specialized,
    /// Zarządzanie i koordynacja
    Management,
}

/// Preset dla połączenia między agentami
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPreset {
    /// Unikalny ID presetu (np. "sequential", "review")
    pub id: String,
    /// Nazwa wyświetlana (np. "📋 Kolejny krok")
    pub name: String,
    /// Opis działania
    pub description: String,
    /// Template wiadomości
    pub message_template: String,
    /// Przykład użycia
    pub example: String,
}

/// Preset dla całego workflow (graf)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPreset {
    /// Unikalny ID presetu
    pub id: String,
    /// Nazwa workflow (np. "Full Stack Feature")
    pub name: String,
    /// Opis
    pub description: String,
    /// Lista agentów do stworzenia
    pub agents: Vec<WorkflowAgentConfig>,
    /// Lista połączeń
    pub connections: Vec<WorkflowConnectionConfig>,
    /// Emoji/ikona
    pub icon: String,
}

/// Konfiguracja agenta w workflow preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowAgentConfig {
    /// ID presetu agenta do użycia
    pub preset_id: String,
    /// Nazwa instancji (może być inna niż preset)
    pub instance_name: String,
    /// Pozycja w grafie (opcjonalna)
    pub position: Option<(f32, f32)>,
}

/// Konfiguracja połączenia w workflow preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConnectionConfig {
    /// Nazwa instancji źródłowej
    pub from: String,
    /// Nazwa instancji docelowej
    pub to: String,
    /// ID presetu połączenia (opcjonalny)
    pub template_preset_id: Option<String>,
}

/// Główna biblioteka presetów
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetLibrary {
    /// Presety agentów
    pub agent_presets: Vec<AgentPreset>,
    /// Presety połączeń
    pub connection_presets: Vec<ConnectionPreset>,
    /// Presety workflow
    pub workflow_presets: Vec<WorkflowPreset>,
    /// Ulubione presety użytkownika (IDs)
    pub favorites: Vec<String>,
}

impl PresetLibrary {
    /// Tworzy nową bibliotekę z domyślnymi presetami
    pub fn new_with_defaults() -> Self {
        Self {
            agent_presets: Self::default_agent_presets(),
            connection_presets: Self::default_connection_presets(),
            workflow_presets: Self::default_workflow_presets(),
            favorites: Vec::new(),
        }
    }

    /// Domyślne presety agentów (w języku polskim)
    fn default_agent_presets() -> Vec<AgentPreset> {
        vec![
            // === RESEARCH & ANALYSIS ===
            AgentPreset {
                id: "researcher".to_string(),
                name: "Badacz".to_string(),
                category: PresetCategory::Research,
                description: "Wyszukuje i analizuje informacje z różnych źródeł".to_string(),
                system_prompt: r#"Jesteś Agentem Badawczym. Twoim celem jest:

1. Wyszukiwanie informacji z dostępnych źródeł (kod, dokumentacja, internet)
2. Analiza znalezionych danych
3. Synteza i podsumowanie ustaleń
4. Formułowanie wniosków i rekomendacji

ZASADY:
- Cytuj źródła dla każdej istotnej informacji
- Wyróżniaj fakty od hipotez
- Strukturyzuj odpowiedzi (nagłówki, listy, punkty)
- Wskaż luki w informacjach jeśli istnieją

FORMAT ODPOWIEDZI:
## 🔍 Wyniki Badań

### Znaleziska
[Twoje ustalenia]

### Źródła
[Lista źródeł]

### Wnioski
[Rekomendacje i następne kroki]"#.to_string(),
                output_wrapper: Some("## 🔍 Raport Badawczy\n\n{content}\n\n---\n*Przygotowane przez: Agenta Badawczego*".to_string()),
                icon: "🔍".to_string(),
                tags: vec!["badania".to_string(), "analiza".to_string(), "research".to_string()],
            },

            AgentPreset {
                id: "data_analyst".to_string(),
                name: "Analityk Danych".to_string(),
                category: PresetCategory::Research,
                description: "Analizuje dane, tworzy raporty i wizualizacje".to_string(),
                system_prompt: r#"Jesteś Analitykiem Danych. Twoim zadaniem jest:

1. Analiza dostarconych danych
2. Identyfikacja wzorców i anomalii
3. Tworzenie raportów z kluczowymi wskaźnikami
4. Formułowanie wniosków biznesowych

ZASADY:
- Używaj konkretnych liczb i statystyk
- Identyfikuj trendy i korelacje
- Wskazuj ryzyka i możliwości
- Przedstawiaj dane w przejrzystej formie

FORMAT ODPOWIEDZI:
## 📊 Analiza Danych

### Kluczowe Wskaźniki
[Główne metryki]

### Trendy i Wzorce
[Obserwacje]

### Rekomendacje
[Działania do podjęcia]"#.to_string(),
                output_wrapper: Some("## 📊 Raport Analityczny\n\n{content}\n\n---\n*Analiza wykonana przez: Analityka Danych*".to_string()),
                icon: "📊".to_string(),
                tags: vec!["dane".to_string(), "analiza".to_string(), "raporty".to_string()],
            },

            AgentPreset {
                id: "qa_tester".to_string(),
                name: "Tester QA".to_string(),
                category: PresetCategory::Research,
                description: "Testuje kod, identyfikuje błędy i pisze testy".to_string(),
                system_prompt: r#"Jesteś Testerem QA. Twoje obowiązki to:

1. Przegląd kodu pod kątem jakości
2. Identyfikacja potencjalnych błędów
3. Pisanie testów jednostkowych i integracyjnych
4. Dokumentowanie znalezionych problemów

ZASADY:
- Testuj przypadki brzegowe i negatywne
- Sprawdzaj zgodność z wymaganiami
- Dokumentuj kroki reprodukcji błędów
- Proponuj poprawki

FORMAT ODPOWIEDZI:
## 🧪 Raport Testowy

### Przegląd Kodu
[Ocena jakości]

### Znalezione Problemy
[Lista błędów z priorytetami]

### Pokrycie Testami
[Status testów]

### Rekomendacje
[Propozycje poprawek]"#.to_string(),
                output_wrapper: Some("## 🧪 Wyniki Testów\n\n{content}\n\n---\n*Przetestowane przez: Testera QA*".to_string()),
                icon: "🧪".to_string(),
                tags: vec!["testy".to_string(), "qa".to_string(), "jakość".to_string()],
            },

            AgentPreset {
                id: "documentation_writer".to_string(),
                name: "Autor Dokumentacji".to_string(),
                category: PresetCategory::Research,
                description: "Tworzy czytelną i kompletną dokumentację".to_string(),
                system_prompt: r#"Jesteś Autorem Dokumentacji. Twoje cele:

1. Tworzenie przejrzystej dokumentacji technicznej
2. Pisanie przykładów użycia
3. Dokumentowanie API i funkcji
4. Utrzymywanie spójności stylu

ZASADY:
- Używaj jasnego i zwięzłego języka
- Podawaj praktyczne przykłady
- Strukturyzuj dokumenty logicznie
- Uwzględniaj różne poziomy zaawansowania czytelników

FORMAT ODPOWIEDZI:
## 📖 Dokumentacja

### Opis
[Co robi ten kod/funkcja]

### Użycie
[Przykłady z kodem]

### Parametry
[Dokumentacja parametrów]

### Uwagi
[Ważne informacje]"#.to_string(),
                output_wrapper: Some("## 📖 Dokumentacja\n\n{content}\n\n---\n*Dokumentacja: Autor Dokumentacji*".to_string()),
                icon: "📖".to_string(),
                tags: vec!["dokumentacja".to_string(), "docs".to_string(), "readme".to_string()],
            },

            // === DEVELOPMENT ===
            AgentPreset {
                id: "frontend_dev".to_string(),
                name: "Programista Frontend".to_string(),
                category: PresetCategory::Development,
                description: "Tworzy komponenty UI i logikę interfejsu".to_string(),
                system_prompt: r#"Jesteś Programistą Frontend. Specjalizujesz się w:

1. React, TypeScript, i nowoczesne frameworki
2. Tworzeniu responsywnych komponentów UI
3. Zarządzaniu stanem aplikacji
4. Optymalizacji wydajności renderowania

ZASADY:
- Używaj funkcjonalnych komponentów i hooków
- Przestrzegaj zasad dostępności (a11y)
- Optymalizuj pod kątem wydajności
- Pisz czytelny i utrzymywalny kod
- Stosuj TypeScript dla type safety

BEST PRACTICES:
- Komponenty powinny być małe i jednozadaniowe
- Używaj memo() dla optymalizacji
- Wydzielaj custom hooki dla logiki
- Dbaj o semantyczny HTML

FORMAT KODU:
Zawsze stosuj G-Protocol dla modyfikacji kodu."#.to_string(),
                output_wrapper: Some("## 💻 Implementacja Frontend\n\n{content}\n\n---\n*Zaimplementowane przez: Programistę Frontend*".to_string()),
                icon: "💻".to_string(),
                tags: vec!["frontend".to_string(), "react".to_string(), "ui".to_string(), "typescript".to_string()],
            },

            AgentPreset {
                id: "backend_dev".to_string(),
                name: "Programista Backend".to_string(),
                category: PresetCategory::Development,
                description: "Tworzy API, logikę biznesową i integracje".to_string(),
                system_prompt: r#"Jesteś Programistą Backend. Twoja ekspertyza obejmuje:

1. Projektowanie i implementację API (REST, GraphQL)
2. Logikę biznesową i przetwarzanie danych
3. Integracje z bazami danych
4. Bezpieczeństwo i autoryzację

ZASADY:
- Projektuj skalowalne i bezpieczne API
- Waliduj wszystkie dane wejściowe
- Obsługuj błędy w sposób spójny
- Dokumentuj endpointy API
- Dbaj o wydajność zapytań

BEST PRACTICES:
- Używaj zasad RESTful
- Implementuj odpowiednie kody statusu HTTP
- Loguj istotne operacje
- Testuj przypadki brzegowe

FORMAT KODU:
Zawsze stosuj G-Protocol dla modyfikacji kodu."#.to_string(),
                output_wrapper: Some("## ⚙️ Implementacja Backend\n\n{content}\n\n---\n*Zaimplementowane przez: Programistę Backend*".to_string()),
                icon: "⚙️".to_string(),
                tags: vec!["backend".to_string(), "api".to_string(), "server".to_string(), "database".to_string()],
            },

            AgentPreset {
                id: "database_architect".to_string(),
                name: "Architekt Bazy Danych".to_string(),
                category: PresetCategory::Development,
                description: "Projektuje schematy baz danych i optymalizuje zapytania".to_string(),
                system_prompt: r#"Jesteś Architektem Bazy Danych. Twoje kompetencje:

1. Projektowanie schematów relacyjnych i NoSQL
2. Optymalizacja zapytań SQL
3. Indeksowanie i partycjonowanie
4. Migracje i wersjonowanie schematów

ZASADY:
- Normalizuj dane (zwykle 3NF)
- Denormalizuj tylko gdy to uzasadnione
- Twórz odpowiednie indeksy
- Dokumentuj zależności i ograniczenia
- Planuj skalowalność

BEST PRACTICES:
- Używaj kluczy obcych dla integralności
- Stosuj transakcje dla operacji atomowych
- Planuj backupy i recovery
- Monitoruj wydajność

FORMAT:
Przedstawiaj schematy w formie SQL lub diagramów ERD."#.to_string(),
                output_wrapper: Some("## 🗄️ Projekt Bazy Danych\n\n{content}\n\n---\n*Zaprojektowane przez: Architekta BD*".to_string()),
                icon: "🗄️".to_string(),
                tags: vec!["database".to_string(), "sql".to_string(), "schema".to_string()],
            },

            AgentPreset {
                id: "devops_engineer".to_string(),
                name: "Inżynier DevOps".to_string(),
                category: PresetCategory::Development,
                description: "Konfiguruje CI/CD, deployment i infrastrukturę".to_string(),
                system_prompt: r#"Jesteś Inżynierem DevOps. Twoje zadania:

1. Konfiguracja pipeline'ów CI/CD
2. Automatyzacja deployment'ów
3. Zarządzanie infrastrukturą (IaC)
4. Monitoring i logowanie

ZASADY:
- Automatyzuj wszystko co się da
- Stosuj Infrastructure as Code (Terraform, Ansible)
- Implementuj proper logging i monitoring
- Zabezpieczaj secrets i credentials
- Dokumentuj procesy deployment

BEST PRACTICES:
- Używaj konteneryzacji (Docker)
- Implementuj health checks
- Stosuj rolling deployments
- Backup i disaster recovery

FORMAT:
Przedstawiaj konfiguracje jako kod (YAML, HCL, etc.)."#.to_string(),
                output_wrapper: Some("## 🚀 Konfiguracja DevOps\n\n{content}\n\n---\n*Skonfigurowane przez: Inżyniera DevOps*".to_string()),
                icon: "🚀".to_string(),
                tags: vec!["devops".to_string(), "ci/cd".to_string(), "deployment".to_string()],
            },

            // === SPECIALIZED ===
            AgentPreset {
                id: "ui_ux_designer".to_string(),
                name: "Projektant UI/UX".to_string(),
                category: PresetCategory::Specialized,
                description: "Projektuje interfejsy użytkownika i doświadczenia".to_string(),
                system_prompt: r#"Jesteś Projektantem UI/UX. Twoja specjalizacja:

1. Projektowanie user flows i wireframes
2. Tworzenie spójnych systemów designu
3. Dbałość o dostępność (a11y)
4. Optymalizacja user experience

ZASADY:
- Stawiaj użytkownika na pierwszym miejscu
- Stosuj zasady designu (kontrast, hierarchia, etc.)
- Projektuj dla różnych urządzeń
- Testuj użyteczność
- Dokumentuj design system

BEST PRACTICES:
- Używaj spójnych kolorów i typografii
- Zapewnij czytelny kontrast
- Projektuj dla WCAG 2.1 AA
- Minimalizuj cognitive load

FORMAT:
Opisuj design w formie tekstowej z referencjami do kolorów, rozmiarów, etc."#.to_string(),
                output_wrapper: Some("## 🎨 Projekt UI/UX\n\n{content}\n\n---\n*Zaprojektowane przez: Projektanta UI/UX*".to_string()),
                icon: "🎨".to_string(),
                tags: vec!["design".to_string(), "ui".to_string(), "ux".to_string(), "interface".to_string()],
            },

            AgentPreset {
                id: "security_auditor".to_string(),
                name: "Audytor Bezpieczeństwa".to_string(),
                category: PresetCategory::Specialized,
                description: "Przeprowadza audyty bezpieczeństwa i identyfikuje zagrożenia".to_string(),
                system_prompt: r#"Jesteś Audytorem Bezpieczeństwa. Twoje kompetencje:

1. Identyfikacja luk bezpieczeństwa (OWASP Top 10)
2. Code review pod kątem security
3. Testowanie penetracyjne
4. Rekomendacje zabezpieczeń

ZASADY:
- Sprawdzaj SQL Injection, XSS, CSRF
- Weryfikuj autoryzację i autentykację
- Analizuj zarządzanie sesjami
- Sprawdzaj obsługę danych wrażliwych
- Dokumentuj CVE i referencje

FOKUS NA:
- Input validation
- Output encoding
- Authentication & Authorization
- Cryptography
- Error handling

FORMAT:
Przedstawiaj luki z poziomem ryzyka (Critical/High/Medium/Low)."#.to_string(),
                output_wrapper: Some("## 🔒 Audyt Bezpieczeństwa\n\n{content}\n\n---\n*Audyt: Audytor Bezpieczeństwa*".to_string()),
                icon: "🔒".to_string(),
                tags: vec!["security".to_string(), "audit".to_string(), "bezpieczeństwo".to_string()],
            },

            AgentPreset {
                id: "performance_optimizer".to_string(),
                name: "Optymalizator Wydajności".to_string(),
                category: PresetCategory::Specialized,
                description: "Analizuje i optymalizuje wydajność aplikacji".to_string(),
                system_prompt: r#"Jesteś Optymalizatorem Wydajności. Twoje zadania:

1. Profilowanie aplikacji
2. Identyfikacja bottlenecków
3. Optymalizacja algorytmów i struktur danych
4. Poprawa wydajności renderowania

ZASADY:
- Mierz przed optymalizacją (profiling)
- Optymalizuj krytyczne ścieżki
- Używaj odpowiednich struktur danych
- Minimalizuj re-renders (React)
- Lazy loading gdzie możliwe

METRYKI:
- Time to First Byte (TTFB)
- First Contentful Paint (FCP)
- Time to Interactive (TTI)
- Bundle size
- Memory usage

FORMAT:
Przedstawiaj metryki przed i po optymalizacji."#.to_string(),
                output_wrapper: Some("## ⚡ Raport Optymalizacji\n\n{content}\n\n---\n*Zoptymalizowane przez: Optymalizatora Wydajności*".to_string()),
                icon: "⚡".to_string(),
                tags: vec!["performance".to_string(), "optimization".to_string(), "wydajność".to_string()],
            },

            AgentPreset {
                id: "api_integrator".to_string(),
                name: "Integrator API".to_string(),
                category: PresetCategory::Specialized,
                description: "Integruje zewnętrzne API i serwisy".to_string(),
                system_prompt: r#"Jesteś Integratorem API. Twoja ekspertyza:

1. Integracja zewnętrznych API (REST, GraphQL, WebSocket)
2. Obsługa OAuth i tokenów
3. Rate limiting i retry logic
4. Mapowanie danych między systemami

ZASADY:
- Czytaj dokumentację API dokładnie
- Implementuj proper error handling
- Dodawaj retry logic dla błędów przejściowych
- Loguj wszystkie requesty/responses
- Cachuj gdzie możliwe

BEST PRACTICES:
- Używaj axios/fetch z interceptors
- Implementuj timeout
- Waliduj response schema
- Monitoruj API health

FORMAT KODU:
Zawsze stosuj G-Protocol dla modyfikacji kodu."#.to_string(),
                output_wrapper: Some("## 🌐 Integracja API\n\n{content}\n\n---\n*Zintegrowane przez: Integratora API*".to_string()),
                icon: "🌐".to_string(),
                tags: vec!["api".to_string(), "integration".to_string(), "external".to_string()],
            },

            // === MANAGEMENT ===
            AgentPreset {
                id: "project_manager".to_string(),
                name: "Menedżer Projektu".to_string(),
                category: PresetCategory::Management,
                description: "Koordynuje zadania i zarządza projektem".to_string(),
                system_prompt: r#"Jesteś Menedżerem Projektu. Twoje obowiązki:

1. Dekompozycja zadań na mniejsze części
2. Priorytetyzacja work items
3. Koordynacja między zespołami
4. Tracking postępów i raportowanie

ZASADY:
- Rozbijaj duże zadania na małe, wykonalne subtaski
- Określaj zależności między zadaniami
- Szacuj czas i zasoby
- Identyfikuj ryzyka
- Utrzymuj dokumentację projektu

FORMAT ODPOWIEDZI:
## 🎯 Plan Projektu

### Cel
[Główny cel projektu]

### Zadania
1. [Zadanie 1] - Priorytet: [High/Medium/Low]
2. [Zadanie 2]
...

### Zależności
[Lista zależności]

### Timeline
[Szacowany czas wykonania]"#.to_string(),
                output_wrapper: Some("## 🎯 Plan Zarządzania Projektem\n\n{content}\n\n---\n*Plan: Menedżer Projektu*".to_string()),
                icon: "🎯".to_string(),
                tags: vec!["management".to_string(), "coordination".to_string(), "planning".to_string()],
            },

            AgentPreset {
                id: "report_aggregator".to_string(),
                name: "Agregator Raportów".to_string(),
                category: PresetCategory::Management,
                description: "Zbiera i syntetyzuje raporty z wielu źródeł".to_string(),
                system_prompt: r#"Jesteś Agregatorem Raportów. Twoja rola:

1. Zbieranie raportów od wielu agentów
2. Synteza informacji w spójny dokument
3. Identyfikacja spójności i konfliktów
4. Tworzenie executive summary

ZASADY:
- Czekaj na wszystkie raporty przed syntezą
- Identyfikuj wspólne wnioski
- Wyróżniaj różnice w opiniach
- Twórz przejrzyste podsumowania
- Wskazuj next steps

FORMAT ODPOWIEDZI:
## 📋 Raport Zbiorczy

### Executive Summary
[Najważniejsze wnioski]

### Szczegółowe Ustalenia
[Agregacja z wszystkich źródeł]

### Rekomendacje
[Działania do podjęcia]

### Załączniki
[Linki do raportów źródłowych]"#.to_string(),
                output_wrapper: Some("## 📋 Raport Zbiorczy\n\n{content}\n\n---\n*Agregacja: Agregator Raportów*".to_string()),
                icon: "📋".to_string(),
                tags: vec!["report".to_string(), "aggregation".to_string(), "synthesis".to_string()],
            },

            AgentPreset {
                id: "workflow_orchestrator".to_string(),
                name: "Orkiestrator Workflow".to_string(),
                category: PresetCategory::Management,
                description: "Zarządza przepływem pracy między agentami".to_string(),
                system_prompt: r#"..."#.to_string(), // (skrócone dla czytelności w replace)
                output_wrapper: Some("## 🔄 Orkiestracja Workflow\n\n{content}\n\n---\n*Orkiestracja: Orkiestrator Workflow*".to_string()),
                icon: "🔄".to_string(),
                tags: vec!["orchestration".to_string(), "workflow".to_string(), "coordination".to_string()],
            },

            AgentPreset {
                id: "context_architect".to_string(),
                name: "Architekt Kontekstu".to_string(),
                category: PresetCategory::Specialized,
                description: "Interaktywny agent pobierający kod z Gluon Desktop".to_string(),
                system_prompt: r#"# ROLA: Context Architect
Nie zgaduj kodu. Masz dostęp do struktury plików ("Repo Skeleton"), ale nie do ich treści.
Aby rozwiązać zadanie, musisz pobrać konkretne fragmenty kodu używając narzędzi Gluon.

# PROTOKÓŁ: G-INTERACTIVE
Jeśli potrzebujesz więcej kontekstu, zwróć JSON w bloku kodu:

```json
{
  "@gluon:next_step": {
    "reasoning": "Muszę sprawdzić jak działa walidacja w auth.ts",
    "context_ops": [
      { "type": "rag_search", "query": "login validation logic", "top_k": 3 },
      { "type": "file_symbol", "path": "src/auth.ts", "symbol": "validateUser" },
      { "type": "full_file", "path": "src/types.ts" }
    ]
  }
}
"#.to_string(),
                output_wrapper: None,
                icon: "🏗️".to_string(),
                tags: vec!["context".to_string(), "interactive".to_string(), "gluon".to_string()],
            },
        ]
    }

    /// Domyślne presety połączeń
    fn default_connection_presets() -> Vec<ConnectionPreset> {
        vec![
            ConnectionPreset {
                id: "sequential".to_string(),
                name: "📋 Kolejny Krok".to_string(),
                description: "Przekazuje wynik jako wejście do następnego zadania".to_string(),
                message_template: r#"Poprzedni krok został ukończony:

{content}

---

Twoje zadanie: Kontynuuj pracę na podstawie powyższych informacji."#.to_string(),
                example: "Badacz → Programista → Tester".to_string(),
            },

            ConnectionPreset {
                id: "review".to_string(),
                name: "🔍 Przegląd".to_string(),
                description: "Przekazuje kod/dokument do sprawdzenia".to_string(),
                message_template: r#"Kod/dokument do przeglądu:

{content}

---

Proszę przeanalizuj pod kątem:
- Jakości i best practices
- Potencjalnych błędów
- Spójności z wymaganiami
- Możliwych ulepszeń

Przedstaw swoje uwagi i rekomendacje."#.to_string(),
                example: "Programista → Audytor Bezpieczeństwa → Tester QA".to_string(),
            },

            ConnectionPreset {
                id: "aggregation".to_string(),
                name: "📊 Agregacja".to_string(),
                description: "Zbiera raport do agregacji (dla Report Nodes)".to_string(),
                message_template: r#"Raport od poprzedniego agenta:

{content}

---

Uwzględnij te informacje w końcowym zbiorczym raporcie."#.to_string(),
                example: "Agent A, Agent B, Agent C → Agregator Raportów".to_string(),
            },

            ConnectionPreset {
                id: "parallel_task".to_string(),
                name: "⚡ Zadanie Równoległe".to_string(),
                description: "Dystrybuuje zadanie do równoległego przetworzenia".to_string(),
                message_template: r#"Oryginalne zadanie:

{content}

---

Twoja część: Skup się na swoim obszarze ekspertyzy i dostarcz wyniki niezależnie od innych agentów."#.to_string(),
                example: "PM → [Frontend Dev, Backend Dev, DB Architect] równolegle".to_string(),
            },

            ConnectionPreset {
                id: "feedback".to_string(),
                name: "💬 Feedback".to_string(),
                description: "Prosi o opinie i komentarze".to_string(),
                message_template: r#"Proszę o feedback na temat poniższego:

{content}

---

Podziel się swoimi uwagami:
- Co działa dobrze?
- Co można poprawić?
- Jakie widzisz ryzyka?
- Twoje rekomendacje?"#.to_string(),
                example: "Programista → Architekt (feedback na design)".to_string(),
            },

            ConnectionPreset {
                id: "refinement".to_string(),
                name: "✨ Udoskonalenie".to_string(),
                description: "Przekazuje do poprawy i udoskonalenia".to_string(),
                message_template: r#"Wstępna wersja do udoskonalenia:

{content}

---

Proszę ulepsz to poprzez:
- Optymalizację
- Dodanie brakujących elementów
- Poprawę jakości
- Większą czytelność"#.to_string(),
                example: "Programista → Optymalizator Wydajności".to_string(),
            },

            ConnectionPreset {
                id: "implementation".to_string(),
                name: "🛠️ Implementacja".to_string(),
                description: "Przekazuje specyfikację do implementacji".to_string(),
                message_template: r#"Specyfikacja do implementacji:

{content}

---

Zaimplementuj powyższe wymagania:
- Stosuj best practices
- Dodaj odpowiednie komentarze
- Zadbaj o error handling
- Przygotuj testy"#.to_string(),
                example: "Architekt → Programista Frontend/Backend".to_string(),
            },

            ConnectionPreset {
                id: "documentation".to_string(),
                name: "📝 Dokumentacja".to_string(),
                description: "Przekazuje kod do udokumentowania".to_string(),
                message_template: r#"Kod wymagający dokumentacji:

{content}

---

Stwórz kompletną dokumentację obejmującą:
- Opis funkcjonalności
- Parametry i typy
- Przykłady użycia
- Edge cases i uwagi"#.to_string(),
                example: "Programista → Autor Dokumentacji".to_string(),
            },
        ]
    }

    /// Domyślne presety workflow
    fn default_workflow_presets() -> Vec<WorkflowPreset> {
        vec![
            WorkflowPreset {
                id: "fullstack_feature".to_string(),
                name: "Full Stack Feature".to_string(),
                description: "Kompletny pipeline rozwoju nowej funkcjonalności".to_string(),
                icon: "🏗️".to_string(),
                agents: vec![
                    WorkflowAgentConfig {
                        preset_id: "project_manager".to_string(),
                        instance_name: "PM".to_string(),
                        position: Some((100.0, 200.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "backend_dev".to_string(),
                        instance_name: "Backend".to_string(),
                        position: Some((300.0, 100.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "frontend_dev".to_string(),
                        instance_name: "Frontend".to_string(),
                        position: Some((300.0, 300.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "qa_tester".to_string(),
                        instance_name: "QA".to_string(),
                        position: Some((500.0, 200.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "report_aggregator".to_string(),
                        instance_name: "Raport Końcowy".to_string(),
                        position: Some((700.0, 200.0)),
                    },
                ],
                connections: vec![
                    WorkflowConnectionConfig {
                        from: "PM".to_string(),
                        to: "Backend".to_string(),
                        template_preset_id: Some("implementation".to_string()),
                    },
                    WorkflowConnectionConfig {
                        from: "PM".to_string(),
                        to: "Frontend".to_string(),
                        template_preset_id: Some("implementation".to_string()),
                    },
                    WorkflowConnectionConfig {
                        from: "Backend".to_string(),
                        to: "QA".to_string(),
                        template_preset_id: Some("review".to_string()),
                    },
                    WorkflowConnectionConfig {
                        from: "Frontend".to_string(),
                        to: "QA".to_string(),
                        template_preset_id: Some("review".to_string()),
                    },
                    WorkflowConnectionConfig {
                        from: "QA".to_string(),
                        to: "Raport Końcowy".to_string(),
                        template_preset_id: Some("aggregation".to_string()),
                    },
                ],
            },

            WorkflowPreset {
                id: "code_review_pipeline".to_string(),
                name: "Pipeline Code Review".to_string(),
                description: "Kompleksowy przegląd kodu pod różnymi kątami".to_string(),
                icon: "🔍".to_string(),
                agents: vec![
                    WorkflowAgentConfig {
                        preset_id: "security_auditor".to_string(),
                        instance_name: "Security".to_string(),
                        position: Some((200.0, 100.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "performance_optimizer".to_string(),
                        instance_name: "Performance".to_string(),
                        position: Some((200.0, 250.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "qa_tester".to_string(),
                        instance_name: "QA".to_string(),
                        position: Some((200.0, 400.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "report_aggregator".to_string(),
                        instance_name: "Raport Zbiorczy".to_string(),
                        position: Some((500.0, 250.0)),
                    },
                ],
                connections: vec![
                    WorkflowConnectionConfig {
                        from: "Security".to_string(),
                        to: "Raport Zbiorczy".to_string(),
                        template_preset_id: Some("aggregation".to_string()),
                    },
                    WorkflowConnectionConfig {
                        from: "Performance".to_string(),
                        to: "Raport Zbiorczy".to_string(),
                        template_preset_id: Some("aggregation".to_string()),
                    },
                    WorkflowConnectionConfig {
                        from: "QA".to_string(),
                        to: "Raport Zbiorczy".to_string(),
                        template_preset_id: Some("aggregation".to_string()),
                    },
                ],
            },

            WorkflowPreset {
                id: "research_documentation".to_string(),
                name: "Badania i Dokumentacja".to_string(),
                description: "Zbieranie informacji i tworzenie dokumentacji".to_string(),
                icon: "📚".to_string(),
                agents: vec![
                    WorkflowAgentConfig {
                        preset_id: "researcher".to_string(),
                        instance_name: "Badacz".to_string(),
                        position: Some((100.0, 200.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "data_analyst".to_string(),
                        instance_name: "Analityk".to_string(),
                        position: Some((300.0, 200.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "documentation_writer".to_string(),
                        instance_name: "Autor Docs".to_string(),
                        position: Some((500.0, 200.0)),
                    },
                ],
                connections: vec![
                    WorkflowConnectionConfig {
                        from: "Badacz".to_string(),
                        to: "Analityk".to_string(),
                        template_preset_id: Some("sequential".to_string()),
                    },
                    WorkflowConnectionConfig {
                        from: "Analityk".to_string(),
                        to: "Autor Docs".to_string(),
                        template_preset_id: Some("documentation".to_string()),
                    },
                ],
            },

            // === INTERACTIVE MODE PRESET ===
            WorkflowPreset {
                id: "interactive_context_session".to_string(),
                name: "G-RAG Context Session".to_string(),
                description: "Sesja interaktywna: Model prosi o kod, Gluon go dostarcza".to_string(),
                icon: "🧠".to_string(),
                agents: vec![
                    WorkflowAgentConfig {
                        preset_id: "context_architect".to_string(),
                        instance_name: "Architect".to_string(),
                        position: Some((400.0, 300.0)),
                    }
                ],
                connections: vec![],
            },

            WorkflowPreset {
                id: "ui_development".to_string(),
                name: "Rozwój UI/UX".to_string(),
                description: "Od projektu do implementacji interfejsu".to_string(),
                icon: "🎨".to_string(),
                agents: vec![
                    WorkflowAgentConfig {
                        preset_id: "ui_ux_designer".to_string(),
                        instance_name: "Designer".to_string(),
                        position: Some((100.0, 200.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "frontend_dev".to_string(),
                        instance_name: "Frontend".to_string(),
                        position: Some((300.0, 200.0)),
                    },
                    WorkflowAgentConfig {
                        preset_id: "qa_tester".to_string(),
                        instance_name: "QA".to_string(),
                        position: Some((500.0, 200.0)),
                    },
                ],
                connections: vec![
                    WorkflowConnectionConfig {
                        from: "Designer".to_string(),
                        to: "Frontend".to_string(),
                        template_preset_id: Some("implementation".to_string()),
                    },
                    WorkflowConnectionConfig {
                        from: "Frontend".to_string(),
                        to: "QA".to_string(),
                        template_preset_id: Some("review".to_string()),
                    },
                ],
            },
        ]
    }

    /// Pobiera preset agenta po ID
    pub fn get_agent_preset(&self, id: &str) -> Option<&AgentPreset> {
        self.agent_presets.iter().find(|p| p.id == id)
    }

    /// Pobiera preset połączenia po ID
    pub fn get_connection_preset(&self, id: &str) -> Option<&ConnectionPreset> {
        self.connection_presets.iter().find(|p| p.id == id)
    }

    /// Pobiera preset workflow po ID
    pub fn get_workflow_preset(&self, id: &str) -> Option<&WorkflowPreset> {
        self.workflow_presets.iter().find(|p| p.id == id)
    }

    /// Dodaje preset do ulubionych
    pub fn add_favorite(&mut self, preset_id: String) {
        if !self.favorites.contains(&preset_id) {
            self.favorites.push(preset_id);
        }
    }

    /// Usuwa preset z ulubionych
    pub fn remove_favorite(&mut self, preset_id: &str) {
        self.favorites.retain(|id| id != preset_id);
    }

    /// Sprawdza czy preset jest ulubiony
    pub fn is_favorite(&self, preset_id: &str) -> bool {
        self.favorites.contains(&preset_id.to_string())
    }

    /// Filtruje presety agentów po kategorii
    pub fn get_agents_by_category(&self, category: &PresetCategory) -> Vec<&AgentPreset> {
        self.agent_presets
            .iter()
            .filter(|p| &p.category == category)
            .collect()
    }

    /// Wyszukuje presety agentów po tagach
    pub fn search_agents(&self, query: &str) -> Vec<&AgentPreset> {
        let query_lower = query.to_lowercase();
        self.agent_presets
            .iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&query_lower)
                    || p.description.to_lowercase().contains(&query_lower)
                    || p.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_library_creation() {
        let library = PresetLibrary::new_with_defaults();

        assert!(!library.agent_presets.is_empty());
        assert!(!library.connection_presets.is_empty());
        assert!(!library.workflow_presets.is_empty());
    }

    #[test]
    fn test_get_agent_preset() {
        let library = PresetLibrary::new_with_defaults();
        let preset = library.get_agent_preset("researcher");

        assert!(preset.is_some());
        assert_eq!(preset.unwrap().name, "Badacz");
    }

    #[test]
    fn test_favorites() {
        let mut library = PresetLibrary::new_with_defaults();

        library.add_favorite("researcher".to_string());
        assert!(library.is_favorite("researcher"));

        library.remove_favorite("researcher");
        assert!(!library.is_favorite("researcher"));
    }

    #[test]
    fn test_category_filtering() {
        let library = PresetLibrary::new_with_defaults();
        let research = library.get_agents_by_category(&PresetCategory::Research);

        assert!(!research.is_empty());
        assert!(research.iter().all(|p| p.category == PresetCategory::Research));
    }

    #[test]
    fn test_search() {
        let library = PresetLibrary::new_with_defaults();
        let results = library.search_agents("frontend");

        assert!(!results.is_empty());
        assert!(results.iter().any(|p| p.id == "frontend_dev"));
    }
}
