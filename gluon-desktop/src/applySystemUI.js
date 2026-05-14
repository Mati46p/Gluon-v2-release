/**
 * Apply System Browser UI Module
 *
 * KROK 37-41: Browser UI Components
 *
 * Implements:
 * - Changes list in V3 Extension overlay
 * - Diff preview modal
 * - Apply All button
 * - Undo All button
 * - Toast notifications
 */

console.log("[Apply System UI] Module loaded");


// ============================================================================
// State
// ============================================================================

let applySystemState = {
    changes: [],
    selectedChangeId: null,
    vsCodeConnected: false,
};

// ============================================================================
// KROK 37: Changes List in Browser
// ============================================================================

/**
 * Initialize changes list UI
 */
function initializeChangesListUI() {
    const changesListEl = document.getElementById('changes-list');
    if (!changesListEl) {
        console.error("[Apply System UI] changes-list element not found");
        return;
    }

    // Render empty state initially
    renderChangesList();

    console.log("[Apply System UI] Changes list initialized");
}

/**
 * Render changes list
 */
function renderChangesList() {
    const changesListEl = document.getElementById('changes-list');
    if (!changesListEl) return;

    if (applySystemState.changes.length === 0) {
        changesListEl.innerHTML = `
            <div class="empty-state">
                <div class="empty-state-icon">📝</div>
                <p class="empty-state-text">No changes to review</p>
                <p class="empty-state-hint">AI-proposed code changes will appear here</p>
            </div>
        `;
        return;
    }

    // Group changes by file
    const changesByFile = new Map();
    for (const change of applySystemState.changes) {
        if (!changesByFile.has(change.file_path)) {
            changesByFile.set(change.file_path, []);
        }
        changesByFile.get(change.file_path).push(change);
    }

    // Render grouped list
    let html = '';
    for (const [filePath, changes] of changesByFile.entries()) {
        const fileName = getFileName(filePath);
        const dirName = getDirName(filePath);

        html += `
            <div class="file-group">
                <div class="file-group-header">
                    <div class="file-info">
                        <div class="file-name">${escapeHtml(fileName)}</div>
                        <div class="file-path">${escapeHtml(dirName)}</div>
                    </div>
                    <div class="file-badge">${changes.length} change${changes.length !== 1 ? 's' : ''}</div>
                </div>
                <div class="changes-list-items">
                    ${changes.map(change => renderChangeItem(change)).join('')}
                </div>
            </div>
        `;
    }

    changesListEl.innerHTML = html;

    // Attach event listeners
    attachChangeItemListeners();
}

/**
 * Render single change item
 */
function renderChangeItem(change) {
    const statusIcon = getStatusIcon(change.status);
    const statusClass = `status-${change.status}`;

    return `
        <div class="change-item ${statusClass}" data-change-id="${change.id}">
            <div class="change-item-header">
                <div class="change-status-icon">${statusIcon}</div>
                <div class="change-description">
                    Lines ${change.line_start}-${change.line_end}
                </div>
                <div class="change-actions">
                    ${change.status === 'pending' ? `
                        <button class="btn-icon preview-change" title="Preview" data-change-id="${change.id}">
                            👁️
                        </button>
                        <button class="btn-icon apply-change" title="Apply" data-change-id="${change.id}">
                            ✅
                        </button>
                        <button class="btn-icon skip-change" title="Skip" data-change-id="${change.id}">
                            ⏭️
                        </button>
                    ` : ''}
                    ${change.status === 'applied' ? `
                        <button class="btn-icon undo-change" title="Undo" data-change-id="${change.id}">
                            ↩️
                        </button>
                    ` : ''}
                </div>
            </div>
            ${change.error_message ? `
                <div class="change-error">
                    ⚠️ ${escapeHtml(change.error_message)}
                </div>
            ` : ''}
        </div>
    `;
}

/**
 * Get status icon for change
 */
function getStatusIcon(status) {
    switch (status) {
        case 'pending': return '⏳';
        case 'matching': return '🔍';
        case 'applying': return '⚙️';
        case 'applied': return '✅';
        case 'failed': return '❌';
        case 'skipped': return '⏭️';
        default: return '❓';
    }
}

/**
 * Attach event listeners to change items
 */
