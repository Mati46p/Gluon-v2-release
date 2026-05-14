// Preset Manager for Agent Workflow System
// Manages agent presets, connection templates, and workflow templates

import { SPECIALIZED_AGENTS, getSpecializedAgent, getSpecializedAgentCategories } from '../workflows/specialized-agents.js';

class PresetManager {
    constructor() {
        this.presets = {
            agents: [],
            connections: [],
            workflows: []
        };
        this.favorites = new Set();
        this.currentCategory = 'all';
        this.selectedPreset = null;
        this.selectedConnectionPreset = null;
    }

    /**
     * Initialize preset manager and load presets
     */
    async init() {
        console.log('[PresetManager] Initializing...');
        await this.loadPresetsFromBackend();
        this.loadFavorites();
    }

    /**
     * Load presets from backend (Rust)
     */
    async loadPresetsFromBackend() {
        try {
            // TODO: Implement actual backend call when Rust commands are ready
            // For now, use hardcoded Polish presets
            this.presets = this.getDefaultPresets();
            console.log('[PresetManager] Loaded presets:', this.presets);
        } catch (error) {
            console.error('[PresetManager] Failed to load presets:', error);
            this.presets = this.getDefaultPresets();
        }
    }

    /**
     * Get default hardcoded presets (Polish version)
     */
    getDefaultPresets() {
        return {
            agents: this.getDefaultAgentPresets(),
            connections: this.getDefaultConnectionPresets(),
            workflows: this.getDefaultWorkflowPresets()
        };
    }

