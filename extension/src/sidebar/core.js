// ============================================================================
// Gluon Sidebar - Main Entry Point
// Punkt wejścia aplikacji, importuje wszystkie moduły i inicjalizuje UI
// ============================================================================

// Import logger
import { sidebarLogger } from '../common/logger.js';

// Import state management
import {
  fileTreeData, selectedNodes, selectedProjects, allProjects, pendingConfigRestore,
  selectedEnvironmentId, enabledPromptIds, licenseStatus,
  setFileTreeData, setAllProjects, setLicenseStatus, setSelectedEnvironmentId,
  setEnabledPromptIds, setPendingConfigRestore, setEnvironments,
  loadStateFromStorage, loadActiveTemplates, handleCheckboxChange, handleLogsCheckboxChange,
  updateBlockingOverlay, updateConnectionStatus, applyTheme,
  showLoading, hideLoading, showError, showStatusMessage
} from './management/stateManagement.js';

// Import file tree management
import {
  populateProjects, handleDeselectAllProjects, triggerFileTreeLoadForSelected,
  handleSearch, handleClearSelection, updateSelectionInfo,
  setupFileTreeResizer, updateFileCount,
  renderMergedFileTree, constructMultiProjectPayload,
  loadEnvironmentForSelectedProjects
} from './management/fileTreeManagement.js';

// Import context management
import {
  loadContextHistory, renderContextHistoryList, handleShowMoreContext,
  handleGenerateContextFile, handleGenerateSimpleFile, handleFileCopyResponse, parseAttachedFilesFromContext,
  initQuickTaskHistory, handleQuickTaskHistoryKeydown
} from './management/contextManagement.js';

// Import template & prompt management
import {
  setupEnvironmentListeners, loadEnvironments, populateEnvironments,
  loadTemplates, showCreateTemplateModal, hideCreateTemplateModal,
  saveTemplate, deleteTemplate, showManageTemplatesModal, hideManageTemplatesModal,
  handleManageTemplatesClick, updateDynamicTemplateFields,
  showPromptInputModal, hidePromptInputModal, handlePromptModalSubmit,
  handlePromptHistoryKeydown, renderPrompts, resetToDefaultEnvironment,
  setEnvironmentForProject
} from './management/templatePromptManagement.js';

// Import AI integration
import {
  handleAutoSelect, handleDetectedResponse, handleApplySelection,
  handleApplyInteractiveContext,
  handlePromptGeneratorClick
} from './management/aiIntegrationCommandParsing.js';

// Import Google Drive integration
import { initializeGoogleDrive } from '../features/google-drive/google-drive-manager.js';
import { initGoogleDrivePicker } from '../features/google-drive/google-drive-picker.js';

// ============================================================================
// Initialization
// ============================================================================

document.addEventListener('DOMContentLoaded', async () => {
  sidebarLogger.log('Sidebar initializing...');
  chrome.runtime.sendMessage({ action: 'init_connection' });
  setupEventListeners();
  chrome.storage.local.get(['uiThemeSettings'], (result) => {
    if (result.uiThemeSettings) {
      sidebarLogger.debug('Stosowanie motywu z pamięci lokalnej:', result.uiThemeSettings);
      applyTheme(result.uiThemeSettings);
    }
  });
  await loadStateFromStorage();
  await loadTemplates();
  await loadActiveTemplates();
  updateBlockingOverlay();

  // Initialize Google Drive integration
  initializeGoogleDrive();
  initGoogleDrivePicker();

  // Initialize Prompt Files Attachment
  initPromptFilesAttachment();
});

// ============================================================================
// Event Listeners Setup
// ============================================================================

