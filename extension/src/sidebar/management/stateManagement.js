import { sidebarLogger } from '../../common/logger.js';

// ============================================================================
// State Management Module
// Zarządza stanem globalnym, storage i konfiguracją
// ============================================================================

// ============================================================================
// Global State Variables
// ============================================================================

export let fileTreeData = [];
export let selectedNodes = new Map();
export let selectedProjects = new Set();
export let allProjects = [];
export let searchQuery = '';
export let searchTimeout;
export let currentFileTreeRequestId = null;
export const tileContentCache = new Map();
export let lastAction = null;
export let environments = [];
export let selectedEnvironmentId = null;
export let enabledPromptIds = new Set();
export let pendingConfigRestore = null;
export let previousContextHistory = [];
export let templates = {};
export let activeTemplates = {
  auto_select: 'default',
  context_handoff: 'default',
  prompt_handoff: 'default'
};
export let contextTilesExpanded = false;
export let collapsedNodes = new Set();
export let licenseStatus = 'MISSING';
export let fileTreePollingInterval = null;
export let gluonModeEnabled = true;

// RAG Selection State
export let ragSelectedNodes = new Map(); // ProjectPath -> Set<FilePath>
export let ragSelectedProjects = new Set(); 

// ============================================================================
// Constants
// ============================================================================

export const POLLING_INTERVAL_MS = 5000; // Co 5 sekund zamiast 1s
export const VIRTUAL_FILES_PROJECT_PATH = '__gluon_virtual_files__';
export const BINARY_EXTENSIONS = new Set([
  'pdf', 'docx', 'doc', 'xlsx', 'xls', 'pptx', 'ppt', 'odt', 'ods',
  'svg', 'png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'tiff'
]);
export const PROJECT_COLORS = ['#00d4ff', '#00ff88', '#6366f1', '#f59e0b', '#ef4444', '#8b5cf6', '#10b981', '#f97316'];

// ============================================================================
// State Setters (for updating state from other modules)
// ============================================================================

export function setFileTreeData(data) { fileTreeData = data; }
export function setSelectedNodes(nodes) { selectedNodes = nodes; }
export function setSelectedProjects(projects) { selectedProjects = projects; }
export function setAllProjects(projects) { allProjects = projects; }
export function setSearchQuery(query) { searchQuery = query; }
export function setSearchTimeout(timeout) { searchTimeout = timeout; }
export function setCurrentFileTreeRequestId(id) { currentFileTreeRequestId = id; }
export function setLastAction(action) { lastAction = action; }
export function setEnvironments(envs) { environments = envs; }
export function setSelectedEnvironmentId(id) { selectedEnvironmentId = id; }
export function setEnabledPromptIds(ids) { enabledPromptIds = ids; }
export function setPendingConfigRestore(config) { pendingConfigRestore = config; }
export function setPreviousContextHistory(history) { previousContextHistory = history; }
export function setTemplates(tmpl) { templates = tmpl; }
export function setActiveTemplates(active) { activeTemplates = active; }
export function setContextTilesExpanded(expanded) { contextTilesExpanded = expanded; }
export function setCollapsedNodes(nodes) { collapsedNodes = nodes; }
export function setLicenseStatus(status) { licenseStatus = status; }
export function setFileTreePollingInterval(interval) { fileTreePollingInterval = interval; }
export function setGluonModeEnabled(enabled) { gluonModeEnabled = enabled; }
export function setRagSelectedNodes(nodes) { ragSelectedNodes = nodes; }
export function setRagSelectedProjects(projects) { ragSelectedProjects = projects; }

// ============================================================================
// Gluon Mode Toggle
// ============================================================================

/**
 * Toggles Gluon Mode on/off and broadcasts to all content scripts
 */
export async function toggleGluonMode(enabled) {
  gluonModeEnabled = enabled;
  await chrome.storage.local.set({ gluonModeEnabled: enabled });

  // Broadcast to all content scripts
  chrome.runtime.sendMessage({
    action: 'gluon_mode_changed',
    enabled: enabled
  });

  sidebarLogger.log(`Gluon Mode ${enabled ? 'enabled' : 'disabled'}`);
}

// ============================================================================
// Storage Functions
// ============================================================================

/**
 * Ładuje stan z chrome.storage.local
 */
export async function loadStateFromStorage() {
  const data = await chrome.storage.local.get({
    includeStructure: true,
    includeLogs: false,
    selectedEnvironmentId: null,
    enabledPromptIds: [],
    gluonModeEnabled: true
  });
  document.getElementById('includeStructure').checked = data.includeStructure;
  document.getElementById('includeLogs').checked = data.includeLogs;
  selectedEnvironmentId = data.selectedEnvironmentId;
  enabledPromptIds = new Set(data.enabledPromptIds);

  // Load Gluon Mode state and update UI
  gluonModeEnabled = data.gluonModeEnabled;
  const gluonSwitch = document.getElementById('gluonModeSwitch');
  if (gluonSwitch) {
    gluonSwitch.checked = gluonModeEnabled;
  }
}

