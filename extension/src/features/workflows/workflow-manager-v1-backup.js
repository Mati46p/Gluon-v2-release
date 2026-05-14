// Workflow Manager for Agent Communication
// Manages the UI and state for multi-agent workflow

console.log('[WorkflowManager] 📦 Module file loading...');

import presetManager from '../prompts/preset-manager.js';

console.log('[WorkflowManager] 📦 Imports successful, defining WorkflowManager class...');

class WorkflowManager {
    constructor() {
        this.graph = null;
        this.selectedAgentFrom = null;
        this.pendingRequests = new Map();
        this.graphEditor = null;
        this.currentView = 'list'; // 'list' or 'graph'
        this.presetManager = presetManager;
        this.currentPresetCategory = 'all';
        this.selectedAgentPreset = null;
        this.selectedConnectionPreset = null;

        // Workflow Tabs System
        this.workflowTabs = [];
        this.activeTabId = null;
        this.nextTabId = 1;

        this.init();
    }

    async init() {
        console.log('[WorkflowManager] 🚀 Initializing...');

        // Inject missing UI elements (Auto-Apply & Terminal)
        this.injectMissingAgentTypes();

        // Initialize preset manager
        try {
            await this.presetManager.init();
            console.log('[WorkflowManager] ✅ Preset manager initialized');
        } catch (error) {
            console.error('[WorkflowManager] ❌ Failed to initialize preset manager:', error);
            console.log('[WorkflowManager] ⚠️ Continuing with default presets...');
        }

        // Event listeners
        const addAgentBtn = document.getElementById('addAgentBtn');
        const refreshWorkflowBtn = document.getElementById('refreshWorkflowBtn');
        const autoForwardSwitch = document.getElementById('autoForwardSwitch');
        const viewListBtn = document.getElementById('viewListBtn');
        const viewGraphBtn = document.getElementById('viewGraphBtn');
        const copySchemaBtn = document.getElementById('copySchemaBtn');
        const loadWorkflowBtn = document.getElementById('loadWorkflowBtn');
        const newWorkflowTabBtn = document.getElementById('newWorkflowTabBtn');
        const exportTabBtn = document.getElementById('exportTabBtn');
        const importTabBtn = document.getElementById('importTabBtn');
        const importTabFileInput = document.getElementById('importTabFileInput');

        console.log('[WorkflowManager] 🔍 Elements found:', {
            addAgentBtn: !!addAgentBtn,
            copySchemaBtn: !!copySchemaBtn,
            refreshWorkflowBtn: !!refreshWorkflowBtn,
            autoForwardSwitch: !!autoForwardSwitch,
            viewListBtn: !!viewListBtn,
            viewGraphBtn: !!viewGraphBtn,
            loadWorkflowBtn: !!loadWorkflowBtn,
            newWorkflowTabBtn: !!newWorkflowTabBtn,
            exportTabBtn: !!exportTabBtn,
            importTabBtn: !!importTabBtn
        });

        if (addAgentBtn) {
            console.log('[WorkflowManager] 🎯 Attaching click listener to addAgentBtn');
            addAgentBtn.addEventListener('click', (e) => {
                console.log('[WorkflowManager] 🖱️ Add Agent button CLICKED!', e);
                console.log('[WorkflowManager] Button element:', addAgentBtn);
                console.log('[WorkflowManager] Button classes:', addAgentBtn.className);
                console.log('[WorkflowManager] Button disabled:', addAgentBtn.disabled);
                this.showAddAgentModal();
            });
            console.log('[WorkflowManager] ✅ Click listener attached to addAgentBtn');
        } else {
            console.error('[WorkflowManager] ❌ addAgentBtn NOT FOUND!');
        }

        if (copySchemaBtn) {
            console.log('[WorkflowManager] 🎯 Attaching click listener to copySchemaBtn');
            copySchemaBtn.addEventListener('click', (e) => {
                console.log('[WorkflowManager] 🖱️ Copy Schema button CLICKED!', e);
                this.handleCopySchema(e);
            });
        }

        if (newWorkflowTabBtn) {
            console.log('[WorkflowManager] 🎯 Attaching click listener to newWorkflowTabBtn');
            newWorkflowTabBtn.addEventListener('click', (e) => {
                console.log('[WorkflowManager] 🖱️ New Workflow Tab button CLICKED!', e);
                this.createNewTab();
            });
        }

        if (exportTabBtn) {
            console.log('[WorkflowManager] 🎯 Attaching click listener to exportTabBtn');
            exportTabBtn.addEventListener('click', (e) => {
                console.log('[WorkflowManager] 🖱️ Save Config button CLICKED!', e);
                this.saveCurrentTabConfig();
            });
        }

        if (importTabBtn) {
            console.log('[WorkflowManager] 🎯 Attaching click listener to importTabBtn');
            importTabBtn.addEventListener('click', (e) => {
                console.log('[WorkflowManager] 🖱️ Import Tab button CLICKED!', e);
                importTabFileInput?.click();
            });
        }

        if (importTabFileInput) {
            importTabFileInput.addEventListener('change', (e) => {
                const file = e.target.files?.[0];
                if (file) {
                    this.importTabFromFile(file);
                    // Reset input
                    e.target.value = '';
                }
            });
        }

        if (refreshWorkflowBtn) {
            console.log('[WorkflowManager] 🎯 Attaching click listener to refreshWorkflowBtn');
            refreshWorkflowBtn.addEventListener('click', (e) => {
                console.log('[WorkflowManager] 🖱️ Refresh Workflow button CLICKED!', e);
                this.refreshWorkflow();
            });
        }

        if (autoForwardSwitch) {
            autoForwardSwitch.addEventListener('change', (e) => {
                console.log('[WorkflowManager] 🔄 Auto-forward switch changed:', e.target.checked);
                this.toggleAutoForward(e.target.checked);
            });
        }

        if (loadWorkflowBtn) {
            console.log('[WorkflowManager] 🎯 Attaching click listener to loadWorkflowBtn');
            loadWorkflowBtn.addEventListener('click', (e) => {
                console.log('[WorkflowManager] 🖱️ Load Workflow button CLICKED!', e);
                this.showLoadWorkflowModal();
            });
        }

        // View toggle buttons
        if (viewListBtn) {
            console.log('[WorkflowManager] 🎯 Attaching click listener to viewListBtn');
            viewListBtn.addEventListener('click', (e) => {
                console.log('[WorkflowManager] 🖱️ View List button CLICKED!', e);
                this.switchView('list');
            });
        }

        if (viewGraphBtn) {
            console.log('[WorkflowManager] 🎯 Attaching click listener to viewGraphBtn');
            viewGraphBtn.addEventListener('click', (e) => {
                console.log('[WorkflowManager] 🖱️ View Graph button CLICKED!', e);
                this.switchView('graph');
            });
        }

        // Modal event listeners
        this.initModalListeners();

        // Listen for workflow responses from background
        chrome.runtime.onMessage.addListener((message) => {
            if (message.type === 'workflow_response') {
                this.handleWorkflowResponse(message);
            } else if (message.type === 'workflow_sync') {
                // 🔥 SYNC 1:1 - Handle real-time graph updates from Rust backend
                console.log('[WorkflowManager] 🔄 SYNC: Graph update received');
                this.handleGraphSync(message.data);
            } else if (message.type === 'workflow_aggregator_update') {
                this.handleAggregatorUpdate(message.data);
            } else if (message.type === 'workflow_auto_apply_trigger') {
                console.log('[WorkflowManager] ⚡ Auto-Apply trigger received:', message.data);
                this.sendToAutoApplyNode(message.data.agent_id, message.data.content);
            }
        });

        // Initialize graph editor
        this.initGraphEditor();

        // Initialize workflow tabs system
        this.initWorkflowTabs();

        // Load initial state
        this.refreshWorkflow();

        console.log('[WorkflowManager] 🎉 Initialization complete! All event listeners attached.');
    }

    initGraphEditor() {
        // Initialize after a delay to ensure Cytoscape is loaded
        setTimeout(() => {
            if (typeof GraphWorkflowEditor !== 'undefined') {
                this.graphEditor = new GraphWorkflowEditor('workflowGraph', this);
                console.log('[WorkflowManager] Graph editor initialized');
            } else {
                console.warn('[WorkflowManager] GraphWorkflowEditor not available');
            }
        }, 1000);
    }

    switchView(viewType) {
        console.log('[WorkflowManager] Switching to view:', viewType);

        this.currentView = viewType;

        const listView = document.getElementById('workflowListView');
        const graphView = document.getElementById('workflowGraphView');
        const viewListBtn = document.getElementById('viewListBtn');
        const viewGraphBtn = document.getElementById('viewGraphBtn');

        if (viewType === 'list') {
            listView.style.display = 'block';
            graphView.style.display = 'none';
            viewListBtn.classList.add('active');
            viewGraphBtn.classList.remove('active');
        } else {
            listView.style.display = 'none';
            graphView.style.display = 'block';
            viewListBtn.classList.remove('active');
            viewGraphBtn.classList.add('active');

            // Render graph when switching to graph view
            if (this.graphEditor && this.graph) {
                this.graphEditor.renderGraph(this.graph);
            }
        }
    }

    handleWorkflowResponse(message) {
        const { action, success, data, error, request_id } = message;

        // Check if we have a pending request for this ID
        if (request_id && this.pendingRequests.has(request_id)) {
            const { resolve, reject } = this.pendingRequests.get(request_id);
            this.pendingRequests.delete(request_id);

            if (success) {
                resolve({ success: true, data });
            } else {
                reject(new Error(error || 'Unknown error'));
            }
        }
    }

    /**
     * 🔥 SYNC 1:1: Handle real-time graph updates
     */
    handleGraphSync(graphData) {
        if (!graphData) return;

        // Update local model
        this.graph = graphData;

        // Persist to current tab
        this.saveCurrentTabWorkflow();

        // Update UI based on current view
        if (this.currentView === 'list') {
            this.renderAgents();
        }

        if (this.graphEditor) {
            // Use renderGraph (it now has smart diffing logic in graph-workflow-editor.js)
            this.graphEditor.renderGraph(this.graph);
        }

        this.updateAutoForwardSwitch();
    }

    /**
     * G-code Protocol Parser
     * Parses orchestrator messages with >>>> TARGET: markers
     * Returns array of { agentName, content } objects
     */
    parseGCodeMessage(messageText) {
        const targetMarkerRegex = /^>>>> TARGET:\s*(.+)$/gim;
        const blocks = [];
        let globalContext = '';
        let currentAgentName = null;
        let currentContent = [];

        const lines = messageText.split('\n');

        for (const line of lines) {
            const match = targetMarkerRegex.exec(line);
            targetMarkerRegex.lastIndex = 0; // Reset regex

            if (match) {
                // Save previous block if exists
                if (currentAgentName) {
                    blocks.push({
                        agentName: currentAgentName.trim(),
                        content: currentContent.join('\n').trim()
                    });
                }

                // Start new block
                currentAgentName = match[1];
                currentContent = [];
            } else {
                // If we haven't seen any TARGET marker yet, it's global context
                if (currentAgentName === null) {
                    globalContext += line + '\n';
                } else {
                    currentContent.push(line);
                }
            }
        }

        // Save last block
        if (currentAgentName) {
            blocks.push({
                agentName: currentAgentName.trim(),
                content: currentContent.join('\n').trim()
            });
        }

        return {
            globalContext: globalContext.trim(),
            blocks: blocks
        };
    }

    /**
     * Routes orchestrator message to multiple agents using G-code protocol
     * Called when receiving message from orchestrator agent
     */
    async routeOrchestratorMessage(orchestratorAgentId, messageText) {
        console.log('[WorkflowManager] 🎯 Routing orchestrator message from:', orchestratorAgentId);

        // Parse G-code
        const parsed = this.parseGCodeMessage(messageText);
        console.log('[WorkflowManager] 📋 Parsed G-code:', parsed);

        if (parsed.blocks.length === 0) {
            console.log('[WorkflowManager] ⚠️ No G-code blocks found, treating as normal message');
            return null; // No routing needed, normal flow
        }

        // Routing results
        const routingResults = [];

        for (const block of parsed.blocks) {
            // Find target agent by name (fuzzy match - case insensitive)
            const targetAgent = Object.values(this.graph.agents).find(agent =>
                agent.name.toLowerCase() === block.agentName.toLowerCase() ||
                agent.name.toLowerCase().includes(block.agentName.toLowerCase()) ||
                block.agentName.toLowerCase().includes(agent.name.toLowerCase())
            );

            if (!targetAgent) {
                console.warn('[WorkflowManager] ⚠️ Agent not found for TARGET:', block.agentName);
                routingResults.push({
                    agentName: block.agentName,
                    status: 'not_found',
                    error: `Agent "${block.agentName}" not found in workflow`
                });
                continue;
            }

            // Build message: Global Context + Agent-specific content
            const fullMessage = parsed.globalContext
                ? `${parsed.globalContext}\n\n---\n\n${block.content}`
                : block.content;

            // Check agent type for special handling
            const agentType = this.normalizeAgentType(targetAgent);

            if (agentType === 'AutoApply') {
                // Auto-Apply nodes process immediately without WebSocket
                console.log('[WorkflowManager] ⚡ Routing to Auto-Apply node:', targetAgent.name);
                await this.sendToAutoApplyNode(targetAgent.id, fullMessage);
                routingResults.push({
                    agentName: targetAgent.name,
                    status: 'auto_apply_queued'
                });
            } else if (agentType === 'Report') {
                // Report nodes don't need messages (they're collectors)
                console.log('[WorkflowManager] 📊 Skipping Report node:', targetAgent.name);
                routingResults.push({
                    agentName: targetAgent.name,
                    status: 'skipped_report_node'
                });
            } else {
                // Normal AI agent - send via workflow system
                console.log('[WorkflowManager] 🧠 Routing to AI agent:', targetAgent.name);
                await this.sendMessageToAgent(targetAgent.id, fullMessage);
                routingResults.push({
                    agentName: targetAgent.name,
                    status: 'sent'
                });
            }
        }

        console.log('[WorkflowManager] ✅ Routing complete:', routingResults);
        return routingResults;
    }