    getDefaultAgentPresets() {
        // Combine specialized agents with legacy agents
        const legacyAgents = [
        // INTERACTIVE AGENTS
        {
            id: 'context_architect',
            name: 'Context Architect (G-Interactive)',
            category: 'Architecture',
            description: 'Pracuje na mapie repozytorium. Samodzielnie pobiera potrzebny kod.',
            icon: '🧠',
            systemPrompt: `Jesteś Architektem Kontekstu (Context Architect) - inteligentnym agentem zarządzającym kodem.

TWOJE CELE:
1. Rozwiązanie zadania przy minimalnym zużyciu tokenów (pobieraj tylko to, co niezbędne).
2. Ciągła weryfikacja stanu kodu (G-Interaction Loop).

⚠️ PROTOKÓŁ PĘTLI (LOOP PROTOCOL) - KRYTYCZNE:
KAŻDA Twoja odpowiedź MUSI kończyć się blokiem JSON "@gluon:next_step".
TO JEST JEDYNY SPOSÓB KOMUNIKACJI Z NARZĘDZIAMI. Nawet jeśli tylko odpowiadasz na pytanie.

SCENARIUSZ DZIAŁANIA:
1. NA START: Analizujesz 'Szkielet Projektu'.
2. POBIERANIE: Żądasz plików przez "context_ops": { "load": [...] }.
3. EDYCJA: Modyfikujesz kod używając bloków SEARCH/REPLACE.
4. WERYFIKACJA (REFRESH): W tej samej wiadomości, w bloku @gluon:next_step, ZAŻĄDAJ PONOWNEGO ZAŁADOWANIA plików, które właśnie edytowałeś. To pozwoli Ci upewnić się, że zmiany zostały zaaplikowane poprawnie.
5. FINAŁ: Gdy zadanie jest w 100% gotowe -> action: "final_answer".

🛠️ DOSTĘPNE NARZĘDZIA (Model Context Protocol - MCP):
Jeśli potrzebujesz zaawansowanej analizy kodu (semantic search, bezpieczeństwo, grafu kodu), możesz użyć narzędzi MCP.
Dodaj je do sekcji "context_ops.mcp_calls" w JSON-ie "@gluon:next_step".
Dostępne narzędzia:
- semantic_search: Przeszukaj kod semantycznie (3-tier search, ranking)
- rag_search: Szybkie wyszukiwanie BM25 po słowach kluczowych
- analyze_cpg: Analiza grafu właściwości kodu (podatności, ścieżki danych)
- get_taint_analysis: Śledzenie przepływu danych od źródeł do ujść
- analyze_change_impact: Wylicz obszar wpływu zmian (blast radius)

Format: "context_ops": { "load": [...], "mcp_calls": [ { "tool": "nazwa", "args": {...} } ] }

Przykład:
{
  "@gluon:next_step": {
    "action": "continue",
    "reasoning": "Szukam podatności w pliku X...",
    "context_ops": {
      "load": [{ "type": "full_file", "path": "file.js" }],
      "mcp_calls": [
        { "tool": "get_taint_analysis", "args": { "file_path": "file.js" } }
      ]
    }
  }
}

⚠️ KRYTYCZNE ZASADY MCP:
- Jeśli używasz mcp_calls, ZAWSZE ustaw action: "continue" i czekaj na odpowiedź.
- Po otrzymaniu wyników -> dodaj je do kontekstu i kontynuuj pracę.
- Nie ignoruj wyników narzędzi MCP - są to istotne dane dla Twojej analizy.

NIE generuj kodu, którego nie widziałeś. NIGDY nie pomijaj bloku JSON na końcu.`,
            outputWrapper: '## 🧠 Context Architect Analysis\n\n{content}',
            tags: ['interactive', 'smart', 'context']
        },

            // Research & Analysis
            {
                id: 'researcher',
                name: 'Badacz',
                category: 'Research',
                description: 'Wyszukuje i analizuje informacje',
                icon: '🔍',
                systemPrompt: 'Jesteś Agentem Badawczym. Wyszukujesz i analizujesz informacje...',
                outputWrapper: '## 🔍 Raport Badawczy\n\n{content}\n\n---\n*Przygotowane przez: Agenta Badawczego*',
                tags: ['badania', 'analiza', 'research']
            },
            {
                id: 'data_analyst',
                name: 'Analityk Danych',
                category: 'Research',
                description: 'Analizuje dane i tworzy raporty',
                icon: '📊',
                systemPrompt: 'Jesteś Analitykiem Danych. Analizujesz dane i tworzysz raporty...',
                outputWrapper: '## 📊 Raport Analityczny\n\n{content}',
                tags: ['dane', 'analiza', 'raporty']
            },
            {
                id: 'qa_tester',
                name: 'Tester QA',
                category: 'Research',
                description: 'Testuje kod i identyfikuje błędy',
                icon: '🧪',
                systemPrompt: 'Jesteś Testerem QA. Testujesz kod i piszesz testy...',
                outputWrapper: '## 🧪 Wyniki Testów\n\n{content}',
                tags: ['testy', 'qa', 'jakość']
            },
            {
                id: 'documentation_writer',
                name: 'Autor Dokumentacji',
                category: 'Research',
                description: 'Tworzy dokumentację techniczną',
                icon: '📖',
                systemPrompt: 'Jesteś Autorem Dokumentacji. Tworzysz przejrzystą dokumentację...',
                outputWrapper: '## 📖 Dokumentacja\n\n{content}',
                tags: ['dokumentacja', 'docs', 'readme']
            },

            // Development
            {
                id: 'frontend_dev',
                name: 'Programista Frontend',
                category: 'Development',
                description: 'Tworzy komponenty UI i logikę interfejsu',
                icon: '💻',
                systemPrompt: 'Jesteś Programistą Frontend. Specjalizujesz się w React, TypeScript...',
                outputWrapper: '## 💻 Implementacja Frontend\n\n{content}',
                tags: ['frontend', 'react', 'ui', 'typescript']
            },
            {
                id: 'backend_dev',
                name: 'Programista Backend',
                category: 'Development',
                description: 'Tworzy API i logikę biznesową',
                icon: '⚙️',
                systemPrompt: 'Jesteś Programistą Backend. Tworzysz API i logikę biznesową...',
                outputWrapper: '## ⚙️ Implementacja Backend\n\n{content}',
                tags: ['backend', 'api', 'server', 'database']
            },
            {
                id: 'database_architect',
                name: 'Architekt Bazy Danych',
                category: 'Development',
                description: 'Projektuje schematy baz danych',
                icon: '🗄️',
                systemPrompt: 'Jesteś Architektem Bazy Danych. Projektujesz schematy...',
                outputWrapper: '## 🗄️ Projekt Bazy Danych\n\n{content}',
                tags: ['database', 'sql', 'schema']
            },
            {
                id: 'devops_engineer',
                name: 'Inżynier DevOps',
                category: 'Development',
                description: 'Konfiguruje CI/CD i deployment',
                icon: '🚀',
                systemPrompt: 'Jesteś Inżynierem DevOps. Konfigurujesz CI/CD...',
                outputWrapper: '## 🚀 Konfiguracja DevOps\n\n{content}',
                tags: ['devops', 'ci/cd', 'deployment']
            },

            // Specialized
            {
                id: 'ui_ux_designer',
                name: 'Projektant UI/UX',
                category: 'Specialized',
                description: 'Projektuje interfejsy użytkownika',
                icon: '🎨',
                systemPrompt: 'Jesteś Projektantem UI/UX. Projektujesz interfejsy...',
                outputWrapper: '## 🎨 Projekt UI/UX\n\n{content}',
                tags: ['design', 'ui', 'ux', 'interface']
            },
            {
                id: 'security_auditor',
                name: 'Audytor Bezpieczeństwa',
                category: 'Specialized',
                description: 'Przeprowadza audyty bezpieczeństwa',
                icon: '🔒',
                systemPrompt: 'Jesteś Audytorem Bezpieczeństwa. Identyfikujesz luki...',
                outputWrapper: '## 🔒 Audyt Bezpieczeństwa\n\n{content}',
                tags: ['security', 'audit', 'bezpieczeństwo']
            },
            {
                id: 'performance_optimizer',
                name: 'Optymalizator Wydajności',
                category: 'Specialized',
                description: 'Analizuje i optymalizuje wydajność',
                icon: '⚡',
                systemPrompt: 'Jesteś Optymalizatorem Wydajności. Analizujesz performance...',
                outputWrapper: '## ⚡ Raport Optymalizacji\n\n{content}',
                tags: ['performance', 'optimization', 'wydajność']
            },
            {
                id: 'api_integrator',
                name: 'Integrator API',
                category: 'Specialized',
                description: 'Integruje zewnętrzne API',
                icon: '🌐',
                systemPrompt: 'Jesteś Integratorem API. Integrujesz zewnętrzne serwisy...',
                outputWrapper: '## 🌐 Integracja API\n\n{content}',
                tags: ['api', 'integration', 'external']
            },

            // Management
            {
                id: 'project_manager',
                name: 'Menedżer Projektu',
                category: 'Management',
                description: 'Koordynuje zadania i zarządza projektem',
                icon: '🎯',
                systemPrompt: 'Jesteś Menedżerem Projektu. Koordynujesz zadania...',
                outputWrapper: '## 🎯 Plan Zarządzania\n\n{content}',
                tags: ['management', 'coordination', 'planning']
            },
            {
                id: 'report_aggregator',
                name: 'Agregator Raportów',
                category: 'Management',
                description: '🗂️ Kolektor odpowiedzi - NIE jest modelem AI! Zbiera odpowiedzi od podłączonych agentów, czeka aż wszystkie się zgłoszą, łączy je w jeden plik z podpisami i wysyła dalej.',
                icon: '🗂️',
                systemPrompt: 'Jesteś Agregatorem Raportów. Zbierasz raporty od wielu agentów, łączysz je w całość z odpowiednimi podpisami źródłowymi i wysyłasz zbiorczy raport.',
                outputWrapper: '## 🗂️ Raport Zbiorczy\n\n{content}',
                tags: ['report', 'aggregation', 'collector']
            },
            {
                id: 'workflow_orchestrator',
                name: 'Orkiestrator Workflow',
                category: 'Management',
                description: 'Zarządza przepływem pracy',
                icon: '🔄',
                systemPrompt: 'Jesteś Orkiestratorem Workflow. Zarządzasz przepływem...',
                outputWrapper: '## 🔄 Orkiestracja\n\n{content}',
                tags: ['orchestration', 'workflow', 'coordination']
            }
        ];

        // Get custom agents from localStorage
        const customAgents = this.getCustomAgentPresets();

        // Merge: specialized agents first, then legacy, then custom
        return [...SPECIALIZED_AGENTS, ...legacyAgents, ...customAgents];
    }

