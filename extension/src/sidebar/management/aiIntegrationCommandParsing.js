import { parserLogger } from '../../common/logger.js';
import { parseGluonResponse } from '../utils/response-parser.js';

// ============================================================================
// AI Integration & Command Parsing Module
// Parsuje komendy AI, obsługuje auto-selekcję i overlay'e
// ============================================================================

import {
  fileTreeData, selectedNodes, allProjects, selectedProjects, searchQuery,
  showStatusMessage, escapeHTML
} from './stateManagement.js';

import {
  renderMergedFileTree, updateSelectionInfo, updateProjectStats,
  findAndSelectInTree, findFileInTree, findAndSelectByFileName,
  getProjectMapping, filterFileTree
} from './fileTreeManagement.js';

// Import G-Interactive Context Node
import { handleNextStepRequest } from '../../features/context/context-node.js';

// ============================================================================
// Gluon Command Parsing
// ============================================================================

/**
 * Parsuje komendy Gluon z tekstu
 */
export function parseGluonCommands(text) {
  try {
    const jsonMatch = text.match(/\{[\s\S]*?"@gluon:select"[\s\S]*?\}/);
    if (!jsonMatch) return null;

    const parsed = JSON.parse(jsonMatch[0]);
    if (parsed["@gluon:select"] && Array.isArray(parsed["@gluon:select"])) {
      return {
        action: 'select_files',
        files: parsed["@gluon:select"]
      };
    }
  } catch (e) {
    parserLogger.error('Failed to parse Gluon command:', e);
  }
  return null;
}

/**
 * Obsługuje odpowiedź AI
 */
export function handleAIResponse(responseText) {
  const command = parseGluonCommands(responseText);
  if (command && command.action === 'select_files') {
    autoSelectFiles(command.files);
    showStatusMessage(`Auto-selected ${command.files.length} files`, 'success');
  }
}

/**
 * Automatycznie zaznacza pliki
 */
export function autoSelectFiles(filePaths) {
  fileTreeData.forEach(project => {
    if (!selectedNodes.has(project.projectPath)) {
      selectedNodes.set(project.projectPath, new Set());
    }
    const projectSelection = selectedNodes.get(project.projectPath);

    filePaths.forEach(requestedPath => {
      const normalizedPath = requestedPath.replace(/\\/g, '/');
      if (fileExistsInTree(project.tree, normalizedPath)) {
        projectSelection.add(normalizedPath);
      }
    });
  });

  const dataToRender = searchQuery ? filterFileTree(fileTreeData) : fileTreeData;
  renderMergedFileTree(dataToRender);
  updateSelectionInfo();
}

/**
 * Sprawdza czy plik istnieje w drzewie
 */
function fileExistsInTree(nodes, path) {
  for (const node of nodes) {
    if (node.path === path) return true;
    if (node.children && fileExistsInTree(node.children, path)) return true;
  }
  return false;
}

// ============================================================================
// Auto-Selection from AI Commands
// ============================================================================

/**
 * Obsługuje auto-selekcję
 */
export function handleAutoSelect(commandData, clearPrevious = true) {
  parserLogger.success('handleAutoSelect called!');
  parserLogger.log('Command data:', commandData);

  let projectFileMap = new Map();
  let isNewFormat = false;

  // NOWY FORMAT: {"@gluon:backend": [...], "@gluon:frontend": [...]}
  if (typeof commandData === 'object' && !Array.isArray(commandData)) {
    isNewFormat = true;

    for (const [key, files] of Object.entries(commandData)) {
      if (key.startsWith('@gluon:') && key !== '@gluon:select' && key !== '@gluon:response') {
        projectFileMap.set(key, files);
      }
    }

    parserLogger.log('New multi-project format detected:', projectFileMap);
  }
  // STARY FORMAT: ["file1.js", "file2.js"]
  else if (Array.isArray(commandData)) {
    parserLogger.log('Legacy format detected (array of files)');
    projectFileMap.set('legacy', commandData);
  }
  else {
    showStatusMessage('Invalid command format', 'error');
    return;
  }

  if (projectFileMap.size === 0) {
    showStatusMessage('No files specified in command', 'error');
    return;
  }

  if (clearPrevious) {
    parserLogger.log('Clearing previous selection...');
    selectedNodes.clear();
    document.querySelectorAll('.tree-node.selected').forEach(el => {
      el.classList.remove('selected');
    });
  }

  let selectedCount = 0;
  let notFoundFiles = [];
  let totalFiles = 0;

  if (isNewFormat) {
    const projectMapping = getProjectMapping();
    parserLogger.log('Project mapping:', projectMapping);

    for (const [gluonKey, files] of projectFileMap.entries()) {
      const projectPath = projectMapping[gluonKey];

      if (!projectPath) {
        parserLogger.warn(`Project key ${gluonKey} not found in current selection`);
        notFoundFiles.push(`${gluonKey} (not loaded)`);
        totalFiles += files.length;
        continue;
      }

      const projectData = fileTreeData.find(p => p.projectPath === projectPath);
      if (!projectData || projectData.error) {
        parserLogger.warn(`Project data not found for: ${projectPath}`);
        files.forEach(file => notFoundFiles.push(`${file} (project not loaded)`));
        totalFiles += files.length;
        continue;
      }

      if (!selectedNodes.has(projectPath)) {
        selectedNodes.set(projectPath, new Set());
      }
      const projectSelection = selectedNodes.get(projectPath);

      const normalizedPaths = files.map(p => p.replace(/\\/g, '/'));

      normalizedPaths.forEach(requestedPath => {
        totalFiles++;
        if (findAndSelectInTree(projectData.tree, requestedPath, projectSelection)) {
          selectedCount++;
        } else {
          const fileName = requestedPath.split('/').pop();
          if (findAndSelectByFileName(projectData.tree, fileName, projectSelection)) {
            selectedCount++;
          } else {
            notFoundFiles.push(`${requestedPath} (project ${gluonKey})`);
          }
        }
      });

      updateProjectStats(projectPath);
    }

  } else {
    // === STARY FORMAT: lista plików bez określenia projektu ===
    const filePaths = projectFileMap.get('legacy');
    totalFiles = filePaths.length;
    const normalizedPaths = filePaths.map(p => p.replace(/\\/g, '/'));

    fileTreeData.forEach(project => {
      if (project.error) return;

      if (!selectedNodes.has(project.projectPath)) {
        selectedNodes.set(project.projectPath, new Set());
      }
      const projectSelection = selectedNodes.get(project.projectPath);

      normalizedPaths.forEach(requestedPath => {
        if (findAndSelectInTree(project.tree, requestedPath, projectSelection)) {
          selectedCount++;
        } else {
          const fileName = requestedPath.split('/').pop();
          if (findAndSelectByFileName(project.tree, fileName, projectSelection)) {
            selectedCount++;
          }
        }
      });

      updateProjectStats(project.projectPath);
    });

    // Zbierz nieznalezione pliki
    normalizedPaths.forEach(path => {
      let found = false;
      fileTreeData.forEach(project => {
        if (project.tree && findFileInTree(project.tree, path)) {
          found = true;
        }
      });
      if (!found) {
        notFoundFiles.push(path);
      }
    });
  }

  // Odśwież UI
  if (selectedCount > 0) {
    const dataToRender = searchQuery ? filterFileTree(fileTreeData) : fileTreeData;
    renderMergedFileTree(dataToRender);
    updateSelectionInfo();

    const formatLabel = isNewFormat ? 'multi-project' : 'legacy';
    const statusMsg = clearPrevious
      ? `🤖 Selected ${selectedCount}/${totalFiles} files [${formatLabel}] (previous cleared)`
      : `🤖 Added ${selectedCount}/${totalFiles} files [${formatLabel}] to selection`;

    showStatusMessage(
      statusMsg,
      selectedCount === totalFiles ? 'success' : 'info'
    );

    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      if (tabs[0]) {
        chrome.tabs.sendMessage(tabs[0].id, {
          type: 'gluon_files_auto_selected',
          format: formatLabel,
          selectedCount: selectedCount,
          totalCount: totalFiles
        });
      }
    });
  }

  // Pokaż informację o nieznalezionych plikach
  if (notFoundFiles.length > 0) {
    parserLogger.warn('Files not found:', notFoundFiles);
    setTimeout(() => {
      showStatusMessage(
        `⚠️ ${notFoundFiles.length} file(s) not found in loaded projects`,
        'error'
      );
    }, 3500);
  }
}

