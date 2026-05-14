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

    // Render unified diff
    const diffContainer = document.getElementById('modal-diff-content');
    if (diffContainer) {
        diffContainer.innerHTML = renderUnifiedDiff(change.old_code, change.new_code, change.line_start);

        // Setup diff navigation and features
        setupDiffNavigation(diffContainer);
        setupDiffZoom(diffContainer);
        renderDiffMinimap(diffContainer);
    } else {
        // Fallback to old two-column layout
        document.getElementById('modal-old-code').innerHTML = renderCodeBlock(change.old_code);
        document.getElementById('modal-new-code').innerHTML = renderCodeBlock(change.new_code);
    }

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

    // Cleanup event listeners
    if (window.cleanupDiffNavigation) {
        window.cleanupDiffNavigation();
        window.cleanupDiffNavigation = null;
    }
    if (window.cleanupDiffZoom) {
        window.cleanupDiffZoom();
        window.cleanupDiffZoom = null;
    }
    if (window.cleanupDiffMinimap) {
        window.cleanupDiffMinimap();
        window.cleanupDiffMinimap = null;
    }
}

/**
 * Render unified diff in Git style with change blocks tracking
 */
function renderUnifiedDiff(oldCode, newCode, startLine = 1) {
    const oldLines = oldCode.split('\n');
    const newLines = newCode.split('\n');

    // Simple diff algorithm using longest common subsequence
    const diff = computeDiff(oldLines, newLines);

    let html = '';
    let oldLineNum = startLine;
    let newLineNum = startLine;
    let changeBlockIndex = 0;

    // Store change block positions for navigation
    window.diffChangeBlocks = [];

    for (const block of diff) {
        if (block.type === 'unchanged') {
            // Render unchanged lines (context)
            for (const line of block.lines) {
                html += `
                    <div class="diff-line">
                        <span class="diff-line-num diff-line-num-old">${oldLineNum}</span>
                        <span class="diff-line-num diff-line-num-new">${newLineNum}</span>
                        <span class="diff-line-prefix"> </span>
                        <span class="diff-line-content">${escapeHtml(line)}</span>
                    </div>
                `;
                oldLineNum++;
                newLineNum++;
            }
        } else if (block.type === 'deleted' || block.type === 'added') {
            if (block.type === 'deleted') {
                // Track this change block for navigation
                window.diffChangeBlocks.push(changeBlockIndex);
            }

            // Render deleted lines
            if (block.type === 'deleted') {
                for (const line of block.lines) {
                    html += `
                        <div class="diff-line diff-line-remove" data-change-block="${changeBlockIndex}">
                            <span class="diff-line-num diff-line-num-old">${oldLineNum}</span>
                            <span class="diff-line-num diff-line-num-new"></span>
                            <span class="diff-line-prefix">-</span>
                            <span class="diff-line-content">${escapeHtml(line)}</span>
                        </div>
                    `;
                    oldLineNum++;
                }
            }

            // Render added lines
            if (block.type === 'added') {
                for (const line of block.lines) {
                    html += `
                        <div class="diff-line diff-line-add" data-change-block="${changeBlockIndex}">
                            <span class="diff-line-num diff-line-num-old"></span>
                            <span class="diff-line-num diff-line-num-new">${newLineNum}</span>
                            <span class="diff-line-prefix">+</span>
                            <span class="diff-line-content">${escapeHtml(line)}</span>
                        </div>
                    `;
                    newLineNum++;
                }
                // Increment block index after additions (completing the change block)
                changeBlockIndex++;
            }
        }
    }

    return html;
}

/**
 * Compute diff between two arrays of lines
 * Returns blocks of changes grouped together (like Git does)
 */
