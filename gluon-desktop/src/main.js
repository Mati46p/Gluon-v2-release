console.log("[JS LOG] main.js script started.");
console.log("[JS LOG] Checking for __TAURI_PLUGIN_UPDATER__:", window.__TAURI_PLUGIN_UPDATER__);

// Eksportuj do window, aby inne skrypty miały dostęp bez konfliktów redeklaracji
window.invoke = window.__TAURI__.core.invoke;
window.listen = window.__TAURI__.event.listen;

// Lokalne referencje dla main.js
const invoke = window.invoke;
const listen = window.listen;

const { open, confirm } = window.__TAURI__.dialog; 
const { openUrl } = window.__TAURI_PLUGIN_OPENER__;
const { getVersion } = window.__TAURI__.app;
const { check } = window.__TAURI_PLUGIN_UPDATER__;
const { relaunch } = window.__TAURI_PLUGIN_PROCESS__;

// === STATE ===
let state = {
  projects: [],
  environments: [],
  prompts: [],
  extensionTemplates: [],
  vectorMaps: [],
  selectedProjectPath: null,
  selectedEnvironmentId: null,
  currentVectorMapId: null,
  wsConnected: false,
  defaultExclusions: ["node_modules", ".git", "target", "dist", "build", ".next", "vendor", "__pycache__", ".cache", "out"],
  currentEditingProjectPath: null,
  updateInfo: null,
};

const MASTER_EXTENSIONS_LIST = [
    // Kod i tekst
    "rs", "toml", "js", "jsx", "ts", "tsx", "html", "css", 
    "json", "md", "yml", "yaml", "txt", "py", "java", "go", "sql", "wxs", "xml",
    // Dokumenty
    "pdf", "docx", "doc", "xlsx", "xls", "pptx", "ppt", "odt", "ods", "csv",
    // Pliki graficzne
    "svg", "png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff",
];

// Predefined emoji icons for environments
const EMOJI_PRESETS = [
  '🌐', '⚛️', '💚', '🔷', '🔶', '🟢', '🔴',
  '🐍', '☕', '💎', '🦀', '🐹', '🐘', '🦕',
  '⚡', '🔥', '💧', '🌊', '🌟', '✨', '💫',
  '🎮', '🎯', '🎨', '🎭', '🎪', '🎬', '🎵',
  '🚀', '🛸', '🌙', '☀️', '🌈', '⭐', '🌌',
  '💻', '🖥️', '📱', '⌨️', '🖱️', '💾', '📡',
  '🔧', '🔨', '⚙️', '🛠️', '🧰', '🔩', '⚗️'
];

// === DOM ELEMENTS ===
let projectListEl, addProjectBtn, removeProjectBtn, environmentsListEl, promptsContainerEl,
  addEnvBtn, addPromptBtn, promptsHeaderEl, toastEl, connectionStatusEl, statusTextEl,
  licenseKeyInputEl, saveLicenseKeyBtnEl, checkForUpdatesBtnEl, installUpdateBtnEl, themeGluonV2Btn, theme80sBtn, colorSettings80s, color1Picker, color2Picker,
  updateStatusTextEl, updateDetailsEl, updateVersionEl, updateNotesEl,
  licenseStatusIndicatorEl, appVersionDisplayEl;

// === WebSocket CONNECTION ===
let ws = null;
let reconnectAttempts = 0;
const MAX_RECONNECT_ATTEMPTS = 5;
const RECONNECT_DELAY = 2000;

function connectWebSocket() {
  if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
    console.log("WebSocket already connected or connecting");
    return;
  }

  ws = new WebSocket('ws://127.0.0.1:8743');

  ws.onopen = () => {
    console.log("WebSocket connected");
    reconnectAttempts = 0;
    updateConnectionStatus(true);
  };

  ws.onerror = (error) => {
    console.error('WebSocket error:', error);
    updateConnectionStatus(false);
  };

  ws.onclose = () => {
    console.log("WebSocket disconnected");
    updateConnectionStatus(false);
    
    if (reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
      reconnectAttempts++;
      const delay = RECONNECT_DELAY * Math.pow(2, reconnectAttempts - 1);
      console.log(`Reconnecting in ${delay}ms (attempt ${reconnectAttempts}/${MAX_RECONNECT_ATTEMPTS})`);
      setTimeout(() => connectWebSocket(), delay);
    } else {
      showToast('Failed to connect to WebSocket server', 'error');
    }
  };
}

async function displayAppVersion() {
  try {
    const version = await getVersion();
    if (appVersionDisplayEl) {
      appVersionDisplayEl.textContent = `(v${version})`;
    }
  } catch (err) {
    console.error("Could not get app version:", err);
    if (appVersionDisplayEl) {
      appVersionDisplayEl.textContent = `(v?.?.?)`;
    }
  }
}

function updateConnectionStatus(connected) {
  state.wsConnected = connected;
  const indicator = connectionStatusEl.querySelector('.indicator');
  
  if (connected) {
    indicator.className = 'indicator connected';
  } else {
    indicator.className = 'indicator disconnected';
  }
}

function setupExternalLinks() {
  document.querySelectorAll('a[target="_blank"]').forEach(link => {
    link.addEventListener('click', async (e) => {
      e.preventDefault();
      const url = link.href;
      if (!url) {
        console.error('External link has no href:', link);
        return;
      }
      try {
        // Otwórz link w domyślnej przeglądarce/aplikacji systemowej
        await openUrl(url);
      } catch (err) {
        console.error(`Failed to open external link: ${url}`, err);
        showToast(`Failed to open link: ${err}`, 'error');
      }
    });
  });
}

// === INITIALIZATION ===
window.addEventListener("DOMContentLoaded", async () => {

  console.log("[JS LOG] DOMContentLoaded event fired.");

  // Wait for backend to be ready before calling any commands
  console.log("[JS LOG] Waiting for backend-ready event...");
  try {
    const { listen } = window.__TAURI__.event;
    await Promise.race([
      new Promise(async (resolve) => {
        const unlisten = await listen('backend-ready', () => {
          console.log("[JS LOG] Backend ready event received!");
          unlisten();
          resolve();
        });
      }),
      new Promise((resolve) => setTimeout(() => {
        console.warn("[JS LOG] Backend-ready timeout (3s), proceeding anyway...");
        resolve();
      }, 3000))
    ]);
  } catch (err) {
    console.error("[JS LOG] Error waiting for backend-ready:", err);
  }

  // Assign UI elements
  projectListEl = document.getElementById("project-list");
  addProjectBtn = document.getElementById("add-project-btn");
  removeProjectBtn = document.getElementById("remove-project-btn");
  environmentsListEl = document.getElementById("environments-list");
  promptsContainerEl = document.getElementById("prompts-container");
  addEnvBtn = document.getElementById("add-env-btn");
  addPromptBtn = document.getElementById("add-prompt-btn");
  promptsHeaderEl = document.getElementById("prompts-header");
  toastEl = document.getElementById("toast");
  connectionStatusEl = document.getElementById("connection-status");
  statusTextEl = document.getElementById("status-text");
  licenseKeyInputEl = document.getElementById('license-key-input');
  saveLicenseKeyBtnEl = document.getElementById('save-license-key-btn');
  checkForUpdatesBtnEl = document.getElementById('check-for-updates-btn');
  installUpdateBtnEl = document.getElementById('install-update-btn');
  updateStatusTextEl = document.getElementById('update-status-text');
  updateDetailsEl = document.getElementById('update-details');
  updateVersionEl = document.getElementById('update-version');
  updateNotesEl = document.getElementById('update-notes');
  licenseStatusIndicatorEl = document.getElementById('license-status-indicator');
  appVersionDisplayEl = document.getElementById('app-version-display');
  themeGluonV2Btn = document.getElementById('theme-gluon-v2');
  theme80sBtn = document.getElementById('theme-80s');
  colorSettings80s = document.getElementById('80s-color-settings');
  color1Picker = document.getElementById('color1-picker');
  color2Picker = document.getElementById('color2-picker');

  await displayAppVersion();

  // Attach event listeners
  setupTabListeners();
  setupProjectListeners();
  setupEnvironmentListeners();
  setupPromptListeners();
  setupTextareaFontControl();
  setupModalListeners();
  setupExternalLinks();
  setupUpdateListeners();
  setupThemeListeners();

  // Initialize Apply System UI
  if (window.ApplySystemUI) {
    window.ApplySystemUI.initialize();
  }
  // Initialize Apply System Settings
  if (window.ApplySystemSettings) {
    window.ApplySystemSettings.initialize();
  }

  // Initialize Auditor UI
  initAuditorUI();

  // Initialize Google Drive UI
  if (window.initGoogleDriveUI) {
    window.initGoogleDriveUI();
  }

  // Initialize AI Chat UI
  if (window.AiChatUI) {
    window.AiChatUI.initialize();
  }

  // Initialize Workflow Palette (Drag & Drop)
  initAgentPalette();

  // Initialize Graph Editor and Canvas
  initGraphCanvas();

  // Initialize Workflow System
  if (typeof WorkflowManager !== 'undefined') {
      console.log('[UI] Initializing Workflow System');
      window.workflowManager = new WorkflowManager();
      window.workflowManager.init();
  } else {
      console.warn('[UI] WorkflowManager script not loaded.');
  }

  // Settings button handler
  const applySettingsBtn = document.getElementById('apply-settings-btn');
  if (applySettingsBtn) {
    applySettingsBtn.addEventListener('click', () => {
      const modal = document.getElementById('apply-settings-modal');
      if (modal) {
        modal.style.display = 'flex';
        if (window.ApplySystemSettings) {
          window.ApplySystemSettings.loadConfig();
        }
      }
    });
  }

  // Settings modal close buttons
  const applySettingsCloseBtn = document.getElementById('apply-settings-close-btn');
  if (applySettingsCloseBtn) {
    applySettingsCloseBtn.addEventListener('click', () => {
      const modal = document.getElementById('apply-settings-modal');
      if (modal) modal.style.display = 'none';
    });
  }

  const applySettingsCancelBtn = document.getElementById('apply-settings-cancel-btn');
  if (applySettingsCancelBtn) {
    applySettingsCancelBtn.addEventListener('click', () => {
      const modal = document.getElementById('apply-settings-modal');
      if (modal) modal.style.display = 'none';
    });
  }

  // Connect WebSocket
  connectWebSocket();

  // Load initial data
  await loadAllData();
  await initializeLicenseState();

});

