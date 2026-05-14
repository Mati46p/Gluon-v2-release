import { contextLogger } from '../../common/logger.js';
import { generatePrompt, GLUON_PROTOCOL_INSTRUCTIONS, CONTEXT_ARCHITECT_PROMPT } from '../utils/prompt-generator.js';

// ============================================================================
// Context Management Module
// Zarządza plikami kontekstowymi, historią i załącznikami
// ============================================================================

import {
  selectedProjects, allProjects, environments, gluonModeEnabled, previousContextHistory, contextTilesExpanded,
  selectedNodes, selectedEnvironmentId, enabledPromptIds, pendingConfigRestore, lastAction,
  VIRTUAL_FILES_PROJECT_PATH,
  setPreviousContextHistory, setPendingConfigRestore, setSelectedProjects, setSelectedEnvironmentId,
  setEnabledPromptIds, setLastAction,
  showStatusMessage, showError, showLoading, hideLoading, escapeHTML, getTimeAgo, formatSize, saveSelectedProjects,
  fileTreeData, showContextProgress
} from './stateManagement.js';

import {
  triggerFileTreeLoadForSelected, renderMergedFileTree, updateSelectionInfo,
  constructMultiProjectPayload, constructMultiProjectPayloadWithSymbols, getProjectMapping
} from './fileTreeManagement.js';

import {
  renderPrompts
} from './templatePromptManagement.js';

import {
  savePromptToHistory,
  PromptHistoryNavigator
} from './promptHistoryManagement.js';

// Przechowuje wirtualne pliki w pamięci (nazwa -> treść)
const virtualFiles = new Map();

export function attachVirtualFile(filename, content) {
    virtualFiles.set(filename, content);
    renderContextHistoryList(); 

    window.gluonVirtualFiles = virtualFiles;

    // Auto-select the newly attached file
    if (!selectedNodes.has(VIRTUAL_FILES_PROJECT_PATH)) {
      selectedNodes.set(VIRTUAL_FILES_PROJECT_PATH, new Set());
    }
    selectedNodes.get(VIRTUAL_FILES_PROJECT_PATH).add(filename);

    // Add visual tile representation
    addContextTile(filename, content);

    // [FIX] Odśwież drzewo plików, aby pokazać nową sekcję Attached Files
    renderMergedFileTree(fileTreeData);
}

/**
 * Add context tile for Google Drive file
 */
function addContextTile(filename, content) {
    const container = document.getElementById('contextTiles');
    if (!container) return;

    const tile = document.createElement('div');
    tile.className = 'context-tile google-drive';
    tile.dataset.filename = filename;

    const size = new Blob([content]).size;
    const sizeFormatted = formatSize(size);

    tile.innerHTML = `
        <div class="tile-header">
            <span class="tile-icon">📄</span>
            <span class="tile-name">${escapeHTML(filename)}</span>
            <button class="tile-remove-btn" title="Remove">×</button>
        </div>
        <div class="tile-meta">
            <span class="tile-source">Google Drive</span>
            <span class="tile-size">${sizeFormatted}</span>
        </div>
    `;

    const removeBtn = tile.querySelector('.tile-remove-btn');
    removeBtn.addEventListener('click', () => {
        virtualFiles.delete(filename);
        tile.remove();
        showStatusMessage(`Removed ${filename} from context`, 'info');
        // [FIX] Odśwież drzewo po usunięciu
        renderMergedFileTree(fileTreeData);
    });

    container.appendChild(tile);
}

// Listen for add-virtual-file event from Google Drive Picker
document.addEventListener('add-virtual-file', (event) => {
    const { name, content, driveFileId } = event.detail;
    contextLogger.log(`Adding virtual file from Google Drive: ${name}`);
    attachVirtualFile(name, content);
});

// ============================================================================
// Quick Task History Navigator
// ============================================================================

let quickTaskNavigator = null;


/**
 * Inicjalizuje navigator historii dla Quick Task
 */
export async function initQuickTaskHistory() {
  const quickTaskInput = document.getElementById('quickTaskInput');
  if (quickTaskInput) {
    quickTaskNavigator = new PromptHistoryNavigator(quickTaskInput, 'quick_task');
    await quickTaskNavigator.init();
  }
}

