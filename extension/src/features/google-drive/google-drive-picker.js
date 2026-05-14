// ============================================================================
// Google Drive File Picker for Sidebar
// Allows selecting files from Google Drive to add to context
// ============================================================================

import { showStatusMessage } from '../../sidebar/management/stateManagement.js';

let selectedFiles = new Map(); // Map<fileId, DriveFile>
let allFiles = [];

/**
 * Initialize Google Drive Picker
 */
export function initGoogleDrivePicker() {
  const addFromDriveBtn = document.getElementById('addFromDriveBtn');
  const modal = document.getElementById('googleDrivePickerModal');
  const cancelBtn = document.getElementById('cancelDrivePickerBtn');
  const addBtn = document.getElementById('addDriveFilesBtn');
  const searchInput = document.getElementById('driveFileSearch');

  if (!addFromDriveBtn || !modal) {
    console.warn('Google Drive Picker UI elements not found');
    return;
  }

  // Open modal
  addFromDriveBtn.addEventListener('click', async () => {
    await openDrivePicker();
  });

  // Close modal
  cancelBtn.addEventListener('click', () => {
    closeDrivePicker();
  });

  // Close on outside click
  modal.addEventListener('click', (e) => {
    if (e.target === modal) {
      closeDrivePicker();
    }
  });

  // Add files
  addBtn.addEventListener('click', async () => {
    await addSelectedFiles();
  });

  // Search (Debounced)
  let debounceTimer;
  searchInput.addEventListener('input', (e) => {
    const query = e.target.value;
    clearTimeout(debounceTimer);

    // Show local filtering immediately if we have files (UX improvement)
    if (allFiles.length > 0) {
        const listContainer = document.getElementById('driveFileList');
        const items = listContainer.getElementsByClassName('drive-file-item');
        const lowerQuery = query.toLowerCase();
        for (let item of items) {
            const name = item.querySelector('.file-name').textContent.toLowerCase();
            item.style.display = name.includes(lowerQuery) ? 'flex' : 'none';
        }
    }

    debounceTimer = setTimeout(() => {
        loadDriveFiles(query);
    }, 600);
  });

  console.log('✅ Google Drive Picker initialized');
}

/**
 * Open Drive Picker modal
 */
async function openDrivePicker() {
  const modal = document.getElementById('googleDrivePickerModal');
  modal.style.display = 'flex';

  // [FIX] Zamiast czyścić, synchronizujemy ze stanem wirtualnych plików
  selectedFiles.clear();

  // Pobieramy globalne pliki wirtualne (z contextManagement.js)
  if (window.gluonVirtualFiles && window.gluonVirtualFiles.size > 0) {
      // Ponieważ gluonVirtualFiles to (nazwa -> treść), a my potrzebujemy ID,
      // musimy polegać na tym, że renderFileList oznaczy je jako 'checked'
      // jeśli mamy metadane.

      // Jednakże, selectedFiles przechowuje obiekty DriveFile.
      // Jeśli nie mamy pełnych obiektów, nie możemy ich tu łatwo odtworzyć przed załadowaniem API.
      // Strategia: 
      // 1. Czyścimy selectedFiles.
      // 2. Pobieramy listę plików z API.
      // 3. W renderFileList sprawdzamy, czy plik o danej nazwie istnieje w gluonVirtualFiles.
      // 4. Jeśli tak -> zaznaczamy checkbox i dodajemy do selectedFiles.
  }

  updateSelectionCount();

  // Clear search
  document.getElementById('driveFileSearch').value = '';

  // Load initial files (no query)
  await loadDriveFiles();
}

/**
 * Close Drive Picker modal
 */
function closeDrivePicker() {
  const modal = document.getElementById('googleDrivePickerModal');
  modal.style.display = 'none';
  selectedFiles.clear();
  document.getElementById('driveFileSearch').value = '';
}

/**
 * Load files from Google Drive
 */
async function loadDriveFiles(searchQuery = null) {
  const listContainer = document.getElementById('driveFileList');
  const statusDiv = document.getElementById('googleDriveStatus');

  try {
    statusDiv.style.display = 'block';
    statusDiv.textContent = searchQuery ? `Searching for "${searchQuery}"...` : 'Loading files...';
    statusDiv.style.color = 'var(--text-secondary)';

    // Only clear if not a search refinement
    if (!searchQuery) {
        listContainer.innerHTML = '<div class="empty-state"><div class="spinner">⚙️</div><div class="empty-text">Loading...</div></div>';
    }

    // Call Tauri backend with searchQuery
    const files = await invokeTauri('list_drive_files', { 
        folderId: null,
        searchQuery: searchQuery 
    });

    allFiles = files;

    if (files.length === 0) {
      listContainer.innerHTML = '<div class="empty-state"><div class="empty-icon">📭</div><div class="empty-text">No documents found</div></div>';
      statusDiv.style.display = 'none';
      return;
    }

    statusDiv.style.display = 'none';
    renderFileList(files);
  } catch (error) {
    console.error('Failed to load Drive files:', error);
    statusDiv.style.display = 'block';
    statusDiv.textContent = `Error: ${error}. Make sure you're logged in to Google Drive.`;
    statusDiv.style.color = 'var(--status-error)';

    listContainer.innerHTML = `
      <div class="empty-state">
        <div class="empty-icon">❌</div>
        <div class="empty-text">Failed to load files. Please check Google Drive settings.</div>
      </div>
    `;
  }
}