async function loadAllData() {
  await Promise.all([loadProjects(), loadEnvironments(), loadSettings(), loadThemeSettings(), loadExtensionTemplates()]);
  renderProjects();
  renderEnvironments();
}

async function loadExtensionTemplates() {
  try {
    state.extensionTemplates = await invoke("get_extension_templates");
  } catch (error) {
    showToast(`Failed to load extension templates: ${error}`, 'error');
  }
}

function updateLicenseStatusIndicator(status) {
  if (!licenseStatusIndicatorEl) return;
  
  // Usuń wszystkie poprzednie klasy statusu
  licenseStatusIndicatorEl.classList.remove('valid', 'invalid', 'missing');

  switch (status) {
    case 'VALID':
      licenseStatusIndicatorEl.classList.add('valid');
      licenseStatusIndicatorEl.title = 'License is valid';
      break;
    case 'INVALID':
      licenseStatusIndicatorEl.classList.add('invalid');
      licenseStatusIndicatorEl.title = 'License is invalid';
      break;
    default:
      licenseStatusIndicatorEl.classList.add('missing');
      licenseStatusIndicatorEl.title = 'License is missing or not verified';
      break;
  }
}

async function initializeLicenseState() {
  updateLicenseStatusIndicator('VALID');
}

async function setupUpdateListeners() {

  console.log("[JS LOG] setupUpdateListeners called."); // <-- DODAJ TEN LOG
  console.log("[JS LOG] Updater plugin object at this point:", window.__TAURI_PLUGIN_UPDATER__); 

  saveLicenseKeyBtnEl.addEventListener('click', async () => {
    const key = licenseKeyInputEl.value.trim();
    if (!key) {
      showToast('License key cannot be empty.', 'error');
      return;
    }

    saveLicenseKeyBtnEl.disabled = true;
    saveLicenseKeyBtnEl.textContent = 'Verifying...';
    try {
      // Wywołaj poprawną funkcję weryfikującą
      const result = await invoke('verify_and_save_license_key', { key });
      
      // Zaktualizuj UI na podstawie odpowiedzi z backendu
      showToast(result.message, result.success ? 'success' : 'error');
      updateLicenseStatusIndicator(result.status);
      checkForUpdatesBtnEl.disabled = !result.success;

    } catch (err) {
      showToast(`Verification failed: ${err}`, 'error');
      console.error("Failed to verify license key:", err);
      updateLicenseStatusIndicator('INVALID');
    } finally {
      saveLicenseKeyBtnEl.disabled = false;
      saveLicenseKeyBtnEl.textContent = 'Save Key';
    }
  });

  checkForUpdatesBtnEl.addEventListener('click', async () => {
    updateStatusTextEl.textContent = 'Updates are disabled.';
    updateStatusTextEl.className = 'update-status-text';
  });

}

async function loadSettings() {
}

// === TABS LOGIC ===
function setupTabListeners() {
  document.querySelectorAll('.tab-link').forEach(button => {
    button.addEventListener('click', () => {
      const tabId = button.dataset.tab;
      document.querySelectorAll('.tab-link').forEach(btn => btn.classList.remove('active'));
      document.querySelectorAll('.tab-content').forEach(content => content.classList.remove('active'));
      button.classList.add('active');
      document.getElementById(tabId).classList.add('active');
    });
  });
}

// === THEME LOGIC ===
function setupThemeListeners() {
  themeGluonV2Btn.addEventListener('click', () => {
    applyTheme('gluon-v2');
    saveThemeSettings('gluon-v2');
  });

  theme80sBtn.addEventListener('click', () => {
    const color1 = color1Picker.value;
    const color2 = color2Picker.value;
    applyTheme('80s', { color1, color2 });
    saveThemeSettings('80s', { color1, color2 });
  });

  color1Picker.addEventListener('input', (e) => {
    const color1 = e.target.value;
    const color2 = color2Picker.value;
    applyTheme('80s', { color1, color2 });
    saveThemeSettings('80s', { color1, color2 });
  });

  color2Picker.addEventListener('input', (e) => {
    const color1 = color1Picker.value;
    const color2 = e.target.value;
    applyTheme('80s', { color1, color2 });
    saveThemeSettings('80s', { color1, color2 });
  });
}

async function loadThemeSettings() {
  try {
    const theme = await invoke("get_setting", { key: 'ui_theme' }) || 'gluon-v2';
    const color1 = await invoke("get_setting", { key: 'theme_80s_color1' }) || '#170b01';
    const color2 = await invoke("get_setting", { key: 'theme_80s_color2' }) || '#ff3300';
    
    color1Picker.value = color1;
    color2Picker.value = color2;
    applyTheme(theme, { color1, color2 });
  } catch (err) {
    console.warn("Could not load theme settings, using defaults.", err);
    applyTheme('gluon-v2');
  }
}

function applyTheme(themeName, colors = {}) {
  document.body.classList.remove('theme-80s');
  colorSettings80s.style.display = 'none';

  if (themeName === '80s') {
    document.body.classList.add('theme-80s');
    document.documentElement.style.setProperty('--theme-80s-bg', colors.color1);
    document.documentElement.style.setProperty('--theme-80s-accent', colors.color2);
    colorSettings80s.style.display = 'flex';
  }
}

async function saveThemeSettings(themeName, colors = {}) {
  try {
    await invoke("set_setting", { key: 'ui_theme', value: themeName });
    if (themeName === '80s') {
      await invoke("set_setting", { key: 'theme_80s_color1', value: colors.color1 });
      await invoke("set_setting", { key: 'theme_80s_color2', value: colors.color2 });
    }
  } catch (err) {
    showToast(`Failed to save theme: ${err}`, 'error');
  }
}

// === PROJECTS LOGIC ===
async function loadProjects() {
  try {
    state.projects = await invoke("get_projects");
    console.log('[JS LOG] Data received from Rust in loadProjects():', JSON.parse(JSON.stringify(state.projects)));
  } catch (error) {
    showToast(`Failed to load projects: ${error}`, 'error');
  }
}

function renderProjects() {
  projectListEl.innerHTML = '';

  if (state.projects.length === 0) {
    projectListEl.innerHTML = '<li class="empty-message">No projects yet. Click "Add Project" to get started.</li>';
    return;
  }

  state.projects.forEach(project => {
    const li = document.createElement('li');
    // Fixed: Added neumorphic classes to match UI style
    li.className = 'project-item neumorphic-raised-sm interactive-lift';
    li.dataset.path = project.path;
    if (project.path === state.selectedProjectPath) {
      li.classList.add('active');
    }

    const pathSpan = document.createElement('span');
    pathSpan.className = 'project-item-path';
    pathSpan.textContent = project.path;
    pathSpan.title = project.path;
    li.appendChild(pathSpan);

    const actionsDiv = document.createElement('div');
    actionsDiv.className = 'project-item-actions';
    actionsDiv.innerHTML = `
      <button class="btn btn-configure" title="Configure ignored folders">⚙️ Configure</button>
      <button class="btn btn-download-folder" data-project-id="${project.id}" title="Set download folder">📁</button>
    `;
    li.appendChild(actionsDiv);

    // Wyświetl aktualny download path jako podpowiedź
    const downloadPathHint = document.createElement('div');
    downloadPathHint.className = 'project-download-hint';
    downloadPathHint.textContent = project.download_path 
      ? `📥 ${project.download_path}` 
      : `📥 ${project.path} (default)`;
    downloadPathHint.title = 'Context files will be saved here';
    li.appendChild(downloadPathHint);

    const selectorDiv = document.createElement('div');
    selectorDiv.className = 'project-item-env-selector';
    const select = document.createElement('select');
    select.dataset.projectId = project.id;
    
    state.environments.forEach(env => {
      const option = new Option(`${env.icon} ${env.name}`, env.id);
      select.add(option);
    });
    
    // Ustawia wybraną wartość na podstawie danych z backendu lub domyślną
    const defaultEnvId = state.environments.find(e => e.is_default)?.id || 1;
    select.value = project.environmentId || defaultEnvId;

    select.addEventListener('click', (e) => e.stopPropagation());
    select.addEventListener('change', (e) => {
      e.stopPropagation();
      handleAssignEnvironment(project.id, e.target.value);
    });
    
    selectorDiv.appendChild(select);
    li.appendChild(selectorDiv);
    
    projectListEl.appendChild(li);
  });
  
  removeProjectBtn.disabled = !state.selectedProjectPath;
}

function setupProjectListeners() {
  projectListEl.addEventListener('click', (event) => {
    const item = event.target.closest('.project-item');
    if (!item) return;

    const path = item.dataset.path;

    if (event.target.closest('.btn-download-folder')) {
      const projectId = event.target.dataset.projectId;
      handleSelectProjectDownloadFolder(parseInt(projectId));
      return;
    }
    
    if (event.target.closest('.btn-configure')) {
        showProjectSettingsModal(path);
    } else if (!event.target.closest('select')) {
      state.selectedProjectPath = (state.selectedProjectPath === path) ? null : path;
      renderProjects();
    }
  });

  addProjectBtn.addEventListener('click', async () => {
    try {
      const selectedPath = await open({ 
        directory: true, 
        multiple: false,
        title: 'Select Project Folder'
      });
      
      if (selectedPath && !state.projects.some(p => p.path === selectedPath)) {
        await invoke("add_project", { path: selectedPath });
        await loadProjects();
        renderProjects();
        showToast("Project added successfully", "success");
      }
    } catch (err) {
      console.error('Error adding project:', err);
      showToast(`Failed to add project: ${err}`, "error");
    }
  });

  removeProjectBtn.addEventListener('click', async () => {
    if (!state.selectedProjectPath) return;
    const confirmed = await confirm(`Remove project: ${state.selectedProjectPath}?`, {
      title: 'Confirm Deletion',
      kind: 'warning'
    });
    if (confirmed) {
      try {
        await invoke("remove_project", { path: state.selectedProjectPath });
        state.selectedProjectPath = null;
        await loadProjects();
        renderProjects();
        showToast("Project removed", "success");
      } catch (err) {
        showToast(`Failed to remove project: ${err}`, "error");
      }
    }
  });
}

