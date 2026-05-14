// Backup System Logic with Project Hierarchy

let backupState = {
    allBackups: [],
    projects: new Map(), // projectPath -> { name, backups[] }
    selectedProject: null,
    selectedBackup: null,
    currentFiles: [],
    selectedFilePaths: new Set(),
    viewMode: 'projects' // 'projects' or 'backups'
};

// Initialize
window.addEventListener('DOMContentLoaded', () => {
    const refreshBtn = document.getElementById('refresh-backups-btn');
    const restoreBtn = document.getElementById('restore-selected-btn');
    const backBtn = document.getElementById('back-to-projects-btn');

    if(refreshBtn) refreshBtn.addEventListener('click', loadBackups);
    if(restoreBtn) restoreBtn.addEventListener('click', handleRestore);
    if(backBtn) backBtn.addEventListener('click', showProjectsList);

    // Load backups when tab is clicked
    document.querySelector('.tab-link[data-tab="backups"]')?.addEventListener('click', () => {
        loadBackups();
    });
});

async function loadBackups() {
    const listContainer = document.getElementById('backup-list-container');
    listContainer.innerHTML = '<p class="placeholder">Scanning for context files...</p>';

    try {
        const backups = await window.invoke('get_available_backups');
        backupState.allBackups = backups;

        // Group by project
        backupState.projects.clear();
        backups.forEach(backup => {
            if (!backupState.projects.has(backup.projectPath)) {
                backupState.projects.set(backup.projectPath, {
                    name: backup.projectName,
                    path: backup.projectPath,
                    backups: []
                });
            }
            backupState.projects.get(backup.projectPath).backups.push(backup);
        });

        showProjectsList();
    } catch (err) {
        listContainer.innerHTML = `<p class="error-message">Failed to load backups: ${err}</p>`;
        showToast('Error loading backups', 'error');
    }
}

function showProjectsList() {
    backupState.viewMode = 'projects';
    backupState.selectedProject = null;
    backupState.selectedBackup = null;

    const listContainer = document.getElementById('backup-list-container');
    const detailTitle = document.getElementById('backup-detail-title');
    const filesContainer = document.getElementById('backup-files-container');
    const backBtn = document.getElementById('back-to-projects-btn');

    detailTitle.textContent = 'Select a Project';
    filesContainer.innerHTML = '<p class="placeholder">Select a project on the left to view snapshots.</p>';

    if (backBtn) backBtn.style.display = 'none';

    listContainer.innerHTML = '';

    if (backupState.projects.size === 0) {
        listContainer.innerHTML = `
            <div class="empty-state">
                <div class="empty-state-icon">📁</div>
                <div class="empty-state-text">No projects with snapshots</div>
                <div class="empty-state-hint">Generate context files to create restore points.</div>
            </div>`;
        return;
    }

    const projectsArray = Array.from(backupState.projects.values());
    projectsArray.forEach(project => {
        const item = document.createElement('div');
        item.className = 'backup-project-item neumorphic-raised-sm interactive-lift';

        const totalSize = project.backups.reduce((sum, b) => sum + b.sizeBytes, 0);

        item.innerHTML = `
            <div class="project-item-header">
                <span class="project-icon">📁</span>
                <div class="project-info">
                    <div class="project-name">${project.name}</div>
                    <div class="project-stats">
                        ${project.backups.length} snapshot${project.backups.length !== 1 ? 's' : ''} •
                        ${(totalSize / 1024).toFixed(1)} KB total
                    </div>
                </div>
                <span class="chevron">›</span>
            </div>
        `;

        item.addEventListener('click', () => {
            showBackupsForProject(project);
        });

        listContainer.appendChild(item);
    });
}