// ============================================================================
// Response Parsing & Overlay
// ============================================================================

/**
 * Obsługuje wykrytą odpowiedź Gluon
 */
export function handleDetectedResponse({ rawText, messageId, sourceTabId, mcpResults, hasMcpCalls }) {
  // [NEW] Handle MCP results if they exist — inject and return early (skip normal overlay)
  if (hasMcpCalls && mcpResults && Array.isArray(mcpResults) && mcpResults.length > 0) {
    parserLogger.log('[MCP] Detected MCP results from background.js:', mcpResults);

    if (!sourceTabId) {
      parserLogger.error('[MCP] No sourceTabId — cannot inject MCP results');
    } else {
      chrome.tabs.sendMessage(sourceTabId, {
        action: 'inject_mcp_results',
        payload: { mcpResults, originalMessageId: messageId }
      }, () => {
        if (chrome.runtime.lastError) {
          parserLogger.error('[MCP] Error sending results to tab:', chrome.runtime.lastError.message);
        } else {
          parserLogger.log('[MCP] ✅ Results injected successfully');
        }
      });
    }

    // Skip regular overlay — the injected prompt drives the next model iteration
    return;
  }

  // Use imported parser from response-parser.js
  const result = parseGluonResponse(rawText, fileTreeData, allProjects);
  parserLogger.log('Parse result:', result);

  let overlayHtml = '';
  let overlayData = null;

  parserLogger.log('Parser status:', result.status, 'Type:', result.type);

  switch (result.status) {
    case 'success':
      if (result.type === 'code_locations') {
        overlayHtml = createCodeLocationsOverlay(result.data);
      } else if (result.type === 'interactive_context') {
        overlayHtml = createInteractiveContextOverlay(result.data);
      } else if (result.type === 'structured_output') {
        overlayHtml = createStructuredOutputOverlay(result.data);
      } else {
        overlayHtml = createSuccessOverlay(result.data);
      }
      overlayData = result.data;
      break;
    case 'partial':
      overlayHtml = createPartialOverlay(result.data);
      overlayData = result.data;
      break;
    case 'error':
      if (result.type !== 'format') {
        overlayHtml = createErrorOverlay(result);
        overlayData = result;
      }
      parserLogger.log('Error overlay generated:', !!overlayHtml, result.message);
      break;
  }

  if (overlayHtml) {
    // [FIX] Include sourceTabId so background.js can route to correct tab
    chrome.runtime.sendMessage({
      action: 'render_gluon_overlay',
      payload: {
        messageId,
        overlayHtml,
        overlayData,
        sourceTabId  // Pass through the tab ID
      }
    });
  }
}

// NOTE: parseGluonResponse was removed from here. 
// We now import the robust version from ../utils/response-parser.js

/**
 * Tworzy overlay dla code locations
 */
function createCodeLocationsOverlay(data) {
  const locationsHtml = data.locations.map(loc => {
    const finalPath = `${loc.project}/${loc.file}`.replace(/\\/g, '/');

    return `
      <li class="gluon-overlay-item">
        <div class="gluon-overlay-item-info">
          <span class="gluon-overlay-item-main" title="${escapeHTML(loc.searchText)}">${escapeHTML(loc.searchText)}</span>
          <span class="gluon-overlay-item-sub" title="${escapeHTML(loc.file)}">${loc.line ? `:${loc.line}` : ''}</span>
        </div>
        <button class="gluon-btn-find"
                data-filepath="${escapeHTML(finalPath)}"
                data-searchtext="${escapeHTML(loc.searchText)}">
          Find
        </button>
      </li>
    `;
  }).join('');

  return `
    <div class="gluon-response-overlay">
      <div class="gluon-overlay-header">
        <span class="gluon-overlay-logo">⚡️</span>
        <span class="gluon-overlay-title">Code Inspector</span>
        <button class="gluon-btn-ignore" title="Close">&times;</button>
      </div>
      <ul class="gluon-overlay-list">${locationsHtml}</ul>
    </div>
  `;
}

/**
 * Tworzy overlay dla trybu interaktywnego (Context Architect)
 */
function createInteractiveContextOverlay(data) {
  const step = data.next_step;
  const reason = step.reasoning || "No reasoning provided.";
  const ops = step.context_ops || { load: [], drop: [] };

  // Extract loaded files from context_ops for display
  const loadedFiles = (ops.load || []).filter(item =>
    item.type === 'full_file' || item.type === 'semantic_map'
  );
  const loadedFilesList = loadedFiles.length > 0 ? `
    <div class="overlay-files-section" style="margin-bottom: 12px; padding: 8px; background: rgba(79, 195, 247, 0.1); border-left: 3px solid #4fc3f7; border-radius: 4px;">
      <strong style="color: #4fc3f7;">📁 Loaded Files (${loadedFiles.length}):</strong>
      <ul style="margin: 8px 0 0 0; padding-left: 20px; list-style: disc;">
        ${loadedFiles.map(item => {
          const filePath = item.type === 'semantic_map'
            ? (item.paths && item.paths.length > 0 ? item.paths[0] : item.path)
            : item.path;
          return `<li style="margin: 4px 0; word-break: break-word; color: #e6edf3;">${escapeHTML(filePath)}</li>`;
        }).join('')}
      </ul>
    </div>
  ` : '';

  const loadItems = (ops.load || []).map(item => {
    let icon = '📄';
    let text = '';
    if (item.type === 'file_symbol') {
      icon = '⚡';
      text = `${escapeHTML(item.symbol)} <span style="opacity:0.6">in ${escapeHTML(item.path)}</span>`;
    } else if (item.type === 'rag_search') {
      icon = '🔍';
      text = `Search: "${escapeHTML(item.query)}"`;
    } else if (item.type === 'full_file') {
      icon = '📂';
      text = escapeHTML(item.path);
    } else if (item.type === 'semantic_map') {
      icon = '🗺️';
      text = escapeHTML((item.paths || [item.path] || []).join(', '));
    } else {
      text = escapeHTML(JSON.stringify(item));
    }
    return `<li class="gluon-op-item load"><span class="op-icon">${icon}</span> ${text}</li>`;
  }).join('');

  const dropItems = (ops.drop || []).map(() => {
    return `<li class="gluon-op-item drop"><span class="op-icon">🗑️</span> Drop Context</li>`;
  }).join('');

  // MCP calls section
  const mcpCalls = ops.mcp_calls || [];
  const mcpItems = mcpCalls.map(call => {
    const argsStr = call.args ? escapeHTML(JSON.stringify(call.args)) : '';
    return `<li class="gluon-op-item mcp"><span class="op-icon">🔧</span> <strong>${escapeHTML(call.tool)}</strong> <span style="opacity:0.6;font-size:11px">${argsStr}</span></li>`;
  }).join('');

  const action = step.action === 'final_answer' ? '🏁 Final Answer' : '🔄 Continue Loop';
  const headerClass = step.action === 'final_answer' ? 'final' : 'interactive';
  const hasMcp = mcpCalls.length > 0;

  return `
    <div class="gluon-response-overlay ${headerClass}">
      <div class="gluon-overlay-header" role="button" tabindex="0">
        <span class="gluon-overlay-logo">🧠</span>
        <span class="gluon-overlay-title">Context Architect${hasMcp ? ' + MCP' : ''}</span>
        <span class="gluon-toggle-icon">▼</span>
      </div>
      <div class="gluon-overlay-expandable">
        <div class="gluon-overlay-content">
          <div class="overlay-reasoning">
            <strong>Plan:</strong> ${escapeHTML(action)}<br>
            <strong>Reasoning:</strong>
            <p>${escapeHTML(reason)}</p>
          </div>

          ${loadedFilesList}

          <div class="overlay-files-section">
            <p><strong>Operations:</strong></p>
            <ul class="gluon-overlay-filelist">
              ${loadItems}
              ${dropItems}
              ${mcpItems}
            </ul>
          </div>
        </div>
      </div>
      <div class="gluon-overlay-actions">
        <button class="gluon-btn-apply interactive-btn">Fetch Data & Continue</button>
      </div>
    </div>
  `;
}