function setupEventListeners() {
  sidebarLogger.log('🔧 Setting up event listeners...');

  // Screen toggle buttons
  const projectScreenBtn = document.getElementById('projectScreenBtn');
  const agentScreenBtn = document.getElementById('agentScreenBtn');
  const knowledgeScreenBtn = document.getElementById('knowledgeScreenBtn');
  const mcpResultsScreenBtn = document.getElementById('mcpResultsScreenBtn');

  const projectScreen = document.getElementById('projectScreen');
  const agentScreen = document.getElementById('agentScreen');
  const knowledgeScreen = document.getElementById('knowledgeScreen');
  const mcpResultsScreen = document.getElementById('mcpResultsScreen');

  function switchScreen(screenName) {
      // Buttons
      projectScreenBtn.classList.toggle('active', screenName === 'project');
      agentScreenBtn.classList.toggle('active', screenName === 'agent');
      if (knowledgeScreenBtn) knowledgeScreenBtn.classList.toggle('active', screenName === 'knowledge');
      if (mcpResultsScreenBtn) mcpResultsScreenBtn.classList.toggle('active', screenName === 'mcpResults');

      // Screens
      projectScreen.classList.toggle('active', screenName === 'project');
      agentScreen.classList.toggle('active', screenName === 'agent');
      if (knowledgeScreen) knowledgeScreen.classList.toggle('active', screenName === 'knowledge');
      if (mcpResultsScreen) mcpResultsScreen.classList.toggle('active', screenName === 'mcpResults');

      sidebarLogger.debug(`✅ Switched to ${screenName} screen`);
  }

  if (projectScreenBtn) projectScreenBtn.addEventListener('click', () => switchScreen('project'));
  if (agentScreenBtn) agentScreenBtn.addEventListener('click', () => switchScreen('agent'));
  if (knowledgeScreenBtn) knowledgeScreenBtn.addEventListener('click', () => switchScreen('knowledge'));
  if (mcpResultsScreenBtn) mcpResultsScreenBtn.addEventListener('click', () => switchScreen('mcpResults'));

  // Copy JSON Schema Button (Header)
  const copyJsonSchemaBtn = document.getElementById('copyJsonSchemaBtn');
  if (copyJsonSchemaBtn) {
    copyJsonSchemaBtn.addEventListener('click', () => {
      const schema = {
        "type": "object",
        "properties": {
          "thought_process": {
            "type": "string",
            "description": "Your internal monologue. Analyze the user request, plan your approach, analyze code context, and verify your decisions here BEFORE generating output."
          },
          "user_message": {
            "type": "string",
            "description": "The conversational response shown to the user (supports Markdown). Use **bold**, *italic*, `code`, lists (- item), and headers (## Title) for better readability. Explain what you are doing in natural language. Do NOT include technical blocks here."
          },
          "gluon_actions": {
            "type": "object",
            "description": "Technical actions to be executed by the Gluon Extension.",
            "properties": {
              "file_changes": {
                "type": "array",
                "description": "List of file modifications using SEARCH/REPLACE logic.",
                "items": {
                  "type": "object",
                  "properties": {
                    "file_path": { "type": "string" },
                    "search_code": {
                      "type": "string",
                      "description": "Exact code block to find. Must be unique. Use '// ... existing code ...' for truncation if allowed."
                    },
                    "replace_code": {
                      "type": "string",
                      "description": "Replacement code. Must be complete implementation."
                    }
                  },
                  "required": ["file_path", "search_code", "replace_code"]
                }
              },
              "context_ops": {
                "type": "object",
                "description": "Operations to load new context or search semantic maps.",
                "properties": {
                  "load": {
                    "type": "array",
                    "items": {
                      "type": "object",
                      "properties": {
                        "type": { "type": "string", "enum": ["full_file", "file_symbol", "semantic_map", "rag_search"] },
                        "path": { "type": "string" },
                        "symbol": { "type": "string" },
                        "query": { "type": "string" }
                      },
                      "required": ["type"]
                    }
                  }
                }
              }
            }
          }
        },
        "required": ["thought_process", "user_message"]
      };

      navigator.clipboard.writeText(JSON.stringify(schema, null, 2))
        .then(() => showStatusMessage('✅ JSON Schema copied to clipboard!', 'success'))
        .catch(err => showStatusMessage('❌ Failed to copy schema', 'error'));
    });
  }

  // Initialize RAG View
  import('./management/ragManagement.js').then(module => {
      module.initRagView();
  }).catch(err => console.error('Failed to load RAG module:', err));

  // Logo click
  const headerLeft = document.querySelector('.header-left');
  if (headerLeft) {
    headerLeft.style.cursor = 'pointer';
    headerLeft.addEventListener('click', () => {
      chrome.tabs.create({ url: 'https://ai-gluon.com' });
    });
    sidebarLogger.debug('✅ Header link listener attached');
  }

  const overlayLogo = document.getElementById('overlay-logo');
  if (overlayLogo) {
    overlayLogo.addEventListener('click', () => {
      const title = document.getElementById('overlay-title');
      if (title) {
        title.textContent = 'Checking Status...';
      }
      chrome.runtime.sendMessage({ action: 'init_connection' });
    });
    sidebarLogger.debug('✅ Overlay logo refresh listener attached');
  }

  // Floating Agent Button on blocking overlay
  const overlayAgentBtn = document.getElementById('overlayAgentBtn');
  if (overlayAgentBtn) {
    overlayAgentBtn.addEventListener('click', () => {
      // Hide the blocking overlay
      const blockingOverlay = document.getElementById('blocking-overlay');
      if (blockingOverlay) {
        blockingOverlay.style.display = 'none';
      }
      // Switch to agent screen
      const agentScreenBtnEl = document.getElementById('agentScreenBtn');
      const projectScreenBtnEl = document.getElementById('projectScreenBtn');
      const agentScreenEl = document.getElementById('agentScreen');
      const projectScreenEl = document.getElementById('projectScreen');
      if (agentScreenBtnEl && projectScreenBtnEl && agentScreenEl && projectScreenEl) {
        agentScreenBtnEl.classList.add('active');
        projectScreenBtnEl.classList.remove('active');
        agentScreenEl.classList.add('active');
        projectScreenEl.classList.remove('active');
      }
      sidebarLogger.debug('✅ Opened Agent screen from overlay FAB');
    });
    sidebarLogger.debug('✅ Overlay Agent FAB listener attached');
  }

  // Deselectuj wszystkie projekty
  const deselectBtn = document.getElementById('deselectAllBtn');
  if (deselectBtn) {
    deselectBtn.addEventListener('click', handleDeselectAllProjects);
    sidebarLogger.debug('✅ Deselect button listener attached');
  }

  // Search input
  const searchInput = document.getElementById('searchInput');
  if (searchInput) {
    searchInput.addEventListener('input', handleSearch);
    sidebarLogger.debug('✅ Search input listener attached');
  }

  // Clear selection
  const clearSelectionBtn = document.getElementById('clearSelection');
  if (clearSelectionBtn) {
    clearSelectionBtn.addEventListener('click', handleClearSelection);
    sidebarLogger.debug('✅ Clear selection listener attached');
  }

  // Copy button
  const copyBtn = document.getElementById('copyBtn');
  if (copyBtn) {
    copyBtn.addEventListener('click', async () => {
      const { handleCopyFiles } = await import('./management/contextManagement.js');
      handleCopyFiles();
    });
    sidebarLogger.debug('✅ Copy button listener attached');
  }

  // Generate button
  const generateBtn = document.getElementById('generateBtn');
  if (generateBtn) {
    generateBtn.addEventListener('click', () => {
      try {
        handleGenerateContextFile();
      } catch (error) {
        sidebarLogger.error('Error calling handleGenerateContextFile:', error);
        showStatusMessage(`Error: ${error.message}`, 'error');
      }
    });
  }

  // Generate Simple button (files only)
  const generateSimpleBtn = document.getElementById('generateSimpleBtn');
  if (generateSimpleBtn) {
    generateSimpleBtn.addEventListener('click', () => {
      try {
        handleGenerateSimpleFile();
      } catch (error) {
        sidebarLogger.error('Error calling handleGenerateSimpleFile:', error);
        showStatusMessage(`Error: ${error.message}`, 'error');
      }
    });
    sidebarLogger.debug('✅ Generate Simple button listener attached');
  }

  // Generate Map button (semantic map)
  const generateMapBtn = document.getElementById('generateMapBtn');
  if (generateMapBtn) {
    generateMapBtn.addEventListener('click', async () => {
      try {
        const { handleGenerateSemanticMap } = await import('./management/contextManagement.js');
        handleGenerateSemanticMap();
      } catch (error) {
        sidebarLogger.error('Error calling handleGenerateSemanticMap:', error);
        showStatusMessage(`Error: ${error.message}`, 'error');
      }
    });
    sidebarLogger.debug('✅ Generate Map button listener attached');
  }

  // Show more context files button
  const showMoreBtn = document.getElementById('showMoreContextBtn');
  if (showMoreBtn) {
    showMoreBtn.addEventListener('click', handleShowMoreContext);
    sidebarLogger.debug('✅ Show more context button listener attached');
  }

  // Connection indicator
  const connectionIndicator = document.getElementById('connectionIndicator');
  if (connectionIndicator) {
    connectionIndicator.addEventListener('click', () => {
      chrome.runtime.sendMessage({ action: 'init_connection' });
    });
    sidebarLogger.debug('✅ Connection indicator listener attached');
  }

  // Checkboxes
  const includeStructureCheckbox = document.getElementById('includeStructure');
  if (includeStructureCheckbox) {
    includeStructureCheckbox.addEventListener('change', (e) => {
      handleCheckboxChange(e);
      updateSelectionInfo();
    });
  }

  const includeLogsCheckbox = document.getElementById('includeLogs');
  if (includeLogsCheckbox) {
    includeLogsCheckbox.addEventListener('change', handleLogsCheckboxChange);
    sidebarLogger.debug('✅ Include logs checkbox listener attached');
  }

  // Gluon Mode Switch
  const gluonModeSwitch = document.getElementById('gluonModeSwitch');
  if (gluonModeSwitch) {
    gluonModeSwitch.addEventListener('change', async (e) => {
      sidebarLogger.log('Gluon Mode Switch toggled:', e.target.checked);
      const { toggleGluonMode } = await import('./management/stateManagement.js');
      await toggleGluonMode(e.target.checked);
      showStatusMessage(
        `Gluon System ${e.target.checked ? 'enabled' : 'disabled'}`,
        e.target.checked ? 'success' : 'info'
      );
    });
    sidebarLogger.debug('✅ Gluon mode switch listener attached');
  } else {
    sidebarLogger.error('❌ Gluon mode switch element NOT found');
  }

    // RAG Model Selector & Switch removed from header (Moved to Knowledge Screen)

  // Template buttons
  const autoSelectBtn = document.getElementById('autoSelectBtn');
  const contextHandoffBtn = document.getElementById('contextHandoffBtn');
  const promptHandoffBtn = document.getElementById('promptHandoffBtn');

  if (autoSelectBtn) {
    autoSelectBtn.addEventListener('click', () => showPromptInputModal('auto_select'));
  }

  if (contextHandoffBtn) {
    contextHandoffBtn.addEventListener('click', () => showPromptInputModal('context_handoff'));
  }

  if (promptHandoffBtn) {
    promptHandoffBtn.addEventListener('click', () => showPromptInputModal('prompt_handoff'));
  }

  const manageTemplatesBtn = document.getElementById('manageTemplatesBtn');
  if (manageTemplatesBtn) {
    manageTemplatesBtn.addEventListener('click', showManageTemplatesModal);
  }


  // Create Template Modal
  const createTemplateModal = document.getElementById('createTemplateModal');
  const createTemplateForm = document.getElementById('createTemplateForm');
  const cancelTemplateBtn = document.getElementById('cancelTemplateBtn');
  const deleteTemplateBtn = document.getElementById('deleteTemplateBtn');

  if (deleteTemplateBtn) {
    deleteTemplateBtn.addEventListener('click', (e) => {
      const type = e.target.dataset.type;
      const id = e.target.dataset.id;
      const name = document.getElementById('templateName').value;

      if (type && id && confirm(`Are you sure you want to delete the template "${name}"?`)) {
        deleteTemplate(type, id);
        hideCreateTemplateModal();
      }
    });
  }

  if (cancelTemplateBtn) {
    cancelTemplateBtn.addEventListener('click', hideCreateTemplateModal);
  }

  if (createTemplateModal) {
    createTemplateModal.addEventListener('click', (e) => {
      if (e.target === createTemplateModal) {
        hideCreateTemplateModal();
      }
    });
  }

  if (createTemplateForm) {
    createTemplateForm.addEventListener('submit', (e) => {
      e.preventDefault();
      saveTemplate();
    });
  }

  // Manage Templates Modal
  const manageTemplatesModal = document.getElementById('manageTemplatesModal');
  const closeManageModalBtn = document.getElementById('closeManageModalBtn');
  const createNewTemplateFromManageBtn = document.getElementById('createNewTemplateFromManageBtn');

  if (closeManageModalBtn) {
    closeManageModalBtn.addEventListener('click', hideManageTemplatesModal);
  }
  if (manageTemplatesModal) {
    manageTemplatesModal.addEventListener('click', (e) => {
      if (e.target === manageTemplatesModal) hideManageTemplatesModal();
    });
    manageTemplatesModal.addEventListener('click', handleManageTemplatesClick);
  }
  if (createNewTemplateFromManageBtn) {
    createNewTemplateFromManageBtn.addEventListener('click', () => {
      hideManageTemplatesModal();
      const activeTab = document.querySelector('#manageTemplatesModal .tab-link.active');
      let type = activeTab ? activeTab.dataset.tab : 'auto_select';

      if (type.startsWith('tab-')) {
        type = type.substring(4);
      }

      showCreateTemplateModal(type);
    });
  }

  // Prompt Input Modal
  const promptInputModal = document.getElementById('promptInputModal');
  const promptInputForm = document.getElementById('promptInputForm');
  const cancelPromptModalBtn = document.getElementById('cancelPromptModalBtn');
  const modalPromptTextarea = document.getElementById('modalPromptTextarea');

  if (cancelPromptModalBtn) {
    cancelPromptModalBtn.addEventListener('click', hidePromptInputModal);
  }
  if (promptInputModal) {
    promptInputModal.addEventListener('click', (e) => {
      if (e.target === promptInputModal) {
        hidePromptInputModal();
      }
    });
  }
  if (promptInputForm) {
    promptInputForm.addEventListener('submit', handlePromptModalSubmit);
  }
  if (modalPromptTextarea) {
    modalPromptTextarea.addEventListener('keydown', handlePromptHistoryKeydown);
  }


  // Quick Task History
  const quickTaskInput = document.getElementById('quickTaskInput');
  if (quickTaskInput) {
    quickTaskInput.addEventListener('keydown', handleQuickTaskHistoryKeydown);
  }

  setupEnvironmentListeners();
  loadContextHistory();
  setupFileTreeResizer();
  initQuickTaskHistory();

  // MCP Tools Controls
  const mcpSearchInput = document.getElementById('mcpSearchInput');
  const mcpSearchBtn = document.getElementById('mcpSearchBtn');
  const mcpProtocolsBtn = document.getElementById('mcpProtocolsBtn');
  const mcpImpactBtn = document.getElementById('mcpImpactBtn');
  const mcpVerifyBtn = document.getElementById('mcpVerifyBtn');
  const mcpCpgBtn = document.getElementById('mcpCpgBtn');
  const mcpToolsStatus = document.getElementById('mcpToolsStatus');

  function setMcpToolsStatus(text, color = 'var(--text-secondary)') {
    if (mcpToolsStatus) {
      mcpToolsStatus.textContent = text;
      mcpToolsStatus.style.color = color;
    }
  }

  function formatMcpResults(results) {
    // Formatuj wyniki do czytelnego tekstu — rozpakuj zagnieżdżone struktury
    let formatted = '';

    // Helper: rozpakuj zagnieżdżone warstwy
    function unwrapContent(obj) {
      if (!obj) return '';

      // Jeśli tablica, rozpakuj każdy element
      if (Array.isArray(obj)) {
        return obj.map((item, idx) => {
          let content = unwrapContent(item);
          return results.length > 1 && typeof obj[0] === 'object'
            ? `─── ${idx + 1} ───\n${content}`
            : content;
        }).join('\n\n');
      }

      // Jeśli string — zwróć od razu (nawet jeśli to JSON string)
      if (typeof obj === 'string') {
        // Spróbuj rozpakować JSON string
        try {
          const parsed = JSON.parse(obj);
          return unwrapContent(parsed);
        } catch (e) {
          // To zwykły tekst
          return obj.trim();
        }
      }

      // Jeśli obiekt — szukaj pola z zawartością
      if (typeof obj === 'object') {
        // Priority: content → text → result → output → data
        for (const key of ['content', 'text', 'result', 'output', 'data', 'message']) {
          if (obj[key]) {
            return unwrapContent(obj[key]);
          }
        }

        // Jeśli obiekt ma pole "results" (semantic_search response)
        if (obj.results && Array.isArray(obj.results)) {
          return obj.results.map((r, idx) => {
            let item = `─── Result ${idx + 1} ───\n`;
            if (r.file || r.path) item += `📄 File: ${r.file || r.path}\n`;
            if (r.score) item += `Score: ${r.score}\n`;
            item += (r.content || r.text || r.chunk || '').trim();
            return item;
          }).join('\n\n');
        }

        // Fallback: listuj wszystkie klucze jako readable
        const entries = Object.entries(obj)
          .filter(([k, v]) => v !== null && v !== undefined)
          .map(([k, v]) => {
            let valStr = typeof v === 'object' ? JSON.stringify(v, null, 2) : String(v);
            return `${k}: ${valStr}`;
          });
        return entries.join('\n\n');
      }

      return String(obj).trim();
    }

    formatted = unwrapContent(results);
    return formatted;
  }

  function displayMcpResults(tool, results) {
    const content = document.getElementById('mcpResultsContent');
    const badge = document.getElementById('mcpResultsToolBadge');
    const sendBtn = document.getElementById('mcpResultsSendBtn');
    const copyBtn = document.getElementById('mcpResultsCopyBtn');

    // Formatuj wyniki do czytanego tekstu
    const formatted = formatMcpResults(results);

    if (content) content.textContent = formatted;
    if (badge) { badge.textContent = tool; badge.style.display = 'block'; }
    if (sendBtn) sendBtn.disabled = false;
    if (copyBtn) copyBtn.disabled = false;

    // Zapisz zarówno raw results jak i formatted do stanu
    window._mcpLastResults = { tool, formatted, rawResults: results };
  }

  function executeMcpTool(tool, args = {}) {
    setMcpToolsStatus(`Running ${tool}...`, 'var(--accent-blue)');
    chrome.runtime.sendMessage(
      { action: 'execute_mcp_tool', payload: { tool, args } },
      (response) => {
        if (chrome.runtime.lastError) {
          sidebarLogger.error('[MCP Tools] Error:', chrome.runtime.lastError);
          showStatusMessage(`MCP error: ${chrome.runtime.lastError.message}`, 'error');
          setMcpToolsStatus('Error', 'var(--status-error)');
          return;
        }
        if (response && response.success && response.results) {
          // Pokaż wyniki w MCP Results ekranie
          displayMcpResults(tool, response.results);
          switchScreen('mcpResults');   // przełącz na nowy tab
          setMcpToolsStatus('Done ✓', 'var(--status-success)');
          setTimeout(() => setMcpToolsStatus('Ready'), 3000);
        } else {
          showStatusMessage(`MCP failed: ${response?.error || 'Unknown error'}`, 'error');
          setMcpToolsStatus('Error', 'var(--status-error)');
          setTimeout(() => setMcpToolsStatus('Ready'), 3000);
        }
      }
    );
  }

  if (mcpSearchBtn) {
    mcpSearchBtn.addEventListener('click', () => {
      const query = mcpSearchInput ? mcpSearchInput.value.trim() : '';
      if (!query) {
        showStatusMessage('Please enter a search query', 'error');
        return;
      }
      executeMcpTool('semantic_search', { query });
    });
  }

  // Also trigger search on Enter key
  if (mcpSearchInput) {
    mcpSearchInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') mcpSearchBtn && mcpSearchBtn.click();
    });
  }

  if (mcpProtocolsBtn) {
    mcpProtocolsBtn.addEventListener('click', () => executeMcpTool('analyze_semantic_protocols'));
  }
  if (mcpImpactBtn) {
    mcpImpactBtn.addEventListener('click', () => executeMcpTool('analyze_change_impact'));
  }
  if (mcpVerifyBtn) {
    mcpVerifyBtn.addEventListener('click', () => executeMcpTool('verify_change'));
  }
  if (mcpCpgBtn) {
    mcpCpgBtn.addEventListener('click', () => {
      showStatusMessage('analyze_cpg running (up to 90s)...', 'info');
      executeMcpTool('analyze_cpg');
    });
  }

  // MCP Results Panel Controls
  const mcpResultsSendBtn = document.getElementById('mcpResultsSendBtn');
  const mcpResultsCopyBtn = document.getElementById('mcpResultsCopyBtn');
  const mcpResultsClearBtn = document.getElementById('mcpResultsClearBtn');

  if (mcpResultsSendBtn) {
    mcpResultsSendBtn.addEventListener('click', () => {
      if (!window._mcpLastResults) return;
      const { tool, formatted } = window._mcpLastResults;
      chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
        if (tabs[0]) {
          chrome.tabs.sendMessage(tabs[0].id, {
            action: 'inject_mcp_results',
            payload: {
              mcpResults: [{ tool, success: true, content: formatted }],
              originalMessageId: null
            }
          });
          switchScreen('project'); // wróć do projektu
        }
      });
    });
  }

  if (mcpResultsCopyBtn) {
    mcpResultsCopyBtn.addEventListener('click', () => {
      if (window._mcpLastResults) {
        navigator.clipboard.writeText(window._mcpLastResults.formatted);
        showStatusMessage('Copied to clipboard', 'success');
      }
    });
  }

  if (mcpResultsClearBtn) {
    mcpResultsClearBtn.addEventListener('click', () => {
      const content = document.getElementById('mcpResultsContent');
      const badge = document.getElementById('mcpResultsToolBadge');
      const sendBtn = document.getElementById('mcpResultsSendBtn');
      const copyBtn = document.getElementById('mcpResultsCopyBtn');
      if (content) content.textContent = 'No results yet. Run a tool from the Project panel.';
      if (badge) badge.style.display = 'none';
      if (sendBtn) sendBtn.disabled = true;
      if (copyBtn) copyBtn.disabled = true;
      window._mcpLastResults = null;
    });
  }

  // Attach Prompt Files Modal
  const attachPromptFilesBtn = document.getElementById('attachPromptFilesBtn');
  const attachPromptFilesModal = document.getElementById('attachPromptFilesModal');
  const cancelAttachPromptFilesBtn = document.getElementById('cancelAttachPromptFilesBtn');
  const confirmAttachPromptFilesBtn = document.getElementById('confirmAttachPromptFilesBtn');
  const selectLocalFilesBtn = document.getElementById('selectLocalFilesBtn');
  const selectDriveFilesBtn = document.getElementById('selectDriveFilesBtn');

  if (attachPromptFilesBtn) {
    attachPromptFilesBtn.addEventListener('click', () => {
      if (attachPromptFilesModal) {
        attachPromptFilesModal.style.display = 'flex';
      }
    });
  }

  if (cancelAttachPromptFilesBtn) {
    cancelAttachPromptFilesBtn.addEventListener('click', () => {
      if (attachPromptFilesModal) {
        attachPromptFilesModal.style.display = 'none';
      }
    });
  }

  if (attachPromptFilesModal) {
    attachPromptFilesModal.addEventListener('click', (e) => {
      if (e.target === attachPromptFilesModal) {
        attachPromptFilesModal.style.display = 'none';
      }
    });
  }

  if (selectLocalFilesBtn) {
    selectLocalFilesBtn.addEventListener('click', () => {
      const input = document.createElement('input');
      input.type = 'file';
      input.multiple = true;
      input.accept = '.txt,.md,.json,.xml,.yaml,.yml,.csv,.log';
      input.onchange = handleLocalPromptFilesSelected;
      input.click();
    });
  }

  if (selectDriveFilesBtn) {
    selectDriveFilesBtn.addEventListener('click', async () => {
      // Open Google Drive picker for prompt files
      const { openDrivePickerForPrompts } = await import('../features/prompts/prompt-files-manager.js');
      openDrivePickerForPrompts();
    });
  }

  if (confirmAttachPromptFilesBtn) {
    confirmAttachPromptFilesBtn.addEventListener('click', () => {
      // Save attached prompt files
      if (attachPromptFilesModal) {
        attachPromptFilesModal.style.display = 'none';
      }
      showStatusMessage('Prompt files attached successfully', 'success');
    });
  }

  // Workflow buttons - direct listeners as fallback
  const addAgentBtn = document.getElementById('addAgentBtn');
  const loadWorkflowBtn = document.getElementById('loadWorkflowBtn');
  const exportTabBtn = document.getElementById('exportTabBtn');
  const importTabBtn = document.getElementById('importTabBtn');
  const copySchemaBtn = document.getElementById('copySchemaBtn');
  const refreshWorkflowBtn = document.getElementById('refreshWorkflowBtn');
  const newWorkflowTabBtn = document.getElementById('newWorkflowTabBtn');
  const viewListBtn = document.getElementById('viewListBtn');
  const viewGraphBtn = document.getElementById('viewGraphBtn');

  sidebarLogger.log('🔍 Checking workflow buttons:', {
    addAgentBtn: !!addAgentBtn,
    loadWorkflowBtn: !!loadWorkflowBtn,
    exportTabBtn: !!exportTabBtn,
    importTabBtn: !!importTabBtn,
    copySchemaBtn: !!copySchemaBtn,
    refreshWorkflowBtn: !!refreshWorkflowBtn,
    newWorkflowTabBtn: !!newWorkflowTabBtn,
    viewListBtn: !!viewListBtn,
    viewGraphBtn: !!viewGraphBtn
  });

  if (addAgentBtn) {
    addAgentBtn.addEventListener('click', () => {
      sidebarLogger.log('🖱️ Add Agent button clicked (via core.js)');
      if (window.workflowManager) {
        window.workflowManager.showAddAgentModal();
      } else {
        sidebarLogger.error('❌ workflowManager not available');
        showStatusMessage('Workflow Manager not initialized', 'error');
      }
    });
    sidebarLogger.debug('✅ Add Agent button listener attached (core.js)');
  }

  if (loadWorkflowBtn) {
    loadWorkflowBtn.addEventListener('click', () => {
      sidebarLogger.log('🖱️ Load Workflow button clicked (via core.js)');
      if (window.workflowManager) {
        window.workflowManager.showLoadWorkflowModal();
      } else {
        showStatusMessage('Workflow Manager not initialized', 'error');
      }
    });
    sidebarLogger.debug('✅ Load Workflow button listener attached (core.js)');
  }

  if (exportTabBtn) {
    exportTabBtn.addEventListener('click', () => {
      sidebarLogger.log('🖱️ Export Tab button clicked (via core.js)');
      if (window.workflowManager) {
        window.workflowManager.saveCurrentTabConfig();
      } else {
        showStatusMessage('Workflow Manager not initialized', 'error');
      }
    });
    sidebarLogger.debug('✅ Export Tab button listener attached (core.js)');
  }

  if (importTabBtn) {
    importTabBtn.addEventListener('click', () => {
      sidebarLogger.log('🖱️ Import Tab button clicked (via core.js)');
      const importTabFileInput = document.getElementById('importTabFileInput');
      if (importTabFileInput) {
        importTabFileInput.click();
      }
    });
    sidebarLogger.debug('✅ Import Tab button listener attached (core.js)');
  }

  if (copySchemaBtn) {
    copySchemaBtn.addEventListener('click', () => {
      sidebarLogger.log('🖱️ Copy Schema button clicked (via core.js)');
      if (window.workflowManager) {
        window.workflowManager.handleCopySchema();
      } else {
        showStatusMessage('Workflow Manager not initialized', 'error');
      }
    });
    sidebarLogger.debug('✅ Copy Schema button listener attached (core.js)');
  }

  if (refreshWorkflowBtn) {
    refreshWorkflowBtn.addEventListener('click', () => {
      sidebarLogger.log('🖱️ Refresh Workflow button clicked (via core.js)');
      if (window.workflowManager) {
        window.workflowManager.refreshWorkflow();
      } else {
        showStatusMessage('Workflow Manager not initialized', 'error');
      }
    });
    sidebarLogger.debug('✅ Refresh Workflow button listener attached (core.js)');
  }

  // Listener removed to prevent double-firing (handled by workflow-manager.js)
  /*
  if (newWorkflowTabBtn) {
    newWorkflowTabBtn.addEventListener('click', () => {
      sidebarLogger.log('🖱️ New Workflow Tab button clicked (via core.js)');
      if (window.workflowManager) {
        window.workflowManager.createNewTab();
      } else {
        showStatusMessage('Workflow Manager not initialized', 'error');
      }
    });
    sidebarLogger.debug('✅ New Workflow Tab button listener attached (core.js)');
  }
  */

  if (viewListBtn) {
    viewListBtn.addEventListener('click', () => {
      sidebarLogger.log('🖱️ View List button clicked (via core.js)');
      if (window.workflowManager) {
        window.workflowManager.switchView('list');
      } else {
        showStatusMessage('Workflow Manager not initialized', 'error');
      }
    });
    sidebarLogger.debug('✅ View List button listener attached (core.js)');
  }

  if (viewGraphBtn) {
    viewGraphBtn.addEventListener('click', () => {
      sidebarLogger.log('🖱️ View Graph button clicked (via core.js)');
      if (window.workflowManager) {
        window.workflowManager.switchView('graph');
      } else {
        showStatusMessage('Workflow Manager not initialized', 'error');
      }
    });
    sidebarLogger.debug('✅ View Graph button listener attached (core.js)');
  }

  sidebarLogger.success('All event listeners setup complete');

  // Debug: Check if workflow-manager module loaded
  setTimeout(() => {
    sidebarLogger.log('🔍 Module check after 2 seconds:', {
      workflowManager: !!window.workflowManager,
      workflowManagerTest: !!window.workflowManagerTest
    });

    if (!window.workflowManagerTest) {
      sidebarLogger.error('❌ Even TEST module did not load! Modules are broken.');
    } else {
      sidebarLogger.success('✅ Test module loaded successfully');
    }

    if (!window.workflowManager) {
      sidebarLogger.error('❌ WorkflowManager still not available after 2s');
      sidebarLogger.log('Attempting manual initialization...');

      // Try to manually import and initialize
      import('../features/workflows/workflow-manager.js')
        .then(() => {
          sidebarLogger.success('✅ Workflow manager module imported manually');
          sidebarLogger.log('window.workflowManager:', !!window.workflowManager);
        })
        .catch(error => {
          sidebarLogger.error('❌ Failed to import workflow-manager.js:', error);
          sidebarLogger.error('Error details:', error.message);
          sidebarLogger.error('Error stack:', error.stack);
        });
    }
  }, 2000);
}