function showBackupsForProject(project) {
    backupState.viewMode = 'backups';
    backupState.selectedProject = project;
    backupState.selectedBackup = null;

    const listContainer = document.getElementById('backup-list-container');
    const detailTitle = document.getElementById('backup-detail-title');
    const filesContainer = document.getElementById('backup-files-container');
    const backBtn = document.getElementById('back-to-projects-btn');

    if (backBtn) backBtn.style.display = 'flex';

    detailTitle.innerHTML = `<span class="project-breadcrumb">${project.name}</span>`;
    filesContainer.innerHTML = '<p class="placeholder">Select a snapshot to view files.</p>';

    listContainer.innerHTML = '';

    // Add header
    const header = document.createElement('div');
    header.className = 'backups-list-header';
    header.innerHTML = `
        <div style="padding: 12px 16px; border-bottom: 2px solid var(--border-color); background: var(--bg-elevated);">
            <div style="font-weight: 600; font-size: 14px; color: var(--text-primary);">
                ${project.name}
            </div>
            <div style="font-size: 11px; color: var(--text-muted); margin-top: 4px;">
                ${project.backups.length} available snapshot${project.backups.length !== 1 ? 's' : ''}
            </div>
        </div>
    `;
    listContainer.appendChild(header);

    project.backups.forEach(backup => {
        const item = document.createElement('div');
        item.className = 'backup-item neumorphic-raised-sm interactive-lift';

        const dateStr = backup.createdAt.replace(/_/g, ' ');
        const date = new Date(dateStr.substring(0, 4), dateStr.substring(4, 6) - 1, dateStr.substring(6, 8),
                              dateStr.substring(9, 11), dateStr.substring(11, 13), dateStr.substring(13, 15));
        const timeAgo = getTimeAgo(date);

        item.innerHTML = `
            <div class="backup-item-content">
                <span class="backup-icon">📦</span>
                <div class="backup-info">
                    <div class="backup-date">${date.toLocaleString()}</div>
                    <div class="backup-meta">
                        ${timeAgo} • ${(backup.sizeBytes / 1024).toFixed(1)} KB
                    </div>
                </div>
            </div>
        `;

        item.addEventListener('click', () => {
            document.querySelectorAll('.backup-item').forEach(el => {
                el.classList.remove('selected');
            });
            item.classList.add('selected');
            selectBackup(backup, project);
        });

        listContainer.appendChild(item);
    });
}

function getTimeAgo(date) {
    const now = new Date();
    const seconds = Math.floor((now - date) / 1000);

    if (seconds < 60) return 'just now';
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    if (seconds < 604800) return `${Math.floor(seconds / 86400)}d ago`;
    return `${Math.floor(seconds / 604800)}w ago`;
}

async function selectBackup(backup, project) {
    backupState.selectedBackup = backup;
    document.getElementById('backup-detail-title').innerHTML = `
        <span class="project-breadcrumb">${project.name}</span>
        <span style="color: var(--text-muted); margin: 0 8px;">›</span>
        <span style="color: var(--accent-blue);">Snapshot</span>
    `;

    const filesContainer = document.getElementById('backup-files-container');
    filesContainer.innerHTML = '<p class="placeholder">Parsing snapshot content...</p>';

    try {
        const files = await window.invoke('preview_backup_content', { filepath: backup.filepath });
        backupState.currentFiles = files;
        backupState.selectedFilePaths.clear();
        renderBackupFiles(files);
    } catch (err) {
        filesContainer.innerHTML = `<p class="error-message">Failed to parse backup: ${err}</p>`;
    }
}

function renderBackupFiles(files) {
    const container = document.getElementById('backup-files-container');
    container.innerHTML = '';

    const list = document.createElement('div');
    list.className = 'backup-files-list';

    const header = document.createElement('div');
    header.className = 'files-header';
    header.innerHTML = `
        <label class="select-all-label">
            <input type="checkbox" id="backup-select-all">
            <span>Select All (${files.length} files)</span>
        </label>
    `;
    container.appendChild(header);

    files.forEach(file => {
        const row = document.createElement('div');
        row.className = 'file-item neumorphic-inset-sm';

        let statusBadge = '';
        let statusColor = '';

        switch(file.status) {
            case 'Modified': statusBadge = 'MODIFIED'; statusColor = '#ff9800'; break;
            case 'New': statusBadge = 'NEW'; statusColor = '#2196f3'; break;
            case 'Unchanged': statusBadge = 'UNCHANGED'; statusColor = '#4caf50'; break;
        }

        row.innerHTML = `
            <div class="file-item-content">
                <div class="file-main">
                    <input type="checkbox" class="backup-file-check" data-path="${file.path}" ${file.status === 'Unchanged' ? '' : 'checked'}>
                    <div class="file-details">
                        <div class="file-name">${file.path.split('/').pop()}</div>
                        <div class="file-path">${file.path}</div>
                    </div>
                </div>
                <div class="file-actions">
                    <span class="status-badge" style="color:${statusColor}; border-color:${statusColor};">${statusBadge}</span>
                    <button class="btn-icon-small diff-btn" title="View Diff">👁️</button>
                </div>
            </div>
        `;

        const cb = row.querySelector('.backup-file-check');
        if (cb.checked) backupState.selectedFilePaths.add(file.path);

        cb.addEventListener('change', (e) => {
            if(e.target.checked) backupState.selectedFilePaths.add(file.path);
            else backupState.selectedFilePaths.delete(file.path);
            updateRestoreButton();
        });

        row.querySelector('.diff-btn').addEventListener('click', () => {
            openDiffModal(file);
        });

        list.appendChild(row);
    });

    container.appendChild(list);

    document.getElementById('backup-select-all').addEventListener('change', (e) => {
        const checked = e.target.checked;
        document.querySelectorAll('.backup-file-check').forEach(cb => {
            cb.checked = checked;
            if(checked) backupState.selectedFilePaths.add(cb.dataset.path);
            else backupState.selectedFilePaths.delete(cb.dataset.path);
        });
        updateRestoreButton();
    });

    updateRestoreButton();
}