/**
 * Tworzy overlay sukcesu
 */
function createSuccessOverlay(data) {
  // 1. Structured Output (G-SOP)
  if (data.responseType === 'structured_output') {
      return createStructuredOutputOverlay(data.structuredData);
  }

  const responseType = data.responseType || 'auto_select';
  // Check if data.found exists before mapping, to support data structures that might not have it
  const filesHtml = (data.found || []).map(f => `
    <li class="gluon-file-tile" data-filepath="${escapeHTML(f.path)}" data-project="${escapeHTML(f.project)}" style="cursor: pointer;" title="Click to find in file tree">
      <span class="file-icon">📄</span>
      <span class="file-path">${escapeHTML(f.path)}</span>
    </li>
  `).join('');

  // Określ tytuł i dodatkową zawartość w zależności od typu
  let title = 'File Handoff';
  let additionalContent = '';
  let overlayClass = '';

  if (responseType === 'auto_select') {
    title = 'Auto Select';

    // Dodaj reasoning jeśli istnieje
    if (data.reasoning) {
      additionalContent = `
        <div class="overlay-reasoning">
          <strong>Reasoning:</strong>
          <p>${escapeHTML(data.reasoning)}</p>
        </div>
      `;
    }
  } else if (responseType === 'context_handoff') {
    title = 'Context Handoff';
    overlayClass = 'context-handoff';

    if (data.handoff) {
      const h = data.handoff;
      additionalContent = `
        <div class="handoff-sections">
          ${h.summary ? `
            <div class="handoff-section">
              <strong>Summary:</strong>
              <p>${escapeHTML(h.summary)}</p>
            </div>
          ` : ''}

          ${h.solved_problems && h.solved_problems.length > 0 ? `
            <div class="handoff-section">
              <strong>Solved Problems:</strong>
              <ul>
                ${h.solved_problems.map(p => `<li>${escapeHTML(p)}</li>`).join('')}
              </ul>
            </div>
          ` : ''}

          ${h.current_problem ? `
            <div class="handoff-section">
              <strong>Current Problem:</strong>
              <p>${escapeHTML(h.current_problem)}</p>
            </div>
          ` : ''}

          ${h.key_insights ? `
            <div class="handoff-section">
              <strong>Key Insights:</strong>
              <p>${escapeHTML(h.key_insights)}</p>
            </div>
          ` : ''}
        </div>
      `;
    }
  } else if (responseType === 'prompt_handoff') {
    title = 'Prompt Handoff';
    overlayClass = 'prompt-handoff';

    if (data.handoff) {
      const h = data.handoff;
      additionalContent = `
        <div class="handoff-sections">
          ${h.task_description ? `
            <div class="handoff-section">
              <strong>Task Description:</strong>
              <p>${escapeHTML(h.task_description)}</p>
            </div>
          ` : ''}

          ${h.implementation_steps && h.implementation_steps.length > 0 ? `
            <div class="handoff-section">
              <strong>Implementation Steps:</strong>
              <ol>
                ${h.implementation_steps.map(step => `<li>${escapeHTML(step)}</li>`).join('')}
              </ol>
            </div>
          ` : ''}

          ${h.technologies ? `
            <div class="handoff-section">
              <strong>Technologies:</strong>
              <p>${escapeHTML(h.technologies)}</p>
            </div>
          ` : ''}

          ${h.architecture ? `
            <div class="handoff-section">
              <strong>Architecture:</strong>
              <p>${escapeHTML(h.architecture)}</p>
            </div>
          ` : ''}

          ${h.code_context ? `
            <div class="handoff-section">
              <strong>Code Context:</strong>
              <p>${escapeHTML(h.code_context)}</p>
            </div>
          ` : ''}
        </div>
      `;
    }

    // Dodaj reasoning jeśli istnieje (dla prompt_handoff też może być)
    if (data.reasoning) {
      additionalContent += `
        <div class="overlay-reasoning">
          <strong>File Selection Reasoning:</strong>
          <p>${escapeHTML(data.reasoning)}</p>
        </div>
      `;
    }
  }

  return `
    <div class="gluon-response-overlay ${overlayClass} collapsed">
      <div class="gluon-overlay-header" role="button" tabindex="0">
        <span class="gluon-overlay-logo">⚡️</span>
        <span class="gluon-overlay-title">${title}</span>
        <span class="gluon-toggle-icon">▼</span>
      </div>
      <div class="gluon-overlay-expandable">
        <div class="gluon-overlay-content">
          ${additionalContent}
          <div class="overlay-files-section">
            <p><strong>Selected Files</strong> (${(data.found || []).length})</p>
            <ul class="gluon-overlay-filelist">${filesHtml}</ul>
          </div>
        </div>
      </div>
      <div class="gluon-overlay-actions">
        <button class="gluon-btn-apply">Apply to Selection</button>
      </div>
    </div>
  `;
}