function attachChangeItemListeners() {
    // Preview buttons
    document.querySelectorAll('.preview-change').forEach(btn => {
        btn.addEventListener('click', (e) => {
            const changeId = e.currentTarget.dataset.changeId;
            handlePreviewChange(changeId);
        });
    });

    // Apply buttons
    document.querySelectorAll('.apply-change').forEach(btn => {
        btn.addEventListener('click', (e) => {
            const changeId = e.currentTarget.dataset.changeId;
            handleApplySingleChange(changeId);
        });
    });

    // Skip buttons
    document.querySelectorAll('.skip-change').forEach(btn => {
        btn.addEventListener('click', (e) => {
            const changeId = e.currentTarget.dataset.changeId;
            handleSkipChange(changeId);
        });
    });

    // Undo buttons
    document.querySelectorAll('.undo-change').forEach(btn => {
        btn.addEventListener('click', (e) => {
            const changeId = e.currentTarget.dataset.changeId;
            handleUndoChange(changeId);
        });
    });
}

// ============================================================================
// KROK 38: Diff Preview Modal
// ============================================================================

/**
 * Show diff preview modal for a change
 */
function showDiffPreviewModal(changeId) {
    const change = applySystemState.changes.find(c => c.id === changeId);
    if (!change) {
        console.error("[Apply System UI] Change not found:", changeId);
        return;
    }

    const modal = document.getElementById('diff-preview-modal');
    if (!modal) {
        console.error("[Apply System UI] diff-preview-modal element not found");
        return;
    }

    // Update modal content
    document.getElementById('modal-file-name').textContent = getFileName(change.file_path);
    document.getElementById('modal-file-path').textContent = change.file_path;
    document.getElementById('modal-lines-range').textContent = `Lines ${change.line_start}-${change.line_end}`;

    // Render old code
    document.getElementById('modal-old-code').innerHTML = renderCodeBlock(change.old_code);

    // Render new code
    document.getElementById('modal-new-code').innerHTML = renderCodeBlock(change.new_code);

    // Store current change ID
    applySystemState.selectedChangeId = changeId;

    // Show modal
    modal.style.display = 'flex';

    console.log("[Apply System UI] Showing diff modal for:", changeId);
}

/**
 * Hide diff preview modal
 */
function hideDiffPreviewModal() {
    const modal = document.getElementById('diff-preview-modal');
    if (modal) {
        modal.style.display = 'none';
    }
    applySystemState.selectedChangeId = null;
}

/**
 * Render code block with line numbers
 */
function renderCodeBlock(code) {
    const lines = code.split('\n');
    let html = '';

    for (let i = 0; i < lines.length; i++) {
        const lineNum = i + 1;
        const lineContent = escapeHtml(lines[i]);

        html += `
            <div class="code-line">
                <span class="line-number">${lineNum}</span>
                <span class="line-content">${lineContent}</span>
            </div>
        `;
    }

    return html;
}

// ============================================================================
// KROK 39 & 40: Apply All / Undo All Buttons
// ============================================================================

/**
 * Handle Apply All action
 */
async function handleApplyAll() {
    const pendingChanges = applySystemState.changes.filter(c => c.status === 'pending');

    if (pendingChanges.length === 0) {
        showToast('No pending changes to apply', 'info');
        return;
    }

    const confirmed = await window.__TAURI__.dialog.confirm(
        `Apply all ${pendingChanges.length} pending changes?`,
        {
            title: 'Confirm Apply All',
            type: 'warning'
        }
    );

    if (!confirmed) return;

    try {
        console.log("[Apply System UI] Applying all changes...");

        const result = await invoke('apply_all_changes');

        showToast(`Applying ${pendingChanges.length} changes...`, 'success');

        // Changes will be updated via event listener

    } catch (error) {
        console.error("[Apply System UI] Apply all failed:", error);
        showToast(`Failed to apply all: ${error}`, 'error');
    }
}

/**
 * Handle Undo All action
 */
async function handleUndoAll() {
    const appliedChanges = applySystemState.changes.filter(c => c.status === 'applied');

    if (appliedChanges.length === 0) {
        showToast('No applied changes to undo', 'info');
        return;
    }

    const confirmed = await window.__TAURI__.dialog.confirm(
        `Undo all ${appliedChanges.length} applied changes? This will restore files to their previous state.`,
        {
            title: 'Confirm Undo All',
            type: 'warning'
        }
    );

    if (!confirmed) return;

    try {
        console.log("[Apply System UI] Undoing all changes...");

        // Undo each change in reverse order
        for (const change of appliedChanges.reverse()) {
            await invoke('undo_change', { changeId: change.id });
        }

        showToast(`Undone ${appliedChanges.length} changes`, 'success');

    } catch (error) {
        console.error("[Apply System UI] Undo all failed:", error);
        showToast(`Failed to undo all: ${error}`, 'error');
    }
}