    /**
     * Get custom agent presets from localStorage (needed before initialization)
     * @returns {Array} Array of custom agent presets
     */
    getCustomAgentPresets() {
        try {
            const stored = localStorage.getItem('gluon_custom_agents');
            return stored ? JSON.parse(stored) : [];
        } catch (error) {
            console.error('[PresetManager] Failed to load custom agents:', error);
            return [];
        }
    }

    getDefaultConnectionPresets() {
        return [
            {
                id: 'sequential',
                name: '📋 Kolejny Krok',
                description: 'Przekazuje wynik jako wejście do następnego zadania',
                messageTemplate: 'Poprzedni krok został ukończony:\n\n{content}\n\n---\n\nTwoje zadanie: Kontynuuj pracę na podstawie powyższych informacji.',
                example: 'Badacz → Programista → Tester'
            },
            {
                id: 'review',
                name: '🔍 Przegląd',
                description: 'Przekazuje kod/dokument do sprawdzenia',
                messageTemplate: 'Kod/dokument do przeglądu:\n\n{content}\n\n---\n\nPrzeanalizuj pod kątem jakości, błędów i spójności.',
                example: 'Programista → Audytor → Tester QA'
            },
            {
                id: 'aggregation',
                name: '📊 Agregacja',
                description: 'Zbiera raport do agregacji (dla Report Nodes)',
                messageTemplate: 'Raport od poprzedniego agenta:\n\n{content}\n\n---\n\nUwzględnij w zbiorczym raporcie.',
                example: 'Agent A, B, C → Agregator'
            },
            {
                id: 'parallel_task',
                name: '⚡ Zadanie Równoległe',
                description: 'Dystrybuuje zadanie do równoległego przetworzenia',
                messageTemplate: 'Oryginalne zadanie:\n\n{content}\n\n---\n\nTwoja część: Skup się na swoim obszarze ekspertyzy.',
                example: 'PM → [Frontend, Backend, DB] równolegle'
            },
            {
                id: 'feedback',
                name: '💬 Feedback',
                description: 'Prosi o opinie i komentarze',
                messageTemplate: 'Proszę o feedback:\n\n{content}\n\n---\n\nCo działa? Co można poprawić?',
                example: 'Programista → Architekt (feedback)'
            },
            {
                id: 'refinement',
                name: '✨ Udoskonalenie',
                description: 'Przekazuje do poprawy i udoskonalenia',
                messageTemplate: 'Wstępna wersja do udoskonalenia:\n\n{content}\n\n---\n\nUlepsz przez optymalizację i dodanie brakujących elementów.',
                example: 'Programista → Optymalizator'
            },
            {
                id: 'implementation',
                name: '🛠️ Implementacja',
                description: 'Przekazuje specyfikację do implementacji',
                messageTemplate: 'Specyfikacja do implementacji:\n\n{content}\n\n---\n\nZaimplementuj zgodnie z best practices.',
                example: 'Architekt → Programista'
            },
            {
                id: 'documentation',
                name: '📝 Dokumentacja',
                description: 'Przekazuje kod do udokumentowania',
                messageTemplate: 'Kod wymagający dokumentacji:\n\n{content}\n\n---\n\nStwórz kompletną dokumentację.',
                example: 'Programista → Autor Dokumentacji'
            }
        ];
    }