/**
 * Zapisuje wybrane projekty do storage
 */
export async function saveSelectedProjects() {
  await chrome.storage.local.set({ selectedProjects: Array.from(selectedProjects) });
}

/**
 * Przywraca wybrane projekty ze storage
 */
export async function restoreSelectedProjects() {
  const { selectedProjects: storedProjects } = await chrome.storage.local.get('selectedProjects');
  if (storedProjects && Array.isArray(storedProjects)) {
    selectedProjects = new Set(storedProjects);
  }
}

/**
 * Obsługuje zmianę checkboxa struktury
 */
export function handleCheckboxChange(event) {
  sidebarLogger.log('handleCheckboxChange:'), {
    checked: event.target.checked,
    id: event.target.id
  };
  chrome.storage.local.set({ includeStructure: event.target.checked });
}

/**
 * Obsługuje zmianę checkboxa logów
 */
export function handleLogsCheckboxChange(event) {
  chrome.storage.local.set({ includeLogs: event.target.checked });
}

/**
 * Ładuje aktywne szablony
 */
export async function loadActiveTemplates() {
  const data = await chrome.storage.local.get({ active_templates: null });
  if (data.active_templates) {
    activeTemplates = data.active_templates;
  } else {
    await chrome.storage.local.set({ active_templates: activeTemplates });
  }
  sidebarLogger.log('Active templates loaded:', activeTemplates);
}

// ============================================================================
// UI Helper Functions
// ============================================================================

/**
 * Pokazuje loading na przycisku
 */
export function showLoading(message, buttonId) {
  const btn = document.getElementById(buttonId);
  if (btn) {
    btn.dataset.originalText = btn.textContent;
    btn.innerHTML = `<span class="spinner"></span> ${message}`;
    btn.disabled = true;
  } else {
    document.getElementById('fileTree').innerHTML = `<div class="empty-state"><div class="empty-icon">⏳</div><div class="empty-text">${message}</div></div>`;
  }
}

/**
 * Ukrywa loading z przycisku
 */
export function hideLoading(buttonId) {
  const btn = document.getElementById(buttonId);
  if (btn) {
    if (btn.dataset.originalText) {
      btn.textContent = btn.dataset.originalText;
      // Usuń spinner, jeśli istnieje
      const spinner = btn.querySelector('.spinner');
      if (spinner) {
        spinner.remove();
      }
    }
    // Ta linia jest kluczowa do ponownego włączenia przycisku
    btn.disabled = false;
  }
}

/**
 * Pokazuje komunikat o błędzie
 */
export function showError(message) {
  document.getElementById('fileTree').innerHTML = `<div class="empty-state is-error"><div class="empty-icon">⚠️</div><div class="empty-text"><strong>Error:</strong><span>${message}</span></div></div>`;
  showStatusMessage(message, 'error');
}

/**
 * Pokazuje status message (toast)
 */
export function showStatusMessage(message, type = 'info') {
  const statusEl = document.getElementById('statusMessage');
  statusEl.textContent = message;
  statusEl.className = `status-message ${type}`;
  statusEl.style.display = 'block';
  setTimeout(() => { statusEl.style.display = 'none'; }, type === 'error' ? 5000 : 3000);
}

/**
 * Formatuje rozmiar w bajtach
 */
export function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Escape HTML
 */