function updateRestoreButton() {
    const btn = document.getElementById('restore-selected-btn');
    const count = backupState.selectedFilePaths.size;
    btn.disabled = count === 0;
    btn.innerHTML = count > 0 ? `♻️ Restore (${count}) Files` : `♻️ Restore Selected`;
}

// Git-style diff algorithm that groups changes into blocks
function computeDiff(oldText, newText) {
    const oldLines = oldText.split('\n');
    const newLines = newText.split('\n');
    const diff = [];
    const LOOKAHEAD = 5; // How far to look for matching lines

    let i = 0, j = 0;

    while (i < oldLines.length || j < newLines.length) {
        // Phase 1: Collect all consecutive matching lines
        while (i < oldLines.length && j < newLines.length && oldLines[i] === newLines[j]) {
            diff.push({
                type: 'unchanged',
                oldLine: i + 1,
                newLine: j + 1,
                content: oldLines[i]
            });
            i++;
            j++;
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
            // We found an anchor - add all deleted lines first, then all added lines
            while (i < bestMatch.old) {
                diff.push({
                    type: 'remove',
                    oldLine: i + 1,
                    newLine: null,
                    content: oldLines[i]
                });
                i++;
            }

            while (j < bestMatch.new) {
                diff.push({
                    type: 'add',
                    oldLine: null,
                    newLine: j + 1,
                    content: newLines[j]
                });
                j++;
            }
        } else {
            // No match found - treat remaining lines as one big change block
            while (i < oldLines.length) {
                diff.push({
                    type: 'remove',
                    oldLine: i + 1,
                    newLine: null,
                    content: oldLines[i]
                });
                i++;
            }

            while (j < newLines.length) {
                diff.push({
                    type: 'add',
                    oldLine: null,
                    newLine: j + 1,
                    content: newLines[j]
                });
                j++;
            }
        }
    }

    return diff;
}

function renderDiffView(diff) {
    let html = '<div class="diff-viewer">';

    // Track change blocks for navigation
    window.diffChangeBlocksBackup = [];
    let currentChangeBlock = -1;
    let inChangeBlock = false;

    diff.forEach((line) => {
        const oldLineNum = line.oldLine !== null ? line.oldLine : '';
        const newLineNum = line.newLine !== null ? line.newLine : '';
        const content = line.content.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

        let className = 'diff-line';
        let prefix = ' ';
        let changeBlockAttr = '';

        if (line.type === 'add') {
            className += ' diff-line-add';
            prefix = '+';

            // Start or continue change block
            if (!inChangeBlock) {
                currentChangeBlock++;
                window.diffChangeBlocksBackup.push(currentChangeBlock);
                inChangeBlock = true;
            }
            changeBlockAttr = `data-change-block="${currentChangeBlock}"`;
        } else if (line.type === 'remove') {
            className += ' diff-line-remove';
            prefix = '-';

            // Start or continue change block
            if (!inChangeBlock) {
                currentChangeBlock++;
                window.diffChangeBlocksBackup.push(currentChangeBlock);
                inChangeBlock = true;
            }
            changeBlockAttr = `data-change-block="${currentChangeBlock}"`;
        } else {
            // Unchanged line ends change block
            if (inChangeBlock) {
                inChangeBlock = false;
            }
        }

        html += `
            <div class="${className}" ${changeBlockAttr}>
                <span class="diff-line-num diff-line-num-old">${oldLineNum}</span>
                <span class="diff-line-num diff-line-num-new">${newLineNum}</span>
                <span class="diff-line-prefix">${prefix}</span>
                <span class="diff-line-content">${content}</span>
            </div>
        `;
    });

    html += '</div>';
    return html;
}