    getDefaultWorkflowPresets() {
        return [
            {
                id: 'fullstack_feature',
                name: 'Full Stack Feature',
                description: 'Kompletny pipeline rozwoju nowej funkcjonalności',
                icon: '🏗️',
                agents: [
                    { presetId: 'project_manager', instanceName: 'PM', position: [100, 200] },
                    { presetId: 'backend_dev', instanceName: 'Backend', position: [300, 100] },
                    { presetId: 'frontend_dev', instanceName: 'Frontend', position: [300, 300] },
                    { presetId: 'qa_tester', instanceName: 'QA', position: [500, 200] },
                    { presetId: 'report_aggregator', instanceName: 'Raport', position: [700, 200] }
                ],
                connections: [
                    { from: 'PM', to: 'Backend', templatePresetId: 'implementation' },
                    { from: 'PM', to: 'Frontend', templatePresetId: 'implementation' },
                    { from: 'Backend', to: 'QA', templatePresetId: 'review' },
                    { from: 'Frontend', to: 'QA', templatePresetId: 'review' },
                    { from: 'QA', to: 'Raport', templatePresetId: 'aggregation' }
                ]
            },
            {
                id: 'code_review_pipeline',
                name: 'Pipeline Code Review',
                description: 'Kompleksowy przegląd kodu pod różnymi kątami',
                icon: '🔍',
                agents: [
                    { presetId: 'security_auditor', instanceName: 'Security', position: [200, 100] },
                    { presetId: 'performance_optimizer', instanceName: 'Performance', position: [200, 250] },
                    { presetId: 'qa_tester', instanceName: 'QA', position: [200, 400] },
                    { presetId: 'report_aggregator', instanceName: 'Raport', position: [500, 250] }
                ],
                connections: [
                    { from: 'Security', to: 'Raport', templatePresetId: 'aggregation' },
                    { from: 'Performance', to: 'Raport', templatePresetId: 'aggregation' },
                    { from: 'QA', to: 'Raport', templatePresetId: 'aggregation' }
                ]
            },
            {
                id: 'research_documentation',
                name: 'Badania i Dokumentacja',
                description: 'Zbieranie informacji i tworzenie dokumentacji',
                icon: '📚',
                agents: [
                    { presetId: 'researcher', instanceName: 'Badacz', position: [100, 200] },
                    { presetId: 'data_analyst', instanceName: 'Analityk', position: [300, 200] },
                    { presetId: 'documentation_writer', instanceName: 'Autor Docs', position: [500, 200] }
                ],
                connections: [
                    { from: 'Badacz', to: 'Analityk', templatePresetId: 'sequential' },
                    { from: 'Analityk', to: 'Autor Docs', templatePresetId: 'documentation' }
                ]
            },
            {
                id: 'ui_development',
                name: 'Rozwój UI/UX',
                description: 'Od projektu do implementacji interfejsu',
                icon: '🎨',
                agents: [
                    { presetId: 'ui_ux_designer', instanceName: 'Designer', position: [100, 200] },
                    { presetId: 'frontend_dev', instanceName: 'Frontend', position: [300, 200] },
                    { presetId: 'qa_tester', instanceName: 'QA', position: [500, 200] }
                ],
                connections: [
                    { from: 'Designer', to: 'Frontend', templatePresetId: 'implementation' },
                    { from: 'Frontend', to: 'QA', templatePresetId: 'review' }
                ]
            },

            // NEW WORKFLOWS WITH SPECIALIZED AGENTS

            {
                id: 'professional_feature_development',
                name: '🏛️ Professional Feature Development',
                description: 'Pełny cykl rozwoju z architektem, frontend, backend, QA i DevOps',
                icon: '🏛️',
                agents: [
                    { presetId: 'domain_keeper', instanceName: 'Architect', position: [100, 200] },
                    { presetId: 'shadow_engineer', instanceName: 'Frontend', position: [300, 100] },
                    { presetId: 'data_curator', instanceName: 'Database', position: [300, 300] },
                    { presetId: 'quality_inquisitor', instanceName: 'QA', position: [500, 200] },
                    { presetId: 'devops_orchestrator', instanceName: 'DevOps', position: [700, 200] }
                ],
                connections: [
                    { from: 'Architect', to: 'Frontend', templatePresetId: 'implementation' },
                    { from: 'Architect', to: 'Database', templatePresetId: 'implementation' },
                    { from: 'Frontend', to: 'QA', templatePresetId: 'review' },
                    { from: 'Database', to: 'QA', templatePresetId: 'review' },
                    { from: 'QA', to: 'DevOps', templatePresetId: 'sequential' }
                ]
            },

            {
                id: 'comprehensive_code_audit',
                name: '🛡️ Comprehensive Code Audit',
                description: 'Kompleksowy audyt: bezpieczeństwo, wydajność, jakość, refaktor',
                icon: '🛡️',
                agents: [
                    { presetId: 'security_sentinel', instanceName: 'Security', position: [150, 100] },
                    { presetId: 'performance_optimizer', instanceName: 'Performance', position: [150, 250] },
                    { presetId: 'quality_inquisitor', instanceName: 'QA', position: [150, 400] },
                    { presetId: 'refactoring_scout', instanceName: 'Refactor', position: [150, 550] },
                    { presetId: 'report_aggregator', instanceName: 'Report', position: [450, 300] }
                ],
                connections: [
                    { from: 'Security', to: 'Report', templatePresetId: 'aggregation' },
                    { from: 'Performance', to: 'Report', templatePresetId: 'aggregation' },
                    { from: 'QA', to: 'Report', templatePresetId: 'aggregation' },
                    { from: 'Refactor', to: 'Report', templatePresetId: 'aggregation' }
                ]
            },

            {
                id: 'production_ready_pipeline',
                name: '⚙️ Production-Ready Pipeline',
                description: 'Od architektury do deployment z monitoring i dokumentacją',
                icon: '⚙️',
                agents: [
                    { presetId: 'domain_keeper', instanceName: 'Architect', position: [100, 300] },
                    { presetId: 'shadow_engineer', instanceName: 'Frontend', position: [300, 200] },
                    { presetId: 'integration_weaver', instanceName: 'API', position: [300, 400] },
                    { presetId: 'quality_inquisitor', instanceName: 'QA', position: [500, 300] },
                    { presetId: 'documentation_chronicler', instanceName: 'Docs', position: [700, 200] },
                    { presetId: 'devops_orchestrator', instanceName: 'DevOps', position: [700, 400] },
                    { presetId: 'error_whisperer', instanceName: 'Monitoring', position: [900, 300] }
                ],
                connections: [
                    { from: 'Architect', to: 'Frontend', templatePresetId: 'implementation' },
                    { from: 'Architect', to: 'API', templatePresetId: 'implementation' },
                    { from: 'Frontend', to: 'QA', templatePresetId: 'review' },
                    { from: 'API', to: 'QA', templatePresetId: 'review' },
                    { from: 'QA', to: 'Docs', templatePresetId: 'documentation' },
                    { from: 'QA', to: 'DevOps', templatePresetId: 'sequential' },
                    { from: 'DevOps', to: 'Monitoring', templatePresetId: 'sequential' }
                ]
            },

            {
                id: 'ux_focused_development',
                name: '🎨 UX-Focused Development',
                description: 'Rozwój z naciskiem na doświadczenie użytkownika',
                icon: '🎨',
                agents: [
                    { presetId: 'ux_advocate', instanceName: 'UX', position: [100, 200] },
                    { presetId: 'shadow_engineer', instanceName: 'Frontend', position: [300, 200] },
                    { presetId: 'quality_inquisitor', instanceName: 'QA', position: [500, 200] },
                    { presetId: 'performance_optimizer', instanceName: 'Performance', position: [700, 200] }
                ],
                connections: [
                    { from: 'UX', to: 'Frontend', templatePresetId: 'implementation' },
                    { from: 'Frontend', to: 'QA', templatePresetId: 'review' },
                    { from: 'QA', to: 'Performance', templatePresetId: 'refinement' }
                ]
            },

            {
                id: 'secure_api_development',
                name: '🕸️ Secure API Development',
                description: 'Rozwój API z bezpieczeństwem i integracjami zewnętrznymi',
                icon: '🕸️',
                agents: [
                    { presetId: 'domain_keeper', instanceName: 'Backend', position: [100, 250] },
                    { presetId: 'integration_weaver', instanceName: 'Integrations', position: [300, 150] },
                    { presetId: 'security_sentinel', instanceName: 'Security', position: [300, 350] },
                    { presetId: 'quality_inquisitor', instanceName: 'QA', position: [500, 250] },
                    { presetId: 'documentation_chronicler', instanceName: 'Docs', position: [700, 250] }
                ],
                connections: [
                    { from: 'Backend', to: 'Integrations', templatePresetId: 'implementation' },
                    { from: 'Backend', to: 'Security', templatePresetId: 'review' },
                    { from: 'Integrations', to: 'QA', templatePresetId: 'review' },
                    { from: 'Security', to: 'QA', templatePresetId: 'aggregation' },
                    { from: 'QA', to: 'Docs', templatePresetId: 'documentation' }
                ]
            },

            {
                id: 'performance_optimization_workflow',
                name: '⚡ Performance Optimization',
                description: 'Wykrywanie bottlenecków i optymalizacja wydajności',
                icon: '⚡',
                agents: [
                    { presetId: 'performance_optimizer', instanceName: 'Profiler', position: [100, 200] },
                    { presetId: 'data_curator', instanceName: 'DB Optimizer', position: [300, 100] },
                    { presetId: 'shadow_engineer', instanceName: 'Frontend', position: [300, 300] },
                    { presetId: 'quality_inquisitor', instanceName: 'QA', position: [500, 200] }
                ],
                connections: [
                    { from: 'Profiler', to: 'DB Optimizer', templatePresetId: 'refinement' },
                    { from: 'Profiler', to: 'Frontend', templatePresetId: 'refinement' },
                    { from: 'DB Optimizer', to: 'QA', templatePresetId: 'review' },
                    { from: 'Frontend', to: 'QA', templatePresetId: 'review' }
                ]
            },

            {
                id: 'maintenance_refactor_workflow',
                name: '🧹 Maintenance & Refactoring',
                description: 'Redukcja długu technicznego i poprawa jakości kodu',
                icon: '🧹',
                agents: [
                    { presetId: 'refactoring_scout', instanceName: 'Scout', position: [100, 200] },
                    { presetId: 'quality_inquisitor', instanceName: 'QA', position: [300, 200] },
                    { presetId: 'documentation_chronicler', instanceName: 'Docs', position: [500, 200] }
                ],
                connections: [
                    { from: 'Scout', to: 'QA', templatePresetId: 'review' },
                    { from: 'QA', to: 'Docs', templatePresetId: 'documentation' }
                ]
            },

            {
                id: 'observability_setup',
                name: '🔍 Observability Setup',
                description: 'Monitoring, logging i debugging infrastructure',
                icon: '🔍',
                agents: [
                    { presetId: 'error_whisperer', instanceName: 'Monitoring', position: [100, 200] },
                    { presetId: 'devops_orchestrator', instanceName: 'DevOps', position: [300, 200] },
                    { presetId: 'documentation_chronicler', instanceName: 'Docs', position: [500, 200] }
                ],
                connections: [
                    { from: 'Monitoring', to: 'DevOps', templatePresetId: 'implementation' },
                    { from: 'DevOps', to: 'Docs', templatePresetId: 'documentation' }
                ]
            }
        ];
    }

