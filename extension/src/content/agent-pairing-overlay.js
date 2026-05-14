// Agent Pairing Overlay for AI Studio (Google Gemini)
// Allows user to pair browser tab with Gluon agent using pairing code

class AgentPairingOverlay {
    constructor() {
        this.isVisible = false;
        this.isPaired = false;
        this.agentInfo = null;
        this.overlay = null;
        this.availableAgents = [];
        this.tabId = null; // Unique tab identifier
        this.isDragging = false;
        this.dragOffset = { x: 0, y: 0 };
        this.init();
    }

    init() {
        console.log('[AgentPairing] Initializing overlay for AI Studio...');

        // Get current tab ID
        chrome.runtime.sendMessage({ action: 'get_tab_id' }, (response) => {
            this.tabId = response?.tabId || `tab_${Date.now()}_${Math.random()}`;
            console.log('[AgentPairing] Tab ID:', this.tabId);

            this.createOverlay();
            this.setupMessageListener();
            this.checkPairingStatus();
            this.loadAvailableAgents();
        });

        // Listen for keyboard shortcut (Ctrl+Shift+P)
        document.addEventListener('keydown', (e) => {
            if (e.ctrlKey && e.shiftKey && e.key === 'P') {
                e.preventDefault();
                this.toggle();
            }
        });
    }

    getStorageKey() {
        return `agent_pairing_${this.tabId}`;
    }