async function handleSelectProjectDownloadFolder(projectId) {
  const project = state.projects.find(p => p.id === projectId);
  if (!project) return;
  
  try {
    const selectedPath = await invoke('select_download_folder');
    if (selectedPath) {
      await invoke('set_project_download_path', { 
        projectId: projectId, 
        downloadPath: selectedPath 
      });
      await loadProjects();
      renderProjects();
      showToast('Download folder updated for project', 'success');
    }
  } catch (err) {
    if (err !== 'No folder selected') {
      console.error('Failed to set project download folder:', err);
      showToast(`Failed to set download folder: ${err}`, 'error');
    }
  }
}

async function handleAssignEnvironment(projectId, envId) {
  try {
    await invoke("assign_project_environment", { 
      projectId: parseInt(projectId), 
      envId: parseInt(envId) 
    });
    showToast("Environment assigned", "success");
  } catch (err) {
    showToast(`Failed to assign environment: ${err}`, "error");
  }
}

// === ENVIRONMENTS LOGIC ===
async function loadEnvironments() {
  try {
    state.environments = await invoke("get_environments");
  } catch (error) {
    showToast(`Failed to load environments: ${error}`, 'error');
  }
}

function renderEnvironments() {
  environmentsListEl.innerHTML = '';
  state.environments.forEach(env => {
    const li = document.createElement('li');
    // Dodajemy klasy do głównego elementu listy
    li.className = 'environment-item neumorphic-raised-sm interactive-lift';
    li.dataset.envId = env.id;
    if (env.id === state.selectedEnvironmentId) {
      li.classList.add('active');
    }
    
    // ZMIANA: Zaktualizowany HTML dla przycisków z nowymi klasami
    li.innerHTML = `
      <span class="env-icon">${env.icon}</span>
      <span class="env-name">${env.name}</span>
      <span class="env-lang-badge">${env.language}</span>
      ${env.is_default ? '<span class="env-badge">Default</span>' : ''}
      <div class="env-actions">
        <button class="env-action-btn edit-btn neumorphic-raised-sm interactive-lift" title="Edit Environment" data-env-id="${env.id}">✏️</button>
        <button class="env-action-btn delete-btn neumorphic-raised-sm interactive-lift" title="Delete Environment" data-env-id="${env.id}" ${env.is_default ? 'disabled title="Cannot delete default environment"' : ''}>🗑️</button>
      </div>
    `;
    environmentsListEl.appendChild(li);
  });
}

async function handleEnvironmentSelect(envId) {
  state.selectedEnvironmentId = envId;
  const selectedEnv = state.environments.find(e => e.id === envId);
  promptsHeaderEl.textContent = selectedEnv ? `Prompts for ${selectedEnv.name}` : 'Prompts';
  addPromptBtn.disabled = !selectedEnv;
  renderEnvironments();
  if (envId) {
    await loadPrompts(envId);
    renderPrompts();
  } else {
    state.prompts = [];
    renderPrompts();
  }
}

function setupEnvironmentListeners() {
  environmentsListEl.addEventListener('click', (e) => {
    const target = e.target;
    const envItem = target.closest('.environment-item');
    if (!envItem) return;
    
    const envId = parseInt(envItem.dataset.envId);

    if (target.closest('.edit-btn')) {
      showEnvModal('edit', envId);
    } else if (target.closest('.delete-btn')) {
      handleDeleteEnvironment(envId);
    } else {
      handleEnvironmentSelect(envId);
    }
  });

  addEnvBtn.addEventListener('click', () => showEnvModal('create'));
}

async function handleDeleteEnvironment(envId) {
  const env = state.environments.find(e => e.id === envId);
  if (!env || env.is_default) return;
  const confirmed = await confirm(`Are you sure you want to delete the "${env.name}" environment?`, {
    title: 'Confirm Deletion',
    kind: 'warning'
  });
  if (confirmed) {
    try {
      await invoke('delete_environment', { id: envId });
      showToast('Environment deleted', 'success');
      if (state.selectedEnvironmentId === envId) {
        state.selectedEnvironmentId = null;
        state.prompts = [];
        renderPrompts();
      }
      await loadEnvironments();
      renderEnvironments();
    } catch (err) {
      showToast(`Error: ${err}`, 'error');
    }
  }
}

// === PROMPTS LOGIC ===
async function loadPrompts(envId) {
  try {
    promptsContainerEl.innerHTML = `<p class="placeholder">Loading prompts...</p>`;
    state.prompts = await invoke("get_prompts", { envId });
  } catch (error) {
    showToast(`Failed to load prompts: ${error}`, 'error');
    state.prompts = [];
  }
}

function renderPrompts() {
  if (state.prompts.length === 0) {
    const message = state.selectedEnvironmentId ? "This environment has no prompts. Add one!" : "Select an environment to see its prompts.";
    promptsContainerEl.innerHTML = `<p class="placeholder">${message}</p>`;
    return;
  }
  promptsContainerEl.innerHTML = '';
  state.prompts.forEach(prompt => {
    const div = document.createElement('div');
    // Dodajemy klasy do głównego elementu
    div.className = 'prompt-item neumorphic-raised-sm';
    div.dataset.promptId = prompt.id;
    
    // ZMIANA: Zaktualizowany HTML dla przycisku edycji
    div.innerHTML = `
      <span class="prompt-drag-handle">☰</span>
      <div class="prompt-info">
        <div class="prompt-header">
          <span class="prompt-name">${prompt.name}</span>
          <span class="prompt-category-badge" data-category="${prompt.category}">${prompt.category}</span>
        </div>
        <pre class="prompt-content">${prompt.content || ''}</pre>
      </div>
      <div class="prompt-actions">
        <button class="env-action-btn edit-btn neumorphic-raised-sm interactive-lift edit-prompt-btn" title="Edit Prompt" data-prompt-id="${prompt.id}">✏️</button>
      </div>
    `;
    promptsContainerEl.appendChild(div);
  });
}

function setupPromptListeners() {
  addPromptBtn.addEventListener('click', () => {
    if(state.selectedEnvironmentId) {
      showPromptModal('create');
    }
  });

  promptsContainerEl.addEventListener('click', (e) => {
    const button = e.target.closest('.edit-prompt-btn');
    if (button) {
      const promptId = parseInt(button.dataset.promptId);
      showPromptModal('edit', promptId);
    }
  });
}

// === FONT SIZE CONTROL ===
function setupTextareaFontControl() {
  console.log('Setting up textarea font control...');
  
  const textareaIds = ['prompt-content', 'env-system-prompt', 'env-environment-prompt'];
  
  textareaIds.forEach(id => {
    const textarea = document.getElementById(id);
    console.log(`Textarea ${id}:`, textarea ? 'Found' : 'NOT FOUND');
    
    if (!textarea) return;
    
    // Initialize font size data attribute
    textarea.dataset.fontSize = '12';
    
    textarea.addEventListener('wheel', (e) => {
      console.log('Wheel event detected, Ctrl pressed:', e.ctrlKey);
      
      if (e.ctrlKey) {
        e.preventDefault();
        e.stopPropagation();
        
        // Get current font size from data attribute
        let currentSize = parseInt(textarea.dataset.fontSize) || 12;
        console.log('Current size:', currentSize);
        
        // Scroll up = increase, scroll down = decrease
        const delta = e.deltaY < 0 ? 1 : -1;
        currentSize = Math.max(8, Math.min(24, currentSize + delta));
        console.log('New size:', currentSize);
        
        // Update both style and data attribute
        textarea.style.fontSize = `${currentSize}px`;
        textarea.dataset.fontSize = currentSize;
      }
    }, { passive: false });
    
    console.log(`Font control attached to ${id}`);
  });
}