    /**
     * Sends message directly to a specific agent (bypassing normal flow)
     */
    async sendMessageToAgent(agentId, content) {
        // This will be implemented to send message via WebSocket to specific agent
        // For now, we log it
        console.log('[WorkflowManager] 📤 Sending message to agent:', agentId, content);

        // TODO: Implement WebSocket message sending
        // chrome.runtime.sendMessage({
        //     action: 'workflow_send_to_agent',
        //     agent_id: agentId,
        //     content: content
        // });
    }

    /**
     * Sends code changes to Auto-Apply node for automatic execution
     * AND chains the result to the next node in the workflow.
     */
    async sendToAutoApplyNode(agentId, content) {
        console.log('[WorkflowManager] ⚡ Processing Auto-Apply request for agent:', agentId);
        const agentName = this.getAgentName(agentId);

        try {
            // 1. Wywołaj backend, aby zaaplikować zmiany
            const response = await this.sendWorkflowMessage('workflow_auto_apply', {
                agent_id: agentId,
                content: content
            });

            // 2. Wygeneruj raport z działania
            let reportContent = '';
            let isSuccess = false;

            if (response && response.success) {
                const result = response.data;
                isSuccess = result.failed_count === 0;

                // Generuj czytelny raport Markdown
                reportContent = this.generateAutoApplyReport(agentName, result);

                this.showSuccessMessage(`Auto-Apply: ${result.applied_count} zmian zaaplikowanych`);
            } else {
                const errorMsg = response?.error || 'Unknown backend error';
                reportContent = `🚨 **AUTO-APPLY FAILED** (${agentName})\n\nSystem encountered a critical error:\n> ${errorMsg}`;
                this.showErrorMessage(`Auto-Apply failed: ${errorMsg}`);
            }

            // 3. Przekaż raport dalej w workflow (Chaining)
            // Używamy mechanizmu agent_send_message, udając że Auto-Apply node "odpowiada" swoim raportem.
            // Backend zajmie się routingiem do Agregatora lub innego Agenta.
            console.log('[WorkflowManager] 🔗 Chaining Auto-Apply result to next nodes...');

            // Wysyłamy wiadomość do background -> desktop, który użyje grafu do routingu
            chrome.runtime.sendMessage({
                action: 'agent_send_message',
                content: reportContent,
                agentId: agentId // ID węzła Auto-Apply jako nadawcy
            });

        } catch (error) {
            console.error('[WorkflowManager] Auto-Apply critical error:', error);
            this.showErrorMessage('Auto-Apply critical error: ' + error.message);

            // Próbuj wysłać raport o błędzie krytycznym dalej
            chrome.runtime.sendMessage({
                action: 'agent_send_message',
                content: `🚨 **CRITICAL EXCEPTION** (${agentName})\n\nJavaScript Error: ${error.message}`,
                agentId: agentId
            });
        }
    }

    /**
     * Helper to generate a nice Markdown report from Auto-Apply results
     */
    generateAutoApplyReport(agentName, result) {
        const icon = result.failed_count === 0 ? '✅' : '⚠️';
        let report = `### ${icon} Raport Auto-Apply: ${agentName}\n\n`;

        report += `**Podsumowanie:**\n`;
        report += `- Sukces: ${result.applied_count}\n`;
        report += `- Błędy: ${result.failed_count}\n\n`;

        if (result.changes && result.changes.length > 0) {
            report += `**Szczegóły zmian:**\n`;

            // Grupuj zmiany po plikach dla czytelności
            result.changes.forEach(change => {
                const statusIcon = change.status === 'success' ? '✔' : '❌';
                const path = change.file_path.split(/[\\/]/).pop(); // Tylko nazwa pliku

                report += `- ${statusIcon} **${path}**: ${change.status}`;
                if (change.error) {
                    report += `\n  > Błąd: ${change.error}`;
                }
                report += '\n';
            });
        } else {
            report += `_(Brak wykrytych bloków kodu do zaaplikowania)_\n`;
        }

        return report;
    }

    handleAggregatorUpdate(data) {
        // data structure: { aggregator_id, source_agent, status }
        console.log('[WorkflowManager] 📊 Handling aggregator update:', data);

        const { aggregator_id, source_agent } = data;
        if (!aggregator_id) return;

        // Znajdź element logów w kafelku agregatora
        const logContent = document.getElementById(`aggregator-log-content-${aggregator_id}`);
        if (logContent) {
            // Dodaj wpis
            const entry = document.createElement('div');
            entry.textContent = `📥 Odebrano od: ${source_agent}`;
            entry.style.cssText = "border-bottom: 1px solid rgba(255,255,255,0.1); padding: 2px 0;";

            // Wyczyść "Brak podłączonych źródeł" lub "Oczekuje..." jeśli to pierwszy wpis
            if (logContent.textContent.includes('Oczekuje') || logContent.textContent.includes('Brak')) {
                logContent.textContent = '';
            }

            logContent.appendChild(entry);

            // Auto-scroll
            const container = document.getElementById(`aggregator-logs-${aggregator_id}`);
            if (container) container.scrollTop = container.scrollHeight;
        }

        // Opcjonalnie: Zaktualizuj status w nagłówku
        const card = document.querySelector(`.agent-card[data-agent-id="${aggregator_id}"]`);
        if (card) {
            const statusBox = card.querySelector('.info-box:last-child');
            if (statusBox) {
                statusBox.innerHTML = `<strong>Status:</strong> ⚡ Przetwarzanie...`;
            }
        }
    }