/**
 * Render file list
 */
function renderFileList(files) {
  const listContainer = document.getElementById('driveFileList');

  if (files.length === 0) {
    listContainer.innerHTML = '<div class="empty-state"><div class="empty-icon">🔍</div><div class="empty-text">No files match your search</div></div>';
    return;
  }

  listContainer.innerHTML = '';

  files.forEach(file => {
    const fileItem = document.createElement('div');
    fileItem.className = 'drive-file-item';
    fileItem.dataset.fileId = file.id;

    const icon = getFileIcon(file.mimeType);

    fileItem.innerHTML = `
      <label class="drive-file-label">
        <input type="checkbox" class="drive-file-checkbox" data-file-id="${file.id}">
        <span class="file-icon">${icon}</span>
        <span class="file-name">${escapeHtml(file.name)}</span>
      </label>
    `;

    const checkbox = fileItem.querySelector('.drive-file-checkbox');

    // [FIX] Sprawdź, czy plik jest już w kontekście (używając nazwy jako klucza, bo wirtualne pliki to mapa nazwa->treść)
    // Bezpieczniej byłoby używać ID, ale window.gluonVirtualFiles jest mapą filename->content.
    // Zakładamy unikalność nazw w kontekście.
    if (window.gluonVirtualFiles && window.gluonVirtualFiles.has(file.name)) {
        checkbox.checked = true;
        // Dodaj do lokalnego setu zaznaczonych, aby licznik działał poprawnie
        selectedFiles.set(file.id, file);
    }

    checkbox.addEventListener('change', (e) => {
      if (e.target.checked) {
        selectedFiles.set(file.id, file);
      } else {
        selectedFiles.delete(file.id);
      }
      updateSelectionCount();
    });

    listContainer.appendChild(fileItem);
  });

  // Zaktualizuj licznik po renderowaniu, bo mogliśmy dodać pliki automatycznie
  updateSelectionCount();
}

/**
 * Get file icon based on MIME type
 */
function getFileIcon(mimeType) {
  if (mimeType === 'application/vnd.google-apps.document') {
    return '📄'; // Google Doc
  } else if (mimeType === 'text/plain') {
    return '📝'; // Text file
  } else if (mimeType === 'text/markdown') {
    return '📋'; // Markdown
  } else if (mimeType === 'application/vnd.google-apps.folder') {
    return '📁'; // Folder
  }
  return '📄'; // Default
}

// Removed local filterFiles function as we now use server-side search via loadDriveFiles

/**
 * Update selection count
 */
function updateSelectionCount() {
  const countEl = document.getElementById('driveSelectedCount');
  const addBtn = document.getElementById('addDriveFilesBtn');

  const count = selectedFiles.size;
  countEl.textContent = `${count} file${count !== 1 ? 's' : ''} selected`;

  addBtn.disabled = count === 0;
}

/**
 * Add selected files to context
 */
async function addSelectedFiles() {
  if (selectedFiles.size === 0) return;

  const statusDiv = document.getElementById('googleDriveStatus');
  statusDiv.style.display = 'block';
  statusDiv.textContent = 'Downloading files...';
  statusDiv.style.color = 'var(--text-secondary)';

  try {
    const filesToAdd = Array.from(selectedFiles.values());

    for (const file of filesToAdd) {
      statusDiv.textContent = `Downloading ${file.name}...`;

      // Download file content
      const content = await invokeTauri('download_file_content', { fileId: file.id });

      // Add to context as virtual file
      addVirtualFileToContext(file.name, content, file.id);
    }

    showStatusMessage(`Added ${filesToAdd.length} file(s) from Google Drive`, 'success');
    closeDrivePicker();
  } catch (error) {
    console.error('Failed to add files:', error);
    statusDiv.style.display = 'block';
    statusDiv.textContent = `Error: ${error}`;
    statusDiv.style.color = 'var(--status-error)';
  }
}

/**
 * Add virtual file to context tiles
 * This integrates with the existing context management system
 */
function addVirtualFileToContext(fileName, content, driveFileId) {
  // Create virtual file object
  const virtualFile = {
    name: fileName,
    content: content,
    source: 'google-drive',
    driveFileId: driveFileId,
    timestamp: Date.now(),
  };

  // Dispatch custom event to add to context
  const event = new CustomEvent('add-virtual-file', {
    detail: virtualFile
  });
  document.dispatchEvent(event);

  console.log('Added virtual file to context:', fileName);
}

/**
 * Invoke Tauri command (works in desktop app context)
 */
async function invokeTauri(command, args = {}) {
  if (window.__TAURI__) {
    const { invoke } = window.__TAURI__.core;
    return await invoke(command, args);
  }

  // Fallback: send through background script (if running as extension)
  return new Promise((resolve, reject) => {
    chrome.runtime.sendMessage(
      {
        action: 'tauri_command',
        command: command,
        args: args,
      },
      (response) => {
        if (chrome.runtime.lastError) {
          reject(chrome.runtime.lastError.message);
        } else if (response && response.error) {
          reject(response.error);
        } else {
          resolve(response);
        }
      }
    );
  });
}

/**
 * Escape HTML
 */
function escapeHtml(unsafe) {
  return unsafe
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}