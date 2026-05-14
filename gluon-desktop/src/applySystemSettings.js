/**
 * KROK 43: Settings Panel for Apply System
 *
 * Allows users to configure:
 * - VS Code integration toggle
 * - UI variants (Tree View, CodeLens, WebView, Modal)
 * - Security settings (whitelist/blacklist)
 * - Performance settings
 * - Connection settings
 */

console.log("[Apply System Settings] Module loaded");

// ============================================================================
// State
// ============================================================================

let currentConfig = null;

// ============================================================================
// Load Configuration
// ============================================================================

async function loadApplySystemConfig() {
    try {
        currentConfig = await invoke('get_config');
        console.log("[Apply System Settings] Loaded config:", currentConfig);

        // Populate UI with config values
        populateSettings(currentConfig);

        return currentConfig;
    } catch (error) {
        console.error("[Apply System Settings] Failed to load config:", error);
        showToast('Failed to load settings', 'error');
        return null;
    }
}

// ============================================================================
// Populate Settings UI
// ============================================================================

function populateSettings(config) {
    // VS Code Integration
    const vsCodeToggle = document.getElementById('vs-code-integration-toggle');
    if (vsCodeToggle) {
        vsCodeToggle.checked = config.vsCodeIntegrationEnabled || false;
    }

    // UI Variants
    if (config.enabledUiVariants) {
        const treeViewToggle = document.getElementById('ui-tree-view-toggle');
        const codeLensToggle = document.getElementById('ui-codelens-toggle');
        const webviewToggle = document.getElementById('ui-webview-toggle');
        const modalToggle = document.getElementById('ui-modal-toggle');

        if (treeViewToggle) treeViewToggle.checked = config.enabledUiVariants.treeView || false;
        if (codeLensToggle) codeLensToggle.checked = config.enabledUiVariants.codeLens || false;
        if (webviewToggle) webviewToggle.checked = config.enabledUiVariants.webviewPanel || false;
        if (modalToggle) modalToggle.checked = config.enabledUiVariants.pseudoModal || false;
    }

    // Path Config
    if (config.pathConfig) {
        const strictWhitelistToggle = document.getElementById('strict-whitelist-toggle');
        if (strictWhitelistToggle) {
            strictWhitelistToggle.checked = config.pathConfig.strictWhitelist || false;
        }

        // Display whitelisted paths
        const whitelistEl = document.getElementById('whitelisted-paths-list');
        if (whitelistEl && config.pathConfig.whitelistedPaths) {
            renderPathList(whitelistEl, config.pathConfig.whitelistedPaths, 'whitelist');
        }

        // Display custom blacklist
        const blacklistEl = document.getElementById('custom-blacklist-list');
        if (blacklistEl && config.pathConfig.customBlacklist) {
            renderPathList(blacklistEl, config.pathConfig.customBlacklist, 'blacklist');
        }
    }

    // Performance Config
    if (config.performance) {
        const fuzzyRangeInput = document.getElementById('fuzzy-search-range');
        const confidenceInput = document.getElementById('min-confidence-threshold');
        const maxHistoryInput = document.getElementById('max-history-size');
        const parallelToggle = document.getElementById('parallel-batch-toggle');
        const maxConcurrentInput = document.getElementById('max-concurrent-applies');

        if (fuzzyRangeInput) fuzzyRangeInput.value = config.performance.fuzzySearchRange || 50;
        if (confidenceInput) confidenceInput.value = config.performance.minConfidenceThreshold || 0.7;
        if (maxHistoryInput) maxHistoryInput.value = config.performance.maxHistorySize || 100;
        if (parallelToggle) parallelToggle.checked = config.performance.parallelBatchApply || false;
        if (maxConcurrentInput) maxConcurrentInput.value = config.performance.maxConcurrentApplies || 4;
    }

    // Connection Config
    if (config.connection) {
        const vscodeTimeoutInput = document.getElementById('vscode-timeout');
        const heartbeatIntervalInput = document.getElementById('heartbeat-interval');
        const reconnectRetryInput = document.getElementById('reconnect-retry');
        const maxReconnectInput = document.getElementById('max-reconnect-attempts');

        if (vscodeTimeoutInput) vscodeTimeoutInput.value = config.connection.vscodeTimeoutSeconds || 30;
        if (heartbeatIntervalInput) heartbeatIntervalInput.value = config.connection.heartbeatIntervalSeconds || 10;
        if (reconnectRetryInput) reconnectRetryInput.value = config.connection.reconnectRetrySeconds || 5;
        if (maxReconnectInput) maxReconnectInput.value = config.connection.maxReconnectAttempts || 5;
    }
}

// ============================================================================
// Render Path Lists
// ============================================================================

function renderPathList(container, paths, type) {
    container.innerHTML = '';

    if (!paths || paths.length === 0) {
        container.innerHTML = `<p class="empty-list">No ${type} paths configured</p>`;
        return;
    }

    const list = document.createElement('ul');
    list.className = 'path-list';

    for (let i = 0; i < paths.length; i++) {
        const path = paths[i];
        const li = document.createElement('li');
        li.className = 'path-list-item';

        const pathText = document.createElement('span');
        pathText.textContent = path;
        pathText.className = 'path-text';

        const removeBtn = document.createElement('button');
        removeBtn.textContent = '×';
        removeBtn.className = 'btn-icon-small';
        removeBtn.title = 'Remove';
        removeBtn.onclick = () => removePath(type, i);

        li.appendChild(pathText);
        li.appendChild(removeBtn);
        list.appendChild(li);
    }

    container.appendChild(list);
}

// ============================================================================
// Save Settings
// ============================================================================