// ============================================================================
// KROK 41: Toast Notifications
// ============================================================================

/**
 * Show toast notification
 */
function showToast(message, type = 'info') {
    const toastContainer = document.getElementById('toast-container');
    if (!toastContainer) {
        console.error("[Apply System UI] toast-container element not found");
        return;
    }

    const toastId = `toast-${Date.now()}`;
    const icons = {
        success: '✅',
        error: '❌',
        warning: '⚠️',
        info: 'ℹ️'
    };

    const icon = icons[type] || icons.info;

    const toast = document.createElement('div');
    toast.id = toastId;
    toast.className = `toast toast-${type}`;
    toast.innerHTML = `
        <div class="toast-icon">${icon}</div>
        <div class="toast-message">${escapeHtml(message)}</div>
        <button class="toast-close" onclick="closeToast('${toastId}')">×</button>
    `;

    toastContainer.appendChild(toast);

    // Trigger animation
    setTimeout(() => {
        toast.classList.add('toast-visible');
    }, 10);

    // Auto-remove after 5 seconds
    setTimeout(() => {
        closeToast(toastId);
    }, 5000);

    console.log(`[Apply System UI] Toast: [${type}] ${message}`);
}

/**
 * Close toast notification
 */
function closeToast(toastId) {
    const toast = document.getElementById(toastId);
    if (toast) {
        toast.classList.remove('toast-visible');
        setTimeout(() => {
            toast.remove();
        }, 300);
    }
}

// Make closeToast globally accessible for onclick
window.closeToast = closeToast;

// ============================================================================
// Action Handlers
// ============================================================================

/**
 * Handle preview change
 */
function handlePreviewChange(changeId) {
    console.log("[Apply System UI] Preview change:", changeId);
    showDiffPreviewModal(changeId);
}

/**
 * Handle apply single change
 */
async function handleApplySingleChange(changeId) {
    try {
        console.log("[Apply System UI] Applying change:", changeId);

        // Get file content (this would normally come from reading the file)
        // For now, we'll let Tauri handle it
        await invoke('apply_change_command', {
            changeId,
            fileContent: '' // Tauri will read the file
        });

        showToast('Applying change...', 'info');

    } catch (error) {
        console.error("[Apply System UI] Apply failed:", error);
        showToast(`Failed to apply: ${error}`, 'error');
    }
}

/**
 * Handle skip change
 */
async function handleSkipChange(changeId) {
    try {
        console.log("[Apply System UI] Skipping change:", changeId);

        // Mark change as skipped
        const change = applySystemState.changes.find(c => c.id === changeId);
        if (change) {
            change.status = 'skipped';
            renderChangesList();
            showToast('Change skipped', 'info');
        }

    } catch (error) {
        console.error("[Apply System UI] Skip failed:", error);
        showToast(`Failed to skip: ${error}`, 'error');
    }
}

/**
 * Handle undo change
 */
async function handleUndoChange(changeId) {
    try {
        console.log("[Apply System UI] Undoing change:", changeId);

        await invoke('undo_change', { changeId });

        showToast('Undoing change...', 'info');

    } catch (error) {
        console.error("[Apply System UI] Undo failed:", error);
        showToast(`Failed to undo: ${error}`, 'error');
    }
}

// ============================================================================
// Tauri Event Listeners
// ============================================================================

/**
 * Listen for apply system events from Tauri
 */
function setupEventListeners() {
    // Listen for apply system updates
    listen('apply-system-status', (event) => {
        console.log("[Apply System UI] Status update:", event.payload);

        const { status, message, data } = event.payload;

        if (status === 'changes_updated') {
            applySystemState.changes = data.changes || [];
            renderChangesList();
        } else if (status === 'vscode_connected') {
            applySystemState.vsCodeConnected = data.connected;
            updateVSCodeConnectionStatus(data.connected);
        }

        if (message) {
            showToast(message, status === 'error' ? 'error' : 'info');
        }
    });

    // Listen for VS Code connection updates
    listen('vscode-connection', (event) => {
        console.log("[Apply System UI] VS Code connection event:", event.payload);
        // Protocol: { type: "vsCodeConnectionStatus", connected: boolean }
        const connected = event.payload.connected;
        applySystemState.vsCodeConnected = connected;
        updateVSCodeConnectionStatus(connected);
        
        if (connected) {
            showToast('VS Code Connected', 'success');
        } else {
            showToast('VS Code Disconnected', 'warning');
        }
    });

    // Listen for apply completion
    listen('apply-system-apply', (event) => {
        console.log("[Apply System UI] Apply event:", event.payload);

        const { success, change_id, error } = event.payload;

        if (success) {
            showToast('Change applied successfully', 'success');

            // Update change status
            const change = applySystemState.changes.find(c => c.id === change_id);
            if (change) {
                change.status = 'applied';
                renderChangesList();
            }
        } else {
            showToast(`Apply failed: ${error}`, 'error');

            // Update change status
            const change = applySystemState.changes.find(c => c.id === change_id);
            if (change) {
                change.status = 'failed';
                change.error_message = error;
                renderChangesList();
            }
        }
    });

    console.log("[Apply System UI] Event listeners setup complete");
}

