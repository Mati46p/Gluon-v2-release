/**
 * Creates the HTML for the success overlay.
 * @param {object} data - The parsed response data from the parser.
 * @returns {string} The HTML string for the overlay.
 */
function createSuccessOverlay(data) {
    // 1. Structured Output (G-SOP)
    // NOTE: Structured output is handled in aiIntegrationCommandParsing.js
    // Commenting out to prevent duplicate overlay rendering
    // if (data.responseType === 'structured_output') {
    //     return createStructuredOutputOverlay(data.structuredData);
    // }

    // Check response type and render accordingly
    if (data.responseType === 'context_handoff') {
        return createContextHandoffOverlay(data);
    }
    if (data.responseType === 'prompt_handoff') {
        return createPromptHandoffOverlay(data);
    }

    // Default: auto_select overlay
    const filesByProject = data.files.reduce((acc, file) => {
        const projectName = file.project.split(/[\\/]/).pop();
        if (!acc[projectName]) acc[projectName] = [];
        acc[projectName].push(file.path);
        return acc;
    }, {});

    const fileLists = Object.entries(filesByProject).map(([projectName, files]) => `
        <div class="files-list">
            <strong>${projectName} (${files.length}):</strong>
            <ul>
                ${files.map(path => `<li>${path}</li>`).join('')}
            </ul>
        </div>
    `).join('');

    return `
    <div class='gluon-response-overlay'>
        <div class='overlay-header'>🎯 Gluon Auto-Select</div>
        <div class='overlay-body'>
            <div class='reasoning'>
                <strong>Reasoning:</strong> ${data.reasoning || 'No reasoning provided.'}
            </div>
            ${fileLists}
        </div>
        <div class='overlay-actions'>
            <button class='gluon-btn-apply'>✔ Select All (${data.files.length})</button>
            <button class='gluon-btn-ignore'>✗ Ignore</button>
        </div>
    </div>`;
}

/**
 * Creates the HTML for context handoff overlay.
 * @param {object} data - The parsed context handoff data.
 * @returns {string} The HTML string for the overlay.
 */
function createContextHandoffOverlay(data) {
    const handoff = data.handoff;
    
    const filesByProject = data.files.reduce((acc, file) => {
        const projectName = file.project.split(/[\\/]/).pop();
        if (!acc[projectName]) acc[projectName] = [];
        acc[projectName].push(file.path);
        return acc;
    }, {});

    const fileLists = Object.entries(filesByProject).map(([projectName, files]) => `
        <div class="files-list">
            <strong>${projectName} (${files.length}):</strong>
            <ul>
                ${files.map(path => `<li>${path}</li>`).join('')}
            </ul>
        </div>
    `).join('');
    
    const solvedProblemsHtml = handoff.solved_problems && handoff.solved_problems.length > 0 
        ? `<div class="handoff-section">
            <strong>✅ Solved:</strong>
            <ul>${handoff.solved_problems.map(p => `<li>${p}</li>`).join('')}</ul>
           </div>`
        : '';
    
    const keyInsightsHtml = handoff.key_insights 
        ? `<div class="handoff-section">
            <strong>💡 Key Insights:</strong>
            <p>${handoff.key_insights}</p>
           </div>`
        : '';

    return `
    <div class='gluon-response-overlay context-handoff'>
        <div class='overlay-header'>📋 Context Handoff</div>
        <div class='overlay-body'>
            <div class="handoff-section">
                <strong>📝 Summary:</strong>
                <p>${handoff.summary || 'No summary provided.'}</p>
            </div>
            ${solvedProblemsHtml}
            <div class="handoff-section">
                <strong>🎯 Current Problem:</strong>
                <p>${handoff.current_problem || 'Not specified.'}</p>
            </div>
            <div class="handoff-section">
                <strong>➡️ Next Steps:</strong>
                <p>${handoff.next_steps || 'Not specified.'}</p>
            </div>
            ${keyInsightsHtml}
            <div class="found-files">
                <strong>Files to attach (${data.files.length}):</strong>
                ${fileLists}
            </div>
        </div>
        <div class='overlay-actions'>
            <button class='gluon-btn-apply'>✔ Select All Files (${data.files.length})</button>
            <button class='gluon-btn-ignore'>✗ Dismiss</button>
        </div>
    </div>`;
}