/**
 * Converts basic Markdown to HTML
 * Supports: **bold**, *italic*, `code`, lists (bullets, numbers, a/b/c, A/B/C), headers, line breaks
 /**
  * Converts basic Markdown to HTML
  * Supports: **bold**, *italic*, `code`, lists (bullets, numbers, a/b/c, A/B/C), headers, line breaks
  */
 function parseMarkdownToHTML(markdown) {
      if (!markdown) return '';

      let html = markdown;

      // Helper to escape HTML entities
      const escapeNonMarkdown = (text) => {
          return text.replace(/&/g, '&amp;')
                     .replace(/</g, '&lt;')
                     .replace(/>/g, '&gt;');
      };

      // 1. Code blocks (```code```) - preserve before processing
      const codeBlocks = [];
      // Regex that handles optional language identifier and uses non-greedy matching for code content
      html = html.replace(/```(?:[a-zA-Z0-9-]+\s*\n)?([\s\S]*?)```/g, (match, code) => {
          const placeholder = `GLUONCODEBLOCK${codeBlocks.length}END`;
          codeBlocks.push(`<pre style="background: rgba(0,0,0,0.3); padding: 8px; border-radius: 4px; overflow-x: auto; margin: 8px 0; font-family: monospace;"><code>${escapeNonMarkdown(code.trim())}</code></pre>`);
          return placeholder;
      });

      // 2. Inline code (`code`) - preserve before processing
      const inlineCodes = [];
      html = html.replace(/`([^`]+)`/g, (match, code) => {
          const placeholder = `GLUONINLINECODE${inlineCodes.length}END`;
          inlineCodes.push(`<code style="background: rgba(0,0,0,0.3); padding: 2px 6px; border-radius: 3px; font-family: monospace; font-size: 0.9em;">${escapeNonMarkdown(code)}</code>`);
          return placeholder;
      });

      // 3. Headers (must be at start of line) - process BEFORE list formatting
      html = html.replace(/^### (.+)$/gm, '<h3 style="font-size: 16px; font-weight: 600; margin: 12px 0 6px 0; color: #00d4ff;">$1</h3>');
      html = html.replace(/^## (.+)$/gm, '<h2 style="font-size: 18px; font-weight: 600; margin: 14px 0 8px 0; color: #00ff88;">$1</h2>');
      html = html.replace(/^# (.+)$/gm, '<h1 style="font-size: 20px; font-weight: 700; margin: 16px 0 10px 0; color: #ffffff;">$1</h1>');

      // 4. Bold (**text** or __text__)
      // Safe regex that avoids matching partial internal code strings if they somehow leak
      html = html.replace(/\*\*(.+?)\*\*/g, '<strong style="font-weight: 700; color: #ffffff;">$1</strong>');
      html = html.replace(/(?<!_)__(.+?)__(?!_)/g, '<strong style="font-weight: 700; color: #ffffff;">$1</strong>');

      // 5. Italic (*text* or _text_) - only if not part of **
      html = html.replace(/(?<!\*)\*([^\*]+)\*(?!\*)/g, '<em style="font-style: italic; color: #e2e8f0;">$1</em>');
      html = html.replace(/(?<!_)_([^_]+)_(?!_)/g, '<em style="font-style: italic; color: #e2e8f0;">$1</em>');

      // 6. Pre-process inline sub-lists: " - A) text - B) text" -> newlines
      html = html.replace(/(\n\s+)- ([A-Da-d])\)/g, '$1\n   $2)');

      // 7. Ordered lists with numbers (1. item, 2. item)
      html = html.replace(/^(\d+)\.\s+(.+?)$/gm, (match, num, content) => {
          return `<li data-num="${num}" style="margin: 6px 0; line-height: 1.8;">${content}</li>`;
      });
      html = html.replace(/(<li data-num="\d+"[^>]*>[\s\S]+?<\/li>)+/g, (match) => {
          return `<ol style="margin: 12px 0; padding-left: 28px; list-style: decimal; color: #e2e8f0;">${match.replace(/data-num="\d+"\s*/g, '')}</ol>`;
      });

      // 8. Alphabetical sub-lists (A), B), C), D) or a), b), c), d) - with or without leading dash
      html = html.replace(/^(\s*)-?\s*([A-Da-d])\)\s+(.+?)$/gm, (match, indent, letter, content) => {
          return `<li data-letter="${letter}" style="margin: 4px 0; line-height: 1.6;">${content}</li>`;
      });
      html = html.replace(/(<li data-letter="[A-Da-d]"[^>]*>[\s\S]+?<\/li>)+/g, (match) => {
          return `<ul style="margin: 8px 0 12px 20px; padding-left: 24px; list-style: none; color: #cbd5e1;">${match.replace(/data-letter="([A-Da-d])"\s*/g, (m, letter) => {
              return `style="margin: 4px 0; line-height: 1.6;" data-marker="${letter}) "`;
          }).replace(/<li([^>]*)data-marker="([^"]+)"([^>]*)>/g, '<li$1$3><span style="font-weight: 600; margin-right: 8px; color: #94a3b8;">$2</span>')}</ul>`;
      });

      // 9. Unordered lists (- item, * item, • item)
      html = html.replace(/^[\-\*•]\s+(.+?)$/gm, '<li style="margin: 4px 0; line-height: 1.6;">$1</li>');
      html = html.replace(/(<li style="margin: 4px 0[^>]*>[\s\S]+?<\/li>)+/g, (match) => {
          if (match.includes('<ol') || match.includes('<ul')) return match;
          return `<ul style="margin: 8px 0; padding-left: 24px; list-style: disc; color: #e2e8f0;">${match}</ul>`;
      });

      // 10. Restore inline codes (before paragraph processing)
      inlineCodes.forEach((code, i) => {
          const placeholder = `GLUONINLINECODE${i}END`;
          html = html.replace(new RegExp(placeholder, 'g'), code);
      });

      // 11. Line breaks (double newline = paragraph break)
      html = html.split('\n\n').map(para => {
          if (para.trim().startsWith('<')) return para;
          if (para.trim() === '') return '';
          return `<p style="margin: 8px 0; line-height: 1.7;">${para.trim()}</p>`;
      }).filter(p => p).join('\n');

      // 12. Single line breaks within paragraphs
      html = html.replace(/<p([^>]*)>([^<]+(?:<(?!\/p>)[^<]*)*)<\/p>/g, (match, attrs, content) => {
          const withBreaks = content.replace(/\n/g, '<br>');
          return `<p${attrs}>${withBreaks}</p>`;
      });

      // 13. Restore code blocks (LAST)
      codeBlocks.forEach((code, i) => {
          const placeholder = `GLUONCODEBLOCK${i}END`;
          html = html.replace(new RegExp(placeholder, 'g'), code);
      });

      return html;
  }

  /**
  * Creates HTML for Structured Output Overlay
 }

 /**
 * Creates HTML for Structured Output Overlay
 * Separates Thought Process, User Message, and Actions
 */