    initModalListeners() {
        // Add Agent Modal - Preset Tabs
        document.querySelectorAll('.preset-tab-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const category = e.target.dataset.category;
                this.currentPresetCategory = category;

                // Update active tab
                document.querySelectorAll('.preset-tab-btn').forEach(b => b.classList.remove('active'));
                e.target.classList.add('active');

                // Render presets for category
                this.renderAgentPresets(category);
            });
        });

        // Add Agent Modal
        const addAgentForm = document.getElementById('addAgentForm');
        const addWrapperCheckbox = document.getElementById('addWrapperCheckbox');
        const wrapperInputGroup = document.getElementById('wrapperInputGroup');
        const cancelAddAgentBtn = document.getElementById('cancelAddAgentBtn');

        // Edit Agent Modal
        const editAgentForm = document.getElementById('editAgentForm');
        const editWrapperCheckbox = document.getElementById('editWrapperCheckbox');
        const editWrapperInputGroup = document.getElementById('editWrapperInputGroup');
        const cancelEditAgentBtn = document.getElementById('cancelEditAgentBtn');

        // Agent type radio buttons - change form based on type
        document.querySelectorAll('input[name="agentType"]').forEach(radio => {
            radio.addEventListener('change', (e) => {
                this.updateModalForAgentType(e.target.value);
            });
        });

        addWrapperCheckbox?.addEventListener('change', (e) => {
            wrapperInputGroup.style.display = e.target.checked ? 'block' : 'none';
        });

        addAgentForm?.addEventListener('submit', (e) => {
            e.preventDefault();
            this.handleAddAgent();
        });

        cancelAddAgentBtn?.addEventListener('click', () => {
            this.closeModal('addAgentModal');
        });

        // Edit Agent Modal listeners
        editWrapperCheckbox?.addEventListener('change', (e) => {
            editWrapperInputGroup.style.display = e.target.checked ? 'block' : 'none';
        });

        editAgentForm?.addEventListener('submit', (e) => {
            e.preventDefault();
            this.handleEditAgent();
        });

        cancelEditAgentBtn?.addEventListener('click', () => {
            this.closeModal('editAgentModal');
        });

        // Connect Agents Modal
        const connectAgentsForm = document.getElementById('connectAgentsForm');
        const addTemplateCheckbox = document.getElementById('addTemplateCheckbox');
        const templateInputGroup = document.getElementById('templateInputGroup');
        const cancelConnectBtn = document.getElementById('cancelConnectBtn');

        addTemplateCheckbox?.addEventListener('change', (e) => {
            templateInputGroup.style.display = e.target.checked ? 'block' : 'none';
        });

        connectAgentsForm?.addEventListener('submit', (e) => {
            e.preventDefault();
            this.handleConnectAgents();
        });

        cancelConnectBtn?.addEventListener('click', () => {
            this.closeModal('connectAgentsModal');
        });

        // Agent Details Modal
        const closeDetailsBtn = document.getElementById('closeDetailsBtn');
        closeDetailsBtn?.addEventListener('click', () => {
            this.closeModal('agentDetailsModal');
        });

        // Load Workflow Modal
        const cancelLoadWorkflowBtn = document.getElementById('cancelLoadWorkflowBtn');
        cancelLoadWorkflowBtn?.addEventListener('click', () => {
            this.closeModal('loadWorkflowModal');
        });

        // Close modals on background click
        document.querySelectorAll('.modal').forEach(modal => {
            modal.addEventListener('click', (e) => {
                if (e.target === modal) {
                    this.closeModal(modal.id);
                }
            });
        });

        // Close modals on Escape key
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') {
                // Close any open workflow modals
                ['addAgentModal', 'connectAgentsModal', 'agentDetailsModal'].forEach(modalId => {
                    const modal = document.getElementById(modalId);
                    if (modal && modal.style.display === 'flex') {
                        this.closeModal(modalId);
                    }
                });
            }
        });
    }
    showModal(modalId) {
        console.log('[WorkflowManager] 🎭 showModal called for:', modalId);
        const modal = document.getElementById(modalId);
        console.log('[WorkflowManager] Modal element:', modal);

        if (modal) {
            console.log('[WorkflowManager] Current modal styles:', {
                display: modal.style.display,
                opacity: modal.style.opacity,
                visibility: modal.style.visibility
            });

            // Reset stylów i pokazanie jako flex
            modal.style.cssText = '';
            modal.style.display = 'flex';
            modal.style.opacity = '1'; // Safety net

            console.log('[WorkflowManager] ✅ Modal display set to flex');
            console.log('[WorkflowManager] New modal styles:', {
                display: modal.style.display,
                opacity: modal.style.opacity,
                visibility: modal.style.visibility
            });
        } else {
            console.error('[WorkflowManager] ❌ Modal not found:', modalId);
        }
    }

    closeModal(modalId) {
        const modal = document.getElementById(modalId);
        if (modal) {
            modal.style.display = 'none';
            modal.style.opacity = '0';
            // Reset forms
            const form = modal.querySelector('form');
            if (form) {
                form.reset();
                // Hide conditional groups
                modal.querySelectorAll('[id$="InputGroup"]').forEach(group => {
                    group.style.display = 'none';
                });
            }
        }
    }

    sendWorkflowMessage(action, payload = {}) {
        return new Promise((resolve, reject) => {
            chrome.runtime.sendMessage({ action, ...payload }, (requestId) => {
                if (chrome.runtime.lastError) {
                    reject(new Error(chrome.runtime.lastError.message));
                    return;
                }

                if (!requestId) {
                    reject(new Error('No request ID returned'));
                    return;
                }

                // Store the promise handlers
                this.pendingRequests.set(requestId, { resolve, reject });

                // Timeout after 60 seconds (increased from 15s to prevent timeouts during heavy operations)
                setTimeout(() => {
                    if (this.pendingRequests.has(requestId)) {
                        this.pendingRequests.delete(requestId);
                        reject(new Error('Request timeout'));
                    }
                }, 60000);
            });
        });
    }

    async refreshWorkflow() {
        const refreshBtn = document.getElementById('refreshWorkflowBtn');
        if (refreshBtn) {
            refreshBtn.disabled = true;
            refreshBtn.style.opacity = '0.5';
        }

        try {
            const response = await this.sendWorkflowMessage('workflow_get_graph');

            if (response && response.success && response.data) {
                this.graph = response.data;

                // Save to active tab
                this.saveCurrentTabWorkflow();

                this.renderAgents();
                this.updateAutoForwardSwitch();
            } else {
                console.error('[Workflow] Invalid response:', response);
            }
        } catch (error) {
            console.error('[Workflow] Failed to refresh:', error);
            this.showErrorMessage('Failed to load workflow data');
        } finally {
            if (refreshBtn) {
                refreshBtn.disabled = false;
                refreshBtn.style.opacity = '1';
            }
        }
    }

    renderAgents() {
        const container = document.getElementById('agentsList');
        if (!container) return;

        if (!this.graph || Object.keys(this.graph.agents).length === 0) {
            container.innerHTML = `
                <div class="empty-state">
                    <div class="empty-icon">🤖</div>
                    <div class="empty-text">No agents configured</div>
                </div>
            `;

            // Also clear graph view if empty
            if (this.graphEditor) {
                this.graphEditor.renderGraph({ agents: {}, connections: [] });
            }
            return;
        }

        const agents = Object.values(this.graph.agents);
        container.innerHTML = agents.map(agent => this.renderAgentCard(agent)).join('');

        // Add event listeners
        agents.forEach(agent => {
            const card = document.querySelector(`[data-agent-id="${agent.id}"]`);

            // Drag & Drop functionality
            if (card) {
                card.draggable = true;

                card.addEventListener('dragstart', (e) => {
                    e.dataTransfer.effectAllowed = 'move';
                    e.dataTransfer.setData('text/html', card.innerHTML);
                    e.dataTransfer.setData('agent-id', agent.id);
                    card.style.opacity = '0.5';
                });

                card.addEventListener('dragend', (e) => {
                    card.style.opacity = '1';
                });

                card.addEventListener('dragover', (e) => {
                    e.preventDefault();
                    e.dataTransfer.dropEffect = 'move';
                    card.style.borderTop = '3px solid var(--accent-blue)';
                });

                card.addEventListener('dragleave', (e) => {
                    card.style.borderTop = '';
                });

                card.addEventListener('drop', (e) => {
                    e.preventDefault();
                    card.style.borderTop = '';

                    const draggedId = e.dataTransfer.getData('agent-id');
                    if (draggedId && draggedId !== agent.id) {
                        this.reorderAgents(draggedId, agent.id);
                    }
                });

                // Click on card header to edit
                const cardHeader = card.querySelector('.agent-header');
                if (cardHeader) {
                    cardHeader.style.cursor = 'pointer';
                    cardHeader.addEventListener('click', (e) => {
                        // Don't trigger if clicking on buttons
                        if (!e.target.closest('button')) {
                            this.showEditAgentModal(agent.id);
                        }
                    });
                }
            }

            // Delete button
            document.getElementById(`delete-${agent.id}`)?.addEventListener('click', (e) => {
                e.stopPropagation();
                this.deleteAgent(agent.id);
            });

            // Clone button
            document.getElementById(`clone-${agent.id}`)?.addEventListener('click', (e) => {
                e.stopPropagation();
                this.cloneAgent(agent.id);
            });

            // Connect buttons
            document.getElementById(`connect-from-${agent.id}`)?.addEventListener('click', (e) => {
                e.stopPropagation();
                this.selectAgentFrom(agent.id);
            });
            document.getElementById(`connect-to-${agent.id}`)?.addEventListener('click', (e) => {
                e.stopPropagation();
                this.connectAgentTo(agent.id);
            });

            // Connection delete buttons
            const outgoing = this.graph.connections.filter(c => c.from_agent_id === agent.id);
            outgoing.forEach((conn, idx) => {
                document.getElementById(`delete-conn-${agent.id}-${idx}`)?.addEventListener('click', (e) => {
                    e.stopPropagation();
                    this.deleteConnection(agent.id, conn.to_agent_id);
                });
            });

            // Auto-Apply specific buttons
            const agentType = this.normalizeAgentType(agent);
            if (agentType === 'AutoApply') {
                document.getElementById(`view-history-${agent.id}`)?.addEventListener('click', (e) => {
                    e.stopPropagation();
                    this.showAutoApplyHistory(agent.id);
                });
                document.getElementById(`clear-queue-${agent.id}`)?.addEventListener('click', (e) => {
                    e.stopPropagation();
                    this.clearAutoApplyQueue(agent.id);
                });
            } else if (agentType === 'Terminal') {
                document.getElementById(`pair-terminal-${agent.id}`)?.addEventListener('click', (e) => {
                    e.stopPropagation();
                    this.showTerminalPairingInstructions(agent);
                });
                document.getElementById(`config-terminal-${agent.id}`)?.addEventListener('click', (e) => {
                    e.stopPropagation();
                    this.showTerminalConfig(agent.id);
                });
            }
        });

        // Always update graph view when we have data (regardless of current view)
        if (this.graphEditor) {
            this.graphEditor.renderGraph(this.graph);
        }
    }

    normalizeAgentType(agent) {
        // Handle both string "Report" and object {"Report": null} from Rust enum
        if (!agent.agent_type) return 'Normal';
        if (typeof agent.agent_type === 'string') return agent.agent_type;
        if (typeof agent.agent_type === 'object') {
            if ('Report' in agent.agent_type) return 'Report';
            if ('AutoApply' in agent.agent_type) return 'AutoApply';
            if ('Terminal' in agent.agent_type) return 'Terminal';
            if ('Normal' in agent.agent_type) return 'Normal';
        }
        return 'Normal';
    }

    /**
     * Shows Auto-Apply history modal with applied changes and diff view
     */
    showAutoApplyHistory(agentId) {
        console.log('[WorkflowManager] 📜 Showing Auto-Apply history for:', agentId);

        // TODO: Fetch actual history from backend
        // For now, show placeholder modal
        const agentName = this.getAgentName(agentId);

        alert(`📜 Historia Auto-Apply dla "${agentName}"\n\n` +
              `Ta funkcja zostanie wkrótce zaimplementowana.\n` +
              `Pokaże:\n` +
              `- Listę zaaplikowanych zmian\n` +
              `- Diff view dla każdej zmiany\n` +
              `- Przycisk "Revert Batch" do cofnięcia grupy zmian`);
    }

    /**
     * Clears Auto-Apply queue
     */
    async clearAutoApplyQueue(agentId) {
        const agentName = this.getAgentName(agentId);
        if (!confirm(`Wyczyścić kolejkę Auto-Apply dla "${agentName}"?`)) return;

        try {
            await this.sendWorkflowMessage('workflow_clear_auto_apply_queue', {
                agent_id: agentId
            });
            this.showSuccessMessage('Kolejka Auto-Apply wyczyszczona');
        } catch (error) {
            console.error('[WorkflowManager] Failed to clear queue:', error);
            this.showErrorMessage('Błąd podczas czyszczenia kolejki');
        }
    }

    renderAgentCard(agent) {
        // Normalize agent type
        const agentType = this.normalizeAgentType(agent);

        // Check for special node types (different rendering)
        if (agentType === 'Report') {
            return this.renderAggregatorCard(agent);
        } else if (agentType === 'AutoApply') {
            return this.renderAutoApplyCard(agent);
        } else if (agentType === 'Terminal') {
            return this.renderTerminalCard(agent);
        }

        // Normal AI Agent rendering
        const statusClass = agent.status === 'Connected' ? 'status-connected' :
                           agent.status === 'Waiting' ? 'status-waiting' : 'status-disconnected';
        const statusIcon = agent.status === 'Connected' ? '🟢' :
                          agent.status === 'Waiting' ? '🟡' : '🔴';

        // Find connections
        const outgoing = this.graph.connections.filter(c => c.from_agent_id === agent.id);
        const incoming = this.graph.connections.filter(c => c.to_agent_id === agent.id);

        // Build connection rows with delete buttons
        const outgoingHtml = outgoing.length > 0
            ? outgoing.map((c, idx) => `
                <span class="connection-item">
                    ${this.getAgentName(c.to_agent_id)}
                    <button id="delete-conn-${agent.id}-${idx}" class="conn-delete-btn" title="Remove connection">×</button>
                </span>
            `).join('')
            : '<span style="color: var(--text-muted);">none</span>';

        const incomingHtml = incoming.length > 0
            ? incoming.map(c => this.getAgentName(c.from_agent_id)).join(', ')
            : '<span style="color: var(--text-muted);">none</span>';

        return `
            <div class="agent-card ${statusClass}" data-agent-id="${agent.id}" data-type="${agentType}">
                <div class="agent-header">
                    <div class="agent-name">🤖 ${this.escapeHtml(agent.name)}</div>
                    <div class="agent-status">${statusIcon} ${agent.status}</div>
                </div>
                <div class="agent-pairing">
                    <strong>Pairing Code:</strong> <code>${agent.pairing_code}</code>
                </div>
                ${agent.output_wrapper ? `
                    <div class="agent-wrapper">
                        <strong>Wrapper:</strong> <code title="${this.escapeHtml(agent.output_wrapper)}">${this.escapeHtml(this.truncate(agent.output_wrapper, 30))}</code>
                    </div>
                ` : ''}
                <div class="agent-connections">
                    <div class="connections-row">
                        <span style="color: var(--text-secondary); margin-right: 6px;">→ Sends to:</span>
                        ${outgoingHtml}
                    </div>
                    <div class="connections-row">
                        <span style="color: var(--text-secondary); margin-right: 6px;">← Receives from:</span>
                        ${incomingHtml}
                    </div>
                </div>
                <div class="agent-actions">
                    <button id="connect-from-${agent.id}" class="mini-btn" title="Ustaw jako źródło połączenia">→ From</button>
                    <button id="connect-to-${agent.id}" class="mini-btn" title="Połącz do tego agenta">To →</button>
                    <button id="clone-${agent.id}" class="mini-btn" title="Klonuj agenta">📋</button>
                    <button id="delete-${agent.id}" class="mini-btn delete-btn" title="Usuń agenta">🗑️</button>
                </div>
            </div>
        `;
    }

    renderAutoApplyCard(agent) {
        // Find connections
        const outgoing = this.graph.connections.filter(c => c.from_agent_id === agent.id);
        const incoming = this.graph.connections.filter(c => c.to_agent_id === agent.id);

        // Build connection lists
        const outgoingHtml = outgoing.length > 0
            ? outgoing.map((c, idx) => `
                <span class="connection-item">
                    ${this.getAgentName(c.to_agent_id)}
                    <button id="delete-conn-${agent.id}-${idx}" class="conn-delete-btn" title="Remove connection">×</button>
                </span>
            `).join('')
            : '<span style="color: var(--text-muted);">brak</span>';

        const incomingHtml = incoming.length > 0
            ? incoming.map(c => this.getAgentName(c.from_agent_id)).join(', ')
            : '<span style="color: var(--text-muted);">brak</span>';

        // Count queued changes (mock data for now)
        const queuedChanges = 0;
        const statusText = queuedChanges > 0
            ? `⚡ ${queuedChanges} zmian w kolejce`
            : '✅ Gotowy do pracy';

        return `
            <div class="agent-card auto-apply-card" data-agent-id="${agent.id}" data-type="AutoApply">
                <div class="agent-header">
                    <div class="agent-name">⚡ ${this.escapeHtml(agent.name)}</div>
                    <div class="agent-status" style="color: #10b981;">🤖 Executor</div>
                </div>
                <div class="aggregator-info">
                    <div class="info-box">
                        <strong>Typ:</strong> Auto-Apply Executor (NIE model AI)
                    </div>
                    <div class="info-box">
                        <strong>Status:</strong> ${statusText}
                    </div>
                </div>
                <div class="agent-connections">
                    <div class="connections-row">
                        <span style="color: var(--text-secondary); margin-right: 6px;">📥 Odbiera od:</span>
                        ${incomingHtml}
                    </div>
                    <div class="connections-row">
                        <span style="color: var(--text-secondary); margin-right: 6px;">📤 Wysyła do (error output):</span>
                        ${outgoingHtml}
                    </div>
                </div>
                <div class="aggregator-description">
                    💡 <em>Automatycznie aplikuje bloki kodu <<<< SEARCH / >>>> REPLACE bez interakcji użytkownika</em>
                </div>
                <div class="auto-apply-actions" style="margin-top: 12px; display: flex; gap: 8px;">
                    <button id="view-history-${agent.id}" class="mini-btn" style="background: #3b82f6;" title="Zobacz historię zmian">📜 Historia</button>
                    <button id="clear-queue-${agent.id}" class="mini-btn" style="background: #f59e0b;" title="Wyczyść kolejkę">🗑️ Wyczyść</button>
                </div>
                <div class="agent-actions" style="margin-top: 8px;">
                    <button id="connect-from-${agent.id}" class="mini-btn" title="Ustaw jako źródło połączenia">→ From</button>
                    <button id="connect-to-${agent.id}" class="mini-btn" title="Połącz do tego węzła">To →</button>
                    <button id="clone-${agent.id}" class="mini-btn" title="Klonuj węzeł">📋</button>
                    <button id="delete-${agent.id}" class="mini-btn delete-btn" title="Usuń węzeł">🗑️</button>
                </div>
            </div>
        `;
    }

    renderAggregatorCard(agent) {
        // Find connections
        const outgoing = this.graph.connections.filter(c => c.from_agent_id === agent.id);
        const incoming = this.graph.connections.filter(c => c.to_agent_id === agent.id);

        // Build connection lists
        const outgoingHtml = outgoing.length > 0
            ? outgoing.map((c, idx) => `
                <span class="connection-item">
                    ${this.getAgentName(c.to_agent_id)}
                    <button id="delete-conn-${agent.id}-${idx}" class="conn-delete-btn" title="Remove connection">×</button>
                </span>
            `).join('')
            : '<span style="color: var(--text-muted);">brak</span>';

        const incomingHtml = incoming.length > 0
            ? incoming.map(c => this.getAgentName(c.from_agent_id)).join(', ')
            : '<span style="color: var(--text-muted);">brak</span>';

        // Count how many sources are connected
        const sourceCount = incoming.length;
        const statusText = sourceCount > 0
            ? `Zbiera od ${sourceCount} ${sourceCount === 1 ? 'agenta' : 'agentów'}`
            : 'Oczekuje na połączenia';

        return `
            <div class="agent-card aggregator-card" data-agent-id="${agent.id}" data-type="Report">
                <div class="agent-header">
                    <div class="agent-name">🗂️ ${this.escapeHtml(agent.name)}</div>
                    <div class="agent-status" style="color: #fbbf24;">📊 Kolektor</div>
                </div>
                <div class="aggregator-info">
                    <div class="info-box">
                        <strong>Typ:</strong> Agregator odpowiedzi (NIE model AI)
                    </div>
                    <div class="info-box">
                        <strong>Status:</strong> ${statusText}
                    </div>
                </div>
                <div class="agent-connections">
                    <div class="connections-row">
                        <span style="color: var(--text-secondary); margin-right: 6px;">📥 Zbiera od:</span>
                        ${incomingHtml}
                    </div>
                    <div class="connections-row">
                        <span style="color: var(--text-secondary); margin-right: 6px;">📤 Wysyła do:</span>
                        ${outgoingHtml}
                    </div>
                </div>
                <div class="aggregator-description">
                    💡 <em>Czeka na wszystkie odpowiedzi, łączy je z podpisami i wysyła dalej</em>
                </div>
                <div class="aggregator-logs" id="aggregator-logs-${agent.id}" style="margin: 8px 0; padding: 8px; background: rgba(0, 0, 0, 0.3); border-radius: 4px; max-height: 100px; overflow-y: auto; font-size: 10px; font-family: monospace; color: #a0aec0;">
                    <div style="color: #fbbf24; font-weight: bold; margin-bottom: 4px;">📋 Logi zbierania:</div>
                    <div id="aggregator-log-content-${agent.id}">
                        ${sourceCount > 0 ? `Oczekuje na ${sourceCount} odpowiedzi...` : 'Brak podłączonych źródeł'}
                    </div>
                </div>
                <div class="agent-actions">
                    <button id="connect-from-${agent.id}" class="mini-btn" title="Ustaw jako źródło połączenia">→ From</button>
                    <button id="connect-to-${agent.id}" class="mini-btn" title="Połącz do tego agenta">To →</button>
                    <button id="clone-${agent.id}" class="mini-btn" title="Klonuj agregator">📋</button>
                    <button id="delete-${agent.id}" class="mini-btn delete-btn" title="Usuń agregator">🗑️</button>
                </div>
            </div>
        `;
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    getAgentName(agentId) {
        return this.graph?.agents?.[agentId]?.name || 'Unknown';
    }

    truncate(str, maxLen) {
        if (!str) return '';
        if (str.length <= maxLen) return str;
        return str.substring(0, maxLen - 3) + '...';
    }

    // Check if connection already exists
    connectionExists(fromId, toId) {
        if (!this.graph || !this.graph.connections) return false;
        return this.graph.connections.some(
            c => c.from_agent_id === fromId && c.to_agent_id === toId
        );
    }

    injectMissingAgentTypes() {
        // Znajdź grupę radiową w formularzu dodawania agenta
        const container = document.querySelector('#addAgentForm .radio-group');
        if (!container) return;

        // Helper do tworzenia HTML
        const createOption = (value, icon, title, desc) => `
            <label>
                <input type="radio" name="agentType" value="${value}">
                <span style="font-size: 1.5em; margin-right: 10px;">${icon}</span>
                <div style="flex: 1;">
                    <div style="font-weight: 600; font-size: 13px;">${title}</div>
                    <div style="font-size: 11px; opacity: 0.7; line-height: 1.2;">${desc}</div>
                </div>
            </label>
        `;

        // 1. Dodaj Auto-Apply jeśli nie istnieje
        if (!container.querySelector('input[value="AutoApply"]')) {
            const html = createOption(
                'AutoApply',
                '⚡',
                'Auto-Apply Executor',
                'Automatycznie aplikuje zmiany w kodzie (NIE model AI)'
            );
            container.insertAdjacentHTML('beforeend', html);
        }

        // 2. Dodaj Terminal jeśli nie istnieje
        if (!container.querySelector('input[value="Terminal"]')) {
            const html = createOption(
                'Terminal',
                '🖥️',
                'Terminal Sensor',
                'Nasłuchuje outputu z VS Code i przekazuje dalej'
            );
            container.insertAdjacentHTML('beforeend', html);
        }
    }

    updateModalForAgentType(agentType) {
        const systemPromptGroup = document.querySelector('#agentSystemPrompt')?.closest('.form-group');
        const wrapperGroup = document.querySelector('#addWrapperCheckbox')?.closest('.form-group');
        const wrapperInputGroup = document.getElementById('wrapperInputGroup');
        const modalTitle = document.querySelector('#addAgentModal .modal-title');
        const agentNameGroup = document.querySelector('#agentNameInput')?.closest('.form-group');

        // Remove any existing info box
        const existingInfo = document.querySelector('.aggregator-mode-info');
        if (existingInfo) existingInfo.remove();

        if (agentType === 'Report') {
            // Agregator mode - hide AI-specific fields
            if (systemPromptGroup) systemPromptGroup.style.display = 'none';
            if (wrapperGroup) wrapperGroup.style.display = 'none';
            if (wrapperInputGroup) wrapperInputGroup.style.display = 'none';
            if (modalTitle) modalTitle.textContent = '🗂️ Dodaj Agregator Raportów';

            // Add info box explaining aggregator
            if (agentNameGroup) {
                const infoBox = document.createElement('div');
                infoBox.className = 'aggregator-mode-info';
                infoBox.style.cssText = 'background: rgba(251, 191, 36, 0.1); border: 2px dashed #fbbf24; border-radius: 8px; padding: 12px; margin-bottom: 16px;';
                infoBox.innerHTML = `
                    <div style="display: flex; align-items: start; gap: 8px;">
                        <div style="font-size: 24px;">🗂️</div>
                        <div style="flex: 1;">
                            <strong style="color: #fbbf24; display: block; margin-bottom: 4px;">Tryb Agregatora</strong>
                            <div style="font-size: 12px; color: var(--text-secondary);">
                                • NIE paruje się z modelem AI<br>
                                • Zbiera odpowiedzi od podłączonych agentów<br>
                                • Czeka aż wszystkie się zgłoszą<br>
                                • Łączy w jeden plik z podpisami źródłowymi<br>
                                • Wysyła zbiorczy raport dalej
                            </div>
                        </div>
                    </div>
                `;
                agentNameGroup.parentNode.insertBefore(infoBox, agentNameGroup);
            }
        } else if (agentType === 'AutoApply') {
            // Auto-Apply mode - hide AI-specific fields
            if (systemPromptGroup) systemPromptGroup.style.display = 'none';
            if (wrapperGroup) wrapperGroup.style.display = 'none';
            if (wrapperInputGroup) wrapperInputGroup.style.display = 'none';
            if (modalTitle) modalTitle.textContent = '⚡ Dodaj Auto-Apply Executor';

            // Add info box explaining Auto-Apply
            if (agentNameGroup) {
                const infoBox = document.createElement('div');
                infoBox.className = 'aggregator-mode-info';
                infoBox.style.cssText = 'background: rgba(16, 185, 129, 0.1); border: 2px dashed #10b981; border-radius: 8px; padding: 12px; margin-bottom: 16px;';
                infoBox.innerHTML = `
                    <div style="display: flex; align-items: start; gap: 8px;">
                        <div style="font-size: 24px;">⚡</div>
                        <div style="flex: 1;">
                            <strong style="color: #10b981; display: block; margin-bottom: 4px;">Tryb Auto-Apply</strong>
                            <div style="font-size: 12px; color: var(--text-secondary);">
                                • NIE paruje się z modelem AI<br>
                                • Automatycznie aplikuje bloki kodu <<<< SEARCH / >>>> REPLACE<br>
                                • Działa w trybie cichym (silent mode)<br>
                                • Zapisuje historię zmian z możliwością cofnięcia<br>
                                • Wysyła raporty błędów przez Error Output
                            </div>
                        </div>
                    </div>
                `;
                agentNameGroup.parentNode.insertBefore(infoBox, agentNameGroup);
            }
        } else if (agentType === 'Terminal') {
            // Terminal mode - hide AI-specific fields
            if (systemPromptGroup) systemPromptGroup.style.display = 'none';
            if (wrapperGroup) wrapperGroup.style.display = 'none';
            if (wrapperInputGroup) wrapperInputGroup.style.display = 'none';
            if (modalTitle) modalTitle.textContent = '🖥️ Dodaj Terminal Sensor';

            // Add info box explaining Terminal
            if (agentNameGroup) {
                const infoBox = document.createElement('div');
                infoBox.className = 'aggregator-mode-info';
                infoBox.style.cssText = 'background: rgba(96, 165, 250, 0.1); border: 2px dotted #60a5fa; border-radius: 8px; padding: 12px; margin-bottom: 16px;';
                infoBox.innerHTML = `
                    <div style="display: flex; align-items: start; gap: 8px;">
                        <div style="font-size: 24px;">🖥️</div>
                        <div style="flex: 1;">
                            <strong style="color: #60a5fa; display: block; margin-bottom: 4px;">Tryb Terminal Sensor</strong>
                            <div style="font-size: 12px; color: var(--text-secondary);">
                                • NIE paruje się z modelem AI<br>
                                • Przechwytuje output z terminala VS Code<br>
                                • Automatycznie strip ANSI codes (opcjonalnie)<br>
                                • Limituje liczbę linii (bufor)<br>
                                • Wysyła logi do agentów w workflow
                            </div>
                        </div>
                    </div>
                `;
                agentNameGroup.parentNode.insertBefore(infoBox, agentNameGroup);
            }
        } else {
            // Normal AI Agent mode - show all fields
            if (systemPromptGroup) systemPromptGroup.style.display = 'block';
            if (wrapperGroup) wrapperGroup.style.display = 'block';
            if (modalTitle) modalTitle.textContent = '🤖 Dodaj Nowego Agenta';
        }
    }

    showAddAgentModal() {
        console.log('[WorkflowManager] 📝 showAddAgentModal called');

        // Reset selection
        this.selectedAgentPreset = null;
        this.currentPresetCategory = 'all';
        console.log('[WorkflowManager] Reset preset selection');

        // Render presets
        console.log('[WorkflowManager] Rendering presets for category: all');
        this.renderAgentPresets('all');

        // Clear form
        const agentNameInput = document.getElementById('agentNameInput');
        const agentSystemPrompt = document.getElementById('agentSystemPrompt');
        const wrapperTemplateInput = document.getElementById('wrapperTemplateInput');
        const addWrapperCheckbox = document.getElementById('addWrapperCheckbox');
        const wrapperInputGroup = document.getElementById('wrapperInputGroup');

        console.log('[WorkflowManager] Form elements:', {
            agentNameInput: !!agentNameInput,
            agentSystemPrompt: !!agentSystemPrompt,
            wrapperTemplateInput: !!wrapperTemplateInput,
            addWrapperCheckbox: !!addWrapperCheckbox,
            wrapperInputGroup: !!wrapperInputGroup
        });

        if (agentNameInput) agentNameInput.value = '';
        if (agentSystemPrompt) agentSystemPrompt.value = '';
        if (wrapperTemplateInput) wrapperTemplateInput.value = '';
        if (addWrapperCheckbox) addWrapperCheckbox.checked = false;
        if (wrapperInputGroup) wrapperInputGroup.style.display = 'none';

        // Reset to Normal agent type
        const normalRadio = document.querySelector('input[name="agentType"][value="Normal"]');
        if (normalRadio) {
            normalRadio.checked = true;
            this.updateModalForAgentType('Normal');
        }

        console.log('[WorkflowManager] Calling showModal with addAgentModal');
        this.showModal('addAgentModal');
        console.log('[WorkflowManager] ✅ showModal called');
    }

    renderAgentPresets(category = 'all') {
        const presetList = document.getElementById('presetList');
        if (!presetList) return;

        const presets = this.presetManager.getFilteredAgentPresets(category);

        if (presets.length === 0) {
            presetList.innerHTML = `
                <div style="text-align: center; color: var(--text-muted); padding: 20px;">
                    ${category === 'favorites' ? '⭐ Brak ulubionych presetów' : 'Brak presetów w tej kategorii'}
                </div>
            `;
            return;
        }

        presetList.innerHTML = presets.map(preset => `
            <div class="preset-item ${this.selectedAgentPreset?.id === preset.id ? 'selected' : ''}"
                 data-preset-id="${preset.id}">
                <div class="preset-icon">${preset.icon}</div>
                <div class="preset-info">
                    <div class="preset-name">${preset.name}</div>
                    <div class="preset-description">${preset.description}</div>
                </div>
                <div class="preset-favorite ${this.presetManager.isFavorite(preset.id) ? 'active' : ''}"
                     data-preset-id="${preset.id}"
                     title="Dodaj do ulubionych">
                    ${this.presetManager.isFavorite(preset.id) ? '⭐' : '☆'}
                </div>
            </div>
        `).join('');

        // Add event listeners
        presetList.querySelectorAll('.preset-item').forEach(item => {
            const presetId = item.dataset.presetId;

            // Click on item (not favorite star)
            item.addEventListener('click', (e) => {
                if (!e.target.classList.contains('preset-favorite')) {
                    this.selectAgentPreset(presetId);
                }
            });
        });

        // Favorite star clicks
        presetList.querySelectorAll('.preset-favorite').forEach(star => {
            star.addEventListener('click', (e) => {
                e.stopPropagation();
                const presetId = star.dataset.presetId;
                this.togglePresetFavorite(presetId);
            });
        });
    }

    selectAgentPreset(presetId) {
        const preset = this.presetManager.getAgentPreset(presetId);
        if (!preset) return;

        this.selectedAgentPreset = preset;

        // Update UI - highlight selected
        document.querySelectorAll('.preset-item').forEach(item => {
            item.classList.toggle('selected', item.dataset.presetId === presetId);
        });

        // Set agent type based on preset
        const agentType = preset.id === 'report_aggregator' ? 'Report' : 'Normal';
        const typeRadio = document.querySelector(`input[name="agentType"][value="${agentType}"]`);
        if (typeRadio) {
            typeRadio.checked = true;
            this.updateModalForAgentType(agentType);
        }

        // Fill form with preset data
        document.getElementById('agentNameInput').value = preset.name;

        // Only fill system prompt for Normal agents
        if (agentType === 'Normal') {
            document.getElementById('agentSystemPrompt').value = preset.systemPrompt || '';
        }

        if (preset.outputWrapper && agentType === 'Normal') {
            document.getElementById('addWrapperCheckbox').checked = true;
            document.getElementById('wrapperTemplateInput').value = preset.outputWrapper;
            document.getElementById('wrapperInputGroup').style.display = 'block';
        }

        console.log('[WorkflowManager] Selected preset:', preset.name);
    }

    togglePresetFavorite(presetId) {
        this.presetManager.toggleFavorite(presetId);

        // Re-render current category
        this.renderAgentPresets(this.currentPresetCategory);

        console.log('[WorkflowManager] Toggled favorite:', presetId);
    }

    showLoadWorkflowModal() {
        console.log('[WorkflowManager] showLoadWorkflowModal called');
        this.renderWorkflowTemplates();
        this.showModal('loadWorkflowModal');
    }

    renderWorkflowTemplates() {
        const templateList = document.getElementById('workflowTemplateList');
        if (!templateList) return;

        const templates = this.presetManager.presets.workflows;

        templateList.innerHTML = templates.map(template => `
            <div class="workflow-template-item" data-template-id="${template.id}">
                <div class="workflow-template-header">
                    <div class="workflow-template-icon">${template.icon}</div>
                    <div class="workflow-template-title">${template.name}</div>
                </div>
                <div class="workflow-template-description">${template.description}</div>
                <div class="workflow-template-stats">
                    <div class="workflow-template-stat">
                        <span>🤖</span>
                        <span>${template.agents.length} agentów</span>
                    </div>
                    <div class="workflow-template-stat">
                        <span>🔗</span>
                        <span>${template.connections.length} połączeń</span>
                    </div>
                </div>
            </div>
        `).join('');

        // Add event listeners
        templateList.querySelectorAll('.workflow-template-item').forEach(item => {
            const templateId = item.dataset.templateId;
            item.addEventListener('click', () => this.loadWorkflowTemplate(templateId));
        });
    }

    async loadWorkflowTemplate(templateId) {
        const template = this.presetManager.getWorkflowPreset(templateId);
        if (!template) return;

        console.log('[WorkflowManager] Loading workflow template:', template.name);

        try {
            // Ask user: load in current tab or new tab?
            const hasContent = this.graph && Object.keys(this.graph.agents).length > 0;
            let loadInNewTab = false;

            if (hasContent) {
                const choice = confirm(
                    `Bieżąca zakładka zawiera workflow.\n\n` +
                    `• OK - Utwórz nową zakładkę dla szablonu\n` +
                    `• Anuluj - Zastąp bieżące workflow`
                );
                loadInNewTab = choice;
            }

            // Close modal
            this.closeModal('loadWorkflowModal');

            // Create new tab if requested
            if (loadInNewTab) {
                this.createNewTab(template.name, true);
            }

            // Clear current workflow
            if (!loadInNewTab && hasContent) {
                // Clear agents via backend (simplified approach - just clear local graph)
                this.graph = {
                    agents: {},
                    connections: [],
                    auto_forward: false
                };
            }

            // Create agents from template
            const createdAgents = {};
            for (const agentConfig of template.agents) {
                const preset = this.presetManager.getAgentPreset(agentConfig.presetId);
                if (!preset) {
                    console.warn('[WorkflowManager] Preset not found:', agentConfig.presetId);
                    continue;
                }

                const response = await this.sendWorkflowMessage('workflow_add_agent', {
                    name: agentConfig.instanceName,
                    output_wrapper: preset.outputWrapper || null,
                    agent_type: preset.id === 'report_aggregator' ? 'Report' : 'Normal',
                    position: agentConfig.position ? [agentConfig.position[0], agentConfig.position[1]] : null
                });

                if (response && response.success) {
                    createdAgents[agentConfig.instanceName] = response.data.id;
                }

                // Small delay between creations
                await new Promise(resolve => setTimeout(resolve, 100));
            }

            // Create connections from template
            for (const connConfig of template.connections) {
                const fromId = createdAgents[connConfig.from];
                const toId = createdAgents[connConfig.to];

                if (!fromId || !toId) {
                    console.warn('[WorkflowManager] Agent not found for connection:', connConfig);
                    continue;
                }

                // Get connection template if specified
                let messageTemplate = null;
                if (connConfig.templatePresetId) {
                    const connPreset = this.presetManager.getConnectionPreset(connConfig.templatePresetId);
                    if (connPreset) {
                        messageTemplate = connPreset.messageTemplate;
                    }
                }

                await this.sendWorkflowMessage('workflow_add_connection', {
                    from_id: fromId,
                    to_id: toId,
                    template: messageTemplate
                });

                await new Promise(resolve => setTimeout(resolve, 100));
            }

            this.showSuccessMessage(`✅ Załadowano workflow: ${template.name}`);
            await this.refreshWorkflow();

        } catch (error) {
            console.error('[WorkflowManager] Failed to load template:', error);
            this.showErrorMessage('Błąd podczas ładowania szablonu');
        }
    }

    renderConnectionPresets() {
        const presetList = document.getElementById('connectionPresetList');
        if (!presetList) return;

        const presets = this.presetManager.presets.connections;

        presetList.innerHTML = presets.map(preset => `
            <div class="connection-preset-item ${this.selectedConnectionPreset?.id === preset.id ? 'selected' : ''}"
                 data-preset-id="${preset.id}">
                <div class="connection-preset-name">${preset.name}</div>
                <div class="connection-preset-description">${preset.description}</div>
                <div class="connection-preset-example">${preset.example}</div>
            </div>
        `).join('');

        // Add event listeners
        presetList.querySelectorAll('.connection-preset-item').forEach(item => {
            const presetId = item.dataset.presetId;
            item.addEventListener('click', () => this.selectConnectionPreset(presetId));
        });
    }

    selectConnectionPreset(presetId) {
        const preset = this.presetManager.getConnectionPreset(presetId);
        if (!preset) return;

        this.selectedConnectionPreset = preset;

        // Update UI
        document.querySelectorAll('.connection-preset-item').forEach(item => {
            item.classList.toggle('selected', item.dataset.presetId === presetId);
        });

        // Fill custom template field (but keep it hidden for now)
        const templateInput = document.getElementById('messageTemplateInput');
        if (templateInput) {
            templateInput.value = preset.messageTemplate;
        }

        // Uncheck custom checkbox since we're using preset
        const customCheckbox = document.getElementById('addTemplateCheckbox');
        if (customCheckbox) {
            customCheckbox.checked = false;
        }

        console.log('[WorkflowManager] Selected connection preset:', preset.name);
    }

    async handleAddAgent() {
        const nameInput = document.getElementById('agentNameInput');
        const wrapperCheckbox = document.getElementById('addWrapperCheckbox');
        const wrapperInput = document.getElementById('wrapperTemplateInput');
        const agentTypeRadio = document.querySelector('input[name="agentType"]:checked');

        const name = nameInput.value.trim();
        if (!name) {
            this.showErrorMessage('Agent name is required');
            return;
        }

        const wrapper = wrapperCheckbox.checked ? wrapperInput.value.trim() : null;
        const agentType = agentTypeRadio ? agentTypeRadio.value : 'Normal';

        // Validate wrapper template if provided
        if (wrapper && !wrapper.includes('{content}')) {
            this.showErrorMessage('Wrapper template must contain {content} placeholder');
            return;
        }

        try {
            const response = await this.sendWorkflowMessage('workflow_add_agent', {
                name: name,
                output_wrapper: wrapper || null,
                agent_type: agentType,
                position: null
            });

            if (response && response.success) {
                this.closeModal('addAgentModal');
                if (agentType === 'Report') {
                    this.showSuccessMessage(`Agregator "${name}" utworzony! 🗂️ (Kolektor odpowiedzi - nie wymaga parowania)`);
                } else {
                    this.showSuccessMessage(`Agent "${name}" utworzony! Kod parowania: ${response.data.pairing_code}`);
                }
                await this.refreshWorkflow();
            } else {
                this.showErrorMessage('Failed to create agent: ' + (response?.error || 'Unknown error'));
            }
        } catch (error) {
            console.error('[Workflow] Failed to add agent:', error);
            this.showErrorMessage('Error adding agent: ' + error.message);
        }
    }

    showEditAgentModal(agentId) {
        const agent = this.graph?.agents?.[agentId];
        if (!agent) {
            this.showErrorMessage('Agent nie znaleziony');
            return;
        }

        console.log('[WorkflowManager] Opening edit modal for agent:', agent.name);

        // Fill form with agent data
        document.getElementById('editAgentId').value = agentId;
        document.getElementById('editAgentName').value = agent.name;

        // Display agent type (read-only)
        const agentType = this.normalizeAgentType(agent);
        const typeDisplay = document.getElementById('editAgentTypeDisplay');
        if (agentType === 'Report') {
            typeDisplay.innerHTML = '🗂️ Agregator Raportów (NIE jest modelem AI)';
            // Hide prompt fields for aggregator
            document.getElementById('editSystemPromptGroup').style.display = 'none';
            document.getElementById('editWrapperGroup').style.display = 'none';
            document.getElementById('editWrapperInputGroup').style.display = 'none';
        } else {
            typeDisplay.innerHTML = '🤖 Agent AI (Paruje się z modelem Claude)';
            // Show prompt fields for normal agent
            document.getElementById('editSystemPromptGroup').style.display = 'block';
            document.getElementById('editWrapperGroup').style.display = 'block';

            // Fill system prompt (currently not stored in backend, but we prepare for future)
            document.getElementById('editAgentSystemPrompt').value = agent.system_prompt || '';

            // Fill wrapper
            const hasWrapper = agent.output_wrapper && agent.output_wrapper.trim() !== '';
            document.getElementById('editWrapperCheckbox').checked = hasWrapper;
            document.getElementById('editWrapperTemplateInput').value = agent.output_wrapper || '';
            document.getElementById('editWrapperInputGroup').style.display = hasWrapper ? 'block' : 'none';
        }

        this.showModal('editAgentModal');
    }

    async handleEditAgent() {
        const agentId = document.getElementById('editAgentId').value;
        const nameInput = document.getElementById('editAgentName');
        const wrapperCheckbox = document.getElementById('editWrapperCheckbox');
        const wrapperInput = document.getElementById('editWrapperTemplateInput');
        const systemPromptInput = document.getElementById('editAgentSystemPrompt');

        const name = nameInput.value.trim();
        if (!name) {
            this.showErrorMessage('Nazwa agenta jest wymagana');
            return;
        }

        const wrapper = wrapperCheckbox.checked ? wrapperInput.value.trim() : '';

        // Validate wrapper template if provided
        if (wrapper && !wrapper.includes('{content}')) {
            this.showErrorMessage('Szablon wrappera musi zawierać placeholder {content}');
            return;
        }

        try {
            const response = await this.sendWorkflowMessage('workflow_update_agent', {
                agent_id: agentId,
                name: name,
                output_wrapper: wrapper, // Send empty string to clear wrapper, or value to set it
                system_prompt: systemPromptInput.value.trim() || ''
            });

            if (response && response.success) {
                this.closeModal('editAgentModal');
                this.showSuccessMessage(`✅ Agent "${name}" zaktualizowany`);
                await this.refreshWorkflow();
            } else {
                this.showErrorMessage('Nie udało się zaktualizować agenta: ' + (response?.error || 'Nieznany błąd'));
            }
        } catch (error) {
            console.error('[Workflow] Failed to update agent:', error);
            this.showErrorMessage('Błąd podczas aktualizacji agenta: ' + error.message);
        }
    }

    async deleteAgent(agentId) {
        const agentName = this.getAgentName(agentId);
        if (!confirm(`Usunąć agenta "${agentName}" i wszystkie jego połączenia?`)) return;

        try {
            const response = await this.sendWorkflowMessage('workflow_remove_agent', {
                agent_id: agentId
            });

            if (response && response.success) {
                this.showSuccessMessage(`Agent "${agentName}" usunięty`);
                await this.refreshWorkflow();
            } else {
                this.showErrorMessage('Nie udało się usunąć agenta');
            }
        } catch (error) {
            console.error('[Workflow] Failed to delete agent:', error);
            this.showErrorMessage('Błąd podczas usuwania agenta');
        }
    }

    async cloneAgent(agentId) {
        const agent = this.graph?.agents?.[agentId];
        if (!agent) {
            this.showErrorMessage('Agent nie znaleziony');
            return;
        }

        console.log('[WorkflowManager] Cloning agent:', agent.name);

        try {
            // Create new agent with same properties (but new name)
            const newName = `${agent.name} (kopia)`;
            const response = await this.sendWorkflowMessage('workflow_add_agent', {
                name: newName,
                output_wrapper: agent.output_wrapper || null,
                agent_type: agent.agent_type,
                position: agent.position ? [agent.position[0] + 50, agent.position[1] + 50] : null
            });

            if (response && response.success) {
                this.showSuccessMessage(`✅ Sklonowano agenta: ${newName}`);
                await this.refreshWorkflow();
            } else {
                this.showErrorMessage('Nie udało się sklonować agenta');
            }
        } catch (error) {
            console.error('[Workflow] Failed to clone agent:', error);
            this.showErrorMessage('Błąd podczas klonowania agenta');
        }
    }

    selectAgentFrom(agentId) {
        this.selectedAgentFrom = agentId;
        // Visual feedback - highlight selected agent
        document.querySelectorAll('.agent-card').forEach(card => {
            card.style.opacity = card.dataset.agentId === agentId ? '1' : '0.5';
        });
        this.showSuccessMessage(`Selected "${this.getAgentName(agentId)}" as source. Now click "To →" on target agent.`);
    }

    async connectAgentTo(toAgentId) {
        if (!this.selectedAgentFrom) {
            this.showErrorMessage('Najpierw wybierz agenta źródłowego używając przycisku "→ From"');
            return;
        }

        if (this.selectedAgentFrom === toAgentId) {
            this.showErrorMessage('Nie można połączyć agenta ze sobą');
            this.selectedAgentFrom = null;
            // Reset opacity
            document.querySelectorAll('.agent-card').forEach(card => {
                card.style.opacity = '1';
            });
            return;
        }

        // Check if connection already exists
        if (this.connectionExists(this.selectedAgentFrom, toAgentId)) {
            this.showErrorMessage('Połączenie już istnieje między tymi agentami');
            return;
        }

        // Show connection modal
        const fromName = this.getAgentName(this.selectedAgentFrom);
        const toName = this.getAgentName(toAgentId);

        document.getElementById('connectFromId').value = this.selectedAgentFrom;
        document.getElementById('connectToId').value = toAgentId;
        document.getElementById('connectFromLabel').textContent = fromName;
        document.getElementById('connectToLabel').textContent = toName;

        // Reset connection preset selection
        this.selectedConnectionPreset = null;

        // Render connection presets
        this.renderConnectionPresets();

        this.showModal('connectAgentsModal');
    }

    async handleConnectAgents() {
        const fromId = document.getElementById('connectFromId').value;
        const toId = document.getElementById('connectToId').value;
        const templateCheckbox = document.getElementById('addTemplateCheckbox');
        const templateInput = document.getElementById('messageTemplateInput');

        // Use selected preset or custom template
        let template = null;

        if (this.selectedConnectionPreset) {
            // Use preset template
            template = this.selectedConnectionPreset.messageTemplate;
        } else if (templateCheckbox.checked) {
            // Use custom template
            template = templateInput.value.trim();
        }

        // Validate template if provided
        if (template && !template.includes('{content}')) {
            this.showErrorMessage('Szablon wiadomości musi zawierać placeholder {content}');
            return;
        }

        try {
            const response = await this.sendWorkflowMessage('workflow_add_connection', {
                from_id: fromId,
                to_id: toId,
                template: template || null
            });

            if (response && response.success) {
                this.closeModal('connectAgentsModal');
                const fromName = this.getAgentName(fromId);
                const toName = this.getAgentName(toId);
                this.showSuccessMessage(`Połączono ${fromName} → ${toName}`);
                this.selectedAgentFrom = null;
                this.selectedConnectionPreset = null;
                // Reset opacity
                document.querySelectorAll('.agent-card').forEach(card => {
                    card.style.opacity = '1';
                });
                await this.refreshWorkflow();
            } else {
                this.showErrorMessage('Nie udało się utworzyć połączenia: ' + (response?.error || 'Nieznany błąd'));
            }
        } catch (error) {
            console.error('[Workflow] Failed to connect agents:', error);
            this.showErrorMessage('Błąd podczas łączenia agentów');
        }
    }

    updateAutoForwardSwitch() {
        const switchEl = document.getElementById('autoForwardSwitch');
        if (switchEl && this.graph) {
            switchEl.checked = this.graph.auto_forward || false;
        }
    }

    async toggleAutoForward(enabled) {
        try {
            const response = await this.sendWorkflowMessage('workflow_set_auto_forward', {
                enabled: enabled
            });

            if (response && response.success) {
                console.log('[Workflow] Auto-forward:', enabled);
                this.showSuccessMessage(`Auto-forward ${enabled ? 'enabled' : 'disabled'}`);
            }
        } catch (error) {
            console.error('[Workflow] Failed to toggle auto-forward:', error);
            this.showErrorMessage('Failed to toggle auto-forward');
        }
    }

    showSuccessMessage(message) {
        const statusMessage = document.getElementById('statusMessage');
        if (statusMessage) {
            statusMessage.textContent = message;
            statusMessage.className = 'status-message success';
            statusMessage.style.display = 'block';
            setTimeout(() => {
                statusMessage.style.display = 'none';
            }, 3000);
        }
    }

    showErrorMessage(message) {
        const statusMessage = document.getElementById('statusMessage');
        if (statusMessage) {
            statusMessage.textContent = message;
            statusMessage.className = 'status-message error';
            statusMessage.style.display = 'block';
            setTimeout(() => {
                statusMessage.style.display = 'none';
            }, 4000);
        }
    }

    async deleteConnection(fromId, toId) {
        const fromName = this.getAgentName(fromId);
        const toName = this.getAgentName(toId);

        if (!confirm(`Remove connection: ${fromName} → ${toName}?`)) return;

        try {
            const response = await this.sendWorkflowMessage('workflow_remove_connection', {
                from_id: fromId,
                to_id: toId
            });

            if (response && response.success) {
                this.showSuccessMessage(`Removed connection: ${fromName} → ${toName}`);
                await this.refreshWorkflow();
            } else {
                this.showErrorMessage('Failed to remove connection');
            }
        } catch (error) {
            console.error('[Workflow] Failed to remove connection:', error);
            this.showErrorMessage('Error removing connection');
        }
    }

    reorderAgents(draggedId, targetId) {
        if (!this.graph || !this.graph.agents) return;

        const agents = Object.values(this.graph.agents);
        const draggedIndex = agents.findIndex(a => a.id === draggedId);
        const targetIndex = agents.findIndex(a => a.id === targetId);

        if (draggedIndex === -1 || targetIndex === -1) return;

        // Przenieś agenta w tablicy
        const [draggedAgent] = agents.splice(draggedIndex, 1);
        agents.splice(targetIndex, 0, draggedAgent);

        // Zaktualizuj lokalną kolejność
        this.graph.agents = {};
        agents.forEach(agent => {
            this.graph.agents[agent.id] = agent;
        });

        // Odśwież widok
        this.renderAgents();
        this.showSuccessMessage(`Moved "${this.getAgentName(draggedId)}" to new position`);
    }

    // ============================================================================
    // Workflow Tabs Management
    // ============================================================================

    initWorkflowTabs() {
        console.log('[WorkflowManager] Initializing workflow tabs system');

        // Load tabs from storage
        this.loadTabsFromStorage();

        // If no tabs exist, create a default one
        if (this.workflowTabs.length === 0) {
            this.createNewTab('Workflow 1', true);
        }

        this.renderTabs();
    }

    loadTabsFromStorage() {
        const stored = localStorage.getItem('gluon_workflow_tabs');
        if (stored) {
            try {
                const data = JSON.parse(stored);
                this.workflowTabs = data.tabs || [];
                this.activeTabId = data.activeTabId || null;
                this.nextTabId = data.nextTabId || 1;
                console.log('[WorkflowManager] Loaded tabs from storage:', this.workflowTabs.length);
            } catch (error) {
                console.error('[WorkflowManager] Failed to load tabs from storage:', error);
                this.workflowTabs = [];
            }
        }
    }

    saveTabsToStorage() {
        const data = {
            tabs: this.workflowTabs,
            activeTabId: this.activeTabId,
            nextTabId: this.nextTabId
        };
        localStorage.setItem('gluon_workflow_tabs', JSON.stringify(data));
        console.log('[WorkflowManager] Saved tabs to storage');
    }

    createNewTab(name = null, makeActive = true) {
        const tabId = `tab-${this.nextTabId++}`;
        const tabName = name || `Workflow ${this.workflowTabs.length + 1}`;

        const newTab = {
            id: tabId,
            name: tabName,
            workflow: {
                agents: {},
                connections: [],
                auto_forward: false
            },
            createdAt: Date.now(),
            modifiedAt: Date.now()
        };

        this.workflowTabs.push(newTab);

        if (makeActive) {
            this.switchToTab(tabId);
        }

        this.saveTabsToStorage();
        this.renderTabs();

        this.showSuccessMessage(`📑 Utworzono nową zakładkę: ${tabName}`);
        console.log('[WorkflowManager] Created new tab:', tabId);
    }

    switchToTab(tabId) {
        const tab = this.workflowTabs.find(t => t.id === tabId);
        if (!tab) {
            console.error('[WorkflowManager] Tab not found:', tabId);
            return;
        }

        // Save current tab's workflow before switching
        if (this.activeTabId) {
            this.saveCurrentTabWorkflow();
        }

        this.activeTabId = tabId;

        // Load the tab's workflow
        this.graph = tab.workflow;

        // Update UI
        this.renderTabs();
        this.renderAgents();
        this.updateAutoForwardSwitch();

        this.saveTabsToStorage();

        console.log('[WorkflowManager] Switched to tab:', tabId, tab.name);
    }

    saveCurrentTabWorkflow() {
        if (!this.activeTabId) return;

        const tab = this.workflowTabs.find(t => t.id === this.activeTabId);
        if (tab && this.graph) {
            tab.workflow = JSON.parse(JSON.stringify(this.graph)); // Deep copy
            tab.modifiedAt = Date.now();
            this.saveTabsToStorage();
        }
    }

    duplicateTab(tabId) {
        const tab = this.workflowTabs.find(t => t.id === tabId);
        if (!tab) return;

        const newTabId = `tab-${this.nextTabId++}`;
        const duplicatedTab = {
            id: newTabId,
            name: `${tab.name} (kopia)`,
            workflow: JSON.parse(JSON.stringify(tab.workflow)), // Deep copy
            createdAt: Date.now(),
            modifiedAt: Date.now()
        };

        this.workflowTabs.push(duplicatedTab);
        this.saveTabsToStorage();
        this.renderTabs();

        this.showSuccessMessage(`📋 Zduplikowano zakładkę: ${tab.name}`);
    }

    renameTab(tabId, newName) {
        const tab = this.workflowTabs.find(t => t.id === tabId);
        if (!tab) return;

        tab.name = newName;
        tab.modifiedAt = Date.now();

        this.saveTabsToStorage();
        this.renderTabs();
    }

    closeTab(tabId) {
        const tabIndex = this.workflowTabs.findIndex(t => t.id === tabId);
        if (tabIndex === -1) return;

        const tab = this.workflowTabs[tabIndex];

        // Prevent closing the last tab
        if (this.workflowTabs.length === 1) {
            this.showErrorMessage('Nie można zamknąć ostatniej zakładki');
            return;
        }

        // Confirm if workflow is not empty
        const hasContent = tab.workflow && Object.keys(tab.workflow.agents || {}).length > 0;
        if (hasContent) {
            if (!confirm(`Zamknąć zakładkę "${tab.name}"? Workflow zostanie usunięty.`)) {
                return;
            }
        }

        // Remove the tab
        this.workflowTabs.splice(tabIndex, 1);

        // If we closed the active tab, switch to another one
        if (this.activeTabId === tabId) {
            // Switch to the next tab, or previous if this was the last one
            const newActiveIndex = tabIndex >= this.workflowTabs.length ? tabIndex - 1 : tabIndex;
            this.switchToTab(this.workflowTabs[newActiveIndex].id);
        }

        this.saveTabsToStorage();
        this.renderTabs();

        this.showSuccessMessage(`🗑️ Zamknięto zakładkę: ${tab.name}`);
    }

    renderTabs() {
        const tabsContainer = document.getElementById('workflowTabs');
        if (!tabsContainer) return;

        tabsContainer.innerHTML = this.workflowTabs.map(tab => {
            const isActive = tab.id === this.activeTabId;
            const agentCount = Object.keys(tab.workflow?.agents || {}).length;

            return `
                <div class="workflow-tab ${isActive ? 'active' : ''}" data-tab-id="${tab.id}">
                    <div class="workflow-tab-icon">${agentCount > 0 ? '📊' : '📄'}</div>
                    <div class="workflow-tab-name" title="${this.escapeHtml(tab.name)}">${this.escapeHtml(tab.name)}</div>
                    <div class="workflow-tab-actions">
                        <button class="workflow-tab-action duplicate" title="Duplikuj zakładkę">⎘</button>
                        <button class="workflow-tab-action delete" title="Zamknij zakładkę">×</button>
                    </div>
                </div>
            `;
        }).join('');

        // Add event listeners
        this.workflowTabs.forEach(tab => {
            const tabEl = tabsContainer.querySelector(`[data-tab-id="${tab.id}"]`);
            if (!tabEl) return;

            // Click on tab to switch
            tabEl.addEventListener('click', (e) => {
                // Don't switch if clicking on action buttons
                if (e.target.closest('.workflow-tab-action')) return;
                this.switchToTab(tab.id);
            });

            // Double-click to rename
            const nameEl = tabEl.querySelector('.workflow-tab-name');
            nameEl.addEventListener('dblclick', (e) => {
                e.stopPropagation();
                const newName = prompt('Nowa nazwa zakładki:', tab.name);
                if (newName && newName.trim()) {
                    this.renameTab(tab.id, newName.trim());
                }
            });

            // Duplicate button
            const duplicateBtn = tabEl.querySelector('.duplicate');
            duplicateBtn?.addEventListener('click', (e) => {
                e.stopPropagation();
                this.duplicateTab(tab.id);
            });

            // Close button
            const closeBtn = tabEl.querySelector('.delete');
            closeBtn?.addEventListener('click', (e) => {
                e.stopPropagation();
                this.closeTab(tab.id);
            });
        });
    }

    // ============================================================================
    // Save / Load Tab Configuration Functions
    // ============================================================================

    async saveCurrentTabConfig() {
        if (!this.activeTabId) {
            this.showErrorMessage('Brak aktywnej zakładki do zapisania');
            return;
        }

        const tab = this.workflowTabs.find(t => t.id === this.activeTabId);
        if (!tab) {
            this.showErrorMessage('Nie znaleziono aktywnej zakładki');
            return;
        }

        // Save current workflow state before saving config
        this.saveCurrentTabWorkflow();

        // Ask user for configuration name
        const configName = prompt('Podaj nazwę konfiguracji:', tab.name);
        if (!configName || !configName.trim()) {
            return; // User cancelled or entered empty name
        }

        try {
            // Generate unique ID for this config
            const configId = `config-${Date.now()}-${Math.random().toString(36).substring(2, 11)}`;

            // Send save request to Rust backend
            await new Promise((resolve, reject) => {
                chrome.runtime.sendMessage({
                    action: 'workflow_save_config',
                    id: configId,
                    name: configName.trim(),
                    workflow: tab.workflow
                }, (response) => {
                    if (chrome.runtime.lastError) {
                        reject(new Error(chrome.runtime.lastError.message));
                    } else {
                        resolve(response);
                    }
                });
            });

            this.showSuccessMessage(`💾 Konfiguracja zapisana: ${configName}`);
            console.log('[WorkflowManager] Config saved:', configName);
        } catch (error) {
            console.error('[WorkflowManager] Failed to save config:', error);
            this.showErrorMessage('Błąd podczas zapisywania konfiguracji: ' + error.message);
        }
    }

    exportCurrentTab() {
        if (!this.activeTabId) {
            this.showErrorMessage('Brak aktywnej zakładki do eksportu');
            return;
        }

        const tab = this.workflowTabs.find(t => t.id === this.activeTabId);
        if (!tab) {
            this.showErrorMessage('Nie znaleziono aktywnej zakładki');
            return;
        }

        // Save current workflow state before exporting
        this.saveCurrentTabWorkflow();

        try {
            // Prepare export data
            const exportData = {
                version: "2.0",
                type: "gluon_workflow_tab",
                exported_at: new Date().toISOString(),
                tab: {
                    name: tab.name,
                    workflow: tab.workflow,
                    metadata: {
                        agent_count: Object.keys(tab.workflow.agents || {}).length,
                        connection_count: (tab.workflow.connections || []).length,
                        aggregator_count: Object.values(tab.workflow.agents || {}).filter(a =>
                            this.normalizeAgentType(a) === 'Report'
                        ).length
                    }
                }
            };

            // Convert to JSON
            const jsonString = JSON.stringify(exportData, null, 2);

            // Create filename with timestamp
            const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
            const sanitizedName = tab.name.replace(/[^a-zA-Z0-9-_]/g, '_');
            const filename = `workflow-${sanitizedName}-${timestamp}.json`;

            // Create blob and download
            const blob = new Blob([jsonString], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = filename;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);

            this.showSuccessMessage(`💾 Zakładka wyeksportowana: ${filename}`);
            console.log('[WorkflowManager] Tab exported:', exportData);
        } catch (error) {
            console.error('[WorkflowManager] Failed to export tab:', error);
            this.showErrorMessage('Błąd podczas eksportowania zakładki');
        }
    }

    async importTabFromFile(file) {
        try {
            const text = await file.text();
            const importData = JSON.parse(text);

            // Validate import data
            if (!importData.type || importData.type !== 'gluon_workflow_tab') {
                this.showErrorMessage('Nieprawidłowy format pliku. Oczekiwano pliku zakładki workflow.');
                return;
            }

            if (!importData.tab || !importData.tab.workflow) {
                this.showErrorMessage('Brak danych workflow w pliku');
                return;
            }

            const tabName = importData.tab.name || 'Imported Workflow';
            const workflow = importData.tab.workflow;

            // Create new tab with imported data
            const tabId = `tab-${this.nextTabId++}`;
            const newTab = {
                id: tabId,
                name: tabName,
                workflow: workflow,
                createdAt: Date.now(),
                modifiedAt: Date.now()
            };

            this.workflowTabs.push(newTab);
            this.switchToTab(tabId);

            this.saveTabsToStorage();
            this.renderTabs();

            // Now we need to recreate agents in the backend
            await this.recreateWorkflowFromImport(workflow);

            this.showSuccessMessage(`📂 Zaimportowano zakładkę: ${tabName}`);
            console.log('[WorkflowManager] Tab imported:', importData);
        } catch (error) {
            console.error('[WorkflowManager] Failed to import tab:', error);
            this.showErrorMessage('Błąd podczas importowania zakładki: ' + error.message);
        }
    }

    async recreateWorkflowFromImport(workflow) {
        if (!workflow || !workflow.agents) return;

        try {
            // Map old agent IDs to new agent IDs
            const agentIdMap = {};

            // Create agents
            for (const [oldId, agent] of Object.entries(workflow.agents)) {
                const response = await this.sendWorkflowMessage('workflow_add_agent', {
                    name: agent.name,
                    output_wrapper: agent.output_wrapper || null,
                    agent_type: agent.agent_type || 'Normal',
                    position: agent.position || null
                });

                if (response && response.success) {
                    agentIdMap[oldId] = response.data.id;
                }

                await new Promise(resolve => setTimeout(resolve, 100));
            }

            // Create connections with mapped IDs
            for (const conn of workflow.connections || []) {
                const newFromId = agentIdMap[conn.from_agent_id];
                const newToId = agentIdMap[conn.to_agent_id];

                if (!newFromId || !newToId) {
                    console.warn('[WorkflowManager] Skipping connection - agent not found:', conn);
                    continue;
                }

                await this.sendWorkflowMessage('workflow_add_connection', {
                    from_id: newFromId,
                    to_id: newToId,
                    template: conn.message_template || null
                });

                await new Promise(resolve => setTimeout(resolve, 100));
            }

            // Set auto-forward if it was enabled
            if (workflow.auto_forward) {
                await this.sendWorkflowMessage('workflow_set_auto_forward', {
                    enabled: true
                });
            }

            await this.refreshWorkflow();

        } catch (error) {
            console.error('[WorkflowManager] Failed to recreate workflow:', error);
            throw error;
        }
    }

    handleCopySchema(event = null) {
        if (!this.graph || !this.graph.agents) {
            this.showErrorMessage('No workflow data available');
            return;
        }

        const agents = Object.values(this.graph.agents);
        if (agents.length === 0) {
            this.showErrorMessage('No agents to generate schema from');
            return;
        }

        // Check if Shift key is pressed for PRO version
        const isProVersion = event?.shiftKey || false;

        let schema = isProVersion
            ? this.generateOrchestratorPrompt()
            : this.generateBasicSchema();

        // Kopiowanie do schowka
        navigator.clipboard.writeText(schema).then(() => {
            const msg = isProVersion
                ? '🎯 Prompt Orkiestratora PRO skopiowany (z protokołem G-code)! 📋'
                : 'Schemat skopiowany do schowka! 📋 (Shift+klik = wersja PRO)';
            this.showSuccessMessage(msg);
        }).catch(err => {
            console.error('Failed to copy schema:', err);
            this.showErrorMessage('Nie udało się skopiować schematu');
        });
    }

    /**
     * Generates basic workflow schema (original functionality)
     */
    generateBasicSchema() {
        const agents = Object.values(this.graph.agents);
        let schema = "📋 **SCHEMAT ARCHITEKTURY WORKFLOW**\n";
        schema += "Użyj tego schematu, aby zrozumieć dostępnych agentów i ich relacje.\n\n";

        // 1. Definicje Agentów
        schema += "### 🤖 DEFINICJE AGENTÓW\n";
        agents.forEach(agent => {
            const typeInfo = agent.agent_type === 'Report' ? ' (Typ: Agregator/Raport)' : '';
            schema += `- **${agent.name}**${typeInfo}\n`;
            if (agent.output_wrapper) {
                schema += `  - Format Wyjścia: Definiuje specjalny format wrapowania.\n`;
            }
            schema += "\n";
        });

        // 2. Przepływ danych (Connections)
        schema += "### 🔄 PRZEPŁYW DANYCH (Graf Połączeń)\n\n";
        const connections = this.graph.connections || [];

        if (connections.length === 0) {
            schema += "Brak zdefiniowanych połączeń.\n";
        } else {
            // Grupuj po źródle dla lepszej wizualizacji
            const flowMap = {};
            connections.forEach(conn => {
                if (!flowMap[conn.from_agent_id]) flowMap[conn.from_agent_id] = [];
                flowMap[conn.from_agent_id].push(conn.to_agent_id);
            });

            // Wizualizacja w formie grafu
            Object.entries(flowMap).forEach(([sourceId, targetIds]) => {
                const sourceName = this.getAgentName(sourceId);
                schema += `**${sourceName}**\n`;
                targetIds.forEach((targetId, index) => {
                    const targetName = this.getAgentName(targetId);
                    const isLast = index === targetIds.length - 1;
                    const prefix = isLast ? '   └──→' : '   ├──→';
                    schema += `${prefix} ${targetName}\n`;
                });
                schema += '\n';
            });
        }

        // 3. Instrukcje dla Agenta (Template)
        schema += "\n### 📝 INSTRUKCJE DLA ORKIESTRATORA\n";
        schema += "Jesteś Orkiestratorem. Twoim celem jest wykorzystanie tych agentów do wykonania zadania użytkownika.\n";
        schema += "1. Przypisz zadania do konkretnych agentów na podstawie ich roli.\n";
        schema += "2. Czekaj na ich wyniki (zgłoszą się z odpowiedzią).\n";
        schema += "3. Stwórz finalny wynik na podstawie wszystkich raportów.\n";

        return schema;
    }

    /**
     * Generates advanced Orchestrator prompt with G-code protocol (PRO version)
     */
    generateOrchestratorPrompt() {
        const agents = Object.values(this.graph.agents);
        let prompt = "# ORKIESTRATOR ZESPOŁU AI - SYSTEM PROMPT\n\n";

        prompt += "Jesteś **ORKIESTRATOREM** w systemie Gluon Agent Workflow.\n";
        prompt += "Zarządzasz zespołem wyspecjalizowanych agentów AI przy użyciu **Protokołu G-code**.\n\n";

        // 1. ZESPÓŁ AGENTÓW
        prompt += "## 🤖 DOSTĘPNI AGENCI\n\n";
        agents.forEach(agent => {
            const agentType = this.normalizeAgentType(agent);
            let typeLabel = '';
            let roleDesc = '';

            if (agentType === 'Report') {
                typeLabel = '📊 AGREGATOR';
                roleDesc = 'Kolektor raportów - NIE jest modelem AI. Zbiera wyniki od innych agentów i łączy je.';
            } else if (agentType === 'AutoApply') {
                typeLabel = '⚡ EXECUTOR';
                roleDesc = 'Automatyczny aplikator kodu - NIE jest modelem AI. Wykonuje zmiany w plikach bezobsługowo.';
            } else {
                typeLabel = '🧠 AI AGENT';
                roleDesc = 'Model Claude - specjalista w swojej dziedzinie.';
            }

            prompt += `### ${agent.name} [${typeLabel}]\n`;
            prompt += `- **Rola**: ${roleDesc}\n`;

            if (agent.output_wrapper) {
                prompt += `- **Format wyjścia**: Stosuje specjalny szablon wrappera\n`;
            }
            prompt += '\n';
        });

        // 2. PRZEPŁYW PRACY
        prompt += "## 🔄 ARCHITEKTURA PRZEPŁYWU\n\n";
        const connections = this.graph.connections || [];

        if (connections.length > 0) {
            const flowMap = {};
            connections.forEach(conn => {
                if (!flowMap[conn.from_agent_id]) flowMap[conn.from_agent_id] = [];
                flowMap[conn.from_agent_id].push(conn.to_agent_id);
            });

            Object.entries(flowMap).forEach(([sourceId, targetIds]) => {
                const sourceName = this.getAgentName(sourceId);
                prompt += `- **${sourceName}** → wysyła wyniki do:\n`;
                targetIds.forEach(targetId => {
                    const targetName = this.getAgentName(targetId);
                    prompt += `  - ${targetName}\n`;
                });
            });
            prompt += '\n';
        }

        // 3. PROTOKÓŁ G-CODE
        prompt += "## 📡 PROTOKÓŁ G-CODE (Multi-Agent Routing)\n\n";
        prompt += "Aby zlecić zadania **wielu agentom jednocześnie**, użyj markerów `>>>> TARGET: [NAZWA_AGENTA]`.\n\n";

        prompt += "### Składnia:\n";
        prompt += "```\n";
        prompt += "[Preambuła globalna - kontekst widzą wszyscy agenci]\n\n";
        prompt += ">>>> TARGET: NAZWA_AGENTA_1\n";
        prompt += "[Instrukcje specyficzne dla agenta 1]\n\n";
        prompt += ">>>> TARGET: NAZWA_AGENTA_2\n";
        prompt += "[Instrukcje specyficzne dla agenta 2]\n";
        prompt += "```\n\n";

        prompt += "### Przykład:\n";
        prompt += "```\n";
        prompt += "Zadanie: Zaimplementować system logowania.\n\n";

        // Generate example based on actual agents
        const normalAgents = agents.filter(a => this.normalizeAgentType(a) === 'Normal');
        if (normalAgents.length >= 2) {
            prompt += `>>>> TARGET: ${normalAgents[0].name.toUpperCase()}\n`;
            prompt += `Zaprojektuj architekturę systemu logowania.\n\n`;
            prompt += `>>>> TARGET: ${normalAgents[1].name.toUpperCase()}\n`;
            prompt += `Napisz kod implementujący zaproponowaną architekturę.\n`;
        } else if (normalAgents.length === 1) {
            prompt += `>>>> TARGET: ${normalAgents[0].name.toUpperCase()}\n`;
            prompt += `Zaimplementuj kompletny system logowania.\n`;
        } else {
            prompt += ">>>> TARGET: FRONTEND\n";
            prompt += "Stwórz formularz logowania w React.\n\n";
            prompt += ">>>> TARGET: BACKEND\n";
            prompt += "Napisz endpoint /api/login w Rust.\n";
        }
        prompt += "```\n\n";

        // 4. ZASADY DZIAŁANIA
        prompt += "## ⚙️ ZASADY ORKIESTRACJI\n\n";
        prompt += "1. **Rozdzielaj zadania równolegle** - wykorzystuj protokół G-code do wysyłania instrukcji do wielu agentów naraz\n";
        prompt += "2. **Agreguj wyniki** - jeśli w przepływie jest Agregator, deleguj zadania do jego źródeł\n";
        prompt += "3. **Nazwij agentów dokładnie** - używaj nazw dokładnie jak w sekcji DOSTĘPNI AGENCI (bez względu na wielkość liter)\n";
        prompt += "4. **Preambuła jest wspólna** - tekst przed pierwszym markerem `>>>> TARGET:` widzą wszyscy agenci\n";
        prompt += "5. **Auto-Apply nie wymaga instrukcji** - wysyłaj do niego gotowy kod w blokach <<<< SEARCH / >>>> REPLACE\n\n";

        // 5. ZAAWANSOWANE FUNKCJE
        prompt += "## 🚀 ZAAWANSOWANE MOŻLIWOŚCI\n\n";

        const hasAutoApply = agents.some(a => this.normalizeAgentType(a) === 'AutoApply');
        if (hasAutoApply) {
            const autoApplyAgent = agents.find(a => this.normalizeAgentType(a) === 'AutoApply');
            prompt += `### Auto-Apply Executor\n`;
            prompt += `Agent **${autoApplyAgent.name}** automatycznie aplikuje zmiany w kodzie.\n`;
            prompt += `Wyślij do niego wiadomość zawierającą bloki kodu w formacie:\n`;
            prompt += '```\n';
            prompt += '<<<< SEARCH\n';
            prompt += 'stary kod do znalezienia\n';
            prompt += '>>>> REPLACE\n';
            prompt += 'nowy kod zastępujący\n';
            prompt += '```\n\n';
        }

        const hasReportAggregator = agents.some(a => this.normalizeAgentType(a) === 'Report');
        if (hasReportAggregator) {
            prompt += `### Report Aggregator\n`;
            prompt += `System automatycznie zbierze wszystkie odpowiedzi i połączy je w jeden raport.\n`;
            prompt += `Nie musisz ręcznie czekać na odpowiedzi - agregator to zrobi za Ciebie.\n\n`;
        }

        prompt += "---\n\n";
        prompt += "**TERAZ JESTEŚ GOTOWY DO ORKIESTRACJI!**\n";
        prompt += "Przetwarzaj zadania użytkownika, rozdzielając pracę między dostępnych agentów przy użyciu protokołu G-code.\n";

        return prompt;
    }

    /**
     * Shows terminal pairing instructions modal
     */
    showTerminalPairingInstructions(agent) {
        const instructions = `
Aby sparować terminal z węzłem "${agent.name}":

1. Otwórz VS Code
2. Otwórz terminal (View → Terminal lub Ctrl+\`)
3. Naciśnij Ctrl+Shift+P
4. Wpisz: "Gluon: Capture Terminal"
5. Podaj kod parowania: ${agent.pairing_code}
6. Skonfiguruj opcje (ANSI strip, max lines)

Terminal będzie automatycznie wysyłał logi do Gluon!

WAŻNE: VS Code API ma ograniczenia - możesz użyć komendy
"Gluon: Send Terminal Output" aby ręcznie wysłać zawartość terminala.
        `.trim();

        alert(instructions);
    }

    /**
     * Shows terminal configuration modal
     */
    showTerminalConfig(agentId) {
        const agentName = this.getAgentName(agentId);

        alert(`⚙️ Konfiguracja Terminala "${agentName}"\n\n` +
              `Ta funkcja zostanie wkrótce zaimplementowana.\n` +
              `Będzie pozwalać na:\n` +
              `- Toggle Strip ANSI codes\n` +
              `- Ustawienie Max Lines (bufor)\n` +
              `- Auto-send on change\n` +
              `- Podgląd live output`);
    }

    renderTerminalCard(agent) {
        // Find connections
        const outgoing = this.graph.connections.filter(c => c.from_agent_id === agent.id);
        const incoming = this.graph.connections.filter(c => c.to_agent_id === agent.id);

        // Build connection lists
        const outgoingHtml = outgoing.length > 0
            ? outgoing.map((c, idx) => `
                <span class="connection-item">
                    ${this.getAgentName(c.to_agent_id)}
                    <button id="delete-conn-${agent.id}-${idx}" class="conn-delete-btn" title="Remove connection">×</button>
                </span>
            `).join('')
            : '<span style="color: var(--text-muted);">brak</span>';

        const incomingHtml = incoming.length > 0
            ? incoming.map(c => this.getAgentName(c.from_agent_id)).join(', ')
            : '<span style="color: var(--text-muted);">brak (terminal jest źródłem)</span>';

        // Status based on pairing
        const isPaired = agent.status === 'Connected';
        const statusText = isPaired
            ? '🟢 Sparowany z VS Code'
            : '🔴 Oczekuje na parowanie';

        return `
            <div class="agent-card terminal-card" data-agent-id="${agent.id}" data-type="Terminal">
                <div class="agent-header">
                    <div class="agent-name">🖥️ ${this.escapeHtml(agent.name)}</div>
                    <div class="agent-status" style="color: ${isPaired ? '#10b981' : '#ef4444'};">📟 Terminal Sensor</div>
                </div>
                <div class="aggregator-info">
                    <div class="info-box">
                        <strong>Typ:</strong> Terminal Listener (NIE model AI)
                    </div>
                    <div class="info-box">
                        <strong>Status:</strong> ${statusText}
                    </div>
                    <div class="info-box">
                        <strong>Kod parowania:</strong> <code>${agent.pairing_code}</code>
                    </div>
                </div>
                <div class="agent-connections">
                    <div class="connections-row">
                        <span style="color: var(--text-secondary); margin-right: 6px;">📥 Odbiera od:</span>
                        ${incomingHtml}
                    </div>
                    <div class="connections-row">
                        <span style="color: var(--text-secondary); margin-right: 6px;">📤 Wysyła do:</span>
                        ${outgoingHtml}
                    </div>
                </div>
                <div class="aggregator-description">
                    💡 <em>Przechwytuje output z terminala VS Code i przekazuje do workflow</em>
                </div>
                <div class="terminal-actions" style="margin-top: 12px; display: flex; gap: 8px;">
                    ${!isPaired ? `<button id="pair-terminal-${agent.id}" class="mini-btn" style="background: #3b82f6;" title="Sparuj z terminalem VS Code">🔗 Paruj Terminal</button>` : ''}
                    <button id="config-terminal-${agent.id}" class="mini-btn" style="background: #6366f1;" title="Konfiguracja logów">⚙️ Konfiguruj</button>
                </div>
                <div class="agent-actions" style="margin-top: 8px;">
                    <button id="connect-from-${agent.id}" class="mini-btn" title="Ustaw jako źródło połączenia">→ From</button>
                    <button id="connect-to-${agent.id}" class="mini-btn" title="Połącz do tego węzła">To →</button>
                    <button id="clone-${agent.id}" class="mini-btn" title="Klonuj węzeł">📋</button>
                    <button id="delete-${agent.id}" class="mini-btn delete-btn" title="Usuń węzeł">🗑️</button>
                </div>
            </div>
        `;
    }
}

// Initialize when DOM is ready
console.log('[WorkflowManager] 🔧 Module loaded, readyState:', document.readyState);

let workflowManagerInstance = null;

function initWorkflowManager() {
    console.log('[WorkflowManager] 🎬 Initializing WorkflowManager instance...');
    workflowManagerInstance = new WorkflowManager();
    window.workflowManager = workflowManagerInstance;
    console.log('[WorkflowManager] ✅ WorkflowManager instance created and attached to window');
}

if (document.readyState === 'loading') {
    console.log('[WorkflowManager] ⏳ DOM still loading, adding DOMContentLoaded listener');
    document.addEventListener('DOMContentLoaded', () => {
        console.log('[WorkflowManager] 🚀 DOMContentLoaded fired!');
        initWorkflowManager();
    });
} else {
    console.log('[WorkflowManager] ✅ DOM already loaded, initializing immediately');
    initWorkflowManager();
}

export default workflowManagerInstance;