/**
 * Obsługuje nawigację strzałkami w Quick Task
 */
export function handleQuickTaskHistoryKeydown(event) {
  if (quickTaskNavigator) {
    quickTaskNavigator.handleKeyDown(event);
  }
}

// ============================================================================
// Context History
// ============================================================================

/**
 * Ładuje historię plików kontekstowych
 */
export function loadContextHistory() {
  chrome.runtime.sendMessage({
    action: 'get_context_files_history',
    payload: {
      selectedProjects: Array.from(selectedProjects)
    }
  });
}

/**
 * Renderuje listę historii kontekstu
 */
export function renderContextHistoryList(historyItems) {
  const container = document.getElementById('contextTiles');
  const showMoreBtn = document.getElementById('showMoreContextBtn');

  if (!historyItems || historyItems.length === 0) {
    container.innerHTML = '<div class="empty-state"><div class="empty-icon">📋</div><div class="empty-text">No saved contexts yet</div></div>';
    setPreviousContextHistory([]);
    showMoreBtn.style.display = 'none';
    return;
  }

  // [FIX] Usuń empty state jeśli istnieje, skoro mamy elementy
  const emptyState = container.querySelector('.empty-state');
  if (emptyState) {
    emptyState.remove();
  }

  if (areHistoryItemsEqual(previousContextHistory, historyItems)) {
    return;
  }

  const currentMap = new Map(historyItems.map(item => [item.filepath, item]));
  const previousMap = new Map(previousContextHistory.map(item => [item.filepath, item]));
  const existingTiles = new Map([...container.querySelectorAll('.context-tile')].map(tile => [tile.dataset.filepath, tile]));

  // Usuń nieistniejące kafelki
  for (const [filepath, tile] of existingTiles.entries()) {
    if (!currentMap.has(filepath)) {
      tile.remove();
      existingTiles.delete(filepath);
    }
  }

  // Dodaj, zaktualizuj i uporządkuj kafelki
  historyItems.forEach((item, index) => {
    const existingTile = existingTiles.get(item.filepath);
    const previousItem = previousMap.get(item.filepath);
    let currentTileElement = existingTile;

    if (existingTile) {
      if (!areItemsEqual(item, previousItem)) {
        const newTile = createContextTile(item);
        existingTile.replaceWith(newTile);
        currentTileElement = newTile;
        existingTiles.set(item.filepath, newTile);
      }
    } else {
      const newTile = createContextTile(item);
      currentTileElement = newTile;
      existingTiles.set(item.filepath, newTile);
    }

    const expectedNode = container.children[index];
    if (expectedNode !== currentTileElement) {
      container.insertBefore(currentTileElement, expectedNode || null);
    }
  });

  // Zarządzaj przyciskiem "Show More"
  const totalCount = historyItems.length;
  const favoritedCount = historyItems.filter(item => item.favorite).length;

  container.classList.toggle('has-favorites', favoritedCount > 0);

  const visibleInCollapsed = favoritedCount > 0 ? favoritedCount : (totalCount > 0 ? 1 : 0);

  if (totalCount > visibleInCollapsed) {
    showMoreBtn.style.display = 'flex';
    const hiddenCount = totalCount - visibleInCollapsed;
    const btnText = showMoreBtn.querySelector('.btn-text');

    if (contextTilesExpanded) {
      container.classList.remove('collapsed');
      btnText.innerHTML = 'Show less';
      showMoreBtn.classList.add('expanded');
    } else {
      container.classList.add('collapsed');
      btnText.innerHTML = `Show <span id="hiddenCount">${hiddenCount}</span> more`;
      showMoreBtn.classList.remove('expanded');
    }
  } else {
    showMoreBtn.style.display = 'none';
    container.classList.remove('collapsed');
  }

  setPreviousContextHistory(historyItems);
}

/**
 * Porównuje dwie listy historii kontekstu
 */
function areHistoryItemsEqual(prev, current) {
  if (prev.length !== current.length) return false;

  for (let i = 0; i < prev.length; i++) {
    if (!areItemsEqual(prev[i], current[i])) {
      return false;
    }
  }

  return true;
}