function createStructuredOutputOverlay(responseData) {
    // [FIX] Access structuredData property where the actual content lives
    // If structuredData is missing, fallback to responseData itself (for safety)
    const data = responseData.structuredData || responseData;

    const changesCount = data.file_changes ? data.file_changes.length : 0;
    const opsCount = data.context_ops?.load ? data.context_ops.load.length : 0;

    // Helper to generate GIT-STYLE diff preview (Apply System styling)
    const generateSmartDiffPreview = (search, replace) => {
        // [FIX] Obsługa nowych plików (pusty search_code) i usuniętych plików (pusty replace_code)
        if (!search && !replace) return '<div style="color: #6e7681; font-style: italic;">No preview available</div>';

        // Git-style diff container
        let html = '<div class="gluon-diff-container" style="border: 1px solid #30363d; border-radius: 6px; background: #0d1117; max-height: 400px; overflow: auto; width: 100%;">';

        // Nowy plik - pokaż tylko added lines z oznaczeniem
        if (!search && replace) {
            const replaceLines = replace.split('\n');
            html += '<div class="diff-row" style="display: table; width: 100%; background: rgba(46, 160, 67, 0.1); padding: 4px 8px; color: #3fb950; font-style: italic; border-bottom: 1px solid #30363d;">✨ New file</div>';
            replaceLines.forEach((line, i) => {
                const lineNum = i + 1;
                html += `
                    <div class="diff-row" style="display: table; width: 100%; table-layout: fixed; border-collapse: collapse;">
                        <div class="diff-num" style="display: table-cell; width: 50px; text-align: right; padding-right: 8px; color: #6e7681; background: rgba(46, 160, 67, 0.15); border-right: 1px solid #30363d; user-select: none; opacity: 0.7; font-size: 11px; line-height: 18px; vertical-align: top;">${lineNum}</div>
                        <div class="diff-line" style="display: table-cell; font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; font-size: 12px; line-height: 18px; white-space: pre-wrap; word-break: break-all; padding: 0 8px; color: #3fb950; background: rgba(46, 160, 67, 0.15);"><span style="user-select: none; margin-right: 8px; opacity: 0.8;">+</span>${escapeHTML(line)}</div>
                    </div>`;
            });
            html += '</div>';
            return html;
        }

        // Usunięty plik - pokaż tylko removed lines z oznaczeniem
        if (search && !replace) {
            const searchLines = search.split('\n');
            html += '<div class="diff-row" style="display: table; width: 100%; background: rgba(248, 81, 73, 0.1); padding: 4px 8px; color: #ff7b72; font-style: italic; border-bottom: 1px solid #30363d;">🗑️ File deleted</div>';
            searchLines.forEach((line, i) => {
                const lineNum = i + 1;
                html += `
                    <div class="diff-row" style="display: table; width: 100%; table-layout: fixed; border-collapse: collapse;">
                        <div class="diff-num" style="display: table-cell; width: 50px; text-align: right; padding-right: 8px; color: #6e7681; background: rgba(248, 81, 73, 0.15); border-right: 1px solid #30363d; user-select: none; opacity: 0.7; font-size: 11px; line-height: 18px; vertical-align: top;">${lineNum}</div>
                        <div class="diff-line" style="display: table-cell; font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; font-size: 12px; line-height: 18px; white-space: pre-wrap; word-break: break-all; padding: 0 8px; color: #ff7b72; background: rgba(248, 81, 73, 0.15);"><span style="user-select: none; margin-right: 8px; opacity: 0.8;">-</span>${escapeHTML(line)}</div>
                    </div>`;
            });
            html += '</div>';
            return html;
        }

        // Standardowa modyfikacja - pokazuj removed, potem added
        const searchLines = search.split('\n');
        const replaceLines = replace.split('\n');

        // REMOVED LINES (-)
        searchLines.forEach((line, i) => {
            const lineNum = i + 1;
            html += `
                <div class="diff-row" style="display: table; width: 100%; table-layout: fixed; border-collapse: collapse;">
                    <div class="diff-num" style="display: table-cell; width: 50px; text-align: right; padding-right: 8px; color: #6e7681; background: rgba(248, 81, 73, 0.15); border-right: 1px solid #30363d; user-select: none; opacity: 0.7; font-size: 11px; line-height: 18px; vertical-align: top;">${lineNum}</div>
                    <div class="diff-line" style="display: table-cell; font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; font-size: 12px; line-height: 18px; white-space: pre-wrap; word-break: break-all; padding: 0 8px; color: #ff7b72; background: rgba(248, 81, 73, 0.15);"><span style="user-select: none; margin-right: 8px; opacity: 0.8;">-</span>${escapeHTML(line)}</div>
                </div>`;
        });

        // ADDED LINES (+)
        replaceLines.forEach((line, i) => {
            const lineNum = i + 1;
            html += `
                <div class="diff-row" style="display: table; width: 100%; table-layout: fixed; border-collapse: collapse;">
                    <div class="diff-num" style="display: table-cell; width: 50px; text-align: right; padding-right: 8px; color: #6e7681; background: rgba(46, 160, 67, 0.15); border-right: 1px solid #30363d; user-select: none; opacity: 0.7; font-size: 11px; line-height: 18px; vertical-align: top;">${lineNum}</div>
                    <div class="diff-line" style="display: table-cell; font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; font-size: 12px; line-height: 18px; white-space: pre-wrap; word-break: break-all; padding: 0 8px; color: #3fb950; background: rgba(46, 160, 67, 0.15);"><span style="user-select: none; margin-right: 8px; opacity: 0.8;">+</span>${escapeHTML(line)}</div>
                </div>`;
        });

        html += '</div>';
        return html;
    };

    let actionsHtml = '';

    if (changesCount > 0) {
        const fileCards = data.file_changes.map((c, index) => `
            <div class="gluon-change-card" data-change-index="${index}" style="margin-bottom: 10px; background: #0d1117; border: 1px solid #30363d; border-radius: 6px; overflow: hidden; max-width: 100%;">
                <div class="change-header" style="padding: 8px 12px; background: #161b22; border-bottom: 1px solid #30363d; display: flex; align-items: center; justify-content: space-between;">
                    <div style="display: flex; align-items: center; gap: 8px; color: #c9d1d9; font-size: 13px; font-weight: 500;">
                        <span style="font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 350px;" title="${escapeHTML(c.file_path)}">${escapeHTML(c.file_path)}</span>
                        <span class="change-status-badge" data-status="pending" style="font-size: 10px; padding: 2px 6px; border-radius: 3px; background: #30363d; color: #8b949e; display: none;">Pending</span>
                    </div>
                    <div style="display: flex; align-items: center; gap: 8px;">
                        <button class="gluon-btn-undo-change" data-change-index="${index}" style="display: none; background: #da3633; border: 1px solid #f85149; color: white; padding: 2px 8px; border-radius: 4px; font-size: 11px; cursor: pointer; transition: all 0.2s;" title="Undo this change">
                            ↶ Undo
                        </button>
                        <span style="font-size: 11px; color: #6e7681; white-space: nowrap;">#${index + 1}</span>
                    </div>
                </div>
                <div class="change-progress-bar" style="display: none; height: 3px; background: #30363d; overflow: hidden;">
                    <div class="change-progress-fill" style="width: 0%; height: 100%; background: linear-gradient(90deg, #1f6feb, #58a6ff); transition: width 0.3s ease;"></div>
                </div>
                <div class="change-body" style="padding: 0; overflow: hidden; max-width: 100%;">
                    ${generateSmartDiffPreview(c.search_code, c.replace_code)}
                </div>
            </div>
        `).join('');

        actionsHtml += `
        <div class="handoff-section" style="border-left: 2px solid #3b82f6; background: rgba(59, 130, 246, 0.03); margin-top: 16px; padding: 12px; border-radius: 4px;">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
                <strong style="color: #94a3b8; font-size: 13px; text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600;">Code Changes (${changesCount})</strong>
            </div>
            ${fileCards}
        </div>`;
    }

    if (opsCount > 0) {
        actionsHtml += `
        <div class="handoff-section" style="border-left: 2px solid #8b5cf6; background: rgba(139, 92, 246, 0.03); margin-top: 10px; padding: 12px; border-radius: 4px;">
            <strong style="color: #94a3b8; font-size: 13px; text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600;">Context Operations (${opsCount})</strong>
            <ul style="margin: 8px 0 0 0; padding-left: 20px; font-size: 13px; color: #cbd5e1; line-height: 1.8;">
                ${data.context_ops.load.map(op => {
                    let desc = '';

                    // file_symbol: pokazujemy symbol + ścieżkę
                    if (op.type === 'file_symbol') {
                        desc = op.symbol
                            ? `<strong>${escapeHTML(op.symbol)}</strong> <span style="opacity:0.6">in ${escapeHTML(op.path)}</span>`
                            : escapeHTML(op.path || 'unknown');
                    }
                    // rag_search: pokazujemy zapytanie
                    else if (op.type === 'rag_search') {
                        desc = `"${escapeHTML(op.query || 'unknown')}"`;
                    }
                    // full_file: pokazujemy ścieżkę
                    else if (op.type === 'full_file') {
                        desc = escapeHTML(op.path || 'unknown');
                    }
                    // fallback: pokazujemy co się da
                    else {
                        desc = escapeHTML(op.path || op.symbol || op.query || 'unknown');
                    }

                    return `<li><span style="color: #64748b;">${op.type}:</span> ${desc}</li>`;
                }).join('')}
            </ul>
        </div>`;
    }

    // User message and reasoning might be at top level or inside structuredData
    let userMessage = data.user_message || responseData.user_message || "No message provided.";
    let reasoning = data.reasoning || data.thought_process || responseData.reasoning || "";

    // Convert escaped newlines to actual newlines before parsing
    userMessage = userMessage.replace(/\\n/g, '\n');
    reasoning = reasoning.replace(/\\n/g, '\n');

    return `
    <style>
        /* [GLUON UPDATE] Hide original JSON block when overlay is present (by default) */
        .gluon-response-overlay.structured-output + pre,
        .gluon-response-overlay.structured-output + ms-code-block,
        .gluon-response-overlay.structured-output + p,
        .gluon-response-overlay.structured-output + div,
        pre:has(+ .gluon-response-overlay.structured-output),
        ms-code-block:has(+ .gluon-response-overlay.structured-output),
        /* Ukryj pre które jest rodzicem overlay (czasami injectuje do środka) */
        pre:has(> .gluon-response-overlay.structured-output),
        ms-code-block:has(> .gluon-response-overlay.structured-output) {
            display: none !important;
        }

        /* Upewnijmy się, że sam kontener na wiadomość użytkownika jest przewijalny */
        .gluon-user-message-box {
            padding: 12px;
            background: rgba(56, 139, 253, 0.1);
            border: 1px solid rgba(56, 139, 253, 0.2);
            border-radius: 6px;
            font-size: 13px;
            line-height: 1.5;
            color: #e6edf3;
            margin-bottom: 16px;
            max-height: 600px;
            overflow-y: auto;
        }

        /* Show original JSON when toggle is active */
        .gluon-response-overlay.structured-output.show-original + pre,
        .gluon-response-overlay.structured-output.show-original + ms-code-block,
        .gluon-response-overlay.structured-output.show-original + p,
        .gluon-response-overlay.structured-output.show-original + div,
        pre:has(+ .gluon-response-overlay.structured-output.show-original),
        ms-code-block:has(+ .gluon-response-overlay.structured-output.show-original),
        pre:has(> .gluon-response-overlay.structured-output.show-original),
        ms-code-block:has(> .gluon-response-overlay.structured-output.show-original) {
            display: block !important;
        }

        /* Hide overlay content when showing original */
        .gluon-response-overlay.structured-output.show-original .gluon-overlay-expandable {
            display: none !important;
        }

        /* Jeśli overlay jest wstawiony ZA pre (standardowy inject) */
        .gluon-response-overlay.structured-output {
            margin-top: -10px; /* Slight pull up if needed */
        }
    </style>

    <div class='gluon-response-overlay structured-output expanded'>
        <div class='gluon-overlay-header' style="background: #161b22; padding: 10px 12px; border: 1px solid #30363d; border-bottom: none; border-radius: 6px 6px 0 0; display: flex; align-items: center; justify-content: space-between;">
            <div style="display: flex; align-items: center; gap: 8px; cursor: pointer;" class="gluon-header-main">
                <span class="gluon-overlay-title" style="font-weight: 500; color: #c9d1d9; font-size: 13px; text-transform: uppercase; letter-spacing: 0.5px;">AI Response</span>
            </div>
            <div style="display: flex; align-items: center; gap: 8px;">
                <button class="gluon-btn-toggle-json" style="background: #0d1117; border: 1px solid #30363d; color: #8b949e; padding: 3px 8px; border-radius: 4px; cursor: pointer; font-size: 11px; transition: all 0.2s;" title="Show/Hide Original JSON">
                    JSON
                </button>
                <span class="gluon-toggle-icon" style="cursor: pointer; color: #8b949e;">▲</span>
            </div>
        </div>

        <div class='gluon-overlay-expandable' style="display: block;">
            <div class='gluon-overlay-content' style="padding: 0 12px 12px 12px;">

                <!-- User Message (Visible) -->
                <div class="gluon-user-message-box">
                    ${parseMarkdownToHTML(userMessage)}
                </div>

                <!-- Reasoning (Collapsible) -->
                ${reasoning ? `
                <details style="margin-top: 12px;">
                    <summary style="cursor: pointer; color: #8b949e; font-size: 12px; user-select: none; text-transform: uppercase; letter-spacing: 0.5px; font-weight: 500;">
                        Thought Process
                    </summary>
                    <div class="overlay-reasoning" style="margin-top: 8px; font-family: monospace; font-size: 12px; line-height: 1.6; white-space: pre-wrap; color: #8b949e; background: #0d1117; padding: 10px; border-radius: 4px; border: 1px solid #30363d;">
                        ${escapeHTML(reasoning)}
                    </div>
                </details>` : ''}

                ${actionsHtml}

            </div>
        </div>

        <div class='gluon-overlay-actions' style="padding: 12px; background: #0d1117; border: 1px solid #30363d; border-top: none; border-radius: 0 0 6px 6px; display: flex; gap: 8px; justify-content: flex-end;">
            ${changesCount > 0 ? `
            <button class='gluon-btn-apply-changes' style="background: #238636; border: 1px solid #2ea043; color: white; padding: 6px 12px; border-radius: 6px; font-size: 13px; font-weight: 500; cursor: pointer; transition: all 0.2s;">
                Apply Changes
            </button>` : ''}
            ${opsCount > 0 ? `
            <button class='gluon-btn-load-context' style="background: #1f6feb; border: 1px solid #388bfd; color: white; padding: 6px 12px; border-radius: 6px; font-size: 13px; font-weight: 500; cursor: pointer; transition: all 0.2s;">
                Load Context
            </button>` : ''}
            <button class='gluon-btn-ignore' style="background: transparent; color: #8b949e; border: 1px solid #30363d; padding: 6px 12px; border-radius: 6px; cursor: pointer; font-size: 13px; font-weight: 500; transition: all 0.2s;">Dismiss</button>
        </div>
    </div>`;
}

