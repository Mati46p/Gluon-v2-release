// ============================================================================
// Prompt Files Manager
// Handles attaching files from various sources as prompts
// ============================================================================

import { showStatusMessage } from '../../sidebar/management/stateManagement.js';

/**
 * Open Google Drive Picker specifically for prompt files
 */
export async function openDrivePickerForPrompts() {
  const modal = document.getElementById('googleDrivePickerModal');

  if (!modal) {
    console.warn('Google Drive Picker modal not found');
    return;
  }

  // Temporarily override the file selection handler
  const originalHandler = window.driveFileSelectionHandler;

  window.driveFileSelectionHandler = 'prompts';

  // Open the drive picker
  modal.style.display = 'flex';

  // Load files
  await loadDriveFilesForPrompts();

  // Restore original handler when modal closes
  const observer = new MutationObserver((mutations) => {
    mutations.forEach((mutation) => {
      if (mutation.target.style.display === 'none') {
        window.driveFileSelectionHandler = originalHandler;
        observer.disconnect();
      }
    });
  });

  observer.observe(modal, {
    attributes: true,
    attributeFilter: ['style']
  });
}

/**
 * Load Google Drive files for prompt selection
 */
async function loadDriveFilesForPrompts() {
  const listContainer = document.getElementById('driveFileList');
  const statusDiv = document.getElementById('googleDriveStatus');
  const addBtn = document.getElementById('addDriveFilesBtn');

  try {
    statusDiv.style.display = 'block';
    statusDiv.textContent = 'Loading files from Google Drive...';
    statusDiv.style.color = 'var(--text-secondary)';

    listContainer.innerHTML = '<div class="empty-state"><div class="spinner">⚙️</div><div class="empty-text">Loading...</div></div>';

    // Call Tauri backend
    const files = await invokeTauri('list_drive_files', { folderId: null });

    if (files.length === 0) {
      listContainer.innerHTML = '<div class="empty-state"><div class="empty-icon">📭</div><div class="empty-text">No documents found in your Google Drive</div></div>';
      statusDiv.style.display = 'none';
      return;
    }

    statusDiv.style.display = 'none';
    renderDriveFilesForPrompts(files);

    // Override add button behavior
    addBtn.onclick = async () => {
      await addSelectedDriveFilesAsPrompts();
    };

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
 * Render Google Drive files for prompt selection
 */
function renderDriveFilesForPrompts(files) {
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
        <input type="checkbox" class="drive-file-checkbox-prompt" data-file-id="${file.id}" data-file-name="${escapeHtml(file.name)}" data-mime-type="${file.mimeType}">
        <span class="file-icon">${icon}</span>
        <span class="file-name">${escapeHtml(file.name)}</span>
      </label>
    `;

    listContainer.appendChild(fileItem);
  });
}

/**
 * Add selected Google Drive files as prompts
 */
async function addSelectedDriveFilesAsPrompts() {
  const checkboxes = document.querySelectorAll('.drive-file-checkbox-prompt:checked');

  if (checkboxes.length === 0) {
    showStatusMessage('Please select at least one file', 'error');
    return;
  }

  const statusDiv = document.getElementById('googleDriveStatus');
  statusDiv.style.display = 'block';
  statusDiv.textContent = 'Downloading files...';
  statusDiv.style.color = 'var(--text-secondary)';

  try {
    for (const checkbox of checkboxes) {
      const fileId = checkbox.dataset.fileId;
      const fileName = checkbox.dataset.fileName;

      statusDiv.textContent = `Downloading ${fileName}...`;

      // Download file content
      const content = await invokeTauri('download_file_content', { fileId: fileId });

      // Dispatch event to add as prompt file
      const event = new CustomEvent('prompt-file-selected', {
        detail: {
          name: fileName,
          content: content,
          source: 'google-drive',
          driveFileId: fileId,
          size: content.length,
          timestamp: Date.now()
        }
      });
      document.dispatchEvent(event);
    }

    showStatusMessage(`Added ${checkboxes.length} file(s) from Google Drive as prompts`, 'success');

    // Close the drive picker modal
    const modal = document.getElementById('googleDrivePickerModal');
    if (modal) {
      modal.style.display = 'none';
    }

  } catch (error) {
    console.error('Failed to add files:', error);
    statusDiv.style.display = 'block';
    statusDiv.textContent = `Error: ${error}`;
    statusDiv.style.color = 'var(--status-error)';
  }
}

/**
 * Get file icon based on MIME type
 */
function getFileIcon(mimeType) {
  if (mimeType === 'application/vnd.google-apps.document') {
    return '📄';
  } else if (mimeType === 'text/plain') {
    return '📝';
  } else if (mimeType === 'text/markdown') {
    return '📋';
  } else if (mimeType === 'application/vnd.google-apps.folder') {
    return '📁';
  }
  return '📄';
}

/**
 * Invoke Tauri command
 */
async function invokeTauri(command, args = {}) {
  if (window.__TAURI__) {
    const { invoke } = window.__TAURI__.core;
    return await invoke(command, args);
  }

  // Fallback: send through background script
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
  if (!unsafe) return '';
  return unsafe
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