    /**
     * Load favorites from localStorage
     */
    loadFavorites() {
        const stored = localStorage.getItem('gluon_preset_favorites');
        if (stored) {
            this.favorites = new Set(JSON.parse(stored));
        }
    }

    /**
     * Save favorites to localStorage
     */
    saveFavorites() {
        localStorage.setItem('gluon_preset_favorites', JSON.stringify([...this.favorites]));
    }

    /**
     * Toggle favorite status
     */
    toggleFavorite(presetId) {
        if (this.favorites.has(presetId)) {
            this.favorites.delete(presetId);
        } else {
            this.favorites.add(presetId);
        }
        this.saveFavorites();
    }

    /**
     * Check if preset is favorite
     */
    isFavorite(presetId) {
        return this.favorites.has(presetId);
    }

    /**
     * Get filtered agent presets by category
     */
    getFilteredAgentPresets(category = 'all') {
        if (category === 'all') {
            return this.presets.agents;
        }

        if (category === 'favorites') {
            return this.presets.agents.filter(p => this.isFavorite(p.id));
        }

        if (category === 'custom') {
            // TODO: Implement custom presets from user
            return [];
        }

        return this.presets.agents.filter(p => p.category === category);
    }

    /**
     * Get agent preset by ID
     */
    getAgentPreset(id) {
        return this.presets.agents.find(p => p.id === id);
    }