/**
 * Tworzy overlay częściowy
 */
function createPartialOverlay(data) {
  const responseType = data.responseType || 'auto_select';
  const foundHtml = data.found.map(f => `
    <li class="gluon-file-tile" data-filepath="${escapeHTML(f.path)}" data-project="${escapeHTML(f.project)}" style="cursor: pointer;" title="Click to find in file tree">
      <span class="file-icon">📄</span>
      <span class="file-path">${escapeHTML(f.path)}</span>
    </li>
  `).join('');
  const notFoundHtml = data.notFound.map(f => `
    <li class="gluon-file-tile-missing" data-filepath="${escapeHTML(f)}" style="cursor: pointer;" title="Click to search in file tree">
      <span class="file-icon-missing">✖️</span>
      <span class="file-path-missing">${escapeHTML(f)}</span>
    </li>
  `).join('');

  // Build additional content (reasoning/handoff) - same logic as success overlay
  let additionalContent = '';
  let overlayClass = '';
  let title = 'Partial Handoff';

  if (responseType === 'auto_select') {
    title = 'Partial Auto Select';
    if (data.reasoning) {
      additionalContent = `
        <div class="overlay-reasoning">
          <strong>Reasoning:</strong>
          <p>${escapeHTML(data.reasoning)}</p>
        </div>
      `;
    }
  } else if (responseType === 'context_handoff') {
    title = 'Partial Context Handoff';
    overlayClass = 'context-handoff';

    if (data.handoff) {
      const h = data.handoff;
      additionalContent = `
        <div class="handoff-sections">
          ${h.summary ? `<div class="handoff-section"><strong>Summary:</strong><p>${escapeHTML(h.summary)}</p></div>` : ''}
          ${h.solved_problems && h.solved_problems.length > 0 ? `<div class="handoff-section"><strong>Solved Problems:</strong><ul>${h.solved_problems.map(p => `<li>${escapeHTML(p)}</li>`).join('')}</ul></div>` : ''}
          ${h.current_problem ? `<div class="handoff-section"><strong>Current Problem:</strong><p>${escapeHTML(h.current_problem)}</p></div>` : ''}
          ${h.key_insights ? `<div class="handoff-section"><strong>Key Insights:</strong><p>${escapeHTML(h.key_insights)}</p></div>` : ''}
        </div>
      `;
    }
  } else if (responseType === 'prompt_handoff') {
    title = 'Partial Prompt Handoff';
    overlayClass = 'prompt-handoff';

    if (data.handoff) {
      const h = data.handoff;
      additionalContent = `
        <div class="handoff-sections">
          ${h.task_description ? `<div class="handoff-section"><strong>Task Description:</strong><p>${escapeHTML(h.task_description)}</p></div>` : ''}
          ${h.implementation_steps && h.implementation_steps.length > 0 ? `<div class="handoff-section"><strong>Implementation Steps:</strong><ol>${h.implementation_steps.map(step => `<li>${escapeHTML(step)}</li>`).join('')}</ol></div>` : ''}
          ${h.technologies ? `<div class="handoff-section"><strong>Technologies:</strong><p>${escapeHTML(h.technologies)}</p></div>` : ''}
          ${h.architecture ? `<div class="handoff-section"><strong>Architecture:</strong><p>${escapeHTML(h.architecture)}</p></div>` : ''}
          ${h.code_context ? `<div class="handoff-section"><strong>Code Context:</strong><p>${escapeHTML(h.code_context)}</p></div>` : ''}
        </div>
      `;
    }

    if (data.reasoning) {
      additionalContent += `<div class="overlay-reasoning"><strong>File Selection Reasoning:</strong><p>${escapeHTML(data.reasoning)}</p></div>`;
    }
  }

  return `
    <div class="gluon-response-overlay ${overlayClass} collapsed">
      <div class="gluon-overlay-header partial" role="button" tabindex="0">
        <span class="gluon-overlay-logo">⚡️</span>
        <span class="gluon-overlay-title">${title}</span>
        <span class="gluon-toggle-icon">▼</span>
      </div>
      <div class="gluon-overlay-expandable">
        <div class="gluon-overlay-content">
          ${additionalContent}
          <div class="overlay-files-section">
            <p><strong>Files Status:</strong> Found ${data.found.length} of ${data.found.length + data.notFound.length}</p>
            <div class="gluon-overlay-split-list">
              <div class="list-section"><strong>Found:</strong><ul class="gluon-overlay-filelist">${foundHtml}</ul></div>
              <div class="list-section"><strong>Not Found:</strong><ul class="gluon-overlay-filelist">${notFoundHtml}</ul></div>
            </div>
          </div>
        </div>
      </div>
      <div class="gluon-overlay-actions">
        <button class="gluon-btn-apply">Apply ${data.found.length} Found Files</button>
      </div>
    </div>
  `;
}

/**
 * Tworzy overlay błędu
 */
/**
 * Analyzes error and provides specific suggestions
 */