/**
 * Creates the HTML for the prompt handoff overlay.
 * @param {object} data - The parsed prompt handoff data.
 * @returns {string} The HTML string for the overlay.
 */
function createPromptHandoffOverlay(data) {
    const filesByProject = data.files.reduce((acc, file) => {
        const projectName = file.project.split(/[\\/]/).pop();
        if (!acc[projectName]) acc[projectName] = [];
        acc[projectName].push(file.path);
        return acc;
    }, {});

    const fileLists = Object.entries(filesByProject).map(([projectName, files]) => `
        <div class="files-list">
            <strong>${projectName} (${files.length}):</strong>
            <ul>
                ${files.map(path => `<li>${path}</li>`).join('')}
            </ul>
        </div>
    `).join('');

    const escapeHtml = (unsafe) => {
        if (!unsafe) return '';
        return unsafe
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#039;");
    };

    return `
    <div class='gluon-response-overlay prompt-handoff'>
        <div class='overlay-header'>🤖 AI Prompt Generated</div>
        <div class='overlay-body'>
            <div class='reasoning'>
                <strong>Reasoning:</strong> ${escapeHtml(data.reasoning)}
            </div>
            <div class="prompt-preview">
                <strong>Prompt Preview:</strong>
                <pre style="white-space: pre-wrap; word-wrap: break-word;"><code>${escapeHtml(data.prompt)}</code></pre>
            </div>
            <div class="found-files">
                <strong>Files to attach (${data.files.length}):</strong>
                ${fileLists}
            </div>
        </div>
        <div class='overlay-actions'>
            <button class="gluon-btn-apply">✔ Select Files & Copy Prompt</button>
            <button class='gluon-btn-ignore'>✗ Dismiss</button>
        </div>
    </div>`;
}

/**
 * Creates the HTML for the partial success overlay.
 * @param {object} data - The parsed response data with found, notFound, and suggestions.
 * @returns {string} The HTML string for the overlay.
 */
function createPartialOverlay(data) {
    const foundFiles = data.files || [];
    const notFoundFiles = data.notFound || [];
    
    // Determine overlay type
    const isContextHandoff = data.responseType === 'context_handoff';
    const isPromptHandoff = data.responseType === 'prompt_handoff';

    let headerIcon = '🎯';
    let headerText = 'Gluon Request - Partial Match';
    if (isContextHandoff) {
        headerIcon = '📋';
        headerText = 'Context Handoff - Partial Match';
    } else if (isPromptHandoff) {
        headerIcon = '🤖';
        headerText = 'AI Prompt - Partial Match';
    }

    // Grupuj pliki według projektów
    const foundByProject = foundFiles.reduce((acc, file) => {
        const projectName = file.project.split(/[\\/]/).pop();
        if (!acc[projectName]) acc[projectName] = [];
        acc[projectName].push(file);
        return acc;
    }, {});

    const notFoundByProject = notFoundFiles.reduce((acc, file) => {
        // Dla 'nieznalezionych' klucz może być ID Gluon, a nie ścieżką
        const projectName = file.reason === 'Unknown project key' 
            ? file.project // Pokaż "@gluon:nazwa"
            : file.project.split(/[\\/]/).pop();
        if (!acc[projectName]) acc[projectName] = [];
        acc[projectName].push(file);
        return acc;
    }, {});

    const foundList = foundFiles.length > 0 ? `
    <div class='found-files'>
        <strong>✓ Found (${foundFiles.length}):</strong>
        ${Object.entries(foundByProject).map(([projectName, files]) => `
            <div class="files-list">
                <strong>${projectName} (${files.length}):</strong>
                <ul>
                    ${files.map(f => `<li>${f.path}</li>`).join('')}
                </ul>
            </div>
        `).join('')}
    </div>` : '';

    const notFoundList = notFoundFiles.length > 0 ? `
    <div class='not-found-files'>
        <strong>✗ Not found (${notFoundFiles.length}):</strong>
        ${Object.entries(notFoundByProject).map(([projectName, files]) => `
            <div class="files-list">
                <strong>${projectName} (${files.length}):</strong>
                <ul>
                    ${files.map(f => `<li>${f.path} - <em>${f.reason}</em></li>`).join('')}
                </ul>
            </div>
        `).join('')}
    </div>` : '';
    
    let contextInfo = '';
    if (isContextHandoff && data.handoff) {
        contextInfo = `
        <div class="handoff-summary">
            <strong>📝 Summary:</strong>
            <p>${data.handoff.summary}</p>
        </div>`;
    } else if (data.reasoning) {
        contextInfo = `
        <div class='reasoning'>
            <strong>Reasoning:</strong> ${data.reasoning}
        </div>`;
    }

    // --- DYNAMIC BUTTON LOGIC ---
    let buttonAttrs = `class='gluon-btn-apply'`; // Usunięto atrybuty data-*
    let buttonText = `✔ Select Found (${(data.files || []).length})`;

    // Logika do ustalenia tekstu przycisku pozostaje, ale bez atrybutów
    if (data.responseType === 'context_handoff' && data.handoff) {
        // buttonAttrs bez zmian
    } else if (data.responseType === 'prompt_handoff' && data.prompt) {
        buttonText = `✔ Select Found & Copy Prompt`;
    }

    return `
    <div class='gluon-response-overlay partial ${data.responseType === 'context_handoff' ? 'context-handoff' : ''}'>
        <div class='overlay-header'>${headerIcon} ${headerText}</div>
        <div class='overlay-body'>
             ${contextInfo}
             <div class="found-split">${foundList}${notFoundList}</div>
        </div>
        <div class='overlay-actions'>
            <button ${buttonAttrs}>${buttonText}</button>
            <button class='gluon-btn-ignore'>✗ Ignore</button>
        </div>
    </div>`;
}

/**
 * Creates the HTML for the error overlay.
 * @param {object} data - The error data from the parser.
 * @returns {string} The HTML string for the overlay.
 */
function createErrorOverlay(data) {
    let detailsHtml = '';
    // data.data to obiekt validationResult przekazany z parsera
    if (data.data && data.data.notFound && data.data.notFound.length > 0) {
        detailsHtml = `
        <div class='not-found-files' style="margin-top: 10px;">
            <strong>✗ Failed Files (${data.data.notFound.length}):</strong>
            <ul>${data.data.notFound.map(f => `<li>${f.path} - <em>${f.reason}</em></li>`).join('')}</ul>
        </div>`;
    }

    return `
    <div class='gluon-response-overlay error'>
        <div class='overlay-header'>⚠️ Invalid Gluon Response</div>
        <div class='overlay-body'>
            <p>${data.message}</p>
            ${detailsHtml}
        </div>
        <div class='overlay-actions'>
            <button class='gluon-btn-ignore'>✗ Dismiss</button>
        </div>
    </div>`;
}

/**
 * Creates HTML for Structured Output Overlay
 * Separates Thought Process, User Message, and Actions
 */
function createStructuredOutputOverlay(data) {
    const changesCount = data.file_changes ? data.file_changes.length : 0;
    
    // Normalize context_ops: handle both array and object { load: [] } formats
    let contextOpsList = [];
    if (data.context_ops) {
        if (Array.isArray(data.context_ops)) {
            contextOpsList = data.context_ops;
        } else if (data.context_ops.load && Array.isArray(data.context_ops.load)) {
            contextOpsList = data.context_ops.load;
        }
    }
    const opsCount = contextOpsList.length;

    let actionsHtml = '';

    // Helper to escape HTML
    const escape = (str) => (str || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

    // Helper to generate SMART diff preview (tylko zmienione linie)
    const generateSmartDiffPreview = (search, replace) => {
        // [FIX] Obsługa nowych plików (pusty search_code) i usuniętych plików (pusty replace_code)
        if (!search && !replace) return '<div style="color: #6e7681; font-style: italic;">No preview available</div>';

        // Nowy plik - pokaż cały replace_code jako dodane linie
        if (!search && replace) {
            const replaceLines = replace.split('\n');
            let html = '<div class="gluon-diff-content" style="background: #0d1117; padding: 8px; border-radius: 4px; font-family: monospace; font-size: 11px; margin-top: 5px;">';
            html += '<div style="color: #3fb950; font-style: italic; margin-bottom: 4px; padding: 4px; background: rgba(63, 185, 80, 0.1);">✨ New file</div>';
            replaceLines.forEach(line => {
                html += `<div style="color: #3fb950; white-space: pre; overflow-x: auto; background: rgba(63, 185, 80, 0.1);"><span style="user-select: none; margin-right: 8px; color: #79c0ff;">+</span>${escape(line)}</div>`;
            });
            html += '</div>';
            return html;
        }

        // Usunięty plik - pokaż cały search_code jako usunięte linie
        if (search && !replace) {
            const searchLines = search.split('\n');
            let html = '<div class="gluon-diff-content" style="background: #0d1117; padding: 8px; border-radius: 4px; font-family: monospace; font-size: 11px; margin-top: 5px;">';
            html += '<div style="color: #ff7b72; font-style: italic; margin-bottom: 4px; padding: 4px; background: rgba(255, 123, 114, 0.1);">🗑️ File deleted</div>';
            searchLines.forEach(line => {
                html += `<div style="color: #ff7b72; white-space: pre; overflow-x: auto; background: rgba(255, 123, 114, 0.1);"><span style="user-select: none; margin-right: 8px; color: #79c0ff;">-</span>${escape(line)}</div>`;
            });
            html += '</div>';
            return html;
        }

        const searchLines = search.split('\n');
        const replaceLines = replace.split('\n');

        // Wykonaj line-by-line diff
        const maxLen = Math.max(searchLines.length, replaceLines.length);
        const diffBlocks = [];
        let currentBlock = null;
        let unchangedCount = 0;

        for (let i = 0; i < maxLen; i++) {
            const searchLine = searchLines[i] || '';
            const replaceLine = replaceLines[i] || '';
            const isChanged = searchLine !== replaceLine;

            if (isChanged) {
                // Jeśli mamy accumulated unchanged lines, dodaj je jako separator
                if (unchangedCount >= 3) {
                    diffBlocks.push({ type: 'separator', count: unchangedCount });
                    unchangedCount = 0;
                    currentBlock = null;
                } else if (unchangedCount > 0) {
                    // Małe unchanged bloki (< 3 lines) pokazuj jako context
                    if (!currentBlock) currentBlock = { type: 'diff', lines: [] };
                    for (let j = 0; j < unchangedCount; j++) {
                        currentBlock.lines.push({ type: 'context', line: searchLines[i - unchangedCount + j] });
                    }
                    unchangedCount = 0;
                }

                // Dodaj zmienione linie
                if (!currentBlock) {
                    currentBlock = { type: 'diff', lines: [] };
                    diffBlocks.push(currentBlock);
                }

                if (searchLine && searchLine !== replaceLine) {
                    currentBlock.lines.push({ type: 'removed', line: searchLine });
                }
                if (replaceLine && replaceLine !== searchLine) {
                    currentBlock.lines.push({ type: 'added', line: replaceLine });
                }
            } else {
                unchangedCount++;
            }
        }

        // Dodaj trailing separator jeśli potrzebny
        if (unchangedCount >= 3) {
            diffBlocks.push({ type: 'separator', count: unchangedCount });
        } else if (unchangedCount > 0 && currentBlock) {
            // Małe trailing context
            for (let j = 0; j < unchangedCount; j++) {
                currentBlock.lines.push({ type: 'context', line: searchLines[searchLines.length - unchangedCount + j] });
            }
        }

        // Renderuj diff blocks
        let html = '<div class="gluon-diff-content" style="background: #0d1117; padding: 8px; border-radius: 4px; font-family: monospace; font-size: 11px; margin-top: 5px;">';

        diffBlocks.forEach(block => {
            if (block.type === 'separator') {
                html += `<div style="color: #6e7681; font-style: italic; text-align: center; padding: 4px 0; border-top: 1px dashed #30363d; border-bottom: 1px dashed #30363d; margin: 4px 0;">⋯ ${block.count} unchanged lines ⋯</div>`;
            } else if (block.type === 'diff') {
                block.lines.forEach(({ type, line }) => {
                    if (type === 'removed') {
                        html += `<div style="color: #ff7b72; white-space: pre; overflow-x: auto; background: rgba(255, 123, 114, 0.1);"><span style="user-select: none; margin-right: 8px; color: #79c0ff;">-</span>${escape(line)}</div>`;
                    } else if (type === 'added') {
                        html += `<div style="color: #3fb950; white-space: pre; overflow-x: auto; background: rgba(63, 185, 80, 0.1);"><span style="user-select: none; margin-right: 8px; color: #79c0ff;">+</span>${escape(line)}</div>`;
                    } else if (type === 'context') {
                        html += `<div style="color: #8b949e; white-space: pre; overflow-x: auto; opacity: 0.6;"><span style="user-select: none; margin-right: 8px; color: transparent;">·</span>${escape(line)}</div>`;
                    }
                });
            }
        });

        html += '</div>';
        return html;
    };

    // Render File Changes Section
    if (changesCount > 0) {
        const fileCards = data.file_changes.map((c, index) => `
            <div class="gluon-change-card" style="margin-bottom: 12px; background: #161b22; border: 1px solid #30363d; border-radius: 6px; overflow: hidden;">
                <div class="change-header" style="padding: 8px 12px; background: #21262d; border-bottom: 1px solid #30363d; display: flex; align-items: center; justify-content: space-between;">
                    <div style="display: flex; align-items: center; gap: 8px; color: #e6edf3; font-size: 12px; font-weight: 600;">
                        <span>📝</span>
                        <span style="font-family: monospace;">${escape(c.file_path)}</span>
                    </div>
                    <span style="font-size: 10px; color: #8b949e;">Modification #${index + 1}</span>
                </div>
                <div class="change-body" style="padding: 0;">
                    ${generateSmartDiffPreview(c.search_code, c.replace_code)}
                </div>
            </div>
        `).join('');

        actionsHtml += `
        <div class="handoff-section" style="border-left: 3px solid #00ff88; background: rgba(0, 255, 136, 0.05); margin-top: 16px; padding: 12px; border-radius: 0 4px 4px 0;">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
                <strong style="color: #00ff88; font-size: 13px;">⚡ Code Modifications (${changesCount})</strong>
            </div>
            ${fileCards}
        </div>`;
    }

    // Render Context Operations Section
    if (opsCount > 0) {
        const opsList = contextOpsList.map(op => {
            let icon = '📥';
            let label = op.type;
            let details = op.path || op.symbol || op.query || '';
            let color = '#e2e8f0';
            
            if (op.type === 'semantic_map') { icon = '🗺️'; label = 'Map Structure'; color = '#f0db4f'; }
            if (op.type === 'rag_search') { icon = '🔍'; label = 'RAG Search'; color = '#ff7b72'; }
            if (op.type === 'full_file') { icon = '📄'; label = 'Read File'; color = '#79c0ff'; }
            if (op.type === 'file_symbol') { icon = '🧩'; label = 'Read Symbol'; color = '#d2a8ff'; }
            
            return `
            <li style="display: flex; align-items: center; gap: 10px; padding: 8px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.05); border-radius: 4px; margin-bottom: 6px;">
                <span style="font-size: 16px;">${icon}</span>
                <div style="flex: 1; min-width: 0;">
                    <div style="color: ${color}; font-size: 12px; font-weight: 600; margin-bottom: 2px;">${label}</div>
                    <div style="color: #8b949e; font-size: 11px; font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;" title="${escape(details)}">${escape(details)}</div>
                </div>
            </li>`;
        }).join('');

        actionsHtml += `
        <div class="handoff-section" style="border-left: 3px solid #00d4ff; background: rgba(0, 212, 255, 0.05); margin-top: 12px; padding: 12px; border-radius: 0 4px 4px 0;">
            <strong style="color: #00d4ff; margin-bottom: 10px; display: block; font-size: 13px;">🧠 Context Operations (${opsCount})</strong>
            <ul style="list-style: none; padding: 0; margin: 0;">
                ${opsList}
            </ul>
        </div>`;
    }

    // Default message if no actions
    if (!changesCount && !opsCount) {
        actionsHtml += `
        <div style="padding: 20px; text-align: center; color: #8b949e; font-style: italic;">
            No automated actions in this step.
        </div>`;
    }

    return `
    <div class='gluon-response-overlay structured-output expanded'>
        <div class='gluon-overlay-header' style="background: linear-gradient(135deg, #161b22, #0d1117); padding: 12px; border-bottom: 1px solid #30363d; display: flex; align-items: center; justify-content: space-between; cursor: pointer;">
            <div style="display: flex; align-items: center; gap: 10px;">
                <div style="width: 24px; height: 24px; background: #1f6feb; border-radius: 4px; display: flex; align-items: center; justify-content: center; font-size: 14px;">🤖</div>
                <span class="gluon-overlay-title" style="font-weight: 600; color: #e6edf3; font-size: 14px;">Gluon Actions</span>
            </div>
            <span class="gluon-toggle-icon" style="color: #8b949e;">▲</span>
        </div>
        
        <div class="gluon-overlay-expandable">
            <div class='overlay-body' style="padding: 16px; background: #0d1117;">

                <!-- User Message (Visible) -->
                <div style="padding: 12px; background: rgba(56, 139, 253, 0.1); border: 1px solid rgba(56, 139, 253, 0.2); border-radius: 6px; font-size: 13px; line-height: 1.5; color: #e6edf3; margin-bottom: 16px;">
                    ${escape(data.user_message) || "No message provided."}
                </div>

                <!-- Reasoning (Collapsible) -->
                <details style="margin-bottom: 16px;">
                    <summary style="cursor: pointer; color: #8b949e; font-size: 11px; user-select: none; display: flex; align-items: center; gap: 4px; padding: 4px 0;">
                        <span style="font-size: 10px;">▶</span> 👁️ View Thought Process
                    </summary>
                    <div class="overlay-reasoning" style="margin-top: 8px; font-family: monospace; font-size: 11px; line-height: 1.4; white-space: pre-wrap; color: #8b949e; background: #161b22; padding: 12px; border-radius: 6px; border: 1px solid #30363d;">${escape(data.thought_process || data.reasoning || "No thoughts recorded.")}</div>
                </details>

                ${actionsHtml}

            </div>
        </div>
        
        <div class='overlay-actions' style="border-top: 1px solid #30363d; background: #161b22; padding: 12px; display: flex; justify-content: flex-end; gap: 8px;">
            ${changesCount > 0 ? `
            <button class='gluon-btn-apply-changes' style="background: #238636; border: 1px solid rgba(240,246,252,0.1); color: white; padding: 6px 12px; border-radius: 6px; font-size: 12px; font-weight: 600; cursor: pointer; transition: background 0.2s;">
                ⚡ Apply Changes
            </button>` : ''}
            ${opsCount > 0 ? `
            <button class='gluon-btn-load-context' style="background: #1f6feb; border: 1px solid rgba(240,246,252,0.1); color: white; padding: 6px 12px; border-radius: 6px; font-size: 12px; font-weight: 600; cursor: pointer; transition: background 0.2s;">
                🧠 Load Context
            </button>` : ''}
            <button class='gluon-btn-ignore' style="background: transparent; border: 1px solid #30363d; color: #8b949e; padding: 6px 12px; border-radius: 6px; font-size: 12px; font-weight: 600; cursor: pointer; transition: color 0.2s;">
                Dismiss
            </button>
        </div>
    </div>`;
}