import { sidebarLogger } from '../../common/logger.js';
import { showStatusMessage, allProjects } from './stateManagement.js';

// State
let ragState = {
    projects: [],
    selectedProject: null,
    statusData: [],
    selection: new Set(), // Przechowuje ścieżki wybranych plików
    isServiceRunning: false,
    selectedSearchResults: new Set() // Przechowuje indeksy zaznaczonych wyników wyszukiwania
};

// Exported for external updates
export function updateRagProjects(projects) {
    if (projects && Array.isArray(projects)) {
        ragState.projects = projects;
        populateRagProjects();
        sidebarLogger.log('[RAG] Projects list updated externally', projects.length);
    }
}

export function initRagView() {
    sidebarLogger.log('Initializing RAG View...');

    // UI Elements
    const toggleBtn = document.getElementById('ragToggleBtn');
    const indexBtn = document.getElementById('ragIndexChangesBtn');
    const rebuildBtn = document.getElementById('ragRebuildBtn');
    const refreshBtn = document.getElementById('ragRefreshBtn');
    const refreshProjectsBtn = document.getElementById('ragRefreshProjectsBtn');

    // Selection Buttons
    const selectAllBtn = document.getElementById('ragSelectAllBtn');
    const selectChangedBtn = document.getElementById('ragSelectChangedBtn');
    const selectNoneBtn = document.getElementById('ragSelectNoneBtn');

    // RAG Search Elements
    const ragSearchBtn = document.getElementById('ragSearchBtn');
    const ragSearchInput = document.getElementById('ragSearchInput');
    const ragTopK = document.getElementById('ragTopK');
    const ragTopKValue = document.getElementById('ragTopKValue');

    // 1. Try to load from global state first (Immediate render)
    if (allProjects && allProjects.length > 0) {
        ragState.projects = allProjects;
        populateRagProjects();
    }

    // 2. Fetch fresh data from backend
    fetchProjects();

    // Check service status
    chrome.runtime.sendMessage({ action: 'tauri_command', command: 'get_local_ai_status' }, (response) => {
        updateServiceStatus(response === true);
    });

    // Listeners
    if (toggleBtn) toggleBtn.addEventListener('click', toggleRagService);
    if (refreshProjectsBtn) refreshProjectsBtn.addEventListener('click', fetchProjects);

    if (refreshBtn) {
        refreshBtn.addEventListener('click', () => {
            if (ragState.selectedProject) loadProjectStatus(ragState.selectedProject);
        });
    }

    if (indexBtn) {
        indexBtn.addEventListener('click', () => triggerIndexing('selection'));
    }

    if (rebuildBtn) {
        rebuildBtn.addEventListener('click', () => {
            if (confirm('This will delete all embeddings for this project and re-index from scratch. Continue?')) {
                triggerIndexing('all');
            }
        });
    }

    // Selection handlers
    if (selectAllBtn) {
        selectAllBtn.addEventListener('click', () => {
            ragState.statusData.forEach(f => ragState.selection.add(f.path));
            renderStatusTree(ragState.statusData);
            updateSelectionStats();
        });
    }

    if (selectChangedBtn) {
        selectChangedBtn.addEventListener('click', () => {
            ragState.selection.clear();
            ragState.statusData.forEach(f => {
                if (f.status !== 'indexed') ragState.selection.add(f.path);
            });
            renderStatusTree(ragState.statusData);
            updateSelectionStats();
        });
    }

    if (selectNoneBtn) {
        selectNoneBtn.addEventListener('click', () => {
            ragState.selection.clear();
            renderStatusTree(ragState.statusData);
            updateSelectionStats();
        });
    }

    // RAG Search handlers
    if (ragSearchBtn && ragSearchInput) {
        ragSearchBtn.addEventListener('click', () => performRagSearch());

        // Ctrl+Enter or Cmd+Enter to search (allow Enter for new lines in textarea)
        ragSearchInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                performRagSearch();
            }
        });
    }

    // Update slider value display
    if (ragTopK && ragTopKValue) {
        ragTopK.addEventListener('input', (e) => {
            ragTopKValue.textContent = e.target.value;
        });
    }
}