/**
 * Porównuje dwa pojedyncze elementy historii
 */
function areItemsEqual(a, b) {
  return a.filepath === b.filepath &&
    a.filename === b.filename &&
    a.timestamp === b.timestamp &&
    a.size === b.size &&
    a.favorite === b.favorite &&
    JSON.stringify(a.config) === JSON.stringify(b.config);
}

/**
 * Tworzy element DOM kafelka kontekstowego
 */
function createContextTile(item) {
  const tile = document.createElement('div');
  tile.className = 'context-tile';
  tile.dataset.filepath = item.filepath;
  if (item.favorite) {
    tile.classList.add('favorited');
  }

  const date = new Date(item.timestamp);
  const timeAgo = getTimeAgo(date);
  const dateStr = date.toLocaleDateString() + ' ' + date.toLocaleTimeString();

  const projectCount = item.config.projects.length;
  const fileCount = Object.values(item.config.selectedFiles).reduce((sum, files) => sum + files.length, 0);
  const promptCount = item.config.promptIds.length;
  const quickTask = item.config.quickTask || 'No task';

  const filenameParts = item.filename.match(/^(.+?)(\.[\w]+)$/);
  const namePrefix = filenameParts ? filenameParts[1] : item.filename;
  const extension = filenameParts && filenameParts[2] !== '.txt' ? filenameParts[2] : '';

  tile.innerHTML = `
    <div class="tile-restore-area">
      <button class="tile-favorite-btn ${item.favorite ? 'active' : ''}" title="${item.favorite ? 'Remove from favorites' : 'Add to favorites'}">
        ★
      </button>
      <div class="tile-icon">📄</div>
      <div class="tile-info">
        <div class="tile-name-container">
          <span class="tile-name-editable" title="${item.filepath}" data-extension="${extension}">${escapeHTML(namePrefix)}</span>
          <span class="tile-name-extension">${extension}</span>
        </div>
        <div class="tile-timestamp">${timeAgo} • ${dateStr}</div>
        <div class="tile-meta">${formatSize(item.size)} • ${fileCount} files • ${projectCount} projects</div>
        <div class="tile-prompts">${promptCount} prompts • ${escapeHTML(quickTask)}</div>
      </div>
      <button class="tile-attach-btn" title="Attach this file to AI">
        🔎 Attach
      </button>
    </div>
  `;

  if (item.warning) {
    const warningBadge = document.createElement('div');
    warningBadge.className = 'tile-warning';
    warningBadge.title = item.warning;
    warningBadge.textContent = '!';
    tile.appendChild(warningBadge);
  }

  const restoreArea = tile.querySelector('.tile-restore-area');
  restoreArea.addEventListener('click', (e) => {
    if (e.target.classList.contains('tile-attach-btn') ||
      e.target.classList.contains('tile-favorite-btn') ||
      e.target.classList.contains('tile-name-editable')) {
      return;
    }
    restoreConfig(item.config);
  });

  const favoriteBtn = tile.querySelector('.tile-favorite-btn');
  favoriteBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleFavorite(item.filepath, !item.favorite);
  });

  const attachBtn = tile.querySelector('.tile-attach-btn');
  attachBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    attachContextFile(item.filepath, item.filename);
  });

  const nameEditable = tile.querySelector('.tile-name-editable');
  nameEditable.addEventListener('click', (e) => {
    e.stopPropagation();
    startEditingTileName(nameEditable, item.filepath, item.filename);
  });

  return tile;
}

/**
 * Przełącza status ulubionego
 */
function toggleFavorite(filepath, isFavorite) {
  contextLogger.log('Toggling favorite for:', filepath, '→', isFavorite);

  const tile = document.querySelector(`.context-tile[data-filepath="${CSS.escape(filepath)}"]`);
  if (tile) {
    const favoriteBtn = tile.querySelector('.tile-favorite-btn');
    if (isFavorite) {
      tile.classList.add('favorited');
      favoriteBtn.classList.add('active');
      favoriteBtn.title = 'Remove from favorites';
    } else {
      tile.classList.remove('favorited');
      favoriteBtn.classList.remove('active');
      favoriteBtn.title = 'Add to favorites';
    }
  }

  chrome.runtime.sendMessage({
    action: 'toggle_context_favorite',
    payload: {
      filepath: filepath,
      favorite: isFavorite
    }
  });

  setTimeout(() => {
    loadContextHistory();
  }, 300);
}