export function escapeHTML(str) {
  if (typeof str !== 'string') return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

/**
 * Formatuje czas jako "X ago"
 */
export function getTimeAgo(date) {
  const seconds = Math.floor((new Date() - date) / 1000);

  const intervals = {
    year: 31536000,
    month: 2592000,
    week: 604800,
    day: 86400,
    hour: 3600,
    minute: 60
  };

  for (const [unit, secondsInUnit] of Object.entries(intervals)) {
    const interval = Math.floor(seconds / secondsInUnit);
    if (interval >= 1) {
      return `${interval} ${unit}${interval > 1 ? 's' : ''} ago`;
    }
  }

  return 'just now';
}

/**
 * Aktualizuje overlay blokujący
 */
export function updateBlockingOverlay() {
  const overlay = document.getElementById('blocking-overlay');
  const title = document.getElementById('overlay-title');
  const message = document.getElementById('overlay-message');
  const link = document.getElementById('overlay-link');

  const indicator = document.getElementById('connectionIndicator');
  const isConnected = indicator.classList.contains('connected');

  if (!isConnected) {
    title.textContent = 'Application Disconnected';
    message.textContent = 'Please launch the Gluon Desktop application to enable all features.';
    link.textContent = 'Download Gluon';
    overlay.style.display = 'flex';
    return;
  }

  overlay.style.display = 'none';
}

/**
 * Aktualizuje status połączenia
 */
export function updateConnectionStatus(status) {
  const indicator = document.getElementById('connectionIndicator');

  // TRUE CONNECTION STATUS: Only "Connected" or "Disconnected" from WebSocket
  // Other messages like "Processed X changes", "Indexed Y files" are NOT connection status
  if (status === 'Connected ✓' || status.includes('Connected')) {
    indicator.className = 'connection-indicator connected';
    indicator.title = 'Connected to Gluon Desktop';
    updateBlockingOverlay();
  } else if (status === 'Disconnected') {
    // ONLY set disconnected state for explicit "Disconnected" status
    indicator.className = 'connection-indicator disconnected';
    indicator.title = 'Disconnected - Click to reconnect';
    showStatusMessage('Desktop app disconnected. Click indicator to reconnect.', 'error');
    updateBlockingOverlay();
  }
  // For other status messages (progress, processing, etc.), do nothing - keep current connection state
}

/**
 * Stosuje motyw
 */
export function applyTheme(settings) {
  if (!settings || !settings.themeName) {
    sidebarLogger.warn('Brak ustawień motywu do zastosowania.');
    return;
  }

  const { themeName, theme80sColor1, theme80sColor2 } = settings;

  document.body.classList.remove('theme-80s');

  if (themeName === '80s') {
    document.body.classList.add('theme-80s');
    document.documentElement.style.setProperty('--theme-80s-bg', theme80sColor1);
    document.documentElement.style.setProperty('--theme-80s-accent', theme80sColor2);
  } else {
    document.documentElement.style.removeProperty('--theme-80s-bg');
    document.documentElement.style.removeProperty('--theme-80s-accent');
  }
}

/**
 * Show context generation progress bar
 */
export function showContextProgress() {
  const container = document.getElementById('contextProgressContainer');
  if (container) {
    container.style.display = 'block';

    // Hide generate buttons
    const generateBtn = document.getElementById('generateBtn');
    const generateSimpleBtn = document.getElementById('generateSimpleBtn');
    if (generateBtn) generateBtn.style.display = 'none';
    if (generateSimpleBtn) generateSimpleBtn.style.display = 'none';
  }
}

/**
 * Hide context generation progress bar
 */
export function hideContextProgress() {
  const container = document.getElementById('contextProgressContainer');
  if (container) {
    container.style.display = 'none';

    // Show generate buttons again
    const generateBtn = document.getElementById('generateBtn');
    const generateSimpleBtn = document.getElementById('generateSimpleBtn');
    if (generateBtn) generateBtn.style.display = 'inline-block';
    if (generateSimpleBtn) generateSimpleBtn.style.display = 'inline-block';
  }

  // Hide shimmer
  const activityIndicator = document.getElementById('contextProgressActivity');
  if (activityIndicator) {
    activityIndicator.style.display = 'none';
  }

  // Reset progress tracking
  lastProgressPercentage = 0;
  lastProgressUpdate = Date.now();

  // Reset progress
  updateContextProgress({
    step: 0,
    stage: 'Initializing',
    message: 'Preparing...',
    percentage: 0
  });
}

// Track last percentage for shimmer effect
let lastProgressPercentage = 0;
let lastProgressUpdate = Date.now();

/**
 * Update context generation progress
 */
export function updateContextProgress(progressData) {
  const { step, stage, message, percentage } = progressData;

  // Update stage
  const stageEl = document.getElementById('contextProgressStage');
  if (stageEl) {
    stageEl.textContent = `[${step}/10] ${stage}`;
  }

  // Update percentage
  const percentageEl = document.getElementById('contextProgressPercentage');
  if (percentageEl) {
    percentageEl.textContent = `${percentage}%`;
  }

  // Update progress bar
  const progressBar = document.getElementById('contextProgressBar');
  const activityIndicator = document.getElementById('contextProgressActivity');
  if (progressBar) {
    progressBar.style.width = `${percentage}%`;
  }

  // Show shimmer effect if progress hasn't changed in 3 seconds
  const now = Date.now();
  if (percentage === lastProgressPercentage && now - lastProgressUpdate > 3000) {
    if (activityIndicator) {
      activityIndicator.style.display = 'block';
    }
  } else {
    if (activityIndicator) {
      activityIndicator.style.display = 'none';
    }
    lastProgressUpdate = now;
  }
  lastProgressPercentage = percentage;

  // Update message
  const messageEl = document.getElementById('contextProgressMessage');
  if (messageEl) {
    messageEl.textContent = message;
  }

  // Update file count (if provided) - but hide the confusing count display
  const filesDiv = document.getElementById('contextProgressFiles');
  if (filesDiv) {
    filesDiv.style.display = 'none'; // Hide confusing file count
  }
}