// ============================================================================
// RAG Indexing Modal Logic
// ============================================================================

async function showRagIndexingModal() {
  const modal = document.getElementById('ragIndexingModal');
  if (!modal) return;

  // Import dynamicznie, aby uniknąć problemów z cyklicznymi zależnościami
  const { ragSelectedProjects, ragSelectedNodes, allProjects, fileTreeData } = await import('./management/stateManagement.js');
  const { renderRagFileTree, updateRagSelectionInfo } = await import('./management/fileTreeManagement.js');

  // 1. Inicjalizacja stanu (domyślnie zaznacz wszystko co jest w fileTreeData)
  ragSelectedProjects.clear();
  ragSelectedNodes.clear();

  // Domyślnie zaznaczamy wszystkie załadowane projekty
  fileTreeData.forEach(p => {
    ragSelectedProjects.add(p.projectPath);
    // Domyślnie zaznaczamy wszystkie pliki (użytkownik może odznaczyć)
    const allFiles = [];
    const traverse = (nodes) => {
        nodes.forEach(n => {
            if (n.nodeType === 'file') allFiles.push(n.path);
            if (n.children) traverse(n.children);
        });
    };
    traverse(p.tree);
    ragSelectedNodes.set(p.projectPath, new Set(allFiles));
  });

  // 2. Renderowanie zakładek projektów w modalu
  const projectContainer = document.getElementById('ragProjectSelect');
  projectContainer.innerHTML = '';

  fileTreeData.forEach(project => {
    const projectName = project.projectPath.split(/[\\/]/).pop();
    const card = document.createElement('div');
    card.className = 'project-tab-card active'; // Domyślnie aktywne
    card.dataset.path = project.projectPath;
    card.innerHTML = `<span class="project-tab-name">${projectName}</span><div class="project-tab-indicator"></div>`;

    card.addEventListener('click', () => {
      if (ragSelectedProjects.has(project.projectPath)) {
        ragSelectedProjects.delete(project.projectPath);
        card.classList.remove('active');
      } else {
        ragSelectedProjects.add(project.projectPath);
        card.classList.add('active');
      }
      renderRagFileTree();
    });

    projectContainer.appendChild(card);
  });

  // 3. Renderowanie drzewa
  renderRagFileTree();
  updateRagSelectionInfo();

  // 4. Pokaż modal
  modal.style.display = 'flex';
}