function computeDiff(oldLines, newLines) {
    const blocks = [];
    let i = 0, j = 0;
    const LOOKAHEAD = 5; // How far to look for matching lines

    while (i < oldLines.length || j < newLines.length) {
        // Phase 1: Collect all consecutive matching lines
        const matchStart = { old: i, new: j };
        while (i < oldLines.length && j < newLines.length && oldLines[i] === newLines[j]) {
            i++;
            j++;
        }

        // Add unchanged block if we found matches
        if (i > matchStart.old) {
            blocks.push({
                type: 'unchanged',
                lines: oldLines.slice(matchStart.old, i)
            });
        }

        // If we reached the end, we're done
        if (i >= oldLines.length && j >= newLines.length) {
            break;
        }

        // Phase 2: Find the next anchor point (matching lines)
        let foundMatch = false;
        let bestMatch = null;
        let bestMatchQuality = 0;

        // Look ahead with limited scope
        const oldSearchEnd = Math.min(i + LOOKAHEAD, oldLines.length);
        const newSearchEnd = Math.min(j + LOOKAHEAD, newLines.length);

        for (let oi = i; oi < oldSearchEnd; oi++) {
            for (let nj = j; nj < newSearchEnd; nj++) {
                if (oldLines[oi] === newLines[nj]) {
                    // Count consecutive matching lines
                    let matchLen = 0;
                    while (oi + matchLen < oldLines.length &&
                           nj + matchLen < newLines.length &&
                           oldLines[oi + matchLen] === newLines[nj + matchLen]) {
                        matchLen++;
                    }

                    // Quality: prefer matches that are closer and longer
                    const distance = (oi - i) + (nj - j);
                    const quality = matchLen * 10 - distance;

                    if (quality > bestMatchQuality) {
                        bestMatchQuality = quality;
                        bestMatch = { old: oi, new: nj, len: matchLen };
                        foundMatch = true;
                    }
                }
            }
        }

        if (foundMatch && bestMatch) {
            // We found an anchor - create deletion and addition blocks
            const deletedLines = oldLines.slice(i, bestMatch.old);
            const addedLines = newLines.slice(j, bestMatch.new);

            // Group deletions and additions together in one change block
            if (deletedLines.length > 0 || addedLines.length > 0) {
                // First add all deletions
                if (deletedLines.length > 0) {
                    blocks.push({
                        type: 'deleted',
                        lines: deletedLines
                    });
                }
                // Then add all additions
                if (addedLines.length > 0) {
                    blocks.push({
                        type: 'added',
                        lines: addedLines
                    });
                }
            }

            i = bestMatch.old;
            j = bestMatch.new;
        } else {
            // No match found - treat remaining lines as one big change
            const deletedLines = oldLines.slice(i);
            const addedLines = newLines.slice(j);

            if (deletedLines.length > 0) {
                blocks.push({
                    type: 'deleted',
                    lines: deletedLines
                });
            }
            if (addedLines.length > 0) {
                blocks.push({
                    type: 'added',
                    lines: addedLines
                });
            }

            i = oldLines.length;
            j = newLines.length;
        }
    }

    return blocks;
}

/**
 * Setup diff navigation (arrow keys and buttons)
 */
function setupDiffNavigation(diffContainer) {
    // Remove old navigation if exists
    const oldNav = document.getElementById('diff-navigation');
    if (oldNav) oldNav.remove();

    // Create navigation controls
    const nav = document.createElement('div');
    nav.id = 'diff-navigation';
    nav.className = 'diff-navigation';
    nav.innerHTML = `
        <button id="prev-change-btn" class="diff-nav-btn" title="Previous change (↑)">↑</button>
        <span id="change-counter" class="diff-nav-counter">0 / 0</span>
        <button id="next-change-btn" class="diff-nav-btn" title="Next change (↓)">↓</button>
    `;

    // Insert navigation before diff container
    diffContainer.parentElement.insertBefore(nav, diffContainer);

    // Current change index
    let currentChangeIndex = -1;
    const totalChanges = window.diffChangeBlocks ? window.diffChangeBlocks.length : 0;

    // Update counter display
    function updateCounter() {
        const counter = document.getElementById('change-counter');
        if (counter) {
            if (totalChanges === 0) {
                counter.textContent = 'No changes';
            } else {
                counter.textContent = `${currentChangeIndex + 1} / ${totalChanges}`;
            }
        }
    }

    // Navigate to specific change
    function navigateToChange(index) {
        if (totalChanges === 0) return;

        // Wrap around
        if (index < 0) index = totalChanges - 1;
        if (index >= totalChanges) index = 0;

        currentChangeIndex = index;

        // Find and scroll to the change block
        const changeBlock = window.diffChangeBlocks[index];
        const element = diffContainer.querySelector(`[data-change-block="${changeBlock}"]`);

        if (element) {
            // Remove previous highlight
            diffContainer.querySelectorAll('.diff-line-highlighted').forEach(el => {
                el.classList.remove('diff-line-highlighted');
            });

            // Highlight all lines in this change block
            diffContainer.querySelectorAll(`[data-change-block="${changeBlock}"]`).forEach(el => {
                el.classList.add('diff-line-highlighted');
            });

            // Scroll to element
            element.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }

        updateCounter();
    }

    // Button handlers
    document.getElementById('prev-change-btn')?.addEventListener('click', () => {
        navigateToChange(currentChangeIndex - 1);
    });

    document.getElementById('next-change-btn')?.addEventListener('click', () => {
        navigateToChange(currentChangeIndex + 1);
    });

    // Keyboard navigation
    const keyHandler = (e) => {
        if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
            e.preventDefault();
            if (e.key === 'ArrowUp') {
                navigateToChange(currentChangeIndex - 1);
            } else {
                navigateToChange(currentChangeIndex + 1);
            }
        }
    };

    document.addEventListener('keydown', keyHandler);

    // Store cleanup function
    diffContainer.dataset.cleanupNav = 'true';
    const oldCleanup = window.cleanupDiffNavigation;
    window.cleanupDiffNavigation = () => {
        document.removeEventListener('keydown', keyHandler);
        if (oldCleanup) oldCleanup();
    };

    // Initialize counter
    updateCounter();

    // Auto-navigate to first change if exists
    if (totalChanges > 0) {
        setTimeout(() => navigateToChange(0), 100);
    }
}