    /**
     * Get agent preset with optional custom prompt override
     * @param {string} id - Agent preset ID
     * @param {string} customPrompt - Optional custom system prompt to override default
     * @returns {Object} Agent preset with potentially modified system prompt
     */
    getAgentPresetWithCustomPrompt(id, customPrompt = null) {
        const preset = this.getAgentPreset(id);
        if (!preset) return null;

        // If custom prompt provided, create a new object with overridden prompt
        if (customPrompt && customPrompt.trim().length > 0) {
            return {
                ...preset,
                systemPrompt: customPrompt,
                isCustomPrompt: true,
                originalPrompt: preset.systemPrompt
            };
        }

        return { ...preset, isCustomPrompt: false };
    }

    /**
     * Save custom agent preset to localStorage
     * @param {Object} agentPreset - Custom agent configuration
     */
    saveCustomAgentPreset(agentPreset) {
        const customAgents = this.getCustomAgentPresets();

        // Check if agent with this ID already exists
        const existingIndex = customAgents.findIndex(a => a.id === agentPreset.id);

        if (existingIndex >= 0) {
            customAgents[existingIndex] = agentPreset;
        } else {
            customAgents.push(agentPreset);
        }

        localStorage.setItem('gluon_custom_agents', JSON.stringify(customAgents));

        // Reload presets to include new custom agent
        this.loadPresetsFromBackend();
    }