/**
 * Rozpoczyna edycję nazwy kafelka
 */
function startEditingTileName(element, filepath, currentFilename) {
  const extension = element.dataset.extension || '';
  const currentPrefix = element.textContent.trim();

  const input = document.createElement('input');
  input.type = 'text';
  input.className = 'tile-name-input';
  input.value = currentPrefix;
  input.maxLength = 100;

  element.style.display = 'none';
  element.parentNode.insertBefore(input, element);

  input.focus();
  input.select();

  const saveChanges = () => {
    const newPrefix = input.value.trim();

    if (!newPrefix) {
      input.remove();
      element.style.display = '';
      showStatusMessage('Name cannot be empty', 'error');
      return;
    }

    if (newPrefix === currentPrefix) {
      input.remove();
      element.style.display = '';
      return;
    }

    const newFilename = newPrefix + '.txt';

    if (!/^[a-zA-Z0-9_\-\s]+$/.test(newPrefix)) {
      showStatusMessage('Name can only contain letters, numbers, spaces, - and _', 'error');
      return;
    }

    contextLogger.log('Renaming context file:', currentFilename, '→', newFilename);

    element.textContent = newPrefix;
    input.remove();
    element.style.display = '';

    chrome.runtime.sendMessage({
      action: 'rename_context_file',
      payload: {
        filepath: filepath,
        newName: newFilename
      }
    });

    setTimeout(() => {
      loadContextHistory();

      setTimeout(() => {
        const updatedItem = previousContextHistory.find(item => item.filename === newFilename);
        if (updatedItem) {
          showStatusMessage('✅ File renamed!', 'success');
        } else {
          element.textContent = currentPrefix;
          showStatusMessage('❌ Failed to rename file', 'error');
        }
      }, 400);
    }, 300);
  };

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      saveChanges();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      input.remove();
      element.style.display = '';
    }
  });

  input.addEventListener('blur', () => {
    setTimeout(() => {
      if (document.contains(input)) {
        saveChanges();
      }
    }, 100);
  });
}

/**
 * Przywraca konfigurację z zapisanego kontekstu
 */
async function restoreConfig(savedConfig) {
  contextLogger.log('Restoring config:', savedConfig);

  try {
    setSelectedProjects(new Set(savedConfig.projects));
    await saveSelectedProjects();

    document.querySelectorAll('.project-tab-card').forEach(card => {
      const path = card.dataset.path;
      if (selectedProjects.has(path)) {
        card.classList.add('active');
      } else {
        card.classList.remove('active');
      }
    });

    setPendingConfigRestore(savedConfig);

    setSelectedEnvironmentId(savedConfig.environmentId);
    document.getElementById('environmentSelect').value = savedConfig.environmentId;
    await chrome.storage.local.set({ selectedEnvironmentId: savedConfig.environmentId });

    setEnabledPromptIds(new Set(savedConfig.promptIds));
    await chrome.storage.local.set({ enabledPromptIds: Array.from(enabledPromptIds) });

    const quickTaskInput = document.getElementById('quickTaskInput');
    if (quickTaskInput) {
      quickTaskInput.value = savedConfig.quickTask || '';
    }

    const logsInput = document.getElementById('logsInput');
    if (logsInput && savedConfig.logs) {
      logsInput.value = savedConfig.logs;
    }

    const includeLogsCheckbox = document.getElementById('includeLogs');
    if (includeLogsCheckbox) {
      includeLogsCheckbox.checked = savedConfig.includeLogs || false;
    }

    await triggerFileTreeLoadForSelected();

    renderPrompts();

    showStatusMessage('✅ Configuration restored!', 'success');

  } catch (error) {
    contextLogger.error('Failed to restore config:', error);
    showStatusMessage('❌ Failed to restore configuration', 'error');
  }
}

