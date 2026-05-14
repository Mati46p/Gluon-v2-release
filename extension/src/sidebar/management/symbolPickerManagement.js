/**
 * Symbol Picker Management
 * Handles inline tree expansion for file symbols (classes, functions, methods)
 */

import { sidebarLogger } from '../../common/logger.js';

// ============================================================================
// State Management
// ============================================================================

// Map: filePath -> { symbols: [], expanded: bool, selectedSymbols: Set }
const fileSymbolCache = new Map();

// Map: "projectPath::filePath" -> Set<symbolName>
const selectedSymbolsPerFile = new Map();

// Map: "projectPath::filePath" -> true (tracks which files have expanded symbols)
const expandedFilesState = new Map();

// Debounce timer for restore operation
let restoreDebounceTimer = null;

// ============================================================================
// Public API
// ============================================================================

/**
 * Toggles file symbol expansion in tree (like folder expand/collapse)
 * @param {HTMLElement} fileElement - The file tree node element
 * @param {string} filePath - Relative file path
 * @param {string} projectPath - Project root path
 */
export async function toggleFileSymbols(fileElement, filePath, projectPath) {
    const fileKey = `${projectPath}::${filePath}`;
    const cached = fileSymbolCache.get(fileKey);

    // Check if already expanded by looking for symbol-children sibling
    let existingContainer = fileElement.nextElementSibling;
    while (existingContainer && !existingContainer.classList.contains('tree-node') && !existingContainer.classList.contains('symbol-children')) {
        existingContainer = existingContainer.nextElementSibling;
    }

    if (existingContainer && existingContainer.classList.contains('symbol-children')) {
        // Collapse - toggle display like folders do
        if (existingContainer.style.display === 'none') {
            existingContainer.style.display = '';
            fileElement.classList.add('symbols-expanded');
            expandedFilesState.set(fileKey, true); // Track expansion
        } else {
            existingContainer.style.display = 'none';
            fileElement.classList.remove('symbols-expanded');
            expandedFilesState.delete(fileKey); // Track collapse
        }
        return;
    }

    // Expand - fetch symbols if not cached
    let symbols = cached?.symbols;
    if (!symbols) {
        try {
            symbols = await fetchFileSymbols(filePath, projectPath);
            fileSymbolCache.set(fileKey, { symbols, expanded: true });
        } catch (error) {
            sidebarLogger.error('[SymbolPicker] Failed to load symbols:', error);
            return;
        }
    }

    if (symbols.length === 0) {
        sidebarLogger.log('[SymbolPicker] No symbols found in file');
        return;
    }

    // Create symbols container (like tree-children for folders)
    const symbolsContainer = document.createElement('div');
    symbolsContainer.className = 'symbol-children';

    // Render each symbol as a tree node
    symbols.forEach(symbol => {
        const symbolNode = createSymbolTreeNode(symbol, filePath, projectPath);
        symbolsContainer.appendChild(symbolNode);
    });

    // Insert after file element
    fileElement.parentElement.insertBefore(symbolsContainer, fileElement.nextSibling);
    fileElement.classList.add('symbols-expanded');
    expandedFilesState.set(fileKey, true); // Track expansion

    if (cached) cached.expanded = true;
}

/**
 * Restores expanded symbols after file tree re-render
 * Call this after renderMergedFileTree completes
 * Debounced to prevent multiple rapid calls
 */
export function restoreExpandedSymbols() {
    // Clear existing timer
    if (restoreDebounceTimer) {
        clearTimeout(restoreDebounceTimer);
    }

    // Debounce: wait 50ms before restoring to batch multiple re-renders
    restoreDebounceTimer = setTimeout(async () => {
        await performRestore();
        restoreDebounceTimer = null;
    }, 50);
}

/**
 * Internal: Performs the actual restore operation
 */