    createOverlay() {
        this.overlay = document.createElement('div');
        this.overlay.id = 'gluon-pairing-overlay';
        this.overlay.innerHTML = `
            <style>
                #gluon-pairing-overlay {
                    position: fixed;
                    top: 20px;
                    right: 20px;
                    z-index: 999999;
                    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
                }

                .gluon-pairing-panel {
                    background: rgba(10, 13, 26, 0.98);
                    border: 2px solid rgba(0, 212, 255, 0.3);
                    border-radius: 12px;
                    padding: 16px;
                    box-shadow: 0 0 20px rgba(0, 212, 255, 0.2), 0 8px 32px rgba(0, 0, 0, 0.5);
                    backdrop-filter: blur(10px);
                    min-width: 300px;
                    color: white;
                    animation: slideIn 0.3s ease-out;
                }

                @keyframes slideIn {
                    from {
                        transform: translateX(100%);
                        opacity: 0;
                    }
                    to {
                        transform: translateX(0);
                        opacity: 1;
                    }
                }

                .gluon-pairing-header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    margin-bottom: 12px;
                }

                .gluon-pairing-title {
                    font-size: 16px;
                    font-weight: 600;
                    display: flex;
                    align-items: center;
                    gap: 8px;
                }

                .gluon-pairing-close {
                    background: rgba(0, 212, 255, 0.1);
                    border: 1px solid rgba(0, 212, 255, 0.3);
                    color: #00d4ff;
                    width: 24px;
                    height: 24px;
                    border-radius: 50%;
                    cursor: pointer;
                    font-size: 16px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    transition: all 0.2s;
                }

                .gluon-pairing-close:hover {
                    background: rgba(0, 212, 255, 0.2);
                    border-color: #00d4ff;
                    box-shadow: 0 0 8px rgba(0, 212, 255, 0.3);
                }

                .gluon-pairing-status {
                    background: rgba(0, 212, 255, 0.05);
                    border: 1px solid rgba(0, 212, 255, 0.2);
                    border-radius: 8px;
                    padding: 8px 12px;
                    margin-bottom: 12px;
                    font-size: 13px;
                    display: flex;
                    align-items: center;
                    gap: 8px;
                }

                .gluon-pairing-status.connected {
                    background: rgba(0, 255, 136, 0.1);
                    border-color: rgba(0, 255, 136, 0.3);
                    box-shadow: 0 0 8px rgba(0, 255, 136, 0.2);
                }

                .gluon-pairing-status.disconnected {
                    background: rgba(248, 113, 113, 0.1);
                    border-color: rgba(248, 113, 113, 0.3);
                }

                .gluon-pairing-form {
                    display: flex;
                    flex-direction: column;
                    gap: 10px;
                }

                .gluon-agents-list {
                    display: flex;
                    flex-direction: column;
                    gap: 8px;
                    max-height: 300px;
                    overflow-y: auto;
                }

                .gluon-agent-item {
                    background: rgba(20, 25, 40, 0.8);
                    border: 1px solid rgba(0, 212, 255, 0.2);
                    border-radius: 8px;
                    padding: 12px;
                    cursor: pointer;
                    transition: all 0.2s;
                    display: flex;
                    flex-direction: column;
                    gap: 4px;
                    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.3);
                }

                .gluon-agent-item:hover {
                    background: rgba(20, 25, 40, 1);
                    border-color: #00d4ff;
                    transform: translateX(-2px);
                    box-shadow: 0 0 12px rgba(0, 212, 255, 0.3), inset 0 1px 2px rgba(0, 0, 0, 0.3);
                }

                .gluon-agent-item-name {
                    font-weight: 600;
                    font-size: 14px;
                    color: #00d4ff;
                    text-shadow: 0 0 8px rgba(0, 212, 255, 0.3);
                }

                .gluon-agent-item-code {
                    font-size: 11px;
                    color: #a3b3cc;
                    font-family: 'Courier New', monospace;
                    background: rgba(0, 0, 0, 0.3);
                    padding: 2px 6px;
                    border-radius: 4px;
                    display: inline-block;
                }

                .gluon-agent-item-status {
                    font-size: 11px;
                    display: flex;
                    align-items: center;
                    gap: 4px;
                }

                .gluon-agent-item.connected {
                    opacity: 0.6;
                    cursor: not-allowed;
                }

                .gluon-agent-item.connected:hover {
                    transform: none;
                    border-color: transparent;
                }

                .gluon-pairing-input {
                    background: rgba(255, 255, 255, 0.9);
                    border: 2px solid transparent;
                    border-radius: 8px;
                    padding: 10px 12px;
                    font-size: 14px;
                    color: #1f2937;
                    font-weight: 500;
                    text-align: center;
                    letter-spacing: 1px;
                    text-transform: uppercase;
                    transition: all 0.2s;
                }

                .gluon-pairing-input:focus {
                    outline: none;
                    border-color: #fbbf24;
                    background: white;
                }

                .gluon-pairing-input::placeholder {
                    color: #9ca3af;
                    text-transform: none;
                    letter-spacing: normal;
                }

                .gluon-pairing-button {
                    background: rgba(0, 212, 255, 0.15);
                    border: 1px solid rgba(0, 212, 255, 0.4);
                    border-radius: 8px;
                    padding: 10px 16px;
                    font-size: 14px;
                    font-weight: 600;
                    color: #00d4ff;
                    cursor: pointer;
                    transition: all 0.2s;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    gap: 6px;
                }

                .gluon-pairing-button:hover {
                    background: rgba(0, 212, 255, 0.25);
                    border-color: #00d4ff;
                    transform: translateY(-1px);
                    box-shadow: 0 0 16px rgba(0, 212, 255, 0.4), 0 4px 12px rgba(0, 0, 0, 0.3);
                }

                .gluon-pairing-button:active {
                    transform: translateY(0);
                }

                .gluon-pairing-button:disabled {
                    opacity: 0.5;
                    cursor: not-allowed;
                    transform: none;
                }

                .gluon-pairing-disconnect {
                    background: rgba(248, 113, 113, 0.9);
                    color: white;
                }

                .gluon-pairing-disconnect:hover {
                    background: rgba(248, 113, 113, 1);
                }

                .gluon-pairing-agent-info {
                    background: rgba(255, 255, 255, 0.15);
                    border-radius: 8px;
                    padding: 10px 12px;
                    margin-top: 8px;
                    font-size: 12px;
                }

                .gluon-pairing-agent-name {
                    font-weight: 600;
                    font-size: 14px;
                    margin-bottom: 4px;
                }

                .gluon-pairing-error {
                    background: rgba(248, 113, 113, 0.2);
                    border: 1px solid rgba(248, 113, 113, 0.4);
                    border-radius: 6px;
                    padding: 8px 10px;
                    font-size: 12px;
                    margin-top: 8px;
                }

                .gluon-pairing-help {
                    font-size: 11px;
                    opacity: 0.8;
                    margin-top: 8px;
                    text-align: center;
                }

                .gluon-pairing-toggle-btn {
                    position: fixed;
                    bottom: 24px;
                    right: 24px;
                    background: rgba(10, 13, 26, 0.95);
                    border: 2px solid rgba(0, 212, 255, 0.4);
                    border-radius: 50%;
                    width: 60px;
                    height: 60px;
                    color: white;
                    font-size: 24px;
                    cursor: move;
                    box-shadow:
                        0 0 20px rgba(0, 212, 255, 0.4),
                        0 4px 15px rgba(0, 0, 0, 0.5);
                    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
                    z-index: 999998;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    overflow: visible;
                    user-select: none;
                }

                .gluon-pairing-toggle-btn.dragging {
                    transition: none;
                    cursor: grabbing;
                    transform: scale(1.1);
                }

                .gluon-pairing-toggle-btn:hover {
                    transform: scale(1.1);
                    border-color: rgba(0, 212, 255, 0.6);
                    box-shadow:
                        0 0 30px rgba(0, 212, 255, 0.6),
                        0 6px 20px rgba(0, 0, 0, 0.5);
                }

                .gluon-pairing-toggle-btn:active {
                    transform: scale(0.95);
                    box-shadow:
                        0 0 15px rgba(0, 212, 255, 0.4),
                        0 2px 10px rgba(0, 0, 0, 0.5);
                }

                .gluon-pairing-toggle-btn img {
                    filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.3));
                    transition: transform 0.3s ease;
                }

                .gluon-pairing-toggle-btn:hover img {
                    transform: rotate(10deg) scale(1.05);
                }

                .gluon-pairing-toggle-btn.connected {
                    border-color: rgba(0, 255, 136, 0.5);
                    box-shadow:
                        0 0 20px rgba(0, 255, 136, 0.5),
                        0 4px 15px rgba(0, 0, 0, 0.5);
                }

                .gluon-pairing-toggle-btn.connected:hover {
                    border-color: rgba(0, 255, 136, 0.7);
                    box-shadow:
                        0 0 30px rgba(0, 255, 136, 0.7),
                        0 6px 20px rgba(0, 0, 0, 0.5);
                }
            </style>

            <!-- Toggle Button -->
            <button class="gluon-pairing-toggle-btn" id="gluonPairingToggle" title="Open Agent Pairing (Ctrl+Shift+P)">
                <img src="" style="width: 40px; height: 40px;">
            </button>

            <!-- Pairing Panel -->
            <div class="gluon-pairing-panel" id="gluonPairingPanel" style="display: none;">
                <div class="gluon-pairing-header">
                    <div class="gluon-pairing-title">
                        <img src="" style="width: 20px; height: 20px;">
                        <span>Agent Pairing</span>
                    </div>
                    <button class="gluon-pairing-close" id="gluonPairingClose">×</button>
                </div>

                <!-- Status -->
                <div class="gluon-pairing-status disconnected" id="gluonPairingStatus">
                    <span>🔴</span>
                    <span>Not Connected</span>
                </div>

                <!-- Form (shown when not paired) -->
                <div class="gluon-pairing-form" id="gluonPairingForm">
                    <div class="gluon-agents-list" id="gluonAgentsList">
                        <!-- Agents will be injected here -->
                        <div style="text-align: center; padding: 20px; color: rgba(255,255,255,0.6);">
                            Loading agents...
                        </div>
                    </div>
                    <div class="gluon-pairing-help" style="margin-top: 8px;">
                        Select an agent from Gluon Workflow<br>
                        to connect this tab
                    </div>
                </div>

                <!-- Agent Info (shown when paired) -->
                <div class="gluon-pairing-agent-info" id="gluonPairingAgentInfo" style="display: none;">
                    <div class="gluon-pairing-agent-name" id="gluonAgentName"></div>
                    <div>Code: <code id="gluonAgentCode"></code></div>
                </div>

                <!-- Disconnect Button (shown when paired) -->
                <button class="gluon-pairing-button gluon-pairing-disconnect" id="gluonPairingDisconnect" style="display: none;">
                    <span>🔌</span>
                    <span>Disconnect</span>
                </button>

                <!-- Error Message -->
                <div class="gluon-pairing-error" id="gluonPairingError" style="display: none;"></div>
            </div>
        `;

        document.body.appendChild(this.overlay);
        this.setIcons();
        this.attachEventListeners();
    }