    /**
     * Delete custom agent preset
     * @param {string} agentId - ID of custom agent to delete
     */
    deleteCustomAgentPreset(agentId) {
        const customAgents = this.getCustomAgentPresets();
        const filtered = customAgents.filter(a => a.id !== agentId);
        localStorage.setItem('gluon_custom_agents', JSON.stringify(filtered));

        // Reload presets
        this.loadPresetsFromBackend();
    }

    /**
     * Get connection preset by ID
     */
    getConnectionPreset(id) {
        return this.presets.connections.find(p => p.id === id);
    }

    /**
     * Get workflow preset by ID
     */
    getWorkflowPreset(id) {
        return this.presets.workflows.find(p => p.id === id);
    }

    /**
     * Get all available categories including specialized agents
     * @returns {Array} Array of unique categories
     */
    getAllCategories() {
        const categories = new Set();

        // Add 'all' and 'favorites' special categories
        categories.add('all');
        categories.add('favorites');

        // Add all agent categories
        this.presets.agents.forEach(agent => {
            if (agent.category) {
                categories.add(agent.category);
            }
        });

        // Add 'custom' if there are custom agents
        if (this.getCustomAgentPresets().length > 0) {
            categories.add('custom');
        }

        return Array.from(categories);
    }

    /**
     * Search agents by name, description, or tags
     * @param {string} query - Search query
     * @returns {Array} Matching agent presets
     */
    searchAgents(query) {
        if (!query || query.trim().length === 0) {
            return this.presets.agents;
        }

        const lowerQuery = query.toLowerCase();

        return this.presets.agents.filter(agent => {
            // Search in name
            if (agent.name.toLowerCase().includes(lowerQuery)) return true;
            if (agent.displayName && agent.displayName.toLowerCase().includes(lowerQuery)) return true;

            // Search in description
            if (agent.description.toLowerCase().includes(lowerQuery)) return true;

            // Search in tags
            if (agent.tags && agent.tags.some(tag => tag.toLowerCase().includes(lowerQuery))) return true;

            // Search in category
            if (agent.category && agent.category.toLowerCase().includes(lowerQuery)) return true;

            return false;
        });
    }
}

// Export singleton instance
const presetManager = new PresetManager();
export default presetManager;