async function performRestore() {
    if (expandedFilesState.size === 0) {
        sidebarLogger.debug('[SymbolPicker] No expanded symbols to restore');
        return;
    }

    sidebarLogger.log('[SymbolPicker] 🔄 Restoring', expandedFilesState.size, 'expanded symbol states');

    // Get all file elements currently in DOM (including collapsed folders)
    const fileElements = document.querySelectorAll('.tree-node[data-node-type="file"]');

    // Get all projects currently loaded
    const projectsInDom = new Set();
    document.querySelectorAll('.project-section').forEach(section => {
        const projectHeader = section.querySelector('.project-header');
        if (projectHeader) {
            const projectPath = projectHeader.getAttribute('title');
            if (projectPath) {
                projectsInDom.add(projectPath);
            }
        }
    });

    // Clean up expanded state ONLY for files from projects that are no longer loaded
    // DO NOT remove files that are just hidden (e.g., in collapsed folders)
    const keysToRemove = [];
    for (const fileKey of expandedFilesState.keys()) {
        const [projectPath] = fileKey.split('::');
        if (!projectsInDom.has(projectPath)) {
            keysToRemove.push(fileKey);
        }
    }

    if (keysToRemove.length > 0) {
        sidebarLogger.log('[SymbolPicker] Cleaning up', keysToRemove.length, 'stale entries from removed projects');
        keysToRemove.forEach(key => {
            expandedFilesState.delete(key);
            fileSymbolCache.delete(key);
            selectedSymbolsPerFile.delete(key);
        });
    }

    // Restore expanded symbols for files that exist in DOM
    for (const [fileKey, isExpanded] of expandedFilesState.entries()) {
        if (!isExpanded) continue;

        const [projectPath, filePath] = fileKey.split('::');

        // Find file element in DOM
        let found = false;
        for (const fileElement of fileElements) {
            if (fileElement.dataset.path === filePath && fileElement.dataset.project === projectPath) {
                found = true;

                // Check if already expanded by looking for symbol-children sibling
                let nextEl = fileElement.nextElementSibling;
                let alreadyExpanded = false;
                while (nextEl && !nextEl.classList.contains('tree-node')) {
                    if (nextEl.classList.contains('symbol-children')) {
                        alreadyExpanded = true;
                        break;
                    }
                    nextEl = nextEl.nextElementSibling;
                }

                if (alreadyExpanded) {
                    sidebarLogger.debug('[SymbolPicker] Skipping already expanded:', filePath);

                    // Still restore selection state even if already expanded
                    const selectedSymbols = selectedSymbolsPerFile.get(fileKey);
                    if (selectedSymbols && selectedSymbols.size > 0) {
                        const symbolContainer = nextEl; // We found it in the while loop above
                        if (symbolContainer) {
                            const symbolNodes = symbolContainer.querySelectorAll('.symbol-node');
                            symbolNodes.forEach(node => {
                                const symbolName = node.dataset.symbolName;
                                if (selectedSymbols.has(symbolName)) {
                                    node.classList.add('selected');
                                }
                            });
                        }
                    }
                    break;
                }

                // Re-expand symbols (will use cache so it's fast)
                try {
                    await toggleFileSymbols(fileElement, filePath, projectPath);

                    // Restore selected state for symbols
                    const selectedSymbols = selectedSymbolsPerFile.get(fileKey);
                    if (selectedSymbols && selectedSymbols.size > 0) {
                        // Wait for symbols to be rendered
                        setTimeout(() => {
                            // Find symbol nodes under this file's symbol-children container
                            const symbolContainer = fileElement.nextElementSibling;
                            if (symbolContainer && symbolContainer.classList.contains('symbol-children')) {
                                const symbolNodes = symbolContainer.querySelectorAll('.symbol-node');
                                symbolNodes.forEach(node => {
                                    const symbolName = node.dataset.symbolName;
                                    if (selectedSymbols.has(symbolName)) {
                                        node.classList.add('selected');
                                    }
                                });
                            }
                        }, 10);
                    }
                } catch (error) {
                    sidebarLogger.error('[SymbolPicker] Failed to restore symbols for', filePath, error);
                }
                break;
            }
        }

        // File not currently visible in DOM (e.g., in collapsed folder)
        // Keep the state for when it becomes visible again
        if (!found) {
            sidebarLogger.debug('[SymbolPicker] File not visible, keeping state:', filePath);
        }
    }
}

// ============================================================================
// Backend Communication
// ============================================================================

/**
 * Fetches symbols from backend via background script
 */