    setIcons() {
        // Set icon sources dynamically after DOM insertion
        const iconUrl = chrome.runtime.getURL('assets/icons/icon128.png');
        const toggleIcon = this.overlay.querySelector('#gluonPairingToggle img');
        const headerIcon = this.overlay.querySelector('.gluon-pairing-title img');

        if (toggleIcon) {
            toggleIcon.src = iconUrl;
            toggleIcon.style.width = '40px';
            toggleIcon.style.height = '40px';
        }
        if (headerIcon) {
            headerIcon.src = iconUrl;
        }
    }

    attachEventListeners() {
        const toggleBtn = document.getElementById('gluonPairingToggle');

        // Toggle button - handle both click and drag
        let clickStartTime = 0;
        let hasMoved = false;

        toggleBtn.addEventListener('mousedown', (e) => {
            clickStartTime = Date.now();
            hasMoved = false;
            this.startDragging(e);
        });

        toggleBtn.addEventListener('touchstart', (e) => {
            clickStartTime = Date.now();
            hasMoved = false;
            const touch = e.touches[0];
            this.startDragging(touch);
        }, { passive: false });

        toggleBtn.addEventListener('click', (e) => {
            // Only toggle if it was a quick click without much movement
            const clickDuration = Date.now() - clickStartTime;
            if (!hasMoved && clickDuration < 200) {
                this.toggle();
            }
        });

        // Close button
        document.getElementById('gluonPairingClose').addEventListener('click', () => {
            this.hide();
        });

        // Disconnect button
        document.getElementById('gluonPairingDisconnect').addEventListener('click', () => {
            this.handleDisconnect();
        });

        // Global mouse move and up events for dragging
        document.addEventListener('mousemove', (e) => {
            if (this.isDragging) {
                hasMoved = true;
                this.drag(e);
            }
        });

        document.addEventListener('touchmove', (e) => {
            if (this.isDragging) {
                hasMoved = true;
                const touch = e.touches[0];
                this.drag(touch);
                e.preventDefault();
            }
        }, { passive: false });

        document.addEventListener('mouseup', () => {
            if (this.isDragging) {
                this.stopDragging();
            }
        });

        document.addEventListener('touchend', () => {
            if (this.isDragging) {
                this.stopDragging();
            }
        });

        // Load saved position
        this.loadButtonPosition();
    }