// === MODALS LOGIC ===
function setupModalListeners() {
  const envModal = document.getElementById('env-modal');
  const envForm = document.getElementById('env-form');
  document.getElementById('env-modal-cancel').addEventListener('click', () => envModal.style.display = 'none');
  envModal.addEventListener('click', (e) => { if (e.target === envModal) envModal.style.display = 'none'; });
  
  envForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    const id = document.getElementById('env-id-input').value;
    const isEdit = !!id;

    try {
      const language = document.getElementById('env-language').value;
      if (isEdit) {
        const payload = {
          id: parseInt(id),
          name: document.getElementById('env-name').value,
          icon: document.getElementById('env-icon').value || '🌐',
          systemPromptContent: document.getElementById('env-system-prompt').value,
          envPromptContent: document.getElementById('env-environment-prompt').value,
          language: language,
        };
        await invoke('update_environment', { payload });
      } else {
        const payload = {
          name: document.getElementById('env-name').value,
          icon: document.getElementById('env-icon').value || '🌐',
          systemPromptContent: document.getElementById('env-system-prompt').value,
          envPromptContent: document.getElementById('env-environment-prompt').value,
          language: language,
        };
        await invoke('create_environment', { payload });
      }
      showToast(`Environment ${isEdit ? 'updated' : 'created'}`, 'success');
      envModal.style.display = 'none';
      await loadAllData();
      if(isEdit) {
        handleEnvironmentSelect(parseInt(id));
      }
    } catch(err) {
      showToast(`Error: ${err}`, 'error');
    }
  });

  // Prompt Modal
  const promptModal = document.getElementById('prompt-modal');
  const promptForm = document.getElementById('prompt-form');
  document.getElementById('prompt-modal-cancel').addEventListener('click', () => promptModal.style.display = 'none');
  promptModal.addEventListener('click', (e) => { if (e.target === promptModal) promptModal.style.display = 'none'; });

  promptForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    const id = document.getElementById('prompt-id-input').value;
    const isEdit = !!id;
    try {
      if (isEdit) {
        const payload = {
          id: parseInt(id),
          name: document.getElementById('prompt-name').value,
          content: document.getElementById('prompt-content').value,
          category: document.getElementById('prompt-category').value,
          sortOrder: 0,
        };
        await invoke('update_prompt', { payload });
      } else {
          const payload = {
            environmentId: state.selectedEnvironmentId,
            name: document.getElementById('prompt-name').value,
            content: document.getElementById('prompt-content').value,
            category: document.getElementById('prompt-category').value,
            enabledByDefault: document.getElementById('prompt-enabled-default').checked,
          };
          await invoke('create_prompt', { payload });
      }
      showToast(`Prompt ${isEdit ? 'updated' : 'created'}`, 'success');
      promptModal.style.display = 'none';
      await handleEnvironmentSelect(state.selectedEnvironmentId);
    } catch(err) {
      showToast(`Error: ${err}`, 'error');
    }
  });

  document.getElementById('prompt-modal-delete').addEventListener('click', async () => {
    const id = document.getElementById('prompt-id-input').value;
    if (!id) return;
    
    const confirmed = await confirm('Are you sure you want to delete this prompt?', {
      title: 'Confirm Deletion',
      kind: 'warning'
    });
    if (confirmed) {
      try {
        await invoke('delete_prompt', { id: parseInt(id) });
        showToast('Prompt deleted', 'success');
        document.getElementById('prompt-modal').style.display = 'none';
        await handleEnvironmentSelect(state.selectedEnvironmentId);
      } catch(err) {
        showToast(`Error: ${err}`, 'error');
      }
    }
  });

  // Emoji Picker Modal
  const emojiPickerModal = document.getElementById('emoji-picker-modal');
  document.getElementById('emoji-picker-cancel').addEventListener('click', () => emojiPickerModal.style.display = 'none');
  emojiPickerModal.addEventListener('click', (e) => { if (e.target === emojiPickerModal) emojiPickerModal.style.display = 'none'; });

  document.getElementById('emoji-preview-btn').addEventListener('click', () => {
    const currentEmoji = document.getElementById('env-icon').value || '🌐';
    showEmojiPicker(currentEmoji);
  });

  projectSettingsModalSetup();
  initVectorMapListeners();
}

async function showEnvModal(mode, envId = null) {
  const modal = document.getElementById('env-modal');
  const title = document.getElementById('env-modal-title');
  const form = document.getElementById('env-form');
  const idInput = document.getElementById('env-id-input');
  const languageSelect = document.getElementById('env-language');
  form.reset();
  
  // Reset font sizes
  document.getElementById('env-system-prompt').style.fontSize = '12px';
  document.getElementById('env-system-prompt').dataset.fontSize = '12';
  document.getElementById('env-environment-prompt').style.fontSize = '12px';
  document.getElementById('env-environment-prompt').dataset.fontSize = '12';
  let selectedIcon = '🌐';
  if (mode === 'edit' && envId) {
      const env = state.environments.find(e => e.id === envId);
      if (!env) return;
      selectedIcon = env.icon;
      languageSelect.value = env.language || 'en';
      const prompts = await invoke("get_prompts", { envId });
      const systemPrompt = prompts.find(p => p.category === 'system');
      const environmentPrompt = prompts.find(p => p.category === 'environment');

      title.textContent = 'Edit Environment';
      idInput.value = env.id;
      document.getElementById('env-name').value = env.name;
      document.getElementById('env-icon').value = env.icon;
      document.getElementById('env-system-prompt').value = systemPrompt?.content || '';
      document.getElementById('env-environment-prompt').value = environmentPrompt?.content || '';
  } else {
      title.textContent = 'Create Environment';
      idInput.value = '';
      languageSelect.value = 'en';
  }
  
  initEmojiPreview(selectedIcon);
  modal.style.display = 'flex';
}

function showPromptModal(mode, promptId = null) {
  const modal = document.getElementById('prompt-modal');
  const title = document.getElementById('prompt-modal-title');
  const form = document.getElementById('prompt-form');
  const idInput = document.getElementById('prompt-id-input');
  const deleteBtn = document.getElementById('prompt-modal-delete');

  form.reset();
  
  // Reset font size
  document.getElementById('prompt-content').style.fontSize = '12px';
  document.getElementById('prompt-content').dataset.fontSize = '12';
  
  if (mode === 'edit' && promptId) {
    const prompt = state.prompts.find(p => p.id === promptId);
    if (!prompt) return;

    title.textContent = 'Edit Prompt';
    idInput.value = prompt.id;
    document.getElementById('prompt-name').value = prompt.name;
    document.getElementById('prompt-content').value = prompt.content || '';
    document.getElementById('prompt-category').value = prompt.category;
    document.getElementById('prompt-enabled-default').checked = prompt.enabled_by_default;

    const canDelete = prompt.category === 'custom';
    deleteBtn.style.display = canDelete ? 'block' : 'none';

    // Zastąp na:
    deleteBtn.style.display = 'block';
  } else {
    title.textContent = 'Create Prompt';
    idInput.value = '';
    deleteBtn.style.display = 'none';
    document.getElementById('prompt-category').value = 'custom';
    document.getElementById('prompt-enabled-default').checked = true;
  }
  modal.style.display = 'flex';
}

// === UTILS ===
function initEmojiPreview(selectedEmoji = '🌐') {
  const emojiPreview = document.getElementById('selected-emoji');
  const emojiInput = document.getElementById('env-icon');
  
  emojiInput.value = selectedEmoji;
  emojiPreview.textContent = selectedEmoji;
}

function showEmojiPicker(currentEmoji = '🌐') {
  const modal = document.getElementById('emoji-picker-modal');
  const emojiGrid = document.getElementById('emoji-grid');
  
  emojiGrid.innerHTML = '';
  
  EMOJI_PRESETS.forEach(emoji => {
    const option = document.createElement('div');
    option.className = 'emoji-option';
    option.textContent = emoji;
    
    if (emoji === currentEmoji) {
      option.classList.add('selected');
    }
    
    option.addEventListener('click', () => {
      const emojiPreview = document.getElementById('selected-emoji');
      const emojiInput = document.getElementById('env-icon');
      
      emojiPreview.textContent = emoji;
      emojiInput.value = emoji;
      
      modal.style.display = 'none';
    });
    
    emojiGrid.appendChild(option);
  });
  
  modal.style.display = 'flex';
}

function showToast(message, type = 'success') {
  toastEl.textContent = message;
  toastEl.className = `toast show ${type}`;
  setTimeout(() => {
    toastEl.className = 'toast';
  }, 3000);
}

// === PROJECT SETTINGS MODAL LOGIC ===

let currentProjectExclusions = {
    activeDefaults: new Set(),
    customExclusions: new Set(),
};
let currentProjectAllowedExtensions = [];

function updateDirectoryTreeState() {
    const treeContainer = document.getElementById('directory-tree-container');
    if (!treeContainer) return;

    // Znajdź wszystkie checkboxy w drzewie
    const allCheckboxes = treeContainer.querySelectorAll('.tree-checkbox');

    allCheckboxes.forEach(checkbox => {
        const nodeName = checkbox.dataset.name;
        const label = checkbox.closest('.tree-label');

        // Sprawdź, czy nazwa tego folderu jest na liście aktywnych domyślnych wykluczeń
        if (currentProjectExclusions.activeDefaults.has(nodeName)) {
            // Jeśli tak, wyłącz checkbox i dodaj styl "pokryte przez domyślne"
            checkbox.disabled = true;
            if (label) label.classList.add('covered-by-default');
        } else {
            // Jeśli nie, włącz checkbox i usuń specjalny styl
            checkbox.disabled = false;
            if (label) label.classList.remove('covered-by-default');
        }
    });
}

async function showProjectSettingsModal(projectPath) {
    state.currentEditingProjectPath = projectPath;
    const project = state.projects.find(p => p.path === projectPath);
    
    // LOG: Sprawdź, na jakich danych operuje modal
    console.log('[JS LOG] Opening modal. Project data from state:', JSON.parse(JSON.stringify(project)));

    if (!project) return;

    const modal = document.getElementById('project-settings-modal');
    document.getElementById('project-settings-modal-path').textContent = projectPath;

    // Reset tabs to default view
    modal.querySelectorAll('.modal-tab-link').forEach(btn => btn.classList.remove('active'));
    modal.querySelectorAll('.modal-tab-content').forEach(content => content.classList.remove('active'));
    modal.querySelector('.modal-tab-link[data-tab="exclusions-tab"]').classList.add('active');
    modal.querySelector('#exclusions-tab').classList.add('active');

    // === EXCLUSIONS LOGIC ===
    try {
        state.defaultExclusions = await invoke('get_default_exclusions');
    } catch (err) {
        console.error('Failed to load default exclusions:', err);
        state.defaultExclusions = ["node_modules", ".git", "target", "dist", "build"];
    }
    // POPRAWKA 1: Użyj project.excludedPaths
    const projectExclusions = (project.excludedPaths && project.excludedPaths !== 'null') 
        ? JSON.parse(project.excludedPaths) 
        : [];
    currentProjectExclusions.activeDefaults = new Set(
        projectExclusions.filter(ex => state.defaultExclusions.includes(ex))
    );
    currentProjectExclusions.customExclusions = new Set(
        projectExclusions.filter(ex => !state.defaultExclusions.includes(ex))
    );
    renderDefaultExclusions();
    await loadDirectoryTree(projectPath);

    // === EXTENSIONS LOGIC ===
    // POPRAWKA 2: Użyj project.allowedExtensions
    currentProjectAllowedExtensions = (project.allowedExtensions && project.allowedExtensions !== 'null')
        ? JSON.parse(project.allowedExtensions)
        : [...MASTER_EXTENSIONS_LIST];
    
    await loadExtensionTemplates();
    renderAllowedExtensions();
    renderExtensionTemplates();

    // === VECTOR MAP LOGIC ===
    const currentMapId = project.vectorMapId || 1; // Default to map 1
    state.currentVectorMapId = currentMapId;
    await populateVectorMapSelector(currentMapId);

    modal.style.display = 'flex';
}