async function fetchFileSymbols(filePath, projectPath) {
    return new Promise((resolve, reject) => {
        const requestId = `symbols_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;

        sidebarLogger.log('[SymbolPicker] Requesting symbols for:', filePath);

        // Setup response listener
        const responseHandler = (message) => {
            if (message.type === 'get_file_symbols_response' && message.request_id === requestId) {
                chrome.runtime.onMessage.removeListener(responseHandler);

                if (message.success) {
                    sidebarLogger.log('[SymbolPicker] Received', message.symbols.length, 'symbols');
                    resolve(message.symbols || []);
                } else {
                    reject(new Error(message.error || 'Failed to fetch symbols'));
                }
            }
        };

        chrome.runtime.onMessage.addListener(responseHandler);

        // Send request to background script
        chrome.runtime.sendMessage({
            action: 'get_file_symbols',
            request_id: requestId,
            payload: {
                file_path: filePath,
                project_root: projectPath
            }
        });

        // Timeout after 15 seconds
        setTimeout(() => {
            chrome.runtime.onMessage.removeListener(responseHandler);
            reject(new Error('Timeout fetching symbols (15s)'));
        }, 15000);
    });
}

// ============================================================================
// UI Rendering
// ============================================================================

/**
 * Creates a symbol as a tree node (like file node but indented)
 */
function createSymbolTreeNode(symbol, filePath, projectPath) {
    const node = document.createElement('div');
    node.className = 'tree-node symbol-node';
    node.dataset.symbolName = symbol.name;
    node.dataset.filePath = filePath;
    node.dataset.project = projectPath;

    const icon = getSymbolIcon(symbol.kind);
    const displayName = symbol.parent ? `${symbol.parent}.${symbol.name}` : symbol.name;

    node.innerHTML = `
        <span class="tree-icon">${icon}</span>
        <span class="tree-name">${escapeHTML(displayName)}</span>
        <span class="symbol-kind-badge">${symbol.kind}</span>
    `;

    // Single click - toggle selection for export
    node.addEventListener('click', (event) => {
        event.stopPropagation();
        toggleSymbolSelection(symbol, filePath, projectPath, node);
    });

    // Double click - immediately attach to context (like files)
    node.addEventListener('dblclick', async (event) => {
        event.stopPropagation();
        await attachSymbolToContext(symbol, filePath, projectPath, node);
    });

    return node;
}

/**
 * Returns emoji icon for symbol kind
 */
function getSymbolIcon(kind) {
    const icons = {
        'function': '⚡',
        'class': '🏛️',
        'method': '🔧',
        'interface': '📐',
        'struct': '🧱',
        'enum': '🔢',
        'const': '📌'
    };
    return icons[kind] || '📄';
}

// ============================================================================
// Context Integration
// ============================================================================

/**
 * Toggles symbol selection (for later export to .txt)
 * Does NOT attach to context immediately - just visual selection
 */
function toggleSymbolSelection(symbol, filePath, projectPath, nodeElement) {
    const fileKey = `${projectPath}::${filePath}`;
    if (!selectedSymbolsPerFile.has(fileKey)) {
        selectedSymbolsPerFile.set(fileKey, new Set());
    }

    const selectedSet = selectedSymbolsPerFile.get(fileKey);
    const isSelected = selectedSet.has(symbol.name);

    if (isSelected) {
        // Deselect
        selectedSet.delete(symbol.name);
        nodeElement.classList.remove('selected');
        sidebarLogger.log(`[SymbolPicker] Deselected ${symbol.name} (for export)`);
    } else {
        // Select
        selectedSet.add(symbol.name);
        nodeElement.classList.add('selected');
        sidebarLogger.log(`[SymbolPicker] Selected ${symbol.name} (for export)`);
    }

    // Update selection info (like files do)
    updateSymbolSelectionInfo();
}

/**
 * Immediately attaches symbol to context (double-click action)
 */
async function attachSymbolToContext(symbol, filePath, projectPath, nodeElement) {
    sidebarLogger.log(`[SymbolPicker] 📎 Attaching symbol ${symbol.name} to context immediately`);

    // Also select it visually
    const fileKey = `${projectPath}::${filePath}`;
    if (!selectedSymbolsPerFile.has(fileKey)) {
        selectedSymbolsPerFile.set(fileKey, new Set());
    }
    selectedSymbolsPerFile.get(fileKey).add(symbol.name);
    nodeElement.classList.add('selected');

    try {
        // Dynamically import context module
        const contextModule = await import('../../features/context/context-node.js');

        const operations = [{
            type: 'file_symbol',
            path: filePath,
            symbol: symbol.name
        }];

        const contextResponse = await contextModule.executeContextOperations(operations, projectPath);
        sidebarLogger.log('[SymbolPicker] Symbol attached to context:', contextResponse);

        // Show brief success indicator
        nodeElement.style.transition = 'background 0.3s ease';
        nodeElement.style.background = 'rgba(0, 255, 0, 0.1)';
        setTimeout(() => {
            nodeElement.style.background = '';
        }, 300);

    } catch (error) {
        sidebarLogger.error('[SymbolPicker] Failed to attach symbol to context:', error);
        alert(`Error attaching symbol: ${error.message}`);
    }
}

/**
 * DEPRECATED: Old function that immediately added to context on single click
 * Kept for compatibility but no longer used
 */
async function addSymbolToContext(symbol, filePath, projectPath, nodeElement) {
    sidebarLogger.log(`[SymbolPicker] Adding symbol ${symbol.name} to context`);

    // Toggle selection visual
    const fileKey = `${projectPath}::${filePath}`;
    if (!selectedSymbolsPerFile.has(fileKey)) {
        selectedSymbolsPerFile.set(fileKey, new Set());
    }

    const selectedSet = selectedSymbolsPerFile.get(fileKey);
    const isSelected = selectedSet.has(symbol.name);

    if (isSelected) {
        // Deselect
        selectedSet.delete(symbol.name);
        nodeElement.classList.remove('selected');
        sidebarLogger.log(`[SymbolPicker] Deselected ${symbol.name}`);
        return;
    }

    // Select and add to context
    selectedSet.add(symbol.name);
    nodeElement.classList.add('selected');

    try {
        // Dynamically import context module
        const contextModule = await import('../../features/context/context-node.js');

        const operations = [{
            type: 'file_symbol',
            path: filePath,
            symbol: symbol.name
        }];

        const contextResponse = await contextModule.executeContextOperations(operations, projectPath);
        sidebarLogger.log('[SymbolPicker] Symbol added to context:', contextResponse);

        // Show brief success indicator
        nodeElement.style.transition = 'background 0.3s ease';
        nodeElement.style.background = 'rgba(0, 255, 0, 0.1)';
        setTimeout(() => {
            nodeElement.style.background = '';
        }, 300);

    } catch (error) {
        sidebarLogger.error('[SymbolPicker] Failed to add symbol to context:', error);
        selectedSet.delete(symbol.name);
        nodeElement.classList.remove('selected');
        alert(`Error adding symbol: ${error.message}`);
    }
}


// ============================================================================
// Selection Management (for .txt export)
// ============================================================================

/**
 * Updates the selection info display
 */
function updateSymbolSelectionInfo() {
    // Count total selected symbols across all files
    let totalSymbols = 0;
    for (const selectedSet of selectedSymbolsPerFile.values()) {
        totalSymbols += selectedSet.size;
    }

    // You can add UI update here if needed
    sidebarLogger.debug(`[SymbolPicker] Total selected symbols: ${totalSymbols}`);
}

/**
 * Gets all selected symbols for export
 * Returns: Map<projectPath::filePath, Set<symbolName>>
 */
export function getSelectedSymbols() {
    return selectedSymbolsPerFile;
}

/**
 * Clears all symbol selections
 */
export function clearSymbolSelections() {
    selectedSymbolsPerFile.clear();
    // Remove selected class from all symbol nodes
    document.querySelectorAll('.symbol-node.selected').forEach(node => {
        node.classList.remove('selected');
    });
    sidebarLogger.log('[SymbolPicker] Cleared all symbol selections');
}

// ============================================================================
// Utilities
// ============================================================================

function escapeHTML(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}