/**
 * Załącza plik kontekstowy
 */
export async function attachContextFile(filepath, filename) {
  showStatusMessage(`📎 Loading ${filename}...`, 'info');

  chrome.runtime.sendMessage({
    action: 'get_context_file_content',
    payload: {
      filepath: filepath,
      filename: filename
    }
  });
}

/**
 * Obsługuje przycisk "Show More"
 */
export function handleShowMoreContext() {
  const container = document.getElementById('contextTiles');
  const btn = document.getElementById('showMoreContextBtn');
  const btnText = btn.querySelector('.btn-text');
  if (container.classList.contains('collapsed')) {
    container.classList.remove('collapsed');
    btn.classList.add('expanded');
    btnText.innerHTML = 'Show less';
  } else {
    container.classList.add('collapsed');
    btn.classList.remove('expanded');

    const totalCount = previousContextHistory.length;
    const favoritedCount = previousContextHistory.filter(item => item.favorite).length;
    const visibleInCollapsed = favoritedCount > 0 ? favoritedCount : (totalCount > 0 ? 1 : 0);
    const hiddenCount = totalCount - visibleInCollapsed;

    btnText.innerHTML = `Show <span id="hiddenCount">${hiddenCount}</span> more`;
  }
}

/**
 * Parsuje załączone pliki z treści kontekstu
 */
export function parseAttachedFilesFromContext(content) {
  const attachedFiles = [];
  const lines = content.split('\n');
  let inAttachedSection = false;

  for (const line of lines) {
    const trimmedLine = line.trim();

    if (trimmedLine === '=== ATTACHED FILES ===') {
      inAttachedSection = true;
      continue;
    }

    if (trimmedLine.startsWith('===')) {
      if (inAttachedSection) break;
      continue;
    }

    if (inAttachedSection && trimmedLine.startsWith('- ')) {
      const contentPart = trimmedLine.substring(2);
      const parts = contentPart.split(':');
      if (parts.length >= 2) {
        const projectName = parts[0].trim();
        const relativePath = parts.slice(1).join(':').trim();
        attachedFiles.push({ projectName, relativePath });
      }
    }
  }

  return attachedFiles;
}

// ============================================================================
// Context File Generation
// ============================================================================

/**
 * Generuje plik kontekstowy
 */