function openDiffModal(fileItem) {
    // Create modal if it doesn't exist
    let modal = document.getElementById('diff-preview-modal');
    if (!modal) {
        modal = document.createElement('div');
        modal.id = 'diff-preview-modal';
        modal.className = 'modal-overlay';
        modal.style.display = 'none';
        modal.innerHTML = `
            <div class="modal-content modal-wide neumorphic-raised">
                <div class="modal-header">
                    <h2 class="text-gradient">File Comparison</h2>
                    <button class="btn-icon" id="modal-close-btn">×</button>
                </div>
                <div class="modal-body">
                    <div class="diff-info">
                        <div class="diff-file-header">
                            <span class="diff-file-icon">📄</span>
                            <div class="diff-file-details">
                                <div id="modal-file-name" class="diff-file-name"></div>
                                <div id="modal-file-path" class="diff-file-path"></div>
                            </div>
                            <div class="diff-stats-container">
                                <span id="diff-stats" class="diff-stats"></span>
                                <span id="modal-lines-range" class="status-badge"></span>
                            </div>
                        </div>
                    </div>
                    <div id="diff-content-container" class="diff-content-container"></div>
                </div>
            </div>
        `;
        document.body.appendChild(modal);
    }

    document.getElementById('modal-file-name').textContent = fileItem.path.split('/').pop();
    document.getElementById('modal-file-path').textContent = fileItem.path;

    // Generate diff
    const oldContent = fileItem.currentContent || '';
    const newContent = fileItem.backupContent || '';

    const diff = computeDiff(oldContent, newContent);

    // Calculate stats
    const additions = diff.filter(d => d.type === 'add').length;
    const deletions = diff.filter(d => d.type === 'remove').length;

    const statsEl = document.getElementById('diff-stats');
    if (additions > 0 || deletions > 0) {
        statsEl.innerHTML = `
            <span class="diff-stat-add">+${additions}</span>
            <span class="diff-stat-remove">-${deletions}</span>
        `;
    } else {
        statsEl.textContent = 'No changes';
    }

    const statusBadge = document.getElementById('modal-lines-range');
    let statusColor = '';
    switch(fileItem.status) {
        case 'Modified': statusColor = '#ff9800'; break;
        case 'New': statusColor = '#2196f3'; break;
        case 'Unchanged': statusColor = '#4caf50'; break;
    }
    statusBadge.textContent = fileItem.status;
    statusBadge.style.color = statusColor;
    statusBadge.style.borderColor = statusColor;

    const diffHtml = renderDiffView(diff);
    const diffContainer = document.getElementById('diff-content-container');
    diffContainer.innerHTML = diffHtml;

    // Setup diff enhancements
    setupDiffNavigationBackup(diffContainer);
    setupDiffZoomBackup(diffContainer);
    renderDiffMinimapBackup(diffContainer);

    modal.style.display = 'flex';

    const closeBtn = document.getElementById('modal-close-btn');
    const tempClose = () => {
        modal.style.display = 'none';
        closeBtn.removeEventListener('click', tempClose);

        // Cleanup
        if (window.cleanupDiffNavigationBackup) {
            window.cleanupDiffNavigationBackup();
            window.cleanupDiffNavigationBackup = null;
        }
        if (window.cleanupDiffZoomBackup) {
            window.cleanupDiffZoomBackup();
            window.cleanupDiffZoomBackup = null;
        }
        if (window.cleanupDiffMinimapBackup) {
            window.cleanupDiffMinimapBackup();
            window.cleanupDiffMinimapBackup = null;
        }
    };
    closeBtn.addEventListener('click', tempClose);

    modal.addEventListener('click', (e) => {
        if (e.target === modal) {
            tempClose();
        }
    });
}