function fetchProjects() {
    chrome.runtime.sendMessage({ action: 'tauri_command', command: 'get_projects' }, (response) => {
        // Handle response wrapped in {data: ...} format
        const data = response?.data || response;

        if (data && !data.error && Array.isArray(data)) {
            ragState.projects = data;
            populateRagProjects();
        } else {
            console.error("Failed to load projects for RAG:", data?.error || response?.error);
            // Fallback to global state if backend fetch fails
            if (allProjects.length > 0) {
                ragState.projects = allProjects;
                populateRagProjects();
            }
        }
    });
}

function renderEmptyState() {
    const tree = document.getElementById('ragFileTree');
    if (tree) {
        tree.innerHTML = '<div class="empty-state"><div class="empty-icon">📂</div><div class="empty-text">Select a project to view knowledge status</div></div>';
    }
    updateStats(0,0,0);
}

function populateRagProjects() {
    const container = document.getElementById('ragProjectTabs');
    if (!container) return;

    container.innerHTML = '';

    if (!ragState.projects || ragState.projects.length === 0) {
        if (allProjects && allProjects.length > 0) {
            ragState.projects = allProjects;
        }
    }

    if (ragState.projects.length === 0) {
        container.innerHTML = '<div style="padding:10px; color:#666;">No projects found</div>';
        return;
    }

    ragState.projects.forEach(p => {
        const path = p.path || p.projectPath;
        if (!path) return;

        const name = path.split(/[\\/]/).pop();

        const card = document.createElement('div');
        card.className = 'project-tab-card';
        if (path === ragState.selectedProject) {
            card.classList.add('active');
        }

        card.innerHTML = `
            <span class="project-tab-name">${name}</span>
            <div class="project-tab-indicator"></div>
        `;

        card.addEventListener('click', () => {
            // Update UI active state
            document.querySelectorAll('#ragProjectTabs .project-tab-card').forEach(c => c.classList.remove('active'));
            card.classList.add('active');

            ragState.selectedProject = path;
            ragState.selection.clear(); // Clear selection on project switch
            loadProjectStatus(path);
        });

        container.appendChild(card);
    });
}

function updateServiceStatus(isRunning) {
    ragState.isServiceRunning = isRunning;
    const indicator = document.getElementById('ragServiceStatus');
    const btn = document.getElementById('ragToggleBtn');
    const indexBtn = document.getElementById('ragIndexChangesBtn');
    const rebuildBtn = document.getElementById('ragRebuildBtn');

    if (indicator) {
        indicator.className = isRunning ? 'connection-indicator connected' : 'connection-indicator disconnected';
    }
    
    if (btn) {
        btn.textContent = isRunning ? 'Stop Service' : 'Start Service';
        if (isRunning) {
             btn.classList.remove('secondary');
             btn.classList.add('primary');
        } else {
             btn.classList.remove('primary');
             btn.classList.add('secondary');
        }
    }

    if (indexBtn) indexBtn.disabled = !ragState.selectedProject;
    if (rebuildBtn) rebuildBtn.disabled = !ragState.selectedProject;
    
    if (!isRunning) {
        if (indexBtn) indexBtn.disabled = true;
        if (rebuildBtn) rebuildBtn.disabled = true;
    }
}

async function toggleRagService() {
    const newState = !ragState.isServiceRunning;
    
    // Send toggle command via background
    chrome.runtime.sendMessage({
        action: 'toggle_local_ai',
        payload: { 
            enabled: newState,
            skip_auto_index: true 
        }
    });
    
    // Optimistic update
    updateServiceStatus(newState);
}

function loadProjectStatus(projectPath) {
    const container = document.getElementById('ragFileTree');
    if (container) {
        container.innerHTML = '<div class="empty-state"><div class="spinner"></div><div class="empty-text">Scanning files...</div></div>';
    }

    chrome.runtime.sendMessage({
        action: 'tauri_command',
        command: 'get_project_rag_status',
        args: { projectPath: projectPath }
    }, (response) => {
        // Handle response wrapped in {data: ...} format
        const data = response?.data || response;

        if (data && !data.error && Array.isArray(data)) {
            ragState.statusData = data;
            renderStatusTree(data);
        } else {
            if (container) {
                container.innerHTML = `<div class="empty-state is-error"><div class="empty-text">Error loading status: ${data?.error || response?.error || 'Unknown error'}</div></div>`;
            }
        }
    });
}