/**
 * Update VS Code connection status indicator
 */
function updateVSCodeConnectionStatus(connected) {
    const statusEl = document.getElementById('vscode-connection-status');
    if (statusEl) {
        statusEl.className = `connection-indicator ${connected ? 'connected' : 'disconnected'}`;
        statusEl.title = connected ? 'VS Code Connected' : 'VS Code Disconnected';
    }
}

// ============================================================================
// Initialization
// ============================================================================

/**
 * Initialize Apply System UI
 */
function initializeApplySystemUI() {
    console.log("[Apply System UI] Initializing...");

    // Initialize changes list
    initializeChangesListUI();

    // Setup event listeners
    setupEventListeners();

    // Attach button event listeners
    const applyAllBtn = document.getElementById('apply-all-btn');
    if (applyAllBtn) {
        applyAllBtn.addEventListener('click', handleApplyAll);
    }

    const undoAllBtn = document.getElementById('undo-all-btn');
    if (undoAllBtn) {
        undoAllBtn.addEventListener('click', handleUndoAll);
    }

    // Modal close button
    const modalCloseBtn = document.getElementById('modal-close-btn');
    if (modalCloseBtn) {
        modalCloseBtn.addEventListener('click', hideDiffPreviewModal);
    }

    // Modal apply button
    const modalApplyBtn = document.getElementById('modal-apply-btn');
    if (modalApplyBtn) {
        modalApplyBtn.addEventListener('click', () => {
            if (applySystemState.selectedChangeId) {
                handleApplySingleChange(applySystemState.selectedChangeId);
                hideDiffPreviewModal();
            }
        });
    }

    // Modal skip button
    const modalSkipBtn = document.getElementById('modal-skip-btn');
    if (modalSkipBtn) {
        modalSkipBtn.addEventListener('click', () => {
            if (applySystemState.selectedChangeId) {
                handleSkipChange(applySystemState.selectedChangeId);
                hideDiffPreviewModal();
            }
        });
    }

    // Close modal on background click
    const modal = document.getElementById('diff-preview-modal');
    if (modal) {
        modal.addEventListener('click', (e) => {
            if (e.target === modal) {
                hideDiffPreviewModal();
            }
        });
    }

    // Load initial changes
    loadChanges();

    // Check initial connection status
    checkInitialVSCodeStatus();

    console.log("[Apply System UI] Initialization complete");
}

/**
 * Load changes from Tauri
 */
async function loadChanges() {
    try {
        const changes = await invoke('get_change_queue');
        applySystemState.changes = changes;
        renderChangesList();

        console.log("[Apply System UI] Loaded", changes.length, "changes");
    } catch (error) {
        console.error("[Apply System UI] Failed to load changes:", error);
    }
}

/**
 * Check initial VS Code connection status
 */
async function checkInitialVSCodeStatus() {
    try {
        const connected = await invoke('check_vscode_connection');
        console.log("[Apply System UI] Initial VS Code status:", connected);
        applySystemState.vsCodeConnected = connected;
        updateVSCodeConnectionStatus(connected);
    } catch (error) {
        console.error("[Apply System UI] Failed to check VS Code status:", error);
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

function getFileName(filePath) {
    return filePath.split(/[\\/]/).pop() || filePath;
}

function getDirName(filePath) {
    const parts = filePath.split(/[\\/]/);
    parts.pop();
    return parts.join('/') || '/';
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// ============================================================================
// Export
// ============================================================================

window.ApplySystemUI = {
    initialize: initializeApplySystemUI,
    showToast,
    loadChanges,
    renderChangesList
};

console.log("[Apply System UI] Module ready");