    toggle() {
        if (this.isVisible) {
            this.hide();
        } else {
            this.show();
        }
    }

    show() {
        document.getElementById('gluonPairingPanel').style.display = 'block';
        this.isVisible = true;
        // Refresh agents list when showing
        if (!this.isPaired) {
            this.loadAvailableAgents();
        }
    }

    hide() {
        document.getElementById('gluonPairingPanel').style.display = 'none';
        this.isVisible = false;
    }

    async loadAvailableAgents() {
        try {
            console.log('[AgentPairing] Loading available agents...');

            // Request workflow graph from background
            chrome.runtime.sendMessage({ action: 'workflow_get_graph' }, (requestId) => {
                if (chrome.runtime.lastError) {
                    console.error('[AgentPairing] Error requesting agents:', chrome.runtime.lastError);
                    this.renderAgentsList([]);
                    return;
                }

                console.log('[AgentPairing] Received requestId:', requestId);

                // Timeout after 25 seconds
                setTimeout(() => {
                    console.warn('[AgentPairing] Timeout waiting for agents response');
                    this.renderAgentsList([]);
                }, 25000);
            });
        } catch (error) {
            console.error('[AgentPairing] Error loading agents:', error);
            this.renderAgentsList([]);
        }
    }

    renderAgentsList(agents) {
        const listContainer = document.getElementById('gluonAgentsList');

        if (!agents || agents.length === 0) {
            listContainer.innerHTML = `
                <div style="text-align: center; padding: 20px; color: rgba(255,255,255,0.6);">
                    No agents available<br>
                    <span style="font-size: 11px;">Create agents in Gluon Sidebar</span>
                </div>
            `;
            return;
        }

        listContainer.innerHTML = agents.map(agent => {
            const statusIcon = agent.status === 'Connected' ? '🟢' :
                              agent.status === 'Waiting' ? '🟡' : '🔴';
            const isConnected = agent.status === 'Connected';

            return `
                <div class="gluon-agent-item ${isConnected ? 'connected' : ''}"
                     data-agent-id="${agent.id}"
                     data-pairing-code="${agent.pairing_code}"
                     ${isConnected ? '' : 'style="cursor: pointer;"'}>
                    <div class="gluon-agent-item-name">${this.escapeHtml(agent.name)}</div>
                    <div class="gluon-agent-item-code">Code: ${agent.pairing_code}</div>
                    <div class="gluon-agent-item-status">
                        <span>${statusIcon}</span>
                        <span style="color: ${isConnected ? '#10b981' : '#6b7280'};">
                            ${agent.status}${isConnected ? ' (on another tab)' : ''}
                        </span>
                    </div>
                </div>
            `;
        }).join('');

        // Add click listeners to agent items
        listContainer.querySelectorAll('.gluon-agent-item:not(.connected)').forEach(item => {
            item.addEventListener('click', () => {
                const pairingCode = item.dataset.pairingCode;
                const agentId = item.dataset.agentId;
                this.handleConnect(pairingCode, agentId);
            });
        });
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    async handleConnect(pairingCode, agentId) {
        if (!pairingCode) {
            this.showError('Invalid agent selection');
            return;
        }

        this.hideError();

        // Visual feedback
        const selectedItem = document.querySelector(`[data-agent-id="${agentId}"]`);
        if (selectedItem) {
            selectedItem.style.opacity = '0.5';
            selectedItem.style.pointerEvents = 'none';
        }

        try {
            // Send pairing request to background script
            chrome.runtime.sendMessage({
                action: 'agent_pair',
                pairing_code: pairingCode
            });

            // Wait for response via message listener
            const response = await new Promise((resolve, reject) => {
                const timeout = setTimeout(() => {
                    reject(new Error('Pairing timeout'));
                }, 50000);

                const listener = (message) => {
                    if (message.type === 'agent_pair_response') {
                        clearTimeout(timeout);
                        chrome.runtime.onMessage.removeListener(listener);
                        resolve(message);
                    }
                };

                chrome.runtime.onMessage.addListener(listener);
            });

            if (response && response.success) {
                this.agentInfo = response.agent;
                this.isPaired = true;
                this.updateUI();

                // Store pairing info in storage (unique per tab)
                const storageKey = this.getStorageKey();
                await chrome.storage.local.set({
                    [storageKey]: {
                        code: pairingCode,
                        agent: response.agent,
                        timestamp: Date.now()
                    }
                });

                console.log('[AgentPairing] Successfully paired with agent:', response.agent, 'on tab:', this.tabId);
                this.hide(); // Auto-hide after successful pairing
            } else {
                this.showError(response?.error || 'Failed to connect to agent');
                // Restore item
                if (selectedItem) {
                    selectedItem.style.opacity = '1';
                    selectedItem.style.pointerEvents = 'auto';
                }
            }
        } catch (error) {
            console.error('[AgentPairing] Connection error:', error);
            this.showError('Connection error: ' + error.message);
            // Restore item
            if (selectedItem) {
                selectedItem.style.opacity = '1';
                selectedItem.style.pointerEvents = 'auto';
            }
        }
    }

    async handleDisconnect() {
        if (!confirm('Disconnect from agent?')) return;

        try {
            // Send disconnect request
            await chrome.runtime.sendMessage({
                action: 'agent_unpair'
            });

            this.isPaired = false;
            this.agentInfo = null;
            this.updateUI();

            // Clear storage (only for this tab)
            const storageKey = this.getStorageKey();
            await chrome.storage.local.remove(storageKey);

            console.log('[AgentPairing] Disconnected from agent on tab:', this.tabId);
        } catch (error) {
            console.error('[AgentPairing] Disconnect error:', error);
            this.showError('Failed to disconnect: ' + error.message);
        }
    }

    async checkPairingStatus() {
        if (!this.tabId) {
            console.warn('[AgentPairing] Tab ID not yet set, skipping pairing check');
            return;
        }

        try {
            const storageKey = this.getStorageKey();
            const data = await chrome.storage.local.get(storageKey);
            if (data[storageKey]) {
                this.agentInfo = data[storageKey].agent;
                this.isPaired = true;
                this.updateUI();
                console.log('[AgentPairing] Restored pairing from storage for tab:', this.tabId);

                // Re-register with backend (in case page was refreshed)
                const pairingCode = data[storageKey].code;
                if (pairingCode) {
                    console.log('[AgentPairing] Re-registering agent with backend:', pairingCode);
                    chrome.runtime.sendMessage({
                        action: 'agent_pair',
                        pairing_code: pairingCode
                    }, (response) => {
                        // Suppress error - response will come via agent_pair_response message
                        if (chrome.runtime.lastError) {
                            // Ignore
                        }
                    });
                }
            }
        } catch (error) {
            console.error('[AgentPairing] Failed to check pairing status:', error);
        }
    }

    updateUI() {
        const statusEl = document.getElementById('gluonPairingStatus');
        const formEl = document.getElementById('gluonPairingForm');
        const agentInfoEl = document.getElementById('gluonPairingAgentInfo');
        const disconnectBtn = document.getElementById('gluonPairingDisconnect');
        const toggleBtn = document.getElementById('gluonPairingToggle');

        if (this.isPaired && this.agentInfo) {
            // Connected state
            statusEl.className = 'gluon-pairing-status connected';
            statusEl.innerHTML = '<span>🟢</span><span>Connected</span>';

            formEl.style.display = 'none';
            agentInfoEl.style.display = 'block';
            disconnectBtn.style.display = 'flex';

            document.getElementById('gluonAgentName').textContent = this.agentInfo.name;
            document.getElementById('gluonAgentCode').textContent = this.agentInfo.pairing_code;

            toggleBtn.classList.add('connected');
            toggleBtn.title = `Connected to ${this.agentInfo.name}`;
        } else {
            // Disconnected state
            statusEl.className = 'gluon-pairing-status disconnected';
            statusEl.innerHTML = '<span>🔴</span><span>Not Connected</span>';

            formEl.style.display = 'flex';
            agentInfoEl.style.display = 'none';
            disconnectBtn.style.display = 'none';

            toggleBtn.classList.remove('connected');
            toggleBtn.title = 'Open Agent Pairing (Ctrl+Shift+P)';
        }
    }

    showError(message) {
        const errorEl = document.getElementById('gluonPairingError');
        errorEl.textContent = message;
        errorEl.style.display = 'block';
    }

    hideError() {
        document.getElementById('gluonPairingError').style.display = 'none';
    }

    startDragging(e) {
        e.preventDefault();
        this.isDragging = true;

        const toggleBtn = document.getElementById('gluonPairingToggle');
        toggleBtn.classList.add('dragging');

        const rect = toggleBtn.getBoundingClientRect();
        this.dragOffset.x = e.clientX - rect.left;
        this.dragOffset.y = e.clientY - rect.top;
    }

    drag(e) {
        if (!this.isDragging) return;

        const toggleBtn = document.getElementById('gluonPairingToggle');

        // Calculate new position
        let newX = e.clientX - this.dragOffset.x;
        let newY = e.clientY - this.dragOffset.y;

        // Keep button within viewport bounds
        const btnWidth = toggleBtn.offsetWidth;
        const btnHeight = toggleBtn.offsetHeight;
        const maxX = window.innerWidth - btnWidth;
        const maxY = window.innerHeight - btnHeight;

        newX = Math.max(0, Math.min(newX, maxX));
        newY = Math.max(0, Math.min(newY, maxY));

        // Update position
        toggleBtn.style.left = newX + 'px';
        toggleBtn.style.top = newY + 'px';
        toggleBtn.style.right = 'auto';
        toggleBtn.style.bottom = 'auto';
    }

    stopDragging() {
        this.isDragging = false;

        const toggleBtn = document.getElementById('gluonPairingToggle');
        toggleBtn.classList.remove('dragging');

        // Save position to storage
        this.saveButtonPosition();
    }

    async saveButtonPosition() {
        const toggleBtn = document.getElementById('gluonPairingToggle');

        const position = {
            left: toggleBtn.style.left,
            top: toggleBtn.style.top,
            right: toggleBtn.style.right,
            bottom: toggleBtn.style.bottom
        };

        await chrome.storage.local.set({
            'gluon_pairing_button_position': position
        });
    }

    async loadButtonPosition() {
        try {
            const data = await chrome.storage.local.get('gluon_pairing_button_position');
            if (data.gluon_pairing_button_position) {
                const toggleBtn = document.getElementById('gluonPairingToggle');
                const pos = data.gluon_pairing_button_position;

                if (pos.left && pos.left !== 'auto') {
                    toggleBtn.style.left = pos.left;
                    toggleBtn.style.right = 'auto';
                }
                if (pos.top && pos.top !== 'auto') {
                    toggleBtn.style.top = pos.top;
                    toggleBtn.style.bottom = 'auto';
                }
            }
        } catch (error) {
            console.error('[AgentPairing] Failed to load button position:', error);
        }
    }

    setupMessageListener() {
        chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
            console.log('[AgentPairing] Received message:', message.type);

            if (message.type === 'workflow_response' && message.action === 'workflow_get_graph') {
                console.log('[AgentPairing] Received workflow response:', message);
                if (message.success && message.data && message.data.agents) {
                    this.availableAgents = Object.values(message.data.agents);
                    console.log('[AgentPairing] Loaded agents:', this.availableAgents);
                    this.renderAgentsList(this.availableAgents);
                } else {
                    console.error('[AgentPairing] Failed to load agents, no data');
                    this.renderAgentsList([]);
                }
            }

            // Refresh agents list when new agent is added
            if (message.type === 'workflow_response' && message.action === 'workflow_add_agent') {
                console.log('[AgentPairing] New agent added, refreshing list...');
                this.loadAvailableAgents();
            }

            if (message.type === 'agent_status_changed') {
                console.log('[AgentPairing] Agent status changed:', message);
                if (message.status === 'disconnected') {
                    this.isPaired = false;
                    this.agentInfo = null;
                    this.updateUI();
                }
            }
        });
    }
}

// Initialize when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        window.gluonAgentPairing = new AgentPairingOverlay();
    });
} else {
    window.gluonAgentPairing = new AgentPairingOverlay();
}