function renderDefaultExclusions() {
    const container = document.getElementById('default-exclusions-list');
    container.innerHTML = '';
    
    if (state.defaultExclusions.length === 0) {
        container.innerHTML = '<p class="empty-message">No default exclusions defined.</p>';
        return;
    }

    state.defaultExclusions.forEach(ex => {
        const isActive = currentProjectExclusions.activeDefaults.has(ex);
        const tag = document.createElement('div');
        // ZMIANA: Dodajemy klasę 'exclusion-tag' zamiast 'extension-tag'
        tag.className = `exclusion-tag ${isActive ? 'active' : ''}`;
        tag.innerHTML = `<span>${ex}</span>`;
        tag.dataset.exclusion = ex;
        
        tag.addEventListener('click', () => {
            if (currentProjectExclusions.activeDefaults.has(ex)) {
                currentProjectExclusions.activeDefaults.delete(ex);
            } else {
                currentProjectExclusions.activeDefaults.add(ex);
            }
            renderDefaultExclusions();
            updateDirectoryTreeState();
        });

        container.appendChild(tag);
    });
}

function handleAddDefaultExclusion() {
    const input = document.getElementById('new-default-exclusion-input');
    const value = input.value.trim();
    if (!value) return;

    if (state.defaultExclusions.includes(value)) {
        showToast('This default already exists.', 'error');
        return;
    }

    state.defaultExclusions.push(value);
    
    invoke('set_default_exclusions', { exclusions: state.defaultExclusions })
        .then(() => {
            renderDefaultExclusions();
            input.value = '';
            input.focus();
            showToast('Default exclusion added', 'success');
        })
        .catch(err => {
            showToast(`Failed to add default: ${err}`, 'error');
        });
}

async function loadDirectoryTree(projectPath) {
    const treeContainer = document.getElementById('directory-tree-container');
    treeContainer.innerHTML = '<p class="placeholder">Loading directory tree...</p>';

    try {
        const allExclusions = [
            ...Array.from(currentProjectExclusions.activeDefaults),
            ...Array.from(currentProjectExclusions.customExclusions)
        ];

        const tree = await invoke('get_directory_tree', {
            rootPath: projectPath,
            excludedPaths: allExclusions,
            maxDepth: 10
        });

        if (tree.length === 0) {
            treeContainer.innerHTML = '<p class="empty-message">No subdirectories found</p>';
            return;
        }

        treeContainer.innerHTML = '';
        renderDirectoryTree(tree, treeContainer);
    } catch (err) {
        treeContainer.innerHTML = `<p class="error-message">Failed to load tree: ${err}</p>`;
        console.error('Error loading directory tree:', err);
    }
}

function renderDirectoryTree(nodes, container, level = 0) {
    nodes.forEach(node => {
        const hasChildren = node.children.length > 0;
        const nodeDiv = document.createElement('div');
        nodeDiv.className = 'tree-node';
        if (hasChildren) {
            nodeDiv.classList.add('collapsed'); // Domyślnie zwinięty
        }
        nodeDiv.style.paddingLeft = `${level * 16}px`;

        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.className = 'tree-checkbox';
        checkbox.dataset.path = node.path;
        checkbox.dataset.name = node.name;

        const isCustomExcluded = currentProjectExclusions.customExclusions.has(node.name);
        const isDefaultExcluded = currentProjectExclusions.activeDefaults.has(node.name);
        
        checkbox.checked = isCustomExcluded;
        checkbox.disabled = isDefaultExcluded;

        checkbox.addEventListener('change', (e) => {
            if (e.target.checked) {
                currentProjectExclusions.customExclusions.add(node.name);
            } else {
                currentProjectExclusions.customExclusions.delete(node.name);
            }
        });

        const label = document.createElement('label');
        label.className = 'tree-label';
        if (isDefaultExcluded) {
            label.classList.add('covered-by-default');
            label.title = 'Excluded by active default filter';
        }
        
        const folderIcon = document.createElement('span');
        folderIcon.className = 'tree-icon';
        
        if (hasChildren) {
            const toggleIcon = document.createElement('span');
            toggleIcon.className = 'tree-toggle';
            toggleIcon.textContent = '▶'; // Ikona zwiniętego
            label.appendChild(toggleIcon);
        } else {
            const placeholder = document.createElement('span');
            placeholder.className = 'tree-toggle-placeholder';
            label.appendChild(placeholder);
        }

        folderIcon.textContent = '📁';

        const folderName = document.createElement('span');
        folderName.textContent = node.name;
        folderName.className = 'tree-name';

        label.appendChild(checkbox);
        label.appendChild(folderIcon);
        label.appendChild(folderName);
        nodeDiv.appendChild(label);

        container.appendChild(nodeDiv);

        let childContainer = null;
        if (hasChildren) {
            childContainer = document.createElement('div');
            childContainer.className = 'tree-children-wrapper';
            childContainer.style.display = 'none'; // Domyślnie ukryty
            container.appendChild(childContainer);
            renderDirectoryTree(node.children, childContainer, level + 1);
        }

        label.addEventListener('click', (e) => {
            // Krok 1: Sprawdź, co zostało kliknięte.
            if (e.target.type === 'checkbox') {
                // Jeśli kliknięto bezpośrednio w checkbox, pozwól przeglądarce
                // na jego domyślne zachowanie (zaznaczenie/odznaczenie).
                // Nie robimy nic więcej, aby nie zakłócać tego procesu.
                return;
            }

            // Krok 2: Jeśli kliknięto w cokolwiek innego w etykiecie (tekst, ikonę),
            // WTEDY zatrzymaj domyślną akcję, aby nie zaznaczyć checkboxa.
            e.preventDefault();
            
            // Krok 3: Wykonaj logikę rozwijania/zwijania.
            if (hasChildren && childContainer) {
                const isCollapsed = childContainer.style.display === 'none';
                childContainer.style.display = isCollapsed ? 'block' : 'none';
                nodeDiv.classList.toggle('collapsed', !isCollapsed);
                const toggleIcon = label.querySelector('.tree-toggle');
                if (toggleIcon) {
                    toggleIcon.textContent = isCollapsed ? '▼' : '▶';
                }
            }
        });
    });
}

function renderAllowedExtensions() {
    const container = document.getElementById('allowed-extensions-list');
    container.innerHTML = '';
    
    MASTER_EXTENSIONS_LIST.forEach(ext => {
        const isActive = currentProjectAllowedExtensions.includes(ext);
        const tag = document.createElement('div');
        tag.className = `extension-tag ${isActive ? 'active' : ''}`;
        tag.textContent = `.${ext}`;
        tag.dataset.extension = ext;
        
        tag.addEventListener('click', () => {
            const index = currentProjectAllowedExtensions.indexOf(ext);
            if (index > -1) {
                currentProjectAllowedExtensions.splice(index, 1);
            } else {
                currentProjectAllowedExtensions.push(ext);
            }
            renderAllowedExtensions();
        });
        container.appendChild(tag);
    });
}

function renderExtensionTemplates() {
    const container = document.getElementById('extension-templates-list');
    container.innerHTML = '';

    if (state.extensionTemplates.length === 0) {
        container.innerHTML = '<p class="empty-message">No templates saved yet.</p>';
        return;
    }

    state.extensionTemplates.forEach(template => {
        const item = document.createElement('div');
        item.className = 'template-item';
        item.innerHTML = `<span class="template-name">${template.name}</span>
                          <button class="template-delete-btn" data-id="${template.id}" title="Delete template">🗑️</button>`;
        
        item.addEventListener('click', (e) => {
            if (e.target.classList.contains('template-delete-btn')) {
                e.stopPropagation();
                handleDeleteExtensionTemplate(template.id, template.name);
            } else {
                currentProjectAllowedExtensions = JSON.parse(template.extensions);
                renderAllowedExtensions();
                showToast(`Template "${template.name}" applied.`, 'success');
            }
        });
        container.appendChild(item);
    });
}

async function handleSaveExtensionTemplate() {
    const input = document.getElementById('new-template-name-input');
    const name = input.value.trim();
    if (!name) {
        showToast('Template name cannot be empty.', 'error');
        return;
    }
    if (state.extensionTemplates.some(t => t.name.toLowerCase() === name.toLowerCase())) {
        showToast('A template with this name already exists.', 'error');
        return;
    }
    if (currentProjectAllowedExtensions.length === 0) {
        showToast('Cannot save an empty selection as a template.', 'error');
        return;
    }

    try {
        await invoke('create_extension_template', {
            name,
            extensions: currentProjectAllowedExtensions
        });
        showToast('Template saved successfully.', 'success');
        input.value = '';
        await loadExtensionTemplates();
        renderExtensionTemplates();
    } catch (err) {
        showToast(`Failed to save template: ${err}`, 'error');
    }
}

async function handleDeleteExtensionTemplate(id, name) {
    const confirmed = await confirm(`Are you sure you want to delete the template "${name}"?`, {
        title: 'Confirm Deletion',
        kind: 'warning'
    });
    if (confirmed) {
        try {
            await invoke('delete_extension_template', { id });
            showToast(`Template "${name}" deleted.`, 'success');
            await loadExtensionTemplates();
            renderExtensionTemplates();
        } catch (err) {
            showToast(`Failed to delete template: ${err}`, 'error');
        }
    }
}

