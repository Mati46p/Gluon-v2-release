// Workflow Manager V2 - Thin Client Architecture
// All business logic moved to Rust backend - this is UI only

import { AgentPresetSelector } from '../../sidebar/components/agent-preset-selector.js';
import { GraphWorkflowEditor } from './graph-workflow-editor.js';

console.log('[WorkflowManager V2] 📦 Module loading (Thin Client)...');

/**
 * WorkflowManager V2 - Thin Client
 *
 * Changes from V1:
 * - ❌ Removed: Local graph state (now in Rust)
 * - ❌ Removed: parseGCodeMessage (routing in Rust)
 * - ❌ Removed: preset-manager.js (using Rust presets)
 * - ❌ Removed: localStorage workflow tabs (using Rust SavedWorkflowConfig)
 * - ✅ Added: WebSocket real-time sync
 * - ✅ Added: Rust API delegation for all operations
 */
class WorkflowManager {
    constructor() {
        // ✅ Cache only - READ-ONLY state from Rust
        this.cachedGraph = null;

        // UI state
        this.selectedAgentFrom = null;
        this.graphEditor = null;
        this.currentView = 'list'; // 'list' or 'graph'
        this.currentPresetCategory = 'all';
        this.selectedAgentPreset = null;
        this.selectedConnectionPreset = null;

        // ✅ Presets from Rust (loaded async)
        this.agentPresets = [];
        this.connectionPresets = [];
        this.workflowPresets = [];

        // ✅ Saved configs from Rust (loaded async)
        this.savedConfigs = [];
        this.activeTabId = null;

        // Agent Preset Selector Modal
        this.agentPresetSelector = new AgentPresetSelector((selectedPreset) => {
            this.handleAgentPresetSelected(selectedPreset);
        });

        this.init();
    }

    async init() {
        console.log('[WorkflowManager V2] 🚀 Initializing Thin Client...');

        // 1. Initialize Graph Editor
        const graphContainer = document.getElementById('workflowGraphContainer');
        if (graphContainer) {
            this.graphEditor = new GraphWorkflowEditor(graphContainer, this);
            console.log('[WorkflowManager V2] ✅ Graph editor initialized');
        }

        // 2. Load presets from Rust
        await this.loadPresetsFromRust();

        // 3. Load saved workflow configs from Rust
        await this.loadSavedConfigsFromRust();

        // 4. Subscribe to real-time updates
        this.subscribeToBackendUpdates();

        // 5. Load initial graph state
        await this.loadGraphFromRust();

        // 6. Setup UI event listeners
        this.setupEventListeners();

        console.log('[WorkflowManager V2] ✅ Initialized successfully');
    }

    // ============================================================================
    // Rust API Communication
    // ============================================================================

    /**
     * Send command to Rust backend via background.js
     */
    async sendToBackground(action, payload = {}) {
        return new Promise((resolve, reject) => {
            chrome.runtime.sendMessage(
                {
                    action: 'workflow_command',
                    workflow_action: action,  // Nested action for workflow commands
                    payload
                },
                (response) => {
                    console.log('[WorkflowManager V2] 🔍 DEBUG: Received response:', response);

                    if (chrome.runtime.lastError) {
                        console.error('[WorkflowManager V2] ❌ Runtime error:', chrome.runtime.lastError);
                        reject(chrome.runtime.lastError);
                        return;
                    }

                    if (response?.error) {
                        console.error('[WorkflowManager V2] ❌ Backend error:', response.error);
                        reject(new Error(response.error));
                        return;
                    }

                    console.log('[WorkflowManager V2] ✅ Resolving with data:', response?.data);
                    resolve(response?.data);
                }
            );
        });
    }