function renderStatusTree(files) {
    const container = document.getElementById('ragFileTree');
    if (!container) return;

    container.innerHTML = '';

    if (files.length === 0) {
        container.innerHTML = '<div class="empty-state"><div class="empty-text">No supported files found.</div></div>';
        updateStats(0,0,0);
        return;
    }

    // Debug: Check for duplicates
    const uniquePaths = new Set();
    const duplicates = [];
    files.forEach(file => {
        if (uniquePaths.has(file.path)) {
            duplicates.push(file.path);
        } else {
            uniquePaths.add(file.path);
        }
    });

    if (duplicates.length > 0) {
        sidebarLogger.warn(`[RAG] Found ${duplicates.length} duplicate file paths in response`);
        // Deduplicate files array by path
        const fileMap = new Map();
        files.forEach(file => {
            // Keep the first occurrence or prioritize by status (indexed > outdated > unindexed)
            if (!fileMap.has(file.path)) {
                fileMap.set(file.path, file);
            }
        });
        files = Array.from(fileMap.values());
        sidebarLogger.log(`[RAG] Deduplicated to ${files.length} unique files`);
    }

    // 1. Oblicz statystyki
    let indexed = 0, outdated = 0, unindexed = 0;
    files.forEach(file => {
        if (file.status === 'indexed') indexed++;
        else if (file.status === 'outdated') outdated++;
        else unindexed++;
    });

    sidebarLogger.log(`[RAG] Stats - Indexed: ${indexed}, Outdated: ${outdated}, Unindexed: ${unindexed}, Total: ${files.length}`);
    updateStats(indexed, outdated, unindexed);

    // 2. Zbuduj strukturę drzewa z płaskiej listy
    const treeRoot = buildHierarchy(files, ragState.selectedProject);

    // 3. Renderuj drzewo rekurencyjnie
    renderRecursiveTree(treeRoot, container, 0);

    updateSelectionStats();
}

/**
 * Konwertuje płaską listę plików na strukturę drzewiastą
 */
function buildHierarchy(files, rootPath) {
    const root = [];

    // Normalizacja ścieżki root (dla Windows/Unix)
    const normRoot = rootPath.replace(/\\/g, '/').replace(/\/$/, '');

    files.forEach(file => {
        const normPath = file.path.replace(/\\/g, '/');

        // Kluczowe: Odetnij rootPath od początku ścieżki pliku
        let relativePath = normPath;
        if (normPath.startsWith(normRoot)) {
            relativePath = normPath.substring(normRoot.length);
        }
        // Usuń wiodące ukośniki
        relativePath = relativePath.replace(/^\/+/, '');

        const parts = relativePath.split('/');
        let currentLevel = root;

        parts.forEach((part, index) => {
            // Sprawdź czy to plik (ostatni element)
            const isFile = index === parts.length - 1;

            // Szukaj istniejącego węzła na tym poziomie
            let existingNode = currentLevel.find(n => n.name === part && n.isFile === isFile);

            if (!existingNode) {
                const newNode = {
                    name: part,
                    isFile: isFile,
                    path: isFile ? file.path : null, // Pełna ścieżka tylko dla plików
                    children: [],
                    status: isFile ? file.status : null,
                    fullData: isFile ? file : null
                };
                currentLevel.push(newNode);
                existingNode = newNode;
            }

            if (!isFile) {
                currentLevel = existingNode.children;
            }
        });
    });

    return sortTree(root);
}

/**
 * Sortuje drzewo: foldery na górze, potem pliki alfabetycznie
 */
function sortTree(nodes) {
    nodes.sort((a, b) => {
        if (a.isFile === b.isFile) return a.name.localeCompare(b.name);
        return a.isFile ? 1 : -1;
    });
    nodes.forEach(node => {
        if (!node.isFile && node.children.length > 0) {
            sortTree(node.children);
        }
    });
    return nodes;
}

/**
 * Rekurencyjne renderowanie HTML
 */