async function handleSaveProjectSettings() {
    if (!state.currentEditingProjectPath) return;

    const finalExclusions = [
        ...Array.from(currentProjectExclusions.activeDefaults),
        ...Array.from(currentProjectExclusions.customExclusions)
    ];
    const finalExtensions = currentProjectAllowedExtensions;

    try {
        // Save vector map ID if changed
        if (state.currentVectorMapId) {
            await invoke('update_project_vector_map', {
                projectPath: state.currentEditingProjectPath,
                vectorMapId: state.currentVectorMapId
            });
        }

        // Krok 1: Wywołaj komendę i odbierz zaktualizowany projekt
        const updatedProject = await invoke('update_project_settings', {
            path: state.currentEditingProjectPath,
            excludedPaths: finalExclusions,
            allowedExtensions: finalExtensions,
        });

        console.log('[JS LOG] Received updated project data from Rust:', updatedProject);

        // Krok 2: Znajdź i zaktualizuj projekt w stanie lokalnym
        const projectIndex = state.projects.findIndex(p => p.path === updatedProject.path);
        if (projectIndex !== -1) {
            state.projects[projectIndex] = updatedProject;
            console.log('[JS LOG] Local project state updated directly from Rust response.');
        } else {
            // Mało prawdopodobne, ale zabezpiecza przed błędem
            state.projects.push(updatedProject);
        }

        showToast('Project settings updated. Cache cleared.', 'success');
        document.getElementById('project-settings-modal').style.display = 'none';
        
        // Krok 3: Przerenderuj listę projektów z nowymi danymi
        renderProjects();

    } catch (err) {
        console.error('[JS LOG] Error invoking `update_project_settings`:', err);
        showToast(`Failed to save settings: ${err}`, 'error');
    }
}
const projectSettingsModalSetup = () => {
    const modal = document.getElementById('project-settings-modal');
    document.getElementById('project-settings-modal-cancel').addEventListener('click', () => modal.style.display = 'none');
    modal.addEventListener('click', (e) => { if (e.target === modal) modal.style.display = 'none'; });

    document.getElementById('add-default-exclusion-btn').addEventListener('click', handleAddDefaultExclusion);
    document.getElementById('new-default-exclusion-input').addEventListener('keydown', (e) => {
        if (e.key === 'Enter') { e.preventDefault(); handleAddDefaultExclusion(); }
    });
    document.getElementById('project-settings-modal-save').addEventListener('click', handleSaveProjectSettings);
    document.getElementById('save-extension-template-btn').addEventListener('click', handleSaveExtensionTemplate);

    modal.querySelectorAll('.modal-tab-link').forEach(button => {
        button.addEventListener('click', () => {
            const tabId = button.dataset.tab;
            modal.querySelectorAll('.modal-tab-link').forEach(btn => btn.classList.remove('active'));
            modal.querySelectorAll('.modal-tab-content').forEach(content => content.classList.remove('active'));
            button.classList.add('active');
            modal.querySelector(`#${tabId}`).classList.add('active');
        });
    });
};

// === AUDITOR UI LOGIC ===
let auditorState = {
    projects: new Map(),
    selectedProject: null,
    currentFiles: [],
    selectedFilePaths: new Set(),
    viewMode: 'projects', // 'projects' or 'files'
    projectHeader: null, // Store the header element
    lastReport: null // Store last audit report for re-rendering
};