    /**
     * Subscribe to WebSocket events from Rust
     */
    subscribeToBackendUpdates() {
        console.log('[WorkflowManager V2] 📡 Subscribing to backend updates...');

        chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
            if (message.type === 'workflow-state-sync') {
                console.log('[WorkflowManager V2] 🔄 Received state sync from Rust');
                this.handleGraphSync(message.payload);
            }

            if (message.type === 'workflow-route-message') {
                console.log('[WorkflowManager V2] 📨 Received routing event:', message.payload);
                this.handleRoutingNotification(message.payload);
            }
        });
    }

    /**
     * Handle graph sync from Rust (real-time updates)
     */
    handleGraphSync(graphData) {
        console.log('[WorkflowManager V2] 🔄 Updating cached graph from Rust');

        // Convert agents object to Map if needed
        if (graphData.agents && !graphData.agents instanceof Map) {
            graphData.agents = new Map(Object.entries(graphData.agents));
        }

        this.cachedGraph = graphData;
        this.renderWorkflow();
    }

    /**
     * Handle routing notification (agent sent task to another agent)
     */
    handleRoutingNotification(payload) {
        const { from_agent_name, to_agent_name, content } = payload;

        this.showNotification(
            `📨 ${from_agent_name} → ${to_agent_name}`,
            `Routed task (${content.substring(0, 50)}...)`,
            'info'
        );
    }

    // ============================================================================
    // Load Data from Rust
    // ============================================================================

    /**
     * Load presets from Rust backend (SSOT)
     */
    async loadPresetsFromRust() {
        console.log('[WorkflowManager V2] 📦 Loading presets from Rust...');

        try {
            this.agentPresets = await this.sendToBackground('workflow_get_agent_presets');
            this.connectionPresets = await this.sendToBackground('workflow_get_connection_presets');
            this.workflowPresets = await this.sendToBackground('workflow_get_workflow_presets');

            console.log('[WorkflowManager V2] ✅ Loaded presets:', {
                agents: this.agentPresets.length,
                connections: this.connectionPresets.length,
                workflows: this.workflowPresets.length
            });
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to load presets:', error);
            this.showNotification('Failed to load presets', error.message, 'error');
        }
    }

    /**
     * Load saved workflow configs from Rust
     */
    async loadSavedConfigsFromRust() {
        console.log('[WorkflowManager V2] 📦 Loading saved configs from Rust...');

        try {
            this.savedConfigs = await this.sendToBackground('workflow_get_saved_configs');
            console.log('[WorkflowManager V2] ✅ Loaded configs:', this.savedConfigs.length);
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to load configs:', error);
        }
    }

    /**
     * Load current workflow graph from Rust
     */
    async loadGraphFromRust() {
        console.log('[WorkflowManager V2] 📥 Loading graph from Rust...');

        try {
            const graph = await this.sendToBackground('workflow_get_graph');
            this.handleGraphSync(graph);
            console.log('[WorkflowManager V2] ✅ Graph loaded successfully');
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to load graph:', error);
            this.showNotification('Failed to load workflow', error.message, 'error');
        }
    }

    // ============================================================================
    // CRUD Operations (delegated to Rust)
    // ============================================================================

    /**
     * Add agent - DELEGATED TO RUST
     */
    async addAgent(name, outputWrapper, agentType, position) {
        console.log('[WorkflowManager V2] ➕ Adding agent (delegated to Rust):', name);

        try {
            const agent = await this.sendToBackground('workflow_add_agent', {
                name,
                output_wrapper: outputWrapper || null,
                agent_type: agentType || 'Normal',
                position: position || null
            });

            console.log('[WorkflowManager V2] ✅ Agent created:', agent);
            this.showNotification('Agent Added', `Created agent: ${name}`, 'success');

            // Graph will auto-update via workflow-state-sync event
            return agent;
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to add agent:', error);
            this.showNotification('Failed to add agent', error.message, 'error');
            throw error;
        }
    }

    /**
     * Remove agent - DELEGATED TO RUST
     */
    async removeAgent(agentId) {
        console.log('[WorkflowManager V2] ➖ Removing agent (delegated to Rust):', agentId);

        try {
            await this.sendToBackground('workflow_remove_agent', { agent_id: agentId });

            console.log('[WorkflowManager V2] ✅ Agent removed');
            this.showNotification('Agent Removed', 'Agent deleted successfully', 'success');

            // Graph will auto-update via workflow-state-sync event
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to remove agent:', error);
            this.showNotification('Failed to remove agent', error.message, 'error');
            throw error;
        }
    }

    editAgent(agentId) {
        console.log('[WorkflowManager V2] ✏️ Editing agent:', agentId);

        const agent = this.cachedGraph?.agents?.get ? this.cachedGraph.agents.get(agentId) : this.cachedGraph?.agents?.[agentId];
        if (!agent) {
            this.showNotification('Error', 'Agent not found', 'error');
            return;
        }

        const newName = prompt('Enter new name:', agent.name);
        if (newName && newName !== agent.name) {
            this.updateAgent(agentId, { name: newName });
        }
    }

    /**
     * Update agent - DELEGATED TO RUST
     */
    async updateAgent(agentId, updates) {
        console.log('[WorkflowManager V2] 🔄 Updating agent (delegated to Rust):', agentId, updates);

        try {
            const agent = await this.sendToBackground('workflow_update_agent', {
                agent_id: agentId,
                name: updates.name || null,
                output_wrapper: updates.outputWrapper !== undefined ? updates.outputWrapper : null,
                system_prompt: updates.systemPrompt || null
            });

            console.log('[WorkflowManager V2] ✅ Agent updated:', agent);
            this.showNotification('Agent Updated', 'Changes saved', 'success');

            return agent;
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to update agent:', error);
            this.showNotification('Failed to update agent', error.message, 'error');
            throw error;
        }
    }

    /**
     * Add connection - DELEGATED TO RUST
     */
    async addConnection(fromId, toId, template) {
        console.log('[WorkflowManager V2] 🔗 Adding connection (delegated to Rust):', fromId, '->', toId);

        try {
            await this.sendToBackground('workflow_add_connection', {
                from_id: fromId,
                to_id: toId,
                template: template || null
            });

            console.log('[WorkflowManager V2] ✅ Connection created');
            this.showNotification('Connection Added', 'Agents connected', 'success');

            // Graph will auto-update via workflow-state-sync event
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to add connection:', error);
            this.showNotification('Failed to add connection', error.message, 'error');
            throw error;
        }
    }

    /**
     * Remove connection - DELEGATED TO RUST
     */
    async removeConnection(fromId, toId) {
        console.log('[WorkflowManager V2] 🔗❌ Removing connection (delegated to Rust):', fromId, '->', toId);

        try {
            await this.sendToBackground('workflow_remove_connection', {
                from_id: fromId,
                to_id: toId
            });

            console.log('[WorkflowManager V2] ✅ Connection removed');
            this.showNotification('Connection Removed', 'Connection deleted', 'success');
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to remove connection:', error);
            this.showNotification('Failed to remove connection', error.message, 'error');
            throw error;
        }
    }

    /**
     * Toggle auto-forward - DELEGATED TO RUST
     */
    async toggleAutoForward(enabled) {
        console.log('[WorkflowManager V2] 🔄 Toggle auto-forward (delegated to Rust):', enabled);

        try {
            await this.sendToBackground('workflow_set_auto_forward', { enabled });
            console.log('[WorkflowManager V2] ✅ Auto-forward updated');
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to toggle auto-forward:', error);
            this.showNotification('Failed to update auto-forward', error.message, 'error');
        }
    }

    // ============================================================================
    // Preset Operations
    // ============================================================================

    /**
     * Create agent from preset - DELEGATED TO RUST
     */
    async createAgentFromPreset(presetId, customName, position) {
        console.log('[WorkflowManager V2] 🎨 Creating agent from preset (delegated to Rust):', presetId);

        try {
            const agent = await this.sendToBackground('workflow_create_agent_from_preset', {
                preset_id: presetId,
                custom_name: customName || null,
                position: position || null
            });

            console.log('[WorkflowManager V2] ✅ Agent created from preset:', agent);
            this.showNotification('Agent Created', `Created from preset: ${presetId}`, 'success');

            return agent;
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to create from preset:', error);
            this.showNotification('Failed to create agent', error.message, 'error');
            throw error;
        }
    }

    /**
     * Create workflow from preset - DELEGATED TO RUST
     */
    async createWorkflowFromPreset(presetId) {
        console.log('[WorkflowManager V2] 🎨 Creating workflow from preset (delegated to Rust):', presetId);

        try {
            const result = await this.sendToBackground('workflow_create_from_preset', {
                preset_id: presetId
            });

            console.log('[WorkflowManager V2] ✅ Workflow created from preset:', result);
            this.showNotification(
                'Workflow Created',
                `Created ${result.agents_created} agents and ${result.connections_created} connections`,
                'success'
            );

            return result;
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to create workflow from preset:', error);
            this.showNotification('Failed to create workflow', error.message, 'error');
            throw error;
        }
    }

    // ============================================================================
    // Workflow Config Management (replaces localStorage tabs)
    // ============================================================================

    /**
     * Save current workflow config - DELEGATED TO RUST
     */
    async saveWorkflowConfig(id, name) {
        console.log('[WorkflowManager V2] 💾 Saving workflow config (delegated to Rust):', name);

        try {
            const config = await this.sendToBackground('workflow_save_config', {
                id: id || `config-${Date.now()}`,
                name,
                workflow: this.cachedGraph
            });

            console.log('[WorkflowManager V2] ✅ Config saved:', config);
            this.showNotification('Config Saved', `Saved: ${name}`, 'success');

            // Reload configs list
            await this.loadSavedConfigsFromRust();

            return config;
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to save config:', error);
            this.showNotification('Failed to save config', error.message, 'error');
            throw error;
        }
    }

    /**
     * Delete saved config - DELEGATED TO RUST
     */
    async deleteWorkflowConfig(id) {
        console.log('[WorkflowManager V2] 🗑️ Deleting workflow config (delegated to Rust):', id);

        try {
            await this.sendToBackground('workflow_delete_saved_config', { id });

            console.log('[WorkflowManager V2] ✅ Config deleted');
            this.showNotification('Config Deleted', 'Workflow config removed', 'success');

            // Reload configs list
            await this.loadSavedConfigsFromRust();
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to delete config:', error);
            this.showNotification('Failed to delete config', error.message, 'error');
            throw error;
        }
    }

    // ============================================================================
    // UI Rendering (kept from V1, but uses cachedGraph)
    // ============================================================================

    renderWorkflow() {
        if (!this.cachedGraph) {
            console.warn('[WorkflowManager V2] ⚠️ No cached graph available');
            return;
        }

        console.log('[WorkflowManager V2] 🎨 Rendering workflow from cached state');

        if (this.currentView === 'list') {
            this.renderListView();
        } else {
            this.renderGraphView();
        }
    }

    renderListView() {
        const container = document.getElementById('workflowAgentsContainer');
        if (!container) return;

        // Show list container, hide graph container
        const graphContainer = document.getElementById('workflowGraphContainer');
        if (graphContainer) graphContainer.style.display = 'none';
        container.style.display = 'block';

        container.innerHTML = '';

        if (!this.cachedGraph?.agents || this.cachedGraph.agents.size === 0) {
            container.innerHTML = '<div class="empty-state">No agents yet. Click "Add Agent" to get started.</div>';
            return;
        }

        // Render each agent
        const agents = Array.from(this.cachedGraph.agents.values());
        agents.forEach(agent => {
            const agentCard = this.createAgentCard(agent);
            container.appendChild(agentCard);
        });
    }

    createAgentCard(agent) {
        const card = document.createElement('div');
        card.className = `agent-card status-${agent.status.toLowerCase()}`;
        card.dataset.agentId = agent.id;

        // Status indicator
        const statusBadge = {
            'Idle': '🟢 Ready',
            'Working': '🟡 Working',
            'Success': '✅ Success',
            'Failed': '❌ Failed',
            'PendingConnection': '⏳ Waiting',
            'WaitingForUser': '👤 Needs Approval',
            'Waiting': '⏳ Waiting', // Legacy
            'Connected': '🟢 Connected', // Legacy
            'Disconnected': '🔴 Disconnected'
        }[agent.status] || agent.status;

        card.innerHTML = `
            <div class="agent-header">
                <h3>${agent.name}</h3>
                <div class="agent-actions">
                    <button class="btn-icon" onclick="workflowManager.editAgent('${agent.id}')">✏️</button>
                    <button class="btn-icon" onclick="workflowManager.removeAgent('${agent.id}')">🗑️</button>
                </div>
            </div>
            <div class="agent-info">
                <div class="agent-status">${statusBadge}</div>
                <div class="agent-pairing-code">Code: <code>${agent.pairing_code}</code></div>
                ${agent.current_task ? `<div class="agent-task">Task: ${agent.current_task}</div>` : ''}
                ${agent.agent_type !== 'Normal' ? `<div class="agent-type">Type: ${agent.agent_type}</div>` : ''}
            </div>
        `;

        return card;
    }

    renderGraphView() {
        console.log('[WorkflowManager V2] 📊 Rendering graph view');

        if (!this.graphEditor) {
            console.error('[WorkflowManager V2] ❌ Graph editor not initialized');
            return;
        }

        if (!this.cachedGraph) {
            console.warn('[WorkflowManager V2] ⚠️ No graph data to render');
            return;
        }

        // Show graph container, hide list container
        const graphContainer = document.getElementById('workflowGraphContainer');
        const listContainer = document.getElementById('workflowAgentsContainer');

        if (graphContainer) graphContainer.style.display = 'block';
        if (listContainer) listContainer.style.display = 'none';

        // Render graph using Cytoscape
        this.graphEditor.renderGraph(this.cachedGraph);
    }

    // ============================================================================
    // Event Listeners Setup
    // ============================================================================

    setupEventListeners() {
        console.log('[WorkflowManager V2] 🎯 Setting up event listeners...');

        // Add Agent button
        const addAgentBtn = document.getElementById('addAgentBtn');
        if (addAgentBtn) {
            addAgentBtn.addEventListener('click', () => this.showAddAgentModal());
        }

        // Refresh button
        const refreshBtn = document.getElementById('refreshWorkflowBtn');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', () => this.loadGraphFromRust());
        }

        // Auto-forward switch
        const autoForwardSwitch = document.getElementById('autoForwardSwitch');
        if (autoForwardSwitch) {
            autoForwardSwitch.addEventListener('change', (e) => this.toggleAutoForward(e.target.checked));
        }

        // View toggle
        const viewListBtn = document.getElementById('viewListBtn');
        const viewGraphBtn = document.getElementById('viewGraphBtn');
        if (viewListBtn) {
            viewListBtn.addEventListener('click', () => {
                this.currentView = 'list';
                this.renderWorkflow();
            });
        }
        if (viewGraphBtn) {
            viewGraphBtn.addEventListener('click', () => {
                this.currentView = 'graph';
                this.renderWorkflow();
            });
        }

        // Save/Load config buttons
        const saveConfigBtn = document.getElementById('saveConfigBtn');
        if (saveConfigBtn) {
            saveConfigBtn.addEventListener('click', () => this.showSaveConfigModal());
        }

        const loadConfigBtn = document.getElementById('loadConfigBtn');
        if (loadConfigBtn) {
            loadConfigBtn.addEventListener('click', () => this.showLoadConfigModal());
        }

        console.log('[WorkflowManager V2] ✅ Event listeners setup complete');
    }

    // ============================================================================
    // Modal Dialogs
    // ============================================================================

    async showAddAgentModal() {
        console.log('[WorkflowManager V2] 📋 Opening agent preset selector...');
        await this.agentPresetSelector.show();
    }

    async handleAgentPresetSelected(preset) {
        console.log('[WorkflowManager V2] ✅ Agent preset selected:', preset.name);

        try {
            // Create agent from preset using Rust API
            const result = await this.sendToBackground('workflow_create_agent_from_preset', {
                preset_id: preset.id,
                custom_name: preset.displayName || preset.name,
                position: [200.0, 200.0]  // Array format for Rust tuple
            });

            console.log('[WorkflowManager V2] ✅ Agent created from preset:', result);
            this.showNotification('Success', `Agent "${preset.displayName || preset.name}" created successfully`, 'success');

            // Graph will auto-update via WebSocket workflow_state_sync event
        } catch (error) {
            console.error('[WorkflowManager V2] ❌ Failed to create agent from preset:', error);
            this.showNotification('Error', `Failed to create agent: ${error.message}`, 'error');
        }
    }

    showSaveConfigModal() {
        const name = prompt('Enter workflow name:');
        if (name) {
            this.saveWorkflowConfig(`config-${Date.now()}`, name);
        }
    }

    showLoadConfigModal() {
        console.log('[WorkflowManager V2] 📋 Show load config modal (TODO)');
        // Display list of this.savedConfigs
    }

    showLoadWorkflowModal() {
        console.log('[WorkflowManager V2] 📋 Show load workflow modal (delegating to showLoadConfigModal)');
        this.showLoadConfigModal();
    }

    async saveCurrentTabConfig() {
        console.log('[WorkflowManager V2] 💾 Saving current workflow config...');
        const name = prompt('Enter workflow name:');
        if (name) {
            await this.saveWorkflowConfig(`config-${Date.now()}`, name);
        }
    }

    handleCopySchema() {
        console.log('[WorkflowManager V2] 📋 Copying schema to clipboard...');
        const schema = JSON.stringify(this.cachedGraph, null, 2);
        navigator.clipboard.writeText(schema).then(() => {
            this.showNotification('Success', 'Schema copied to clipboard', 'success');
        }).catch(err => {
            console.error('Failed to copy schema:', err);
            this.showNotification('Error', 'Failed to copy schema', 'error');
        });
    }

    switchView(viewType) {
        console.log(`[WorkflowManager V2] 🔄 Switching to ${viewType} view`);
        this.currentView = viewType;
        this.renderWorkflow();
    }

    // ============================================================================
    // Utility
    // ============================================================================

    showNotification(title, message, type = 'info') {
        console.log(`[WorkflowManager V2] 🔔 ${type.toUpperCase()}: ${title} - ${message}`);
        // TODO: Implement UI notification
    }

    // Backward compatibility method names
    async refreshWorkflow() {
        await this.loadGraphFromRust();
    }
}

// Initialize
console.log('[WorkflowManager V2] 📦 Module loaded, creating instance...');
const workflowManager = new WorkflowManager();
window.workflowManager = workflowManager;

console.log('[WorkflowManager V2] ✅ Module ready');

export default workflowManager;