export async function handleGenerateContextFile() {
  contextLogger.log('handleGenerateContextFile called');

  const projects = await constructMultiProjectPayloadWithSymbols();
  const quickTaskValue = document.getElementById('quickTaskInput').value.trim();
  const includeStructure = document.getElementById('includeStructure').checked;
  const includeLogs = document.getElementById('includeLogs').checked;
  const logsContent = document.getElementById('logsInput').value;

  if (projects.length === 0 && !(includeStructure && selectedProjects.size > 0)) {
    showStatusMessage('Select files OR enable structure export with selected projects.', 'error');
    return;
  }

  if (!selectedEnvironmentId) {
    showStatusMessage('Please select an environment first.', 'error');
    return;
  }

  contextLogger.log('Validation passed');
  showLoading('Generating...', 'generateBtn');
  showContextProgress();

  let finalProjects = projects;
  if (includeStructure && projects.length === 0 && selectedProjects.size > 0) {
    contextLogger.log('Creating payload for structure-only export');
    finalProjects = Array.from(selectedProjects).map(projectPath => ({
      rootPath: projectPath,
      relativePaths: []
    }));
  }

  // Determine language for protocol instructions
  const selectedEnv = environments.find(e => e.id === selectedEnvironmentId);
  const language = selectedEnv ? selectedEnv.language : 'en';

  // Get attached prompt files (from "Attach as Prompts" modal)
  const attachedPromptFiles = window.attachedPromptFiles || [];

  // Get selected Virtual Files (from Google Drive/Attachments)
  const virtualFilesPayload = [];
  const selectedVirtual = selectedNodes.get(VIRTUAL_FILES_PROJECT_PATH);

  if (selectedVirtual && selectedVirtual.size > 0 && window.gluonVirtualFiles) {
    selectedVirtual.forEach(filename => {
      const content = window.gluonVirtualFiles.get(filename);
      if (content) {
        virtualFilesPayload.push({
          name: filename,
          content: content
        });
      }
    });
  }

  const payload = {
    projects: finalProjects,
    includeStructure,
    includeLogs,
    logs: (includeLogs && logsContent) ? logsContent : null,
    environmentId: selectedEnvironmentId,
    enabledPromptIds: Array.from(enabledPromptIds),
    attachedPromptFiles: attachedPromptFiles.map(f => ({
      name: f.name,
      content: f.content,
      source: f.source
    })),
    virtualFiles: virtualFilesPayload, // Send raw content for virtual files
    quickTask: quickTaskValue ? quickTaskValue : null,
    projectMapping: getProjectMapping(),
    // Send protocol instructions if Gluon Mode is enabled
    protocolInstructions: gluonModeEnabled ? (GLUON_PROTOCOL_INSTRUCTIONS[language] || GLUON_PROTOCOL_INSTRUCTIONS['en']) : null,
    // Send Context Architect Prompt
    contextArchitectPrompt: gluonModeEnabled ? (CONTEXT_ARCHITECT_PROMPT[language] || CONTEXT_ARCHITECT_PROMPT['en']) : null
  };

  contextLogger.log('Payload details:');
  contextLogger.log('  - Projects count:', payload.projects.length);
  contextLogger.log('  - Include structure:', payload.includeStructure);
  contextLogger.log('  - Selected projects:', Array.from(selectedProjects));
  payload.projects.forEach((p, i) => {
    contextLogger.log(`  - Project ${i + 1}:`, p.rootPath, `(${p.relativePaths.length} files)`);
  });

  // Zapisz quick task do historii jeśli nie jest pusty
  if (quickTaskValue) {
    await savePromptToHistory('quick_task', quickTaskValue);
  }

  chrome.runtime.sendMessage({
    action: 'generate_context_file',
    payload: payload
  }, (response) => {
    if (response && response.request_id) {
      window.currentContextRequestId = response.request_id;
    }
  });
}

/**
 * Generuje prosty plik - tylko zaznaczone pliki (bez promptów, struktury, logów)
 */
export async function handleGenerateSimpleFile() {
  contextLogger.log('handleGenerateSimpleFile called');

  const projects = constructMultiProjectPayload();

  if (projects.length === 0) {
    showStatusMessage('Please select at least one file.', 'error');
    return;
  }

  // Nie wymagamy environment dla prostego generowania
  // Użyjemy null jako fallback
  const environmentId = selectedEnvironmentId || null;

  contextLogger.log('Validation passed for simple file generation');
  showLoading('Generating...', 'generateSimpleBtn');

  const payload = {
    projects: projects,
    includeStructure: false,
    includeLogs: false,
    logs: null,
    environmentId: environmentId,
    enabledPromptIds: [],
    quickTask: null,
    projectMapping: getProjectMapping()
  };

  contextLogger.log('Simple file payload details:');
  contextLogger.log('  - Projects count:', payload.projects.length);
  payload.projects.forEach((p, i) => {
    contextLogger.log(`  - Project ${i + 1}:`, p.rootPath, `(${p.relativePaths.length} files)`);
  });

  chrome.runtime.sendMessage({
    action: 'generate_context_file',
    payload: payload
  });
}

/**
 * Obsługuje odpowiedź kopiowania plików
 */