async function saveApplySystemSettings() {
    try {
        // Collect values from UI
        const vsCodeToggle = document.getElementById('vs-code-integration-toggle');

        const config = {
            vsCodeIntegrationEnabled: vsCodeToggle ? vsCodeToggle.checked : currentConfig.vsCodeIntegrationEnabled,

            enabledUiVariants: {
                treeView: document.getElementById('ui-tree-view-toggle')?.checked ?? true,
                codeLens: document.getElementById('ui-codelens-toggle')?.checked ?? true,
                webviewPanel: document.getElementById('ui-webview-toggle')?.checked ?? true,
                pseudoModal: document.getElementById('ui-modal-toggle')?.checked ?? false,
            },

            pathConfig: {
                whitelistedPaths: currentConfig.pathConfig.whitelistedPaths,
                customBlacklist: currentConfig.pathConfig.customBlacklist,
                strictWhitelist: document.getElementById('strict-whitelist-toggle')?.checked ?? false,
            },

            performance: {
                fuzzySearchRange: parseInt(document.getElementById('fuzzy-search-range')?.value || 50),
                minConfidenceThreshold: parseFloat(document.getElementById('min-confidence-threshold')?.value || 0.7),
                maxHistorySize: parseInt(document.getElementById('max-history-size')?.value || 100),
                parallelBatchApply: document.getElementById('parallel-batch-toggle')?.checked ?? true,
                maxConcurrentApplies: parseInt(document.getElementById('max-concurrent-applies')?.value || 4),
            },

            connection: {
                vscodeTimeoutSeconds: parseInt(document.getElementById('vscode-timeout')?.value || 30),
                heartbeatIntervalSeconds: parseInt(document.getElementById('heartbeat-interval')?.value || 10),
                reconnectRetrySeconds: parseInt(document.getElementById('reconnect-retry')?.value || 5),
                maxReconnectAttempts: parseInt(document.getElementById('max-reconnect-attempts')?.value || 5),
            },
        };

        console.log("[Apply System Settings] Saving config:", config);

        await invoke('update_config', { newConfig: config });

        currentConfig = config;

        showToast('Settings saved successfully', 'success');

        console.log("[Apply System Settings] Settings saved");
    } catch (error) {
        console.error("[Apply System Settings] Failed to save settings:", error);
        showToast(`Failed to save settings: ${error}`, 'error');
    }
}

// ============================================================================
// Add/Remove Paths
// ============================================================================

async function addWhitelistedPath() {
    const input = document.getElementById('add-whitelist-path');
    if (!input || !input.value) return;

    const path = input.value.trim();
    if (!path) return;

    currentConfig.pathConfig.whitelistedPaths.push(path);

    const whitelistEl = document.getElementById('whitelisted-paths-list');
    if (whitelistEl) {
        renderPathList(whitelistEl, currentConfig.pathConfig.whitelistedPaths, 'whitelist');
    }

    input.value = '';
}

async function addBlacklistedPath() {
    const input = document.getElementById('add-blacklist-path');
    if (!input || !input.value) return;

    const path = input.value.trim();
    if (!path) return;

    currentConfig.pathConfig.customBlacklist.push(path);

    const blacklistEl = document.getElementById('custom-blacklist-list');
    if (blacklistEl) {
        renderPathList(blacklistEl, currentConfig.pathConfig.customBlacklist, 'blacklist');
    }

    input.value = '';
}

function removePath(type, index) {
    if (type === 'whitelist') {
        currentConfig.pathConfig.whitelistedPaths.splice(index, 1);
        const whitelistEl = document.getElementById('whitelisted-paths-list');
        if (whitelistEl) {
            renderPathList(whitelistEl, currentConfig.pathConfig.whitelistedPaths, 'whitelist');
        }
    } else if (type === 'blacklist') {
        currentConfig.pathConfig.customBlacklist.splice(index, 1);
        const blacklistEl = document.getElementById('custom-blacklist-list');
        if (blacklistEl) {
            renderPathList(blacklistEl, currentConfig.pathConfig.customBlacklist, 'blacklist');
        }
    }
}

// ============================================================================
// Initialization
// ============================================================================

function initializeApplySystemSettings() {
    console.log("[Apply System Settings] Initializing...");

    // Load config
    loadApplySystemConfig();

    // Attach event listeners
    const saveBtn = document.getElementById('save-apply-settings-btn');
    if (saveBtn) {
        saveBtn.addEventListener('click', saveApplySystemSettings);
    }

    const addWhitelistBtn = document.getElementById('add-whitelist-btn');
    if (addWhitelistBtn) {
        addWhitelistBtn.addEventListener('click', addWhitelistedPath);
    }

    const addBlacklistBtn = document.getElementById('add-blacklist-btn');
    if (addBlacklistBtn) {
        addBlacklistBtn.addEventListener('click', addBlacklistedPath);
    }

    // Enter key on inputs
    const addWhitelistInput = document.getElementById('add-whitelist-path');
    if (addWhitelistInput) {
        addWhitelistInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') addWhitelistedPath();
        });
    }

    const addBlacklistInput = document.getElementById('add-blacklist-path');
    if (addBlacklistInput) {
        addBlacklistInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') addBlacklistedPath();
        });
    }

    console.log("[Apply System Settings] Initialization complete");
}

// ============================================================================
// Toast Helper (uses main applySystemUI toast)
// ============================================================================

function showToast(message, type) {
    if (window.ApplySystemUI && window.ApplySystemUI.showToast) {
        window.ApplySystemUI.showToast(message, type);
    } else {
        console.log(`[Toast] [${type}] ${message}`);
    }
}

// ============================================================================
// Export
// ============================================================================

window.ApplySystemSettings = {
    initialize: initializeApplySystemSettings,
    loadConfig: loadApplySystemConfig,
    saveSettings: saveApplySystemSettings,
};

console.log("[Apply System Settings] Module ready");
