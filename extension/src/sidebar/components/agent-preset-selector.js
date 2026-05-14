// Agent Preset Selector Component
// Allows users to select predefined agent templates or provide custom prompts

import workflowManager from '../../features/workflows/workflow-manager.js';

export class AgentPresetSelector {
    constructor(onAgentSelected) {
        this.onAgentSelected = onAgentSelected; // Callback when agent is selected
        this.selectedPresetId = null;
        this.customPrompt = '';
        this.searchQuery = '';
        this.selectedCategory = 'all';
        this.modal = null;

        // Preset data loaded from Rust backend
        this.agentPresets = [];

        // Load favorites from localStorage
        const storedFavorites = localStorage.getItem('agent_favorites');
        this.favorites = storedFavorites ? new Set(JSON.parse(storedFavorites)) : new Set();
    }

    /**
     * Load presets from Rust backend
     */
    async loadPresets() {
        try {
            this.agentPresets = await workflowManager.sendToBackground('workflow_get_agent_presets');
            console.log(`✅ Loaded ${this.agentPresets.length} agent presets from Rust`);
        } catch (error) {
            console.error('Failed to load agent presets:', error);
            this.agentPresets = [];
        }
    }

    /**
     * Show the agent preset selector modal
     */
    async show() {
        await this.loadPresets(); // Load from Rust before showing
        this.createModal();
        this.modal.style.display = 'flex';
        this.renderAgentList();
    }

    /**
     * Hide the modal
     */
    hide() {
        if (this.modal) {
            this.modal.style.display = 'none';
        }
    }

    /**
     * Create the modal structure
     */
    createModal() {
        if (this.modal) {
            return; // Already created
        }

        this.modal = document.createElement('div');
        this.modal.className = 'agent-preset-modal';
        this.modal.innerHTML = `
            <div class="agent-preset-modal-content">
                <div class="agent-preset-modal-header">
                    <h2>🤖 Select Agent Preset</h2>
                    <button class="close-modal-btn" aria-label="Close">×</button>
                </div>

                <div class="agent-preset-search-bar">
                    <input
                        type="text"
                        class="agent-search-input"
                        placeholder="Search agents by name, description, or tags..."
                    />
                </div>

                <div class="agent-preset-categories">
                    <!-- Categories will be rendered here -->
                </div>

                <div class="agent-preset-list">
                    <!-- Agent cards will be rendered here -->
                </div>

                <div class="agent-custom-prompt-section">
                    <div class="custom-prompt-header">
                        <label for="custom-prompt-input">
                            <input type="checkbox" id="use-custom-prompt-checkbox" />
                            Use Custom System Prompt (Override Template)
                        </label>
                    </div>
                    <textarea
                        id="custom-prompt-input"
                        class="custom-prompt-textarea"
                        placeholder="Enter custom system prompt here... (Leave empty to use preset prompt)"
                        disabled
                    ></textarea>
                </div>

                <div class="agent-preset-modal-footer">
                    <button class="btn-secondary cancel-btn">Cancel</button>
                    <button class="btn-primary confirm-btn" disabled>
                        Add Agent
                    </button>
                </div>
            </div>
        `;

        document.body.appendChild(this.modal);
        this.attachEventListeners();
        this.renderCategories();
    }

    /**
     * Attach event listeners to modal elements
     */
    attachEventListeners() {
        // Close button
        const closeBtn = this.modal.querySelector('.close-modal-btn');
        closeBtn.addEventListener('click', () => this.hide());

        // Cancel button
        const cancelBtn = this.modal.querySelector('.cancel-btn');
        cancelBtn.addEventListener('click', () => this.hide());

        // Confirm button
        const confirmBtn = this.modal.querySelector('.confirm-btn');
        confirmBtn.addEventListener('click', () => this.confirmSelection());

        // Search input
        const searchInput = this.modal.querySelector('.agent-search-input');
        searchInput.addEventListener('input', (e) => {
            this.searchQuery = e.target.value;
            this.renderAgentList();
        });

        // Custom prompt checkbox
        const customCheckbox = this.modal.querySelector('#use-custom-prompt-checkbox');
        const customTextarea = this.modal.querySelector('#custom-prompt-input');

        customCheckbox.addEventListener('change', (e) => {
            customTextarea.disabled = !e.target.checked;
            if (!e.target.checked) {
                customTextarea.value = '';
                this.customPrompt = '';
            }
        });

        customTextarea.addEventListener('input', (e) => {
            this.customPrompt = e.target.value;
        });

        // Close on backdrop click
        this.modal.addEventListener('click', (e) => {
            if (e.target === this.modal) {
                this.hide();
            }
        });
    }