// Setup RAG Modal Listeners
document.addEventListener('DOMContentLoaded', () => {
  const cancelBtn = document.getElementById('cancelRagIndexingBtn');
  const startBtn = document.getElementById('startRagIndexingBtn');
  const modal = document.getElementById('ragIndexingModal');
  const localAiSwitch = document.getElementById('localAiSwitch');

  if (cancelBtn) {
    cancelBtn.addEventListener('click', () => {
      if (modal) modal.style.display = 'none';
      // Przywróć switch na off
      if (localAiSwitch) localAiSwitch.checked = false;
    });
  }

  if (startBtn) {
    startBtn.addEventListener('click', async () => {
      const { ragSelectedNodes } = await import('./management/stateManagement.js');
      const { showStatusMessage } = await import('./management/stateManagement.js');

      // Przygotuj payload wybranych plików
      const selectedFilesPayload = [];
      ragSelectedNodes.forEach((files, rootPath) => {
        if (files.size > 0) {
          selectedFilesPayload.push({
            rootPath: rootPath,
            relativePaths: Array.from(files)
          });
        }
      });

      if (selectedFilesPayload.length === 0) {
        showStatusMessage('Please select at least one file to index.', 'error');
        return;
      }

      // Zamknij modal
      if (modal) modal.style.display = 'none';

      // Włącz RAG (wizualnie i logicznie)
      if (localAiSwitch) localAiSwitch.checked = true;
      handleRagToggle(true, selectedFilesPayload);
    });
  }
});

function handleRagToggle(enabled, selectedFilesPayload = null) {
  const localAiSwitch = document.getElementById('localAiSwitch');
  const slider = localAiSwitch?.querySelector('.toggle-slider');
  if(slider) slider.style.opacity = '0.5';

  if (enabled) {
    // 1. Włącz usługę (z pominięciem auto-indexowania wszystkiego)
    chrome.runtime.sendMessage({
      action: 'toggle_local_ai',
      payload: { 
        enabled: true,
        skip_auto_index: true // NOWA FLAGA
      }
    });

    // 2. Wyślij precyzyjne żądanie indeksowania
    if (selectedFilesPayload) {
      setTimeout(() => {
        chrome.runtime.sendMessage({
          action: 'trigger_indexing',
          payload: {
            selectedFiles: selectedFilesPayload
          }
        });
      }, 1000); // Daj chwilę na start procesu
    }
  } else {
    // Wyłączanie
    chrome.runtime.sendMessage({
      action: 'toggle_local_ai',
      payload: { enabled: false }
    });
  }
}

// ============================================================================
// Prompt Files Attachment
// ============================================================================

let attachedPromptFiles = [];
// Make it globally accessible
window.attachedPromptFiles = attachedPromptFiles;

function initPromptFilesAttachment() {
  sidebarLogger.log('Initializing prompt files attachment system');

  // Listen for custom events from Google Drive picker
  document.addEventListener('prompt-file-selected', (e) => {
    handlePromptFileSelected(e.detail);
  });
}

function handleLocalPromptFilesSelected(event) {
  const files = Array.from(event.target.files);

  files.forEach(file => {
    const reader = new FileReader();
    reader.onload = (e) => {
      const fileData = {
        name: file.name,
        content: e.target.result,
        source: 'local',
        size: file.size,
        timestamp: Date.now()
      };
      handlePromptFileSelected(fileData);
    };
    reader.readAsText(file);
  });
}

function handlePromptFileSelected(fileData) {
  // Add to attached files list
  attachedPromptFiles.push(fileData);

  // Update global reference
  window.attachedPromptFiles = attachedPromptFiles;

  // Update UI
  renderAttachedPromptFiles();

  sidebarLogger.log('Prompt file attached:', fileData.name);
}


function renderAttachedPromptFiles() {
  const listContainer = document.getElementById('attachedPromptFilesList');

  // Update badge on attach button
  updateAttachPromptFilesBadge();

  if (!listContainer) return;

  if (attachedPromptFiles.length === 0) {
    listContainer.innerHTML = `
      <div class="empty-state">
        <div class="empty-icon">📎</div>
        <div class="empty-text">No files attached yet</div>
      </div>
    `;
    return;
  }

  listContainer.innerHTML = '';

  attachedPromptFiles.forEach((file, index) => {
    const fileItem = document.createElement('div');
    fileItem.className = 'attached-file-item';
    fileItem.style.cssText = 'display: flex; align-items: center; justify-content: space-between; padding: 8px; background: rgba(0,0,0,0.3); border-radius: 4px; margin-bottom: 4px;';

    const fileIcon = file.source === 'google-drive' ? '☁️' : '📄';
    const fileSize = formatFileSize(file.size || 0);

    fileItem.innerHTML = `
      <div style="display: flex; align-items: center; gap: 8px; flex: 1;">
        <span>${fileIcon}</span>
        <div style="flex: 1;">
          <div style="font-weight: 500;">${escapeHtml(file.name)}</div>
          <div style="font-size: 11px; color: var(--text-secondary);">${fileSize}</div>
        </div>
      </div>
      <button class="icon-btn tiny remove-prompt-file-btn" data-index="${index}" title="Remove">
        ❌
      </button>
    `;

    const removeBtn = fileItem.querySelector('.remove-prompt-file-btn');
    removeBtn.addEventListener('click', () => {
      attachedPromptFiles.splice(index, 1);
      window.attachedPromptFiles = attachedPromptFiles;
      renderAttachedPromptFiles();
    });

    listContainer.appendChild(fileItem);
  });
}

function formatFileSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

function escapeHtml(unsafe) {
  if (!unsafe) return '';
  return unsafe
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

function updateAttachPromptFilesBadge() {
  const attachBtn = document.getElementById('attachPromptFilesBtn');
  if (!attachBtn) return;

  const count = attachedPromptFiles.length;

  // Remove existing badge
  const existingBadge = attachBtn.querySelector('.file-count-badge');
  if (existingBadge) {
    existingBadge.remove();
  }

  // Add new badge if files are attached
  if (count > 0) {
    const badge = document.createElement('span');
    badge.className = 'file-count-badge';
    badge.textContent = count;
    badge.style.cssText = `
      position: absolute;
      top: -4px;
      right: -4px;
      background: var(--accent-blue);
      color: white;
      border-radius: 50%;
      width: 16px;
      height: 16px;
      font-size: 10px;
      font-weight: 600;
      display: flex;
      align-items: center;
      justify-content: center;
      pointer-events: none;
    `;
    attachBtn.style.position = 'relative';
    attachBtn.appendChild(badge);
  }
}

// ============================================================================
// Message Handling from Background
// ============================================================================

chrome.runtime.onMessage.addListener(async (message, sender, sendResponse) => {
  switch (message.type) {
    case 'status_update':
      updateConnectionStatus(message.status);
      break;
    case 'processing_complete':
      // Show completion message without affecting connection status
      if (message.status) {
        showStatusMessage(message.status, 'success');
      }
      break;
    case 'license_status_loaded':
      setLicenseStatus('VALID');
      updateBlockingOverlay();

      const themeSettings = { themeName: message.data?.themeName, theme80sColor1: message.data?.theme80sColor1, theme80sColor2: message.data?.theme80sColor2 };
      applyTheme(themeSettings);
      chrome.storage.local.set({ uiThemeSettings: themeSettings });
      break;
    case 'projects_loaded':
      setAllProjects(message.data || []);
      const { restoreSelectedProjects } = await import('./management/stateManagement.js');
      await restoreSelectedProjects();
      populateProjects(message.data);
      await loadEnvironmentForSelectedProjects();
      triggerFileTreeLoadForSelected();

      // Update RAG view with new projects
      import('./management/ragManagement.js').then(module => {
          module.updateRagProjects(message.data);
      }).catch(err => console.error('Failed to update RAG projects:', err));
      break;
    case 'environments_loaded':
      setEnvironments(message.data || []);
      populateEnvironments();
      break;
    case 'project_environment_loaded':
      await setEnvironmentForProject(message.data);
      break;
    case 'local_ai_status_update':
      const localAiSwitch = document.getElementById('localAiSwitch');
      if (localAiSwitch) {
        localAiSwitch.checked = message.data;
        const slider = localAiSwitch.querySelector('.toggle-slider');
        if(slider) slider.style.opacity = '1';
        localAiSwitch.title = `RAG Service: ${message.data ? 'ON' : 'OFF'}`;

        if (message.request_id) {
             showStatusMessage(`RAG Service ${message.data ? 'Started 🧠' : 'Stopped 💤'}`, message.data ? 'success' : 'info');
        }
      }
      break;
    case 'embedding_models_list':
      const modelListContainer = document.getElementById('ragModelList');
      sidebarLogger.log('Received embedding_models_list:', message.data);
      if (modelListContainer && message.data) {
        modelListContainer.innerHTML = '';
        message.data.forEach(model => {
          const item = document.createElement('div');
          item.className = 'dropdown-item';
          if (model.is_active) {
            item.classList.add('active');
          }
          item.dataset.model = model.filename;

          const sizeStr = model.size_mb.toFixed(0);
          const modelName = model.filename.replace('.gguf', '').replace('nomic-embed-text-', '');

          item.innerHTML = `
            <div class="model-info">
              <div class="model-name">${modelName}</div>
              <div class="model-size">${sizeStr} MB</div>
            </div>
          `;

          modelListContainer.appendChild(item);
        });
        sidebarLogger.log('✅ RAG models loaded:', message.data.length, 'models');
      } else {
        sidebarLogger.error('Failed to populate model list:', modelListContainer ? 'No data' : 'List container not found');
      }
      break;
    case 'embedding_model_changed':
      if (message.success) {
        showStatusMessage(`✅ RAG model switched: ${message.model}`, 'success');
        // Reload model list to update active marker
        chrome.runtime.sendMessage({ action: 'list_embedding_models' });
      } else {
        showStatusMessage(`❌ Failed to switch RAG model: ${message.error}`, 'error');
      }
      break;
    case 'indexing_progress':
      // Obsługa postępu indeksowania RAG
      const data = message.data;
      if (data.status === 'scanning') {
        const projectName = data.project.split(/[\\/]/).pop();
        showStatusMessage(`📂 Scanning: ${projectName}`, 'info');
      } else if (data.status === 'indexing') {
        const progress = Math.round((data.current / data.total) * 100);
        const fileName = data.current_file.split(/[\\/]/).pop();
        // Bardziej szczegółowy komunikat z nazwą pliku i postępem
        showStatusMessage(
          `🔍 RAG Indexing [${data.current}/${data.total}] ${progress}% | ${fileName}`,
          'info'
        );
        sidebarLogger.log(`[RAG] Processing: ${data.current_file} (${progress}%)`);
      } else if (data.status === 'file_complete') {
        // Event po zakończeniu pojedynczego pliku
        const fileName = data.file.split(/[\\/]/).pop();
        if (data.success) {
          const timeMs = data.elapsed_ms || 0;
          sidebarLogger.debug(`[RAG] ✅ Completed: ${fileName} in ${timeMs}ms (${data.current}/${data.total})`);
        } else {
          sidebarLogger.warn(`[RAG] ❌ Failed: ${fileName} - ${data.error || 'Unknown error'}`);
        }
      } else if (data.status === 'complete') {
        const total = data.processed + data.skipped;
        const successRate = Math.round((data.processed / total) * 100);

        // Display benchmark summary if available
        if (data.benchmark) {
          const bench = data.benchmark;
          const modelName = bench.model.replace('.gguf', '').replace('nomic-embed-text-', '');
          const throughput = bench.chunks_per_second.toFixed(2);
          const avgFileTime = bench.avg_time_per_file.toFixed(2);

          console.log(`\n╔════════════════════════════════════════════════════╗`);
          console.log(`║     📊 RAG BENCHMARK (${modelName})         ║`);
          console.log(`╠════════════════════════════════════════════════════╣`);
          console.log(`║ Files: ${data.processed}/${total} (${successRate}% success)              ║`);
          console.log(`║ Chunks: ${bench.total_chunks}                                  ║`);
          console.log(`║ Time: ${bench.total_time_secs.toFixed(1)}s                                ║`);
          console.log(`║ Throughput: ${throughput} chunks/s                      ║`);
          console.log(`║ Avg/File: ${avgFileTime}s                              ║`);
          console.log(`╚════════════════════════════════════════════════════╝`);

          showStatusMessage(
            `✅ RAG Complete! ${data.processed}/${total} files | ${throughput} chunks/s | ${modelName}`,
            'success'
          );
        } else {
          showStatusMessage(
            `✅ RAG Indexing Complete! ${data.processed}/${total} files (${successRate}% success)`,
            'success'
          );
        }
      }
      break;
    case 'file_trees_loaded':
      const { currentFileTreeRequestId } = await import('./management/stateManagement.js');
      if (currentFileTreeRequestId && message.request_id !== currentFileTreeRequestId) {
        sidebarLogger.debug('Ignoring file_trees_loaded - request ID mismatch');
        return;
      }

      // Check if data actually changed (to avoid unnecessary re-renders)
      const oldDataJson = JSON.stringify(fileTreeData);
      const newDataJson = JSON.stringify(message.data || []);
      const dataChanged = oldDataJson !== newDataJson;

      setFileTreeData(message.data || []);

      if (pendingConfigRestore) {
        selectedNodes.clear();
        for (const [projectPath, files] of Object.entries(pendingConfigRestore.selectedFiles)) {
          selectedNodes.set(projectPath, new Set(files));
        }
        setPendingConfigRestore(null);
        sidebarLogger.debug('Restored selected files from config');
      }

      // ONLY re-render if data actually changed (prevents destroying symbol-children during polling)
      if (dataChanged || !document.getElementById('fileTree').hasChildNodes()) {
        sidebarLogger.log('[FileTree] Data changed, re-rendering tree');
        // Apply search filter if search is active
        const { searchQuery } = await import('./management/stateManagement.js');
        const { filterFileTree } = await import('./management/fileTreeManagement.js');
        const dataToRender = searchQuery ? filterFileTree(fileTreeData) : fileTreeData;
        renderMergedFileTree(dataToRender);
      } else {
        sidebarLogger.debug('[FileTree] Data unchanged, skipping re-render (preserves symbol state)');
      }

      updateFileCount();
      hideLoading('generateBtn');
      const { setCurrentFileTreeRequestId } = await import('./management/stateManagement.js');
      setCurrentFileTreeRequestId(null);
      break;
    case 'files_multi_content_loaded':
      handleFileCopyResponse(message.data);
      hideLoading('copyBtn');
      break;
    case 'context_generation_progress':
      const { showContextProgress, updateContextProgress } = await import('./management/stateManagement.js');
      showContextProgress();
      updateContextProgress(message.data);
      break;
    case 'context_file_generated':
      const { hideContextProgress } = await import('./management/stateManagement.js');
      hideContextProgress();
      setTimeout(() => {
        hideLoading('generateBtn');
        hideLoading('generateSimpleBtn');
        hideLoading('generateMapBtn');
      }, 2000);

      const filepath = message.data?.filepath;
      const filename = message.data?.filename;

      if (!filepath || !filename) {
        showStatusMessage('Error: Context file generated but path/filename is missing.', 'error');
        return;
      }

      showStatusMessage(`✅ File saved: ${filename}`, 'success');
      sidebarLogger.log('Context file saved at:', filepath);

      const { attachContextFile } = await import('./management/contextManagement.js');
      // Po wygenerowaniu pliku od razu go załącz
      attachContextFile(filepath, filename);
      setTimeout(() => loadContextHistory(), 500);
      break;
    case 'context_generation_cancelled':
      const { hideContextProgress: hideProgress } = await import('./management/stateManagement.js');
      hideProgress();
      hideLoading('generateBtn');
      hideLoading('generateSimpleBtn');
      hideLoading('generateMapBtn');
      showStatusMessage('❌ Generation cancelled', 'error');
      break;
    case 'context_history_loaded':
      renderContextHistoryList(message.data);
      break;
    case 'context_file_content_ready':
      chrome.runtime.sendMessage({
        action: 'inject_file_to_gemini',
        payload: {
          filename: message.data.filename,
          content: message.data.content,
          type: 'text/plain'
        }
      });

      const attachedFiles = parseAttachedFilesFromContext(message.data.content);

      if (attachedFiles.length > 0) {
        showStatusMessage(`✅ ${message.data.filename} attached! Attaching ${attachedFiles.length} binary file(s)...`, 'success');

        attachedFiles.forEach(fileToAttach => {
          const project = allProjects.find(p => p.path.endsWith(fileToAttach.projectName));

          if (project) {
            const fullPath = `${project.path.replace(/\\/g, '/')}/${fileToAttach.relativePath}`;
            sidebarLogger.debug(`Requesting binary attachment from context: ${fullPath}`);

            chrome.runtime.sendMessage({
              action: 'get_binary_file_for_upload',
              payload: {
                filepath: fullPath
              }
            });
          } else {
            sidebarLogger.warn(`Could not find project matching name: '${fileToAttach.projectName}'`);
            showStatusMessage(`Warning: Project '${fileToAttach.projectName}' not found for attachment.`, 'error');
          }
        });

      } else {
        showStatusMessage(`✅ ${message.data.filename} attached!`, 'success');
      }

      sendResponse({ success: true });
      break;
    case 'gluon_command_detected':
      handleAutoSelect(message.files, message.clearPrevious);
      sendResponse({ success: true });
      break;
    case 'mcp_status_update':
      // Display MCP tool execution status in sidebar with progress bar
      if (message.payload?.status) {
        const { status, isError } = message.payload;
        showStatusMessage(status, isError ? 'error' : 'info');

        // Update MCP progress bar visibility and content
        const mcpContainer = document.getElementById('mcpProgressContainer');
        const mcpProgressFill = document.getElementById('mcpProgressFill');
        const mcpProgressText = document.getElementById('mcpProgressText');

        if (mcpContainer && mcpProgressFill && mcpProgressText) {
          if (status.includes('[')) {
            // Format: "🔧 semantic_search [1/5]"
            const matchArray = status.match(/\[(\d+)\/(\d+)\]/);
            if (matchArray) {
              const [, current, total] = matchArray;
              const percentage = (parseInt(current) / parseInt(total)) * 100;

              mcpContainer.style.display = 'flex';
              mcpProgressFill.style.width = percentage + '%';
              mcpProgressText.textContent = `${current}/${total} tools executed`;

              // Hide when complete
              if (parseInt(current) === parseInt(total)) {
                setTimeout(() => {
                  mcpContainer.style.display = 'none';
                }, 1500);
              }
            }
          }
        }
      }
      break;
    case 'gluon_response_detected':
      handleDetectedResponse(message.payload);
      break;
    case 'apply_gluon_selection':
      handleApplySelection(message.payload);
      break;
    case 'execute_interactive_context':
      handleApplyInteractiveContext(message.payload);
      break;
    case 'search_file_in_tree':
      handleSearchFileInTree(message.payload);
      break;
    case 'error':
      showError(message.message);
      setTimeout(() => {
        hideLoading('generateBtn');
        hideLoading('generateSimpleBtn');
        hideLoading('generateMapBtn');
        hideLoading('copyBtn');
      }, 2000);
      break;

  }
  return true;
});

/**
 * Obsługuje wyszukiwanie pliku w drzewie po kliknięciu kafelka
 */
function handleSearchFileInTree(payload) {
  const { filePath, projectPath, isMissing } = payload;
  sidebarLogger.log('Searching for file in tree:', { filePath, projectPath, isMissing });

  // Use search to find the file by filename (works even if file is in collapsed folder)
  const fileName = filePath.split('/').pop();
  const searchInput = document.getElementById('searchInput');

  if (searchInput) {
    searchInput.value = fileName;
    searchInput.dispatchEvent(new Event('input', { bubbles: true }));

    // Wait a bit for the search to render, then try to find and highlight the file
    setTimeout(() => {
      const fileNode = document.querySelector(
        `.tree-node.file[data-path="${filePath}"]${projectPath ? `[data-project="${projectPath}"]` : ''}`
      );

      if (fileNode) {
        fileNode.scrollIntoView({ behavior: 'smooth', block: 'center' });

        // Add temporary highlight
        fileNode.style.background = 'rgba(139, 92, 246, 0.3)';
        fileNode.style.transition = 'background 0.3s ease';

        setTimeout(() => {
          fileNode.style.background = '';
        }, 2000);

        showStatusMessage(`📄 Found: ${fileName}`, 'success');
      } else {
        // Scroll to search input to show results
        searchInput.scrollIntoView({ behavior: 'smooth', block: 'start' });

        if (isMissing) {
          showStatusMessage(`⚠️ File not found, searching for: ${fileName}`, 'info');
        } else {
          showStatusMessage(`🔍 Searching for: ${fileName}`, 'info');
        }
      }

      // Highlight search input
      searchInput.style.background = 'rgba(255, 193, 7, 0.2)';
      setTimeout(() => {
        searchInput.style.background = '';
      }, 1500);
    }, 300);
  }
}

// ============================================================================
// Cancel Context Generation Button
// ============================================================================

const cancelContextBtn = document.getElementById('cancelContextBtn');
if (cancelContextBtn) {
  cancelContextBtn.addEventListener('click', () => {
    const requestId = window.currentContextRequestId;
    if (requestId) {
      chrome.runtime.sendMessage({
        action: 'cancel_context_generation',
        payload: { request_id: requestId }
      });
      showStatusMessage('Cancelling generation...', 'info');
    }
  });
}

// ============================================================================
// AI Loading Progress Overlay
sidebarLogger.success('Gluon Sidebar loaded');