function initAuditorUI() {
    const refreshBtn = document.getElementById('auditor-refresh-btn');
    const backBtn = document.getElementById('auditor-back-to-projects-btn');
    const runBtn = document.getElementById('run-audit-btn');
    const openReportBtn = document.getElementById('open-report-btn');
    const listContainer = document.getElementById('auditor-list-container');
    const reportContainer = document.getElementById('audit-report-container');

    // Load projects with their context files
    async function loadProjects() {
        listContainer.innerHTML = '<p class="placeholder">Scanning for context files...</p>';

        try {
            const backups = await invoke('get_available_backups');

            // Group by project
            auditorState.projects.clear();
            backups.forEach(backup => {
                if (!auditorState.projects.has(backup.projectPath)) {
                    auditorState.projects.set(backup.projectPath, {
                        name: backup.projectName,
                        path: backup.projectPath,
                        backups: []
                    });
                }
                auditorState.projects.get(backup.projectPath).backups.push(backup);
            });

            showProjectsList();
        } catch (err) {
            listContainer.innerHTML = `<p class="error-message">Failed to load projects: ${err}</p>`;
            showToast('Error loading projects for audit', 'error');
        }
    }

    function showProjectsList() {
        auditorState.viewMode = 'projects';
        auditorState.selectedProject = null;

        document.getElementById('auditor-detail-title').textContent = 'Select a Project';
        reportContainer.innerHTML = '<p class="placeholder">Select a project on the left to view files and run audit.</p>';
        runBtn.disabled = true;

        if (backBtn) backBtn.style.display = 'none';

        listContainer.innerHTML = '';

        if (auditorState.projects.size === 0) {
            listContainer.innerHTML = `
                <div class="empty-state">
                    <div class="empty-state-icon">📁</div>
                    <div class="empty-state-text">No projects with context files</div>
                    <div class="empty-state-hint">Generate context files first to enable auditing.</div>
                </div>`;
            return;
        }

        const projectsArray = Array.from(auditorState.projects.values());
        projectsArray.forEach(project => {
            const item = document.createElement('div');
            item.className = 'backup-project-item neumorphic-raised-sm interactive-lift';

            item.innerHTML = `
                <div class="project-item-header">
                    <span class="project-icon">📁</span>
                    <div class="project-info">
                        <div class="project-name">${project.name}</div>
                        <div class="project-stats">
                            ${project.backups.length} context file${project.backups.length !== 1 ? 's' : ''}
                        </div>
                    </div>
                    <span class="chevron">›</span>
                </div>
            `;

            item.addEventListener('click', () => {
                showFilesForProject(project);
            });

            listContainer.appendChild(item);
        });
    }

    function showFilesForProject(project) {
        auditorState.viewMode = 'files';
        auditorState.selectedProject = project;
        auditorState.selectedFilePaths.clear();

        document.getElementById('auditor-detail-title').innerHTML = `<span class="project-breadcrumb">${project.name}</span>`;
        reportContainer.innerHTML = '<p class="placeholder">Select files to audit and click "Run Audit".</p>';

        if (backBtn) backBtn.style.display = 'flex';

        listContainer.innerHTML = '';

        // Create and store header for later use
        auditorState.projectHeader = document.createElement('div');
        auditorState.projectHeader.className = 'backups-list-header';
        auditorState.projectHeader.innerHTML = `
            <div style="padding: 12px 16px; border-bottom: 2px solid var(--border-color); background: var(--bg-elevated);">
                <div style="font-weight: 600; font-size: 14px; color: var(--text-primary);">
                    ${project.name}
                </div>
                <div style="font-size: 11px; color: var(--text-muted); margin-top: 4px;">
                    ${project.backups.length} context file${project.backups.length !== 1 ? 's' : ''}
                </div>
            </div>
        `;
        listContainer.appendChild(auditorState.projectHeader);

        // Render context files
        project.backups.forEach(backup => {
            const item = document.createElement('div');
            item.className = 'backup-item neumorphic-raised-sm interactive-lift';

            const dateStr = backup.createdAt.replace(/_/g, ' ');
            const date = new Date(dateStr.substring(0, 4), dateStr.substring(4, 6) - 1, dateStr.substring(6, 8),
                                  dateStr.substring(9, 11), dateStr.substring(11, 13), dateStr.substring(13, 15));
            const timeAgo = getTimeAgo(date);

            item.innerHTML = `
                <div class="backup-item-content">
                    <span class="backup-icon">📄</span>
                    <div class="backup-info">
                        <div class="backup-date">${backup.filename}</div>
                        <div class="backup-meta">
                            ${timeAgo} • ${(backup.sizeBytes / 1024).toFixed(1)} KB
                        </div>
                    </div>
                </div>
            `;

            item.addEventListener('click', async () => {
                document.querySelectorAll('.backup-item').forEach(el => {
                    el.classList.remove('selected');
                });
                item.classList.add('selected');

                listContainer.innerHTML = '';
                listContainer.appendChild(auditorState.projectHeader);

                const placeholder = document.createElement('p');
                placeholder.className = 'placeholder';
                placeholder.textContent = 'Loading files from context...';
                listContainer.appendChild(placeholder);

                try {
                    const files = await invoke('preview_backup_content', { filepath: backup.filepath });
                    auditorState.currentFiles = files;
                    renderAuditorFilesList(files, backup.filepath);
                    runBtn.disabled = false;
                } catch (err) {
                    listContainer.innerHTML = `<p class="error-message">Failed to load files: ${err}</p>`;
                }
            });

            listContainer.appendChild(item);
        });
    }

    function renderAuditorFilesList(files, contextPath) {
        listContainer.innerHTML = '';
        if (auditorState.projectHeader) {
            listContainer.appendChild(auditorState.projectHeader);
        }

        const list = document.createElement('div');
        list.className = 'backup-files-list';

        // Select All checkbox
        const selectAllDiv = document.createElement('div');
        selectAllDiv.className = 'files-header';
        selectAllDiv.innerHTML = `
            <label class="select-all-label">
                <input type="checkbox" id="auditor-select-all" checked>
                <span>Select All (${files.length} files)</span>
            </label>
        `;
        list.appendChild(selectAllDiv);

        files.forEach(file => {
            const row = document.createElement('div');
            row.className = 'file-item neumorphic-inset-sm';

            row.innerHTML = `
                <div class="file-item-content">
                    <div class="file-main">
                        <input type="checkbox" class="auditor-file-check" data-path="${file.path}" checked>
                        <div class="file-details">
                            <div class="file-name">${file.path.split('/').pop()}</div>
                            <div class="file-path">${file.path}</div>
                        </div>
                    </div>
                </div>
            `;

            const cb = row.querySelector('.auditor-file-check');
            if (cb.checked) auditorState.selectedFilePaths.add(file.path);

            cb.addEventListener('change', (e) => {
                if(e.target.checked) auditorState.selectedFilePaths.add(file.path);
                else auditorState.selectedFilePaths.delete(file.path);
            });

            list.appendChild(row);
        });

        listContainer.appendChild(list);

        // Select All logic
        document.getElementById('auditor-select-all').addEventListener('change', (e) => {
            const checked = e.target.checked;
            document.querySelectorAll('.auditor-file-check').forEach(cb => {
                cb.checked = checked;
                if(checked) auditorState.selectedFilePaths.add(cb.dataset.path);
                else auditorState.selectedFilePaths.delete(cb.dataset.path);
            });
        });

        // Store context path for audit run
        listContainer.dataset.contextPath = contextPath;
    }

    // Event Listeners
    refreshBtn.addEventListener('click', loadProjects);
    backBtn.addEventListener('click', showProjectsList);

    // Run Audit
    runBtn.addEventListener('click', async () => {
        if (auditorState.selectedFilePaths.size === 0) {
            showToast('Please select at least one file to audit.', 'error');
            return;
        }

        const contextPath = listContainer.dataset.contextPath;
        const selectedFiles = Array.from(auditorState.selectedFilePaths);

        reportContainer.innerHTML = '<p class="placeholder">Running structural analysis (Tree-sitter)... This may take a moment.</p>';
        runBtn.disabled = true;
        runBtn.textContent = 'Running Analysis...';
        if (openReportBtn) openReportBtn.style.display = 'none';

        try {
            const report = await invoke('run_integrity_audit', {
                contextFilePath: contextPath,
                selectedFiles: selectedFiles
            });

            // Store report in state for later use
            auditorState.lastReport = report;
            renderReport(report);

            // Enable report opening
            if (openReportBtn) {
                openReportBtn.style.display = 'inline-block';
                openReportBtn.onclick = async () => {
                    try {
                        // Get HTML content for display in Gluon
                        const htmlContent = await invoke('get_audit_report_html', {
                            contextFilePath: contextPath,
                            selectedFiles: selectedFiles
                        });

                        // Display in iframe
                        reportContainer.innerHTML = `
                            <div style="width: 100%; height: 100%; position: relative;">
                                <button id="close-report-btn" style="position: absolute; top: 10px; right: 10px; z-index: 1000; padding: 8px 16px; background: #6366f1; color: white; border: none; border-radius: 6px; cursor: pointer; font-weight: 600;">
                                    ← Back to Audit
                                </button>
                                <iframe
                                    style="width: 100%; height: 100%; border: none; border-radius: 8px;"
                                    srcdoc="${htmlContent.replace(/"/g, '&quot;')}"
                                ></iframe>
                            </div>
                        `;

                        // Add close button handler
                        document.getElementById('close-report-btn').onclick = () => {
                            // Re-render the last report
                            if (auditorState.lastReport) {
                                renderReport(auditorState.lastReport);
                            }
                        };

                        showToast(`Certificate opened in Gluon`, 'success');

                        // Also save to Downloads in background
                        try {
                            await invoke('export_audit_report', {
                                contextFilePath: contextPath,
                                selectedFiles: selectedFiles
                            });
                        } catch (e) {
                            console.log("Could not save to Downloads:", e);
                        }
                    } catch (e) {
                        showToast(`Failed to open report: ${e}`, 'error');
                    }
                };
            }

        } catch (err) {
            reportContainer.innerHTML = `<p class="error-message">Audit Failed: ${err}</p>`;
        } finally {
            runBtn.disabled = false;
            runBtn.textContent = '🛡️ Run Audit';
        }
    });

    function renderReport(reportItems) {
        reportContainer.innerHTML = '';

        let issuesCount = 0;
        let criticalCount = 0;

        // Sort: Critical first, then Warning, then OK
        reportItems.sort((a, b) => {
            const score = s => s === 'CRITICAL' ? 3 : s === 'WARNING' ? 2 : 1;
            return score(b.status) - score(a.status);
        });

        reportItems.forEach(item => {
            if (item.status === 'OK') return;

            issuesCount += item.discrepancies.length;
            if (item.status === 'CRITICAL') criticalCount++;

            const card = document.createElement('div');
            card.className = `audit-file-card status-${item.status}`;

            let badgeClass = item.status === 'CRITICAL' ? 'critical' : 'warning';

            let issuesHtml = item.discrepancies.map((d, index) => {
                let icon = d.issueType === 'MISSING_SYMBOL' ? '🔴' :
                           d.issueType === 'SIGNATURE_MISMATCH' ? '⚠️' : '📉';

                return `
                    <li class="audit-issue-item" data-file-path="${item.filePath}" data-line-number="${d.lineNumber}" data-issue-index="${index}">
                        <div class="issue-header" style="cursor: pointer; display: flex; justify-content: space-between; align-items: center;">
                            <div style="display: flex; align-items: center; gap: 8px; flex: 1;">
                                <span class="issue-icon">${icon}</span>
                                <div class="issue-content" style="flex: 1;">
                                    <div><span class="issue-symbol">${d.symbolName}</span></div>
                                    <div class="issue-desc">${d.description} (Line ${d.lineNumber})</div>
                                </div>
                            </div>
                            <span class="issue-toggle" style="font-size: 12px; color: var(--text-muted);">▶</span>
                        </div>
                        <div class="issue-diff" style="display: none; margin-top: 12px; padding-top: 12px; border-top: 1px solid var(--border-color);">
                            <div class="diff-loading">Loading code fragment...</div>
                        </div>
                    </li>
                `;
            }).join('');

            card.innerHTML = `
                <div class="audit-file-header">
                    <span class="audit-file-name">${item.filePath.split('/').pop()}</span>
                    <span class="audit-badge ${badgeClass}">${item.status}</span>
                </div>
                <ul class="audit-issues-list">
                    ${issuesHtml}
                </ul>
            `;
            reportContainer.appendChild(card);

            // Add click handlers for expanding diff views
            card.querySelectorAll('.audit-issue-item').forEach(issueItem => {
                const header = issueItem.querySelector('.issue-header');
                const diffContainer = issueItem.querySelector('.issue-diff');
                const toggle = issueItem.querySelector('.issue-toggle');

                header.addEventListener('click', async () => {
                    const isExpanded = diffContainer.style.display === 'block';

                    if (isExpanded) {
                        // Collapse
                        diffContainer.style.display = 'none';
                        toggle.textContent = '▶';
                    } else {
                        // Expand and load diff if not loaded yet
                        diffContainer.style.display = 'block';
                        toggle.textContent = '▼';

                        const filePath = issueItem.dataset.filePath;
                        const lineNumber = parseInt(issueItem.dataset.lineNumber);

                        // Check if already loaded
                        if (diffContainer.querySelector('.diff-loading')) {
                            try {
                                const codeFragment = await invoke('get_code_fragment', {
                                    filePath: filePath,
                                    lineNumber: lineNumber,
                                    contextLines: 5
                                });

                                diffContainer.innerHTML = `
                                    <pre style="background: var(--bg-base); padding: 12px; border-radius: 6px; overflow-x: auto; font-family: 'Cascadia Code', 'Fira Code', monospace; font-size: 12px; line-height: 1.5;">
<code>${escapeHtml(codeFragment)}</code></pre>
                                `;
                            } catch (err) {
                                diffContainer.innerHTML = `<div style="color: var(--text-error); padding: 8px;">Failed to load code: ${err}</div>`;
                            }
                        }
                    }
                });
            });
        });

        if (issuesCount === 0) {
            reportContainer.innerHTML = `
                <div class="empty-state">
                    <div class="empty-state-icon">✅</div>
                    <div class="empty-state-text">No Regressions Detected</div>
                    <div class="empty-state-hint">All ${reportItems.length} files match their definitions in the context file.</div>
                </div>
            `;
        }
    }

    // Load projects when tab is opened
    document.querySelector('.tab-link[data-tab="auditor"]')?.addEventListener('click', () => {
        loadProjects();
    });

    // Initial load
    loadProjects();
}

// Initialize Workflow Palette Drag & Drop
function initAgentPalette() {
    console.log('[UI] Initializing Agent Palette Drag & Drop');
    const paletteItems = document.querySelectorAll('.palette-item');
    const dropzone = document.getElementById('graph-dropzone');

    paletteItems.forEach(item => {
        // Ensure draggable attribute is explicitly set
        item.setAttribute('draggable', 'true');

        item.addEventListener('dragstart', (e) => {
            const type = item.getAttribute('data-type');
            if (type) {
                e.dataTransfer.setData('type', type);
                e.dataTransfer.effectAllowed = 'copy';
                item.style.opacity = '0.5';

                console.log('[UI] Drag started for:', type);

                // [FIX] Enable "Pass-through" mode on the graph container
                // This makes the cytoscape canvas transparent to mouse events so the dropzone catches them
                if (dropzone) dropzone.classList.add('dragging-active');
            }
        });

        item.addEventListener('dragend', (e) => {
            item.style.opacity = '1';
            // [FIX] Disable "Pass-through" mode to restore graph interactivity
            if (dropzone) dropzone.classList.remove('dragging-active');
        });
    });
}