    /**
     * Get all unique categories from presets
     */
    getAllCategories() {
        const categories = new Set(['all']);
        this.agentPresets.forEach(preset => {
            if (preset.category) {
                categories.add(preset.category);
            }
        });
        return Array.from(categories);
    }

    /**
     * Render category filters
     */
    renderCategories() {
        const categoriesContainer = this.modal.querySelector('.agent-preset-categories');
        const categories = this.getAllCategories();

        categoriesContainer.innerHTML = categories.map(category => {
            const isActive = this.selectedCategory === category;
            return `
                <button
                    class="category-btn ${isActive ? 'active' : ''}"
                    data-category="${category}"
                >
                    ${this.getCategoryDisplayName(category)}
                </button>
            `;
        }).join('');

        // Attach click listeners to category buttons
        categoriesContainer.querySelectorAll('.category-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                this.selectedCategory = btn.dataset.category;
                this.renderCategories();
                this.renderAgentList();
            });
        });
    }

    /**
     * Get display name for category
     */
    getCategoryDisplayName(category) {
        const displayNames = {
            'all': '📋 All',
            'favorites': '⭐ Favorites',
            'custom': '✏️ Custom',
            'Architecture': '🏛️ Architecture',
            'Frontend': '🌑 Frontend',
            'Quality': '⚖️ Quality',
            'Data': '🗄️ Data',
            'Maintenance': '🧹 Maintenance',
            'Security': '🛡️ Security',
            'Integration': '🕸️ Integration',
            'Documentation': '📜 Documentation',
            'DevOps': '⚙️ DevOps',
            'Design': '🎨 Design',
            'Observability': '🔍 Observability',
            'Performance': '⚡ Performance',
            'Research': '🔬 Research',
            'Development': '💻 Development',
            'Specialized': '🎯 Specialized',
            'Management': '📊 Management'
        };

        return displayNames[category] || category;
    }

    /**
     * Filter agents by search query
     */
    searchAgents(query) {
        const lowerQuery = query.toLowerCase();
        return this.agentPresets.filter(agent => {
            return (
                agent.name?.toLowerCase().includes(lowerQuery) ||
                agent.displayName?.toLowerCase().includes(lowerQuery) ||
                agent.description?.toLowerCase().includes(lowerQuery) ||
                agent.tags?.some(tag => tag.toLowerCase().includes(lowerQuery))
            );
        });
    }

    /**
     * Filter agents by category
     */
    getFilteredAgentPresets(category) {
        if (category === 'all') {
            return this.agentPresets;
        }
        return this.agentPresets.filter(agent => agent.category === category);
    }

    /**
     * Check if agent is favorite
     */
    isFavorite(agentId) {
        return this.favorites.has(agentId);
    }

    /**
     * Render list of agent presets
     */
    renderAgentList() {
        const listContainer = this.modal.querySelector('.agent-preset-list');

        // Get filtered agents
        let agents = this.searchQuery.length > 0
            ? this.searchAgents(this.searchQuery)
            : this.getFilteredAgentPresets(this.selectedCategory);

        if (agents.length === 0) {
            listContainer.innerHTML = `
                <div class="no-agents-found">
                    <p>No agents found matching your criteria</p>
                </div>
            `;
            return;
        }

        listContainer.innerHTML = agents.map(agent => {
            const isSelected = this.selectedPresetId === agent.id;
            const isFavorite = this.isFavorite(agent.id);

            return `
                <div
                    class="agent-preset-card ${isSelected ? 'selected' : ''}"
                    data-agent-id="${agent.id}"
                    style="border-left: 4px solid ${agent.color || '#4A5568'}"
                >
                    <div class="agent-card-header">
                        <span class="agent-icon">${agent.icon}</span>
                        <div class="agent-card-title">
                            <h3>${agent.displayName || agent.name}</h3>
                            <span class="agent-category-badge">${agent.category}</span>
                        </div>
                        <button
                            class="favorite-btn ${isFavorite ? 'active' : ''}"
                            data-agent-id="${agent.id}"
                            aria-label="Toggle favorite"
                        >
                            ${isFavorite ? '⭐' : '☆'}
                        </button>
                    </div>
                    <p class="agent-description">${agent.description}</p>
                    <div class="agent-tags">
                        ${(agent.tags || []).map(tag => `<span class="tag">${tag}</span>`).join('')}
                    </div>
                </div>
            `;
        }).join('');

        // Attach click listeners to agent cards
        listContainer.querySelectorAll('.agent-preset-card').forEach(card => {
            card.addEventListener('click', (e) => {
                // Don't select if clicking favorite button
                if (e.target.closest('.favorite-btn')) return;

                this.selectAgent(card.dataset.agentId);
            });
        });

        // Attach favorite button listeners
        listContainer.querySelectorAll('.favorite-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                this.toggleFavorite(btn.dataset.agentId);
            });
        });
    }

    /**
     * Get agent preset by ID
     */
    getAgentPreset(agentId) {
        return this.agentPresets.find(agent => agent.id === agentId);
    }

    /**
     * Select an agent preset
     */
    selectAgent(agentId) {
        this.selectedPresetId = agentId;

        // Update UI
        this.modal.querySelectorAll('.agent-preset-card').forEach(card => {
            card.classList.toggle('selected', card.dataset.agentId === agentId);
        });

        // Enable confirm button
        const confirmBtn = this.modal.querySelector('.confirm-btn');
        confirmBtn.disabled = false;

        // Load preset prompt into custom textarea as reference
        const agent = this.getAgentPreset(agentId);
        const customTextarea = this.modal.querySelector('#custom-prompt-input');

        // Show original prompt as placeholder
        if (agent) {
            const prompt = agent.systemPrompt || agent.system_prompt || '';
            customTextarea.placeholder = prompt ? `Default prompt for ${agent.displayName || agent.name}:\n\n${prompt.substring(0, 200)}...` : `Custom prompt for ${agent.displayName || agent.name}`;
        }
    }

    /**
     * Toggle favorite status
     */
    toggleFavorite(agentId) {
        if (this.favorites.has(agentId)) {
            this.favorites.delete(agentId);
        } else {
            this.favorites.add(agentId);
        }
        // Save to localStorage
        localStorage.setItem('agent_favorites', JSON.stringify(Array.from(this.favorites)));
        this.renderAgentList();
    }

    /**
     * Get agent preset with custom prompt override
     */
    getAgentPresetWithCustomPrompt(agentId, customPrompt) {
        const agent = this.getAgentPreset(agentId);
        if (!agent) return null;

        // Return copy with custom prompt if provided (handle both snake_case and camelCase)
        const originalPrompt = agent.systemPrompt || agent.system_prompt || '';
        return {
            ...agent,
            systemPrompt: customPrompt || originalPrompt,
            system_prompt: customPrompt || originalPrompt
        };
    }

    /**
     * Confirm selection and trigger callback
     */
    confirmSelection() {
        if (!this.selectedPresetId) return;

        const agent = this.getAgentPresetWithCustomPrompt(
            this.selectedPresetId,
            this.customPrompt
        );

        if (agent && this.onAgentSelected) {
            this.onAgentSelected(agent);
        }

        this.hide();
        this.reset();
    }

    /**
     * Reset selector state
     */
    reset() {
        this.selectedPresetId = null;
        this.customPrompt = '';
        this.searchQuery = '';
        this.selectedCategory = 'all';

        if (this.modal) {
            const confirmBtn = this.modal.querySelector('.confirm-btn');
            confirmBtn.disabled = true;

            const searchInput = this.modal.querySelector('.agent-search-input');
            searchInput.value = '';

            const customCheckbox = this.modal.querySelector('#use-custom-prompt-checkbox');
            customCheckbox.checked = false;

            const customTextarea = this.modal.querySelector('#custom-prompt-input');
            customTextarea.value = '';
            customTextarea.disabled = true;
        }
    }

    /**
     * Destroy modal and cleanup
     */
    destroy() {
        if (this.modal) {
            this.modal.remove();
            this.modal = null;
        }
    }
}

// Export default
export default AgentPresetSelector;