function renderRecursiveTree(nodes, container, level) {
    nodes.forEach(node => {
        const nodeDiv = document.createElement('div');
        nodeDiv.className = `tree-node ${node.isFile ? 'file' : 'directory'}`;
        // Wcięcia identyczne jak w głównym sidebarze
        nodeDiv.style.paddingLeft = `${(level * 14) + 6}px`; 

        // Logika zwijania folderów
        if (!node.isFile && node.children.length > 0) {
            if (level > 2) nodeDiv.classList.add('collapsed');
        }

        // --- STYL 1:1 - Zaznaczenie (Klasa CSS, bez checkboxa) ---
        let isSelected = false;
        if (node.isFile) {
            isSelected = ragState.selection.has(node.path);
        } else {
            // Opcjonalne: Sprawdź czy folder jest "częściowo" zaznaczony (nie zaimplementowane w głównym drzewie, ale przydatne)
            // Tutaj trzymamy się 1:1 - brak wizualnego zaznaczenia folderu, chyba że kliknięty
        }

        if (isSelected) {
            nodeDiv.classList.add('selected');
        }

        // --- Icon & Name ---
        const iconSpan = document.createElement('span');
        iconSpan.className = 'tree-icon';

        let statusHtml = '';
        if (node.isFile) {
            // Status RAG (kropka)
            statusHtml = `<span class="status-dot ${node.status}" title="${node.status}" style="margin-right: 6px; flex-shrink: 0;"></span>`;
            iconSpan.textContent = getFileIcon(node.name);
        } else {
            iconSpan.textContent = '📁';
        }

        const nameSpan = document.createElement('span');
        nameSpan.className = 'tree-name';
        nameSpan.textContent = node.name;

        // Składanie wiersza (bez checkboxa)
        nodeDiv.innerHTML = `${statusHtml}`;

        // Toggle Icon dla folderów
        if (!node.isFile) {
            const toggleIcon = document.createElement('span');
            toggleIcon.className = 'tree-toggle';
            toggleIcon.textContent = nodeDiv.classList.contains('collapsed') ? '▶' : '▼';
            // Styl 1:1 ze standardowym drzewem
            toggleIcon.style.marginRight = '4px'; 
            toggleIcon.style.fontSize = '10px';
            toggleIcon.style.width = '12px';
            toggleIcon.style.display = 'inline-block';
            toggleIcon.style.textAlign = 'center';
            toggleIcon.style.color = 'var(--text-muted)';

            nodeDiv.appendChild(toggleIcon);
        } else {
            // Placeholder dla plików aby wyrównać z folderami (jeśli potrzebne)
            // W głównym drzewie pliki nie mają placeholdera toggle, więc pomijamy
        }

        nodeDiv.appendChild(iconSpan);
        nodeDiv.appendChild(nameSpan);

        container.appendChild(nodeDiv);

        // Kontener dzieci
        let childContainer = null;
        if (!node.isFile && node.children.length > 0) {
            childContainer = document.createElement('div');
            childContainer.className = 'tree-children';
            if (nodeDiv.classList.contains('collapsed')) {
                childContainer.style.display = 'none';
            }
            container.appendChild(childContainer);

            renderRecursiveTree(node.children, childContainer, level + 1);
        }

        // --- Interaction Logic ---
        nodeDiv.addEventListener('click', (e) => {
            e.stopPropagation();

            if (!node.isFile) {
                // FOLDER:
                // 1. Jeśli kliknięto w strzałkę (toggle) -> Zwiń/Rozwiń
                if (e.target.classList.contains('tree-toggle')) {
                    const isCollapsed = nodeDiv.classList.toggle('collapsed');
                    if (childContainer) childContainer.style.display = isCollapsed ? 'none' : 'block';
                    e.target.textContent = isCollapsed ? '▶' : '▼';
                    return;
                }

                // 2. Jeśli kliknięto w nazwę/wiersz -> Zaznacz wszystko wewnątrz
                // Sprawdzamy czy cokolwiek w środku jest niezaznaczone
                const allPaths = getAllFilePathsInNode(node);
                const allSelected = allPaths.every(p => ragState.selection.has(p));

                if (allSelected) {
                    // Odznacz wszystko
                    allPaths.forEach(p => ragState.selection.delete(p));
                } else {
                    // Zaznacz wszystko
                    allPaths.forEach(p => ragState.selection.add(p));
                }

                // Odśwież widok (aby zaktualizować klasy .selected w dzieciach)
                // Zachowaj pozycję scrolla
                const scrollPos = document.getElementById('ragFileTree').scrollTop;
                renderStatusTree(ragState.statusData);
                document.getElementById('ragFileTree').scrollTop = scrollPos;

            } else {
                // PLIK: Toggle selection
                if (ragState.selection.has(node.path)) {
                    ragState.selection.delete(node.path);
                    nodeDiv.classList.remove('selected');
                } else {
                    ragState.selection.add(node.path);
                    nodeDiv.classList.add('selected');
                }
                updateSelectionStats();
            }
        });
    });
}