function analyzeError(result) {
  const errorDetails = {
    icon: '⚠️',
    title: 'Error',
    suggestions: [],
    retryPrompt: ''
  };

  const message = result.message || '';
  const type = result.type || '';

  // Error Type 1: Invalid G-PROTOCOL Format
  if (message.includes('No JSON block') || message.includes('Invalid JSON format')) {
    errorDetails.icon = '📋';
    errorDetails.title = 'Invalid Response Format';
    errorDetails.suggestions = [
      'The AI response must be a JSON object wrapped in a markdown code block',
      'Format: ```json\\n{ ... }\\n```',
      'Check the RESPONSE FORMAT section in the prompt'
    ];
    errorDetails.retryPrompt = 'Please respond with a valid JSON object following the RESPONSE FORMAT in the prompt.';
  }
  // Error Type 2: Missing G-PROTOCOL Tags
  else if (message.includes('missing required keys') || message.includes('@gluon:response') || message.includes('@gluon:files')) {
    errorDetails.icon = '🏷️';
    errorDetails.title = 'Missing Required Fields';
    errorDetails.suggestions = [
      'JSON must include "@gluon:response" field (e.g., "auto_select")',
      'JSON must include "@gluon:files" object with project files',
      'Example: { "@gluon:response": "auto_select", "@gluon:files": {...} }'
    ];
    errorDetails.retryPrompt = 'Please include both "@gluon:response" and "@gluon:files" fields in your JSON response.';
  }
  // Error Type 3: Markdown in XML Detected
  else if (type === 'code_patch' || message.includes('gluon_patch')) {
    errorDetails.icon = '🔧';
    errorDetails.title = 'Code Patch Format Issue';
    errorDetails.suggestions = [
      'DO NOT use markdown (```) inside <search> or <replace> tags',
      'DO NOT include UI artifacts like "code", "Code", or language labels',
      'Use RAW source code only - exactly as it appears in the file',
      'Preserve exact whitespace and indentation'
    ];
    errorDetails.retryPrompt = 'Please use G-PROTOCOL XML format with raw source code only (no markdown, no UI artifacts).';
  }
  // Error Type 4: File/Project Not Found
  else if (message.includes('not found') || message.includes('not loaded') || message.includes('do not exist')) {
    errorDetails.icon = '📁';
    errorDetails.title = 'Files or Projects Not Found';
    errorDetails.suggestions = [
      'Use project IDs from the AVAILABLE PROJECTS list',
      'Use relative paths from project root (e.g., "src/components/Button.tsx")',
      'Do NOT use leading slashes in paths',
      'Verify the project is loaded in Gluon sidebar'
    ];

    // Extract suggestions if available
    if (result.data && result.data.suggestions && result.data.suggestions.length > 0) {
      errorDetails.suggestions.push('');
      errorDetails.suggestions.push('Did you mean:');
      result.data.suggestions.forEach(s => {
        errorDetails.suggestions.push(`  → ${s.from} → ${s.to}`);
      });
    }

    errorDetails.retryPrompt = 'Please use correct project IDs and file paths from the AVAILABLE PROJECTS list.';
  }
  // Error Type 5: Generic Format Error
  else {
    errorDetails.icon = '❌';
    errorDetails.title = 'Response Error';
    errorDetails.suggestions = [
      'Review the RESPONSE FORMAT section in the prompt',
      'Ensure JSON syntax is valid (no trailing commas, proper quotes)',
      'Use project IDs from AVAILABLE PROJECTS list',
      'Follow the exact format shown in the examples'
    ];
    errorDetails.retryPrompt = 'Please review the prompt instructions and try again with the correct format.';
  }

  return errorDetails;
}

function createErrorOverlay(result) {
  const errorDetails = analyzeError(result);

  const suggestionsHtml = errorDetails.suggestions.map(s =>
    `<li>${escapeHTML(s)}</li>`
  ).join('');

  return `
    <div class="gluon-response-overlay collapsed">
      <div class="gluon-overlay-header error" role="button" tabindex="0">
        <span class="gluon-overlay-logo">${errorDetails.icon}</span>
        <span class="gluon-overlay-title">${errorDetails.title}</span>
        <span class="gluon-toggle-icon">▼</span>
      </div>
      <div class="gluon-overlay-expandable">
        <div class="gluon-overlay-content">
          <p><strong>Error:</strong> ${escapeHTML(result.message)}</p>

          ${suggestionsHtml.length > 0 ? `
          <div class="gluon-error-suggestions">
            <strong>💡 Suggestions:</strong>
            <ul>${suggestionsHtml}</ul>
          </div>
          ` : ''}

          ${errorDetails.retryPrompt ? `
          <div class="gluon-retry-prompt">
            <strong>🔄 Retry Instructions:</strong>
            <p>${escapeHTML(errorDetails.retryPrompt)}</p>
          </div>
          ` : ''}
        </div>
      </div>
    </div>
  `;
}

/**
 * Aplikuje selekcję z overlay
 */