/**
 * Setup diff zoom (Ctrl +/-)
 */
function setupDiffZoom(diffContainer) {
    let currentZoom = 100; // percentage
    const MIN_ZOOM = 50;
    const MAX_ZOOM = 200;
    const ZOOM_STEP = 10;

    // Remove old zoom controls if exists
    const oldZoom = document.getElementById('diff-zoom-controls');
    if (oldZoom) oldZoom.remove();

    // Create zoom controls
    const zoomControls = document.createElement('div');
    zoomControls.id = 'diff-zoom-controls';
    zoomControls.className = 'diff-zoom-controls';
    zoomControls.innerHTML = `
        <button id="zoom-out-btn" class="diff-zoom-btn" title="Zoom out (Ctrl -)">-</button>
        <span id="zoom-level" class="diff-zoom-level">100%</span>
        <button id="zoom-in-btn" class="diff-zoom-btn" title="Zoom in (Ctrl +)">+</button>
        <button id="zoom-reset-btn" class="diff-zoom-btn" title="Reset zoom (Ctrl 0)">⟲</button>
    `;

    // Insert zoom controls before diff container
    diffContainer.parentElement.insertBefore(zoomControls, diffContainer);

    // Apply zoom - find the diff-viewer inside
    function applyZoom(zoom) {
        currentZoom = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, zoom));

        // Apply to diff-viewer or all diff lines
        const viewer = diffContainer.querySelector('.diff-viewer');
        const target = viewer || diffContainer;

        // Calculate actual font size (base is typically 14px)
        const baseFontSize = 14;
        const newFontSize = (baseFontSize * currentZoom) / 100;
        target.style.fontSize = `${newFontSize}px`;

        const zoomLevel = document.getElementById('zoom-level');
        if (zoomLevel) {
            zoomLevel.textContent = `${currentZoom}%`;
        }
    }

    // Button handlers
    document.getElementById('zoom-in-btn')?.addEventListener('click', () => {
        applyZoom(currentZoom + ZOOM_STEP);
    });

    document.getElementById('zoom-out-btn')?.addEventListener('click', () => {
        applyZoom(currentZoom - ZOOM_STEP);
    });

    document.getElementById('zoom-reset-btn')?.addEventListener('click', () => {
        applyZoom(100);
    });

    // Keyboard zoom
    const zoomHandler = (e) => {
        if (e.ctrlKey || e.metaKey) {
            if (e.key === '+' || e.key === '=') {
                e.preventDefault();
                applyZoom(currentZoom + ZOOM_STEP);
            } else if (e.key === '-' || e.key === '_') {
                e.preventDefault();
                applyZoom(currentZoom - ZOOM_STEP);
            } else if (e.key === '0') {
                e.preventDefault();
                applyZoom(100);
            }
        }
    };

    document.addEventListener('keydown', zoomHandler);

    // Store cleanup
    const oldCleanup = window.cleanupDiffZoom;
    window.cleanupDiffZoom = () => {
        document.removeEventListener('keydown', zoomHandler);
        if (oldCleanup) oldCleanup();
    };
}

/**
 * Render diff minimap (like VS Code)
 */