// Pomocnicza do zbierania ścieżek z folderu
function getAllFilePathsInNode(node) {
    let paths = [];
    if (node.isFile) {
        paths.push(node.path);
    } else if (node.children) {
        node.children.forEach(child => {
            paths = paths.concat(getAllFilePathsInNode(child));
        });
    }
    return paths;
}

function toggleFolderSelection(node, isChecked) {
    if (node.isFile) {
        if (isChecked) ragState.selection.add(node.path);
        else ragState.selection.delete(node.path);
    } else if (node.children) {
        node.children.forEach(child => toggleFolderSelection(child, isChecked));
    }
}

function getFileIcon(filename) {
    const ext = filename.split('.').pop().toLowerCase();
    const icons = {
        'js': '📜', 'ts': '📘', 'rs': '🦀', 'html': '🌐', 'css': '🎨', 
        'json': '🔧', 'md': '📝', 'py': '🐍', 'sql': '🗄️'
    };
    return icons[ext] || '📄';
}

function updateStats(indexed, outdated, unindexed) {
    const elIndexed = document.getElementById('ragStatIndexed');
    const elOutdated = document.getElementById('ragStatOutdated');
    const elNew = document.getElementById('ragStatNew');

    if (elIndexed) elIndexed.textContent = indexed;
    if (elOutdated) elOutdated.textContent = outdated;
    if (elNew) elNew.textContent = unindexed;
}

function updateSelectionStats() {
    const indexBtn = document.getElementById('ragIndexChangesBtn');
    const count = ragState.selection.size;

    if (ragState.isServiceRunning && indexBtn) {
        if (count > 0) {
            indexBtn.disabled = false;
            indexBtn.textContent = `🚀 Index Selected (${count})`;
        } else {
            indexBtn.disabled = true;
            indexBtn.textContent = 'Select files to index';
        }
    }
}

function triggerIndexing(mode) {
    if (!ragState.selectedProject) return;

    let targetFiles = [];

    if (mode === 'selection') {
        const selectedPaths = Array.from(ragState.selection);
        if (selectedPaths.length === 0) return;

        targetFiles.push({
            rootPath: ragState.selectedProject,
            relativePaths: selectedPaths.map(f => f.replace(ragState.selectedProject + '/', ''))
        });
    } else if (mode === 'all') {
        // All files in project (Rebuild)
        targetFiles.push({
            rootPath: ragState.selectedProject,
            relativePaths: ragState.statusData.map(f => f.path.replace(ragState.selectedProject + '/', ''))
        });
    }

    showStatusMessage('Indexing started...', 'info');

    chrome.runtime.sendMessage({
        action: 'trigger_indexing',
        payload: { selectedFiles: targetFiles }
    }, (response) => {
        if(response && response.error) {
            showStatusMessage(`Indexing failed: ${response.error}`, 'error');
        }
    });
}

/**
 * Perform manual RAG search with user query
 */
function performRagSearch() {
    const input = document.getElementById('ragSearchInput');
    const topKInput = document.getElementById('ragTopK');
    const resultsContainer = document.getElementById('ragSearchResults');

    const query = input?.value?.trim();
    const topK = parseInt(topKInput?.value || '5');

    if (!query) {
        showStatusMessage('Please enter a search query', 'error');
        return;
    }

    if (!ragState.isServiceRunning) {
        showStatusMessage('RAG service is not running. Please start it first.', 'error');
        return;
    }

    // Clear previous selection on new search
    ragState.selectedSearchResults.clear();

    // Show loading state
    if (resultsContainer) {
        resultsContainer.style.display = 'block';
        resultsContainer.innerHTML = `
            <div class="rag-results-empty">
                <div class="spinner" style="margin: 0 auto 8px;"></div>
                <div>Searching knowledge base...</div>
            </div>
        `;
    }

    sidebarLogger.log(`[RAG Search] Query: "${query}", Top K: ${topK}, Project: ${ragState.selectedProject || 'none'}`);

    // Send search request to backend with current project context
    chrome.runtime.sendMessage({
        action: 'tauri_command',
        command: 'rag_search_manual',
        args: {
            query: query,
            top_k: topK,
            project_path: ragState.selectedProject || null
        }
    }, (response) => {
        // Handle response wrapped in {data: ...} format
        const data = response?.data || response;

        if (data && !data.error && Array.isArray(data)) {
            renderRagSearchResults(data, query);
            showStatusMessage(`Found ${data.length} results`, 'success');
        } else {
            const errorMsg = data?.error || response?.error || 'Unknown error';
            if (resultsContainer) {
                resultsContainer.innerHTML = `
                    <div class="rag-results-empty" style="color: var(--status-error);">
                        ❌ Search failed: ${errorMsg}
                    </div>
                `;
            }
            showStatusMessage(`Search failed: ${errorMsg}`, 'error');
        }
    });
}