export async function handleFileCopyResponse(projectsWithFiles) {
  if (!Array.isArray(projectsWithFiles) || projectsWithFiles.length === 0) {
    return showError("Failed to receive file content.");
  }

  if (lastAction === 'attach') {
    // Zlicz wszystkie pliki
    let totalFiles = 0;
    let errorCount = 0;

    for (const project of projectsWithFiles) {
      for (const file of project.files) {
        if (file && file.content != null) {
          totalFiles++;
        } else {
          errorCount++;
        }
      }
    }

    // Jeśli jest więcej niż 1 plik, połącz je w jeden
    if (totalFiles > 1) {
      const output = [];
      const project = projectsWithFiles[0];
      const projectName = project.projectPath.split(/[\\/]/).pop() || 'files';

      // Header dla połączonego pliku
      output.push(`=== Combined Files from: ${projectName} ===`);
      output.push(`Total files: ${totalFiles}`);
      output.push(`Generated: ${new Date().toLocaleString()}\n`);

      // Dodaj zawartość wszystkich plików z headerami
      for (const project of projectsWithFiles) {
        for (const file of project.files) {
          if (file && file.content != null) {
            output.push(`\n${'='.repeat(80)}`);
            output.push(`File: ${file.path}`);
            output.push('='.repeat(80));
            output.push(file.content);
          } else if (file?.error) {
            contextLogger.error(`Could not read file: ${file.path} - ${file.error}`);
          }
        }
      }

      output.push(`\n${'='.repeat(80)}`);
      output.push(`End of combined files (${totalFiles} files)`);
      output.push('='.repeat(80));

      // Wyślij jako jeden plik
      const combinedContent = output.join('\n');
      const combinedFilename = `${projectName}_combined_${totalFiles}files.txt`;

      chrome.runtime.sendMessage({
        action: 'inject_file_to_gemini',
        payload: {
          filename: combinedFilename,
          content: combinedContent,
          type: 'text/plain'
        }
      });

      showStatusMessage(`Attached ${totalFiles} files as ${combinedFilename}`, 'success');

    } else if (totalFiles === 1) {
      // Pojedynczy plik - załącz normalnie
      const project = projectsWithFiles[0];
      const file = project?.files[0];

      if (file && file.content != null) {
        const filename = file.path.split(/[\\/]/).pop() || file.path;
        chrome.runtime.sendMessage({
          action: 'inject_file_to_gemini',
          payload: {
            filename: filename,
            content: file.content,
            type: 'text/plain'
          }
        });
        showStatusMessage(`Attached ${filename}`, 'success');
      }
    }

    if (errorCount > 0) {
      showError(`Failed to read ${errorCount} file${errorCount > 1 ? 's' : ''}`);
    }

  } else {
    let totalFiles = 0, totalSize = 0;
    const output = [];
    projectsWithFiles.forEach(p => p.files.forEach(f => {
      if (f.content) { totalFiles++; totalSize += new Blob([f.content]).size; }
    }));
    const generated = new Date().toLocaleString();
    output.push(`=== GLUON CONTEXT PACKAGE ===\nGenerated: ${generated}\nProjects: ${projectsWithFiles.length}\nTotal files: ${totalFiles}\nTotal size: ${formatSize(totalSize)}\n`);
    for (const project of projectsWithFiles) {
      const projectName = project.projectPath.split(/[\\/]/).pop() || project.projectPath;
      output.push(`\n=== PROJECT: ${projectName} ===\nPath: ${project.projectPath}\n`);
      for (const file of project.files) {
        if (file.error) output.push(`// ERROR reading file: ${file.path}\n// ${file.error}\n`);
        else if (file.content != null) {
          output.push(`// ${file.path}\n${file.content}\n`);
        }
      }
    }
    output.push('=== END CONTEXT ===');
    try {
      await navigator.clipboard.writeText(output.join('\n'));
      showStatusMessage(`Copied ${totalFiles} files (${formatSize(totalSize)})`, 'success');
    } catch (err) {
      showError('Could not copy to clipboard.');
    }
  }

  setLastAction(null);
}

/**
 * Kopiuje pliki
 */
export function handleCopyFiles() {
  const projects = constructMultiProjectPayload();
  if (projects.length === 0) return;
  showLoading('Copying...', 'copyBtn');
  setLastAction('copy');
  chrome.runtime.sendMessage({ action: 'get_files_multi', payload: { projects } });
}

/**
 * Generuje mapę semantyczną zaznaczonych plików
 * Używa context_operations z typem semantic_map - pokazuje tylko symbole (klasy, funkcje) bez kodu
 */