function renderDiffMinimap(diffContainer) {
    // Remove old minimap if exists
    const oldMinimap = document.getElementById('diff-minimap');
    if (oldMinimap) oldMinimap.remove();

    // Create minimap wrapper
    const minimapWrapper = document.createElement('div');
    minimapWrapper.id = 'diff-minimap';
    minimapWrapper.className = 'diff-minimap';

    // Create line numbers column
    const lineNumbers = document.createElement('div');
    lineNumbers.className = 'minimap-line-numbers';

    // Create colored blocks column
    const blocks = document.createElement('div');
    blocks.className = 'minimap-blocks';

    // Get all diff lines
    const allLines = diffContainer.querySelectorAll('.diff-line');
    const totalLines = allLines.length;

    if (totalLines === 0) return;

    // Calculate which line numbers to show (show ~10 evenly spaced)
    const numberCount = Math.min(10, totalLines);
    const numberStep = Math.floor(totalLines / numberCount);

    // Build minimap
    let blocksHtml = '';
    let numbersHtml = '';

    allLines.forEach((line, index) => {
        // Determine color class
        let colorClass = 'minimap-line-unchanged';
        if (line.classList.contains('diff-line-add')) {
            colorClass = 'minimap-line-add';
        } else if (line.classList.contains('diff-line-remove')) {
            colorClass = 'minimap-line-remove';
        }

        const height = 100 / totalLines;
        blocksHtml += `<div class="minimap-block ${colorClass}" style="height: ${height}%" data-line-index="${index}"></div>`;

        // Add line number markers at intervals
        if (index % numberStep === 0 || index === totalLines - 1) {
            const lineNum = index + 1;
            const topPosition = (index / totalLines) * 100;
            numbersHtml += `<div class="minimap-line-number" style="top: ${topPosition}%">${lineNum}</div>`;
        }
    });

    blocks.innerHTML = blocksHtml;
    lineNumbers.innerHTML = numbersHtml;

    minimapWrapper.appendChild(lineNumbers);
    minimapWrapper.appendChild(blocks);

    // Add viewport indicator
    const viewport = document.createElement('div');
    viewport.className = 'minimap-viewport';
    blocks.appendChild(viewport);

    // Insert minimap - make sure it's at the end of the parent
    diffContainer.parentElement.style.position = 'relative';
    diffContainer.parentElement.appendChild(minimapWrapper);

    // Update viewport position on scroll
    function updateViewport() {
        const containerHeight = diffContainer.clientHeight;
        const scrollTop = diffContainer.scrollTop;
        const scrollHeight = diffContainer.scrollHeight;

        if (scrollHeight === 0) return;

        const viewportHeight = (containerHeight / scrollHeight) * 100;
        const viewportTop = (scrollTop / scrollHeight) * 100;

        viewport.style.height = `${Math.max(2, viewportHeight)}%`;
        viewport.style.top = `${viewportTop}%`;
    }

    diffContainer.addEventListener('scroll', updateViewport);
    updateViewport();

    // Click on minimap to scroll
    blocks.addEventListener('click', (e) => {
        const rect = blocks.getBoundingClientRect();
        const clickY = e.clientY - rect.top;
        const percentage = clickY / rect.height;

        diffContainer.scrollTop = percentage * diffContainer.scrollHeight;
    });

    // Store cleanup
    const oldCleanup = window.cleanupDiffMinimap;
    window.cleanupDiffMinimap = () => {
        diffContainer.removeEventListener('scroll', updateViewport);
        if (oldCleanup) oldCleanup();
    };
}

/**
 * Render code block with line numbers (legacy fallback)
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
        <button class="toast-close">×</button>
    `;

    toastContainer.appendChild(toast);

    // Add event listener for close button (CSP-compliant)
    const closeBtn = toast.querySelector('.toast-close');
    closeBtn.addEventListener('click', () => closeToast(toastId));

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
        }

        if (message) {
            showToast(message, status === 'error' ? 'error' : 'info');
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

    // Listen for undo/redo status changes from VS Code
    listen('apply-system-status-change', (event) => {
        console.log("[Apply System UI] Status change event:", event.payload);

        const { changeId, status } = event.payload;

        // Find change by changeId
        const change = applySystemState.changes.find(c => c.id === changeId);

        if (change) {
            if (status === 'undone') {
                // Change was undone - mark as pending
                change.status = 'pending';
                renderChangesList();
                showToast('Change undone successfully', 'success');
                console.log(`[Apply System UI] Change ${changeId} marked as undone`);
            } else if (status === 'success') {
                // Change was redone - mark as applied
                change.status = 'applied';
                renderChangesList();
                showToast('Change redone successfully', 'success');
                console.log(`[Apply System UI] Change ${changeId} marked as redone`);
            }
        } else {
            console.warn(`[Apply System UI] Change ${changeId} not found in state`);
        }
    });

    console.log("[Apply System UI] Event listeners setup complete");
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