/**
 * Render RAG search results
 */
function renderRagSearchResults(results, query) {
    const container = document.getElementById('ragSearchResults');
    if (!container) return;

    container.style.display = 'block';

    if (results.length === 0) {
        container.innerHTML = `
            <div class="rag-results-empty">
                <div style="font-size: 24px; margin-bottom: 8px;">🔍</div>
                <div>No results found for "${escapeHtml(query)}"</div>
            </div>
        `;
        return;
    }

    // Header with action buttons
    let html = `
        <div class="rag-results-header">
            <span class="rag-results-title">Results: "${escapeHtml(query)}"</span>
            <div style="display: flex; gap: 6px; align-items: center;">
                <span class="rag-results-count">${results.length}</span>
                <button id="ragSelectAllResultsBtn" class="mini-btn" style="padding: 3px 8px; font-size: 10px; background: var(--bg-primary); border: 1px solid var(--border-color); color: var(--text-primary); border-radius: 4px; cursor: pointer;">All</button>
                <button id="ragSelectNoneResultsBtn" class="mini-btn" style="padding: 3px 8px; font-size: 10px; background: var(--bg-primary); border: 1px solid var(--border-color); color: var(--text-primary); border-radius: 4px; cursor: pointer;">None</button>
                <button id="ragExportSelectedBtn" class="action-button primary" style="padding: 5px 10px; font-size: 11px; background: var(--accent-blue); color: var(--text-on-accent-dark); border: none; border-radius: 4px; font-weight: 600; cursor: pointer;" disabled>
                    📄 Export <span id="ragSelectedResultsCount">0</span>
                </button>
            </div>
        </div>
    `;

    // Results
    results.forEach((result, index) => {
        const score = (result.score * 100).toFixed(1);
        const fileName = result.file_path.split(/[\\/]/).pop();
        const content = result.content || result.text || '';
        const isSelected = ragState.selectedSearchResults.has(index);

        html += `
            <div class="rag-result-item ${isSelected ? 'selected' : ''}" data-result-index="${index}">
                <div class="rag-result-header" data-toggle-index="${index}">
                    <div style="display: flex; align-items: center; gap: 8px; flex: 1; min-width: 0;">
                        <span class="rag-toggle-icon">▶</span>
                        <label class="result-checkbox-container" onclick="event.stopPropagation();">
                            <input type="checkbox" class="result-checkbox" data-index="${index}" ${isSelected ? 'checked' : ''}>
                        </label>
                        <div class="rag-result-file">
                            <span>${getFileIcon(fileName)}</span>
                            <span title="${escapeHtml(fileName)}">${escapeHtml(fileName)}</span>
                        </div>
                    </div>
                    <div class="rag-result-score" title="Similarity Score">${score}%</div>
                </div>
                <div class="rag-result-content" style="display: none;">${escapeHtml(content)}</div>
                <div class="rag-result-meta" style="display: none;">
                    <span title="${escapeHtml(result.file_path)}">📁 ${escapeHtml(result.file_path)}</span>
                </div>
            </div>
        `;
    });

    container.innerHTML = html;

    // Attach event listeners to checkboxes
    attachSearchResultListeners(results);
    updateExportButtonState();
}

/**
 * Escape HTML to prevent XSS
 */
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

/**
 * Attach event listeners to search result checkboxes and action buttons
 */