export async function handleGenerateSemanticMap() {
  contextLogger.log('handleGenerateSemanticMap called');

  const projects = await constructMultiProjectPayloadWithSymbols();

  if (projects.length === 0) {
    showStatusMessage('Please select at least one file or folder.', 'error');
    return;
  }

  contextLogger.log('Validation passed for semantic map generation');
  showLoading('Generating map...', 'generateMapBtn');

  // Wybierz pierwszy projekt jako project root (wymagane przez backend)
  const projectRoot = projects[0].rootPath.replace(/\\/g, '/');

  // Zbierz wszystkie zaznaczone ścieżki (pliki i foldery)
  const paths = [];

  projects.forEach(project => {
    if (project.relativePaths && project.relativePaths.length > 0) {
      // Jeśli są zaznaczone konkretne pliki - użyj ścieżek relatywnych
      project.relativePaths.forEach(relPath => {
        // Store relative path only (relative to projectRoot)
        paths.push(relPath.replace(/\\/g, '/'));
      });
    } else {
      // Jeśli zaznaczony cały projekt - użyj "." dla całego projektu
      paths.push('.');
    }
  });

  if (paths.length === 0) {
    showStatusMessage('No files or folders selected for mapping.', 'error');
    hideLoading('generateMapBtn');
    return;
  }

  contextLogger.log('Project root:', projectRoot);
  contextLogger.log('Semantic map paths:', paths);

  // Przygotuj operację semantic_map z tablicą paths (zgodnie z Rust enum)
  const operations = [{
    type: 'semantic_map',
    paths: paths  // ⚠️ WAŻNE: Rust oczekuje "paths" (liczba mnoga) jako Vec<String>
  }];

  contextLogger.log('Semantic map operations:', operations);

  // Wyślij do backendu przez akcję execute_context_operations
  return new Promise((resolve, reject) => {
    // Listener dla odpowiedzi
    const responseListener = (message) => {
      if (message.type === 'execute_context_operations_response') {
        chrome.runtime.onMessage.removeListener(responseListener);
        hideLoading('generateMapBtn');

        if (message.data.error) {
          showStatusMessage(`Error: ${message.data.error}`, 'error');
          contextLogger.error('Semantic map error:', message.data.error);
          reject(new Error(message.data.error));
        } else {
          contextLogger.log('✅ Semantic map generated successfully');
          contextLogger.log(`Success rate: ${message.data.successful}/${message.data.total_operations}`);
          contextLogger.log('📦 Full response data:', message.data);

          // Zapisz mapę jako plik w folderze kontekstów
          if (message.data.items && message.data.items.length > 0) {
            const mapContent = message.data.items
              .map(r => r.content || '')
              .join('\n\n' + '='.repeat(80) + '\n\n');

            contextLogger.log('📊 Semantic Map generated:', mapContent.length, 'characters');

            // Przygotuj nazwę pliku z timestampem
            const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, -5);
            const filename = `semantic_map_${timestamp}.md`;

            // Wyślij żądanie zapisu pliku do backendu
            chrome.runtime.sendMessage({
              action: 'save_semantic_map',
              payload: {
                filename: filename,
                content: mapContent,
                projectRoot: projectRoot
              }
            }, (response) => {
              contextLogger.log('📨 Response from save_semantic_map:', response);
              if (response && response.data && response.data.success) {
                showStatusMessage(`🗺️ Semantic map saved: ${filename}`, 'success');
                contextLogger.log('Semantic map saved to:', response.data.filepath);

                // Odśwież listę plików kontekstowych
                setTimeout(() => loadContextHistory(), 500);
              } else {
                showStatusMessage('⚠️ Map generated but save failed - check console', 'warning');
                contextLogger.error('Failed to save semantic map:', response);

                // Fallback: skopiuj do schowka
                navigator.clipboard.writeText(mapContent)
                  .then(() => showStatusMessage('📋 Map copied to clipboard instead', 'info'))
                  .catch(() => contextLogger.error('Failed to copy to clipboard'));
              }
            });
          }

          resolve(message.data);
        }
      }
    };

    chrome.runtime.onMessage.addListener(responseListener);

    // Wyślij żądanie do background script z projectRoot
    chrome.runtime.sendMessage({
      action: 'execute_context_operations',
      payload: {
        operations: operations,
        projectRoot: projectRoot  // WYMAGANE przez backend
      }
    });
  });
}