// Initialize Graph Canvas and Drop Logic
function initGraphCanvas() {
    console.log("[UI] initGraphCanvas called");

    // 1. Initialize Editor
    if (typeof GraphWorkflowEditor !== 'undefined') {
        window.graphEditor = new GraphWorkflowEditor('cy');

        const workflowsTabBtn = document.querySelector('.tab-link[data-tab="workflows"]');
        if (workflowsTabBtn) {
            workflowsTabBtn.addEventListener('click', () => {
                console.log("[UI] Workflows tab clicked, initializing editor...");
                setTimeout(() => {
                    if (!window.graphEditor.initialized) {
                        window.graphEditor.init();
                    } else {
                        window.graphEditor.fit();
                    }
                }, 200); // Increased delay slightly
            });
        }
    } else {
        console.error("[UI] GraphWorkflowEditor class is undefined!");
    }

    // 2. Setup Drop Zone
    const dropzone = document.getElementById('graph-dropzone');
    const placeholder = document.getElementById('canvas-placeholder');

    if (!dropzone) {
        console.error("[UI] Dropzone element #graph-dropzone not found!");
        return;
    }

    console.log("[UI] Dropzone listeners attached to:", dropzone);

    // DRAG ENTER
    dropzone.addEventListener('dragenter', (e) => {
        e.preventDefault();
        // console.log("[UI] Drag Enter"); // Uncomment for verbose logs
        dropzone.classList.add('drag-over');
    });

    // DRAG OVER - CRITICAL: Must prevent default to allow drop
    dropzone.addEventListener('dragover', (e) => {
        e.preventDefault(); 
        e.stopPropagation(); 
        e.dataTransfer.dropEffect = 'copy';

        // Ensure visual feedback class is present (sometimes dragenter misses)
        if (!dropzone.classList.contains('drag-over')) {
            dropzone.classList.add('drag-over');
        }
    });

    // DRAG LEAVE
    dropzone.addEventListener('dragleave', (e) => {
        // Prevent flickering when hovering over children
        if (e.target === dropzone) {
            // console.log("[UI] Drag Leave");
            dropzone.classList.remove('drag-over');
        }
    });

    // DROP - THE MOMENT OF TRUTH
    dropzone.addEventListener('drop', (e) => {
        e.preventDefault();
        e.stopPropagation();
        dropzone.classList.remove('drag-over');

        console.log("[UI] 🔥 DROP EVENT FIRED 🔥");
        console.log("[UI] Drop target:", e.target);

        const type = e.dataTransfer.getData('type');
        console.log("[UI] Dropped data type:", type);

        if (!type) {
            console.error("[UI] No 'type' data found in DataTransfer. Check dragstart handler.");
            return;
        }

        if (!window.graphEditor) {
            console.error("[UI] window.graphEditor instance is missing.");
            return;
        }

        if (!window.graphEditor.initialized) {
            console.error("[UI] Editor is not initialized yet (is Cytoscape loaded?).");
            // Attempt panic init
            window.graphEditor.init();
        }

        console.log("[UI] Calling addNodeAtEvent...");

        // Hide placeholder on first drop
        if (placeholder) placeholder.style.display = 'none';

        // Add node
        const nodeId = window.graphEditor.addNodeAtEvent(type, e);

        if (nodeId) {
            console.log(`[UI] ✅ Node created successfully: ${nodeId}`);
        } else {
            console.error("[UI] ❌ Failed to create node (addNodeAtEvent returned null).");
        }
    });

    // Wire up toolbar buttons
    document.getElementById('wf-layout-btn')?.addEventListener('click', () => {
        console.log("[UI] Layout button clicked");
        window.graphEditor?.layout();
    });

    document.getElementById('wf-fit-btn')?.addEventListener('click', () => {
        console.log("[UI] Fit button clicked");
        window.graphEditor?.fit();
    });

    document.getElementById('wf-clear-btn')?.addEventListener('click', () => {
        if(confirm('Clear entire workflow?')) {
            window.graphEditor?.clear();
            if (placeholder) placeholder.style.display = 'flex';
        }
    });
}

// ========================================
// Vector Map Management Functions
// ========================================

async function loadVectorMaps() {
    try {
        const response = await invoke('get_vector_maps');
        state.vectorMaps = response || [];
        console.log('[Vector Maps] Loaded', state.vectorMaps.length, 'maps');
        return state.vectorMaps;
    } catch (err) {
        console.error('[Vector Maps] Failed to load:', err);
        showToast('Failed to load vector maps: ' + err, 'error');
        return [];
    }
}

async function populateVectorMapSelector(currentMapId) {
    const select = document.getElementById('project-vector-map-select');
    if (!select) return;

    const maps = await loadVectorMaps();

    if (maps.length === 0) {
        select.innerHTML = '<option value="1">Default</option>';
        return;
    }

    select.innerHTML = maps.map(map =>
        `<option value="${map.id}" ${map.id === currentMapId ? 'selected' : ''}>
            ${escapeHtml(map.name)} (${map.total_chunks} chunks)
         </option>`
    ).join('');

    // Load stats for selected map
    if (currentMapId) {
        await updateVectorMapInfo(currentMapId);
    }
}

async function updateVectorMapInfo(mapId) {
    try {
        const stats = await invoke('get_vector_map_stats', { mapId: mapId });
        const sharedProjects = await invoke('get_shared_projects', { mapId: mapId });

        // Update UI elements
        document.getElementById('selected-map-name').textContent = stats.name || 'Unknown';
        document.getElementById('map-total-chunks').textContent = stats.total_chunks || 0;
        document.getElementById('map-total-files').textContent = stats.total_files || 0;
        document.getElementById('map-project-count').textContent = stats.projects_using || 0;

        const sizeMB = ((stats.size_bytes || 0) / 1048576).toFixed(2);
        document.getElementById('map-size-bytes').textContent = `${sizeMB} MB`;

        // Update shared projects list
        const sharedList = document.getElementById('shared-projects-list');
        const currentPath = state.currentEditingProjectPath;
        const otherProjects = sharedProjects.filter(p => p !== currentPath);

        if (otherProjects.length === 0) {
            sharedList.innerHTML = '<p class="placeholder">No other projects share this map</p>';
        } else {
            sharedList.innerHTML = otherProjects.map(path =>
                `<div class="shared-project-item">${escapeHtml(path)}</div>`
            ).join('');
        }

        // Disable delete button for default map or maps in use by multiple projects
        const deleteBtn = document.getElementById('delete-map-btn');
        if (deleteBtn) {
            deleteBtn.disabled = (mapId === 1 || stats.projects_using > 1);
        }

    } catch (err) {
        console.error('[Vector Maps] Failed to load stats:', err);
        showToast('Failed to load map statistics: ' + err, 'error');
    }
}

async function handleVectorMapChange() {
    const select = document.getElementById('project-vector-map-select');
    if (!select) return;

    const mapId = parseInt(select.value);
    state.currentVectorMapId = mapId;

    await updateVectorMapInfo(mapId);
}

async function showCreateMapModal() {
    const modal = document.getElementById('new-vector-map-modal');
    if (!modal) return;

    // Clear inputs
    document.getElementById('new-map-name').value = '';
    document.getElementById('new-map-description').value = '';

    modal.style.display = 'flex';
}

async function handleCreateVectorMap() {
    const nameInput = document.getElementById('new-map-name');
    const descInput = document.getElementById('new-map-description');

    const name = nameInput.value.trim();
    if (!name) {
        showToast('Please enter a map name', 'error');
        nameInput.focus();
        return;
    }

    try {
        const newMap = await invoke('create_vector_map', {
            name: name,
            description: descInput.value.trim() || null
        });

        showToast(`Vector map "${name}" created successfully!`, 'success');

        // Close modal
        document.getElementById('new-vector-map-modal').style.display = 'none';

        // Refresh selector and select new map
        await populateVectorMapSelector(newMap.id);
        state.currentVectorMapId = newMap.id;

        // Update info
        await updateVectorMapInfo(newMap.id);

    } catch (err) {
        console.error('[Vector Maps] Failed to create:', err);
        showToast('Failed to create vector map: ' + err, 'error');
    }
}

async function handleClearVectorMap() {
    const mapId = state.currentVectorMapId;
    if (!mapId) return;

    const mapName = document.getElementById('selected-map-name').textContent;

    if (!confirm(`Are you sure you want to clear all embeddings from "${mapName}"? This will delete all indexed data for this map.`)) {
        return;
    }

    try {
        await invoke('clear_vector_map', { mapId: mapId });
        showToast(`Vector map "${mapName}" cleared successfully`, 'success');

        // Refresh stats
        await updateVectorMapInfo(mapId);
    } catch (err) {
        console.error('[Vector Maps] Failed to clear:', err);
        showToast('Failed to clear vector map: ' + err, 'error');
    }
}

async function handleDeleteVectorMap() {
    const mapId = state.currentVectorMapId;
    if (!mapId || mapId === 1) {
        showToast('Cannot delete the default vector map', 'error');
        return;
    }

    const mapName = document.getElementById('selected-map-name').textContent;

    if (!confirm(`Are you sure you want to delete "${mapName}"? Projects using this map will be moved to the default map.`)) {
        return;
    }

    try {
        await invoke('delete_vector_map', { mapId: mapId });
        showToast(`Vector map "${mapName}" deleted successfully`, 'success');

        // Switch to default map
        state.currentVectorMapId = 1;
        await populateVectorMapSelector(1);

    } catch (err) {
        console.error('[Vector Maps] Failed to delete:', err);
        showToast('Failed to delete vector map: ' + err, 'error');
    }
}

// Initialize Vector Map event listeners
function initVectorMapListeners() {
    const selector = document.getElementById('project-vector-map-select');
    if (selector) {
        selector.addEventListener('change', handleVectorMapChange);
    }

    const createBtn = document.getElementById('create-new-vector-map-btn');
    if (createBtn) {
        createBtn.addEventListener('click', showCreateMapModal);
    }

    const clearBtn = document.getElementById('clear-map-btn');
    if (clearBtn) {
        clearBtn.addEventListener('click', handleClearVectorMap);
    }

    const deleteBtn = document.getElementById('delete-map-btn');
    if (deleteBtn) {
        deleteBtn.addEventListener('click', handleDeleteVectorMap);
    }

    // New map modal buttons
    const cancelBtn = document.getElementById('new-map-cancel');
    if (cancelBtn) {
        cancelBtn.addEventListener('click', () => {
            document.getElementById('new-vector-map-modal').style.display = 'none';
        });
    }

    const saveBtn = document.getElementById('new-map-save');
    if (saveBtn) {
        saveBtn.addEventListener('click', handleCreateVectorMap);
    }
}

// Helper function to escape HTML
function escapeHtml(unsafe) {
    return unsafe
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
}