async function handleRestore() {
    if (backupState.selectedFilePaths.size === 0) return;

    const confirmed = await window.__TAURI__.dialog.confirm(
        `Are you sure you want to overwrite ${backupState.selectedFilePaths.size} files from the snapshot?`,
        { title: 'Confirm Restore', kind: 'warning' }
    );

    if (!confirmed) return;

    const filesToRestore = backupState.currentFiles.filter(f => backupState.selectedFilePaths.has(f.path));

    try {
        const count = await window.invoke('restore_backup_files', { files: filesToRestore });
        showToast(`Successfully restored ${count} files.`, 'success');

        // Refresh the current view
        if (backupState.selectedBackup && backupState.selectedProject) {
            selectBackup(backupState.selectedBackup, backupState.selectedProject);
        }
    } catch (err) {
        showToast(`Restore failed: ${err}`, 'error');
    }
}

// ============================================================================
// Diff Navigation, Zoom, and Minimap for Backup UI
// ============================================================================

function setupDiffNavigationBackup(diffContainer) {
    const oldNav = document.getElementById('diff-navigation');
    if (oldNav) oldNav.remove();

    const nav = document.createElement('div');
    nav.id = 'diff-navigation';
    nav.className = 'diff-navigation';
    nav.innerHTML = `
        <button id="prev-change-btn" class="diff-nav-btn" title="Previous change (↑)">↑</button>
        <span id="change-counter" class="diff-nav-counter">0 / 0</span>
        <button id="next-change-btn" class="diff-nav-btn" title="Next change (↓)">↓</button>
    `;

    diffContainer.parentElement.insertBefore(nav, diffContainer);

    let currentChangeIndex = -1;
    const totalChanges = window.diffChangeBlocksBackup ? window.diffChangeBlocksBackup.length : 0;

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

    function navigateToChange(index) {
        if (totalChanges === 0) return;

        if (index < 0) index = totalChanges - 1;
        if (index >= totalChanges) index = 0;

        currentChangeIndex = index;

        const changeBlock = window.diffChangeBlocksBackup[index];
        const element = diffContainer.querySelector(`[data-change-block="${changeBlock}"]`);

        if (element) {
            diffContainer.querySelectorAll('.diff-line-highlighted').forEach(el => {
                el.classList.remove('diff-line-highlighted');
            });

            diffContainer.querySelectorAll(`[data-change-block="${changeBlock}"]`).forEach(el => {
                el.classList.add('diff-line-highlighted');
            });

            element.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }

        updateCounter();
    }

    document.getElementById('prev-change-btn')?.addEventListener('click', () => {
        navigateToChange(currentChangeIndex - 1);
    });

    document.getElementById('next-change-btn')?.addEventListener('click', () => {
        navigateToChange(currentChangeIndex + 1);
    });

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

    window.cleanupDiffNavigationBackup = () => {
        document.removeEventListener('keydown', keyHandler);
    };

    updateCounter();

    if (totalChanges > 0) {
        setTimeout(() => navigateToChange(0), 100);
    }
}

function setupDiffZoomBackup(diffContainer) {
    let currentZoom = 100;
    const MIN_ZOOM = 50;
    const MAX_ZOOM = 200;
    const ZOOM_STEP = 10;

    const oldZoom = document.getElementById('diff-zoom-controls');
    if (oldZoom) oldZoom.remove();

    const zoomControls = document.createElement('div');
    zoomControls.id = 'diff-zoom-controls';
    zoomControls.className = 'diff-zoom-controls';
    zoomControls.innerHTML = `
        <button id="zoom-out-btn" class="diff-zoom-btn" title="Zoom out (Ctrl -)">-</button>
        <span id="zoom-level" class="diff-zoom-level">100%</span>
        <button id="zoom-in-btn" class="diff-zoom-btn" title="Zoom in (Ctrl +)">+</button>
        <button id="zoom-reset-btn" class="diff-zoom-btn" title="Reset zoom (Ctrl 0)">⟲</button>
    `;

    diffContainer.parentElement.insertBefore(zoomControls, diffContainer);

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

    document.getElementById('zoom-in-btn')?.addEventListener('click', () => {
        applyZoom(currentZoom + ZOOM_STEP);
    });

    document.getElementById('zoom-out-btn')?.addEventListener('click', () => {
        applyZoom(currentZoom - ZOOM_STEP);
    });

    document.getElementById('zoom-reset-btn')?.addEventListener('click', () => {
        applyZoom(100);
    });

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

    window.cleanupDiffZoomBackup = () => {
        document.removeEventListener('keydown', zoomHandler);
    };
}

function renderDiffMinimapBackup(diffContainer) {
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

    window.cleanupDiffMinimapBackup = () => {
        diffContainer.removeEventListener('scroll', updateViewport);
    };
}