function attachSearchResultListeners(results) {
    // Individual checkboxes
    const checkboxes = document.querySelectorAll('.result-checkbox');
    checkboxes.forEach(checkbox => {
        checkbox.addEventListener('change', (e) => {
            e.stopPropagation();
            const index = parseInt(e.target.dataset.index);
            const resultItem = e.target.closest('.rag-result-item');

            if (e.target.checked) {
                ragState.selectedSearchResults.add(index);
                resultItem.classList.add('selected');
            } else {
                ragState.selectedSearchResults.delete(index);
                resultItem.classList.remove('selected');
            }

            updateExportButtonState();
        });
    });

    // Click on header to toggle expand/collapse
    const resultHeaders = document.querySelectorAll('.rag-result-header[data-toggle-index]');
    resultHeaders.forEach(header => {
        header.addEventListener('click', (e) => {
            // Ignore if clicking on checkbox
            if (e.target.classList.contains('result-checkbox') ||
                e.target.closest('.result-checkbox-container')) {
                return;
            }

            const item = header.closest('.rag-result-item');
            const content = item.querySelector('.rag-result-content');
            const meta = item.querySelector('.rag-result-meta');
            const toggleIcon = item.querySelector('.rag-toggle-icon');

            const isExpanded = content.style.display !== 'none';

            if (isExpanded) {
                content.style.display = 'none';
                meta.style.display = 'none';
                toggleIcon.style.transform = 'rotate(0deg)';
            } else {
                content.style.display = 'block';
                meta.style.display = 'flex';
                toggleIcon.style.transform = 'rotate(90deg)';
            }
        });
    });

    // Select All button
    const selectAllBtn = document.getElementById('ragSelectAllResultsBtn');
    if (selectAllBtn) {
        selectAllBtn.addEventListener('click', () => {
            ragState.selectedSearchResults.clear();
            results.forEach((_, index) => ragState.selectedSearchResults.add(index));

            // Update UI
            document.querySelectorAll('.rag-result-item').forEach(item => {
                item.classList.add('selected');
                item.querySelector('.result-checkbox').checked = true;
            });

            updateExportButtonState();
        });
    }

    // Select None button
    const selectNoneBtn = document.getElementById('ragSelectNoneResultsBtn');
    if (selectNoneBtn) {
        selectNoneBtn.addEventListener('click', () => {
            ragState.selectedSearchResults.clear();

            // Update UI
            document.querySelectorAll('.rag-result-item').forEach(item => {
                item.classList.remove('selected');
                item.querySelector('.result-checkbox').checked = false;
            });

            updateExportButtonState();
        });
    }

    // Export button
    const exportBtn = document.getElementById('ragExportSelectedBtn');
    if (exportBtn) {
        exportBtn.addEventListener('click', () => exportSelectedResults(results));
    }
}

/**
 * Update export button state based on selection
 */
function updateExportButtonState() {
    const exportBtn = document.getElementById('ragExportSelectedBtn');
    const countSpan = document.getElementById('ragSelectedResultsCount');
    const count = ragState.selectedSearchResults.size;

    if (exportBtn) {
        exportBtn.disabled = count === 0;
    }

    if (countSpan) {
        countSpan.textContent = count;
    }
}

/**
 * Export selected search results to .txt file
 */
function exportSelectedResults(results) {
    if (ragState.selectedSearchResults.size === 0) {
        showStatusMessage('No results selected for export', 'error');
        return;
    }

    // Get selected results in order
    const selectedIndices = Array.from(ragState.selectedSearchResults).sort((a, b) => a - b);
    const selectedResults = selectedIndices.map(index => results[index]);

    // Build text content
    let content = '';
    content += '='.repeat(80) + '\n';
    content += 'RAG SEARCH RESULTS EXPORT\n';
    content += '='.repeat(80) + '\n\n';
    content += `Query: ${document.getElementById('ragSearchInput')?.value || 'N/A'}\n`;
    content += `Project: ${ragState.selectedProject || 'All Projects'}\n`;
    content += `Date: ${new Date().toLocaleString()}\n`;
    content += `Total Results: ${results.length}\n`;
    content += `Exported Results: ${selectedResults.length}\n\n`;
    content += '='.repeat(80) + '\n\n';

    selectedResults.forEach((result, index) => {
        const fileName = result.file_path.split(/[\\/]/).pop();
        const score = (result.score * 100).toFixed(1);
        const contentText = result.content || result.text || '';

        content += `[${index + 1}/${selectedResults.length}] ${fileName}\n`;
        content += '-'.repeat(80) + '\n';
        content += `File Path: ${result.file_path}\n`;
        content += `Similarity Score: ${score}%\n`;
        content += `Content:\n\n`;
        content += contentText + '\n\n';
        content += '='.repeat(80) + '\n\n';
    });

    // Create filename with timestamp
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, -5);
    const filename = `rag-export-${timestamp}.txt`;

    // Trigger download
    const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);

    showStatusMessage(`Exported ${selectedResults.length} results to ${filename}`, 'success');
    sidebarLogger.log(`[RAG Export] Exported ${selectedResults.length} results to ${filename}`);
}