export function handleApplySelection(payload) {
  parserLogger.log('===== APPLY SELECTION DEBUG =====');
  parserLogger.log('Full payload:', payload);

  const filesToSelect = payload.files;
  parserLogger.log('Applying selection from overlay:', filesToSelect);

  selectedNodes.clear();
  filesToSelect.forEach(file => {
    if (!selectedNodes.has(file.project)) {
      selectedNodes.set(file.project, new Set());
    }
    selectedNodes.get(file.project).add(file.path);
  });

  const dataToRender = searchQuery ? filterFileTree(fileTreeData) : fileTreeData;
  renderMergedFileTree(dataToRender);
  updateSelectionInfo();

  const quickTaskInput = document.getElementById('quickTaskInput');
  let actionTaken = false;

  if (payload.responseType === 'context_handoff' && payload.handoff && quickTaskInput) {
    actionTaken = true;
    const handoff = payload.handoff;

    const solvedProblemsText = Array.isArray(handoff.solved_problems) && handoff.solved_problems.length > 0
      ? `✅ Solved:\n${handoff.solved_problems.map(p => `- ${p}`).join('\n')}\n\n`
      : '';

    const formattedHandoff = `📋 CONTEXT HANDOFF:\n\nSummary: ${handoff.summary}\n\n${solvedProblemsText}🎯 Current: ${handoff.current_problem}\n\n${handoff.key_insights ? `💡 Insights: ${handoff.key_insights}` : ''}`;

    quickTaskInput.value = formattedHandoff;
    quickTaskInput.scrollIntoView({ behavior: 'smooth', block: 'center' });
    quickTaskInput.style.background = 'rgba(139, 92, 246, 0.2)';
    setTimeout(() => { quickTaskInput.style.background = ''; }, 2000);
    showStatusMessage(`✅ Context handoff loaded to Quick Task!`, 'success');
  }

  if (payload.responseType === 'prompt_handoff' && payload.handoff && quickTaskInput) {
    actionTaken = true;
    const handoff = payload.handoff;
    const formattedPrompt = `=== TASK DESCRIPTION ===\n${handoff.task_description || ''}\n\n=== IMPLEMENTATION STEPS ===\n${(Array.isArray(handoff.implementation_steps) ? handoff.implementation_steps.map(s => `- ${s}`).join('\n') : '')}\n\n=== TECHNOLOGIES ===\n${handoff.technologies || ''}\n\n=== ARCHITECTURE ===\n${handoff.architecture || ''}\n\n=== CODE CONTEXT ===\n${handoff.code_context || ''}`;

    quickTaskInput.value = formattedPrompt;
    quickTaskInput.scrollIntoView({ behavior: 'smooth', block: 'center' });
    quickTaskInput.style.background = 'rgba(0, 212, 255, 0.15)';
    setTimeout(() => { quickTaskInput.style.background = ''; }, 2000);
    showStatusMessage(`✅ Prompt loaded to Quick Task!`, 'success');
  }

  if (!actionTaken && filesToSelect.length > 0) {
    showStatusMessage(`✅ Applied selection of ${filesToSelect.length} files!`, 'success');
  }

  const firstFile = filesToSelect[0];
  if (firstFile) {
    const firstNode = document.querySelector(`.tree-node[data-path="${firstFile.path}"][data-project="${firstFile.project}"]`);
    if (firstNode) {
      firstNode.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  }
}

/**
 * Obsługuje logikę "Context Node" - Wykonanie operacji G-INTERACTIVE
 * NOWA IMPLEMENTACJA: Używa zmodernizowanego context-node.js (batch operations)
 */
export async function handleApplyInteractiveContext(payload) {
  parserLogger.log('🧠 [G-Interactive] Executing Context Node request:', payload);
  showStatusMessage('🧠 Gluon RAG: Fetching context...', 'info');

  // Extract routing IDs injected by the content script / background
  const overlayMessageId = payload._overlayMessageId;
  const sourceTabId = payload._sourceTabId;

  // Helper: notify content script overlay of the result
  const sendFeedback = (success, itemCount = 0, errorMsg = '') => {
      if (!overlayMessageId || !sourceTabId) return;
      chrome.runtime.sendMessage({
          action: 'notify_interactive_context_done',
          payload: { overlayMessageId, sourceTabId, success, itemCount, errorMsg }
      });
  };

  try {
    // MULTI-PROJECT SUPPORT: Match each operation to its appropriate project

    // Use all projects without filtering out Gluon (user might be working on Gluon itself)
    const workspaceProjects = allProjects;

    parserLogger.log('[G-Interactive] Workspace projects available:',
        workspaceProjects.map(p => p.path));

    // Helper function to find best matching project for a file path
    const findProjectForPath = (filePath) => {
        if (!filePath || !workspaceProjects || workspaceProjects.length === 0) return null;

        const normalizedFilePath = filePath.replace(/\\/g, '/').toLowerCase();

        // [GLUON UPDATE] Strict Path Matching
        // Najpierw sprawdzamy pełne ścieżki (czy plik jest wewnątrz projektu)
        for (const project of workspaceProjects) {
            const projectPath = project.path.replace(/\\/g, '/').toLowerCase();

            // Jeśli ścieżka pliku zawiera ścieżkę projektu (np. "c:/users/.../projekt/src/plik.js")
            if (normalizedFilePath.includes(projectPath)) {
                parserLogger.log(`[G-Interactive] Matched by inclusion "${filePath}" inside "${project.path}"`);
                return project.path;
            }
        }

        // 1. Exact Match Strategy: Check if any project path contains this file path
        // OR if the file path starts with the project name
        for (const project of workspaceProjects) {
            const projectPath = project.path.replace(/\\/g, '/').toLowerCase();
            const projectName = projectPath.split('/').pop();

            // Case A: filePath starts with project name (e.g. "gluon-v2/extension/src")
            if (normalizedFilePath.startsWith(projectName + '/')) {
                parserLogger.log(`[G-Interactive] Matched by prefix "${filePath}" to project "${project.path}"`);
                return project.path;
            }

            // Case B: project path ends with the first segment of file path
            // (e.g. project ".../gluon-v2", file "gluon-v2/src...")
            // Uwaga: To działa dla względnych ścieżek typu "gluon-v2/src"
            const firstSegment = normalizedFilePath.split('/')[0];
            if (projectPath.endsWith('/' + firstSegment) || projectPath === firstSegment) {
                 parserLogger.log(`[G-Interactive] Matched by segment "${filePath}" to project "${project.path}"`);
                 return project.path;
            }
        }

        // 2. Fuzzy Match Strategy (LAST RESORT)
        // Uwaga: Fuzzy match może być niebezpieczny, jeśli mamy wiele projektów o podobnych nazwach
        // Dlatego używamy go tylko jeśli nie znaleziono dopasowania ścisłego
        for (const project of workspaceProjects) {
            const projectName = project.path.split(/[\\/]/).pop()?.toLowerCase();
            if (normalizedFilePath.includes(projectName)) {
                parserLogger.log(`[G-Interactive] Fuzzy matched "${filePath}" to project "${project.path}"`);
                return project.path;
            }
        }

        return null;
    };

    // Helper to get default project (fallback)
    const getDefaultProject = () => {
        // Strategy 1: Use first selected project from state
        if (selectedProjects && selectedProjects.size > 0) {
            const firstSelected = Array.from(selectedProjects)[0];
            parserLogger.log(`[G-Interactive] Using selected project as default: ${firstSelected}`);
            return firstSelected;
        }
        // Strategy 2: Use first available workspace project
        if (workspaceProjects && workspaceProjects.length > 0) {
            const firstAvailable = workspaceProjects[0].path;
            parserLogger.log(`[G-Interactive] Using first available project as default: ${firstAvailable}`);
            return firstAvailable;
        }
        return null;
    };

    // Extract operations and match each to a project
    const contextOps = payload.next_step?.context_ops;
    const operationsByProject = new Map(); // Map<projectRoot, operations[]>

    if (contextOps && contextOps.load) {
        for (const op of contextOps.load) {
            // Get file path from operation
            let filePath = op.path;
            if (!filePath && op.paths && Array.isArray(op.paths) && op.paths.length > 0) {
                filePath = op.paths[0]; // Use first path for matching
            }

            // Match to project
            let projectRoot = filePath ? findProjectForPath(filePath) : null;

            // Fallback to default if no match
            if (!projectRoot) {
                projectRoot = getDefaultProject();
                if (filePath) {
                    parserLogger.warn(`[G-Interactive] ⚠️ No project match for "${filePath}", using default:`, projectRoot);
                }
            }

            if (!projectRoot) {
                parserLogger.error(`[G-Interactive] ❌ Cannot determine project for operation:`, op);
                continue; // Skip this operation
            }

            // Group by project
            if (!operationsByProject.has(projectRoot)) {
                operationsByProject.set(projectRoot, []);
            }
            operationsByProject.get(projectRoot).push(op);
        }
    }

    // Note: contextOps.rag_search should not exist in the modern format
    // All operations should be in contextOps.load (see response-parser.js)
    // This block is kept for legacy compatibility but should rarely be triggered

    if (operationsByProject.size === 0) {
        showStatusMessage('⚠️ No operations could be matched to projects', 'warning');
        sendFeedback(false, 0, 'No operations matched to any project');
        return;
    }

    parserLogger.log(`[G-Interactive] 📊 Operations grouped by project:`,
        Array.from(operationsByProject.entries()).map(([proj, ops]) =>
            `${proj}: ${ops.length} ops`
        ));

    // Call new unified Context Node handler with multi-project support
    const result = await handleNextStepRequest(payload, operationsByProject);

    if (!result.success) {
      showStatusMessage(`❌ Context fetch failed: ${result.error}`, 'error');
      sendFeedback(false, 0, result.error);
      return;
    }

    if (result.action === 'final_answer') {
      showStatusMessage('✅ Model indicated task completion.', 'success');
      sendFeedback(true, 0);
      return;
    }

    // Success - context was loaded and injected
    const { contextResponse } = result;
    const { successful, failed, total_operations } = contextResponse;

    if (successful > 0) {
        showStatusMessage(
          `✅ Pasted ${successful} context items to chat.`,
          'success'
        );
        sendFeedback(true, successful);
    } else {
        showStatusMessage(
          `⚠️ No data found (${failed} failed). Check console.`,
          'warning'
        );
        sendFeedback(false, 0, `${failed} operation(s) failed`);
    }

    parserLogger.log('[G-Interactive] ✅ Context injected and sent to AI');

  } catch (err) {
    parserLogger.error('[G-Interactive] ❌ Error:', err);
    showStatusMessage(`❌ Context fetch failed: ${err.message}`, 'error');
    sendFeedback(false, 0, err.message);
  }
}

/**
 * Obsługuje kliknięcie prompt generator
 */
export function handlePromptGeneratorClick() {
  const promptText = `w tej rozmowie tworzymy tylko plan. Kod będę pisał z innym modelem który otrzyma precyzyjny kontekst plików projektu, lub projektów. Napisz dla niego prompt bez podawania kodu. Model musi dostać prompt który określa zadanie i kierunek zadania, bardzo precyzyjnie, Ponieważ będzie pracował na mniejszym bardziej precyzyjnym kontekście. Teraz podaj prompt + [json] z plikami które uważasz za potrzebne przy zadaniu`;

  showStatusMessage('Pasting prompt generator instruction...', 'info');
  chrome.runtime.sendMessage({
    action: 'paste_prompt',
    payload: promptText
  });
}