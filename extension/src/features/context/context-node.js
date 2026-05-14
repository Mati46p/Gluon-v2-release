/**
 * G-INTERACTIVE CONTEXT NODE
 *
 * Module responsible for executing context operations on behalf of the AI model.
 * This implements the "PULL" model where the AI requests specific code fragments
 * instead of receiving a gigantic dump of all files.
 *
 * Architecture:
 * - Model (Gemini/Claude) → Analyzes task with Repo Skeleton
 * - Model → Requests specific context via JSON (@gluon:next_step)
 * - Extension (This Module) → Detects request, calls Rust backend
 * - Backend → Executes operations (FileSymbol, FullFile, RAG)
 * - Extension → Formats results into .txt file
 * - Extension → Attaches .txt file to AI Studio input (like regular file attachment)
 * - Model → Receives context as file attachment and continues work
 */

import { sidebarLogger } from '../../common/logger.js';

// ============================================================================
// CONTEXT NODE CONFIGURATION
// ============================================================================

const CONFIG = {
    // Maximum time to wait for backend response (ms)
    BACKEND_TIMEOUT: 30000,

    // Whether to auto-inject results into AI Studio
    AUTO_INJECT: true,

    // Whether to auto-send after injection (NOTE: Not applicable for file attachments)
    AUTO_SEND: false, // Disabled because files cannot be auto-sent

    // Format for injected context
    INJECT_FORMAT: 'formatted', // 'formatted' or 'raw'

    // Whether to use file attachment instead of text paste
    USE_FILE_ATTACHMENT: true, // true = attach .txt file, false = paste text

    // Auto-cleanup old context files
    AUTO_CLEANUP_ENABLED: true, // true = delete previous context when attaching new one
    KEEP_FIRST_CONTEXT: true,    // true = never delete the first complete context file

    // [ENTERPRISE] Context Retention Rules
    RETENTION_RULES: {
        // ADDED 'gluon_rag_context_' to ensure generated files are cleaned up
        EPHEMERAL_PREFIX: ['gluon-v2-context-', 'gluon_ctx_', 'gluon_code_', 'gluon_rag_context_'], 
        PERSISTENT_PREFIX: ['gluon_map_', 'gluon_skeleton', 'gluon_meta_'] // Files to keep
    }
};

// ============================================================================
// CONTEXT ATTACHMENT HISTORY
// ============================================================================

/**
 * State management for tracking attached context files
 * - firstContextFile: The initial complete context (never deleted)
 * - previousContextFile: The last attached context (deleted when new one is attached)
 * - currentContextFile: The most recently attached context
 */
const contextHistory = {
    firstContextFile: null,
    previousContextFile: null,
    currentContextFile: null
};

/**
 * Loads context history from Chrome storage
 */
async function loadContextHistory() {
    return new Promise((resolve) => {
        chrome.storage.local.get(['gluon_context_history'], (result) => {
            if (result.gluon_context_history) {
                Object.assign(contextHistory, result.gluon_context_history);
                sidebarLogger.log('[Context Cleanup] 📂 Loaded history:', contextHistory);
            }
            resolve(contextHistory);
        });
    });
}

/**
 * Saves context history to Chrome storage
 */
async function saveContextHistory() {
    return new Promise((resolve) => {
        chrome.storage.local.set({ gluon_context_history: contextHistory }, () => {
            sidebarLogger.log('[Context Cleanup] 💾 Saved history:', contextHistory);
            resolve();
        });
    });
}

/**
 * Checks if a file is ephemeral (should be deleted)
 */
function isEphemeralFile(filename) {
    if (!filename) return false;
    return CONFIG.RETENTION_RULES.EPHEMERAL_PREFIX.some(prefix => filename.startsWith(prefix));
}

/**
 * Deletes a context file (or turn) from the chat by simulating deletion
 * [ENTERPRISE] Upgrade: Uses 'delete_context_turn' for full turn removal
 * @param {string} filename - Name of the file to delete
 */
async function deleteContextFile(filename) {
    if (!filename) return false;

    // Safety check: Only delete if it matches ephemeral patterns
    if (!isEphemeralFile(filename)) {
        sidebarLogger.log(`[Context Cleanup] 🛡️ Skipping deletion of persistent file: ${filename}`);
        return false;
    }

    try {
        sidebarLogger.log(`[Context Cleanup] 🗑️ Attempting to delete TURN containing: ${filename}`);

        // Send message to content script to find and delete the entire turn
        // This is critical for keeping the chat clean
        return new Promise((resolve) => {
            chrome.runtime.sendMessage({
                action: 'delete_context_turn',
                payload: { filename: filename }
            }, (response) => {
                if (chrome.runtime.lastError) {
                    sidebarLogger.warn('[Context Cleanup] Msg error:', chrome.runtime.lastError.message);
                    resolve(false);
                } else if (response && response.success) {
                    sidebarLogger.log(`[Context Cleanup] ✅ Turn deleted successfully: ${filename}`);
                    resolve(true);
                } else {
                    sidebarLogger.warn('[Context Cleanup] ⚠️ Delete failed or file not found.');
                    resolve(false);
                }
            });
        });
    } catch (error) {
        sidebarLogger.error(`[Context Cleanup] ❌ Exception deleting ${filename}:`, error);
        return false;
    }
}

/**
 * Resets the context history tracking
 * Useful when starting a new chat or switching projects
 * @param {boolean} clearStorage - Whether to clear from Chrome storage as well
 */
async function resetContextHistory(clearStorage = true) {
    sidebarLogger.log('[Context Cleanup] 🔄 Resetting context history');

    contextHistory.firstContextFile = null;
    contextHistory.previousContextFile = null;
    contextHistory.currentContextFile = null;

    if (clearStorage) {
        await new Promise((resolve) => {
            chrome.storage.local.remove(['gluon_context_history'], () => {
                sidebarLogger.log('[Context Cleanup] ✅ Context history cleared from storage');
                resolve();
            });
        });
    }

    sidebarLogger.log('[Context Cleanup] ✅ Context history reset complete');
}

/**
 * Gets the current context history
 * @returns {object} - Current context history state
 */
function getContextHistory() {
    return { ...contextHistory };
}

// ============================================================================
// MAIN API: Execute Context Operations
// ============================================================================

/**
 * Executes context operations requested by the AI model
 *
 * @param {Array} operations - List of context operations from model
 * @param {string} projectRoot - Project root path
 * @returns {Promise<object>} - ContextResponse from backend
 */
async function executeContextOperations(operations, projectRoot) {
    sidebarLogger.log('[Context Node] Executing', operations.length, 'operations');
    sidebarLogger.log('[Context Node] Project root:', projectRoot);
    sidebarLogger.log('[Context Node] Raw operations:', JSON.stringify(operations, null, 2));

    // Deduplicate operations: remove exact duplicates
    // Create a Set of stringified operations to track unique ones
    const seen = new Set();
    const uniqueOps = [];
    for (const op of operations) {
        const opKey = JSON.stringify(op);
        if (!seen.has(opKey)) {
            seen.add(opKey);
            uniqueOps.push(op);
        } else {
            sidebarLogger.warn('[Context Node] ⚠️ Skipping duplicate operation:', op);
        }
    }

    if (uniqueOps.length < operations.length) {
        sidebarLogger.log(`[Context Node] Deduplicated ${operations.length} operations to ${uniqueOps.length}`);
    }

    // Validate and filter operations to match Rust ContextOperation enum.
    // 'semantic_search' is the new canonical name for search operations — it maps to
    // 'rag_search' before being sent to the Rust backend (which still uses that variant).
    const VALID_TYPES = ['file_symbol', 'rag_search', 'semantic_search', 'full_file', 'semantic_map'];
    const validatedOps = uniqueOps.filter(op => {
        if (!op || !op.type) {
            sidebarLogger.warn('[Context Node] ⚠️ Skipping operation without type:', op);
            return false;
        }
        if (!VALID_TYPES.includes(op.type)) {
            sidebarLogger.warn(`[Context Node] ⚠️ Skipping unsupported operation type: "${op.type}"`, op);
            return false;
        }
        // Validate required fields per type
        if (op.type === 'file_symbol' && (!op.path || !op.symbol)) {
            sidebarLogger.warn('[Context Node] ⚠️ file_symbol missing path or symbol:', op);
            return false;
        }
        if (op.type === 'full_file' && !op.path) {
            sidebarLogger.warn('[Context Node] ⚠️ full_file missing path:', op);
            return false;
        }
        if ((op.type === 'rag_search' || op.type === 'semantic_search') && !op.query) {
            sidebarLogger.warn(`[Context Node] ⚠️ ${op.type} missing query:`, op);
            return false;
        }
        if (op.type === 'semantic_map') {
            // Handle both single 'path' string and 'paths' array
            if (op.path && typeof op.path === 'string') {
                // Normalize single path to paths array
                op.paths = [op.path];
                delete op.path;
            }
            if (!op.paths || !Array.isArray(op.paths)) {
                sidebarLogger.warn('[Context Node] ⚠️ semantic_map missing path/paths:', op);
                return false;
            }
        }
        return true;
    });

    if (validatedOps.length === 0) {
        throw new Error('No valid operations to execute after filtering. Check model output format.');
    }

    sidebarLogger.log(`[Context Node] Validated ${validatedOps.length}/${operations.length} operations`);

    return new Promise((resolve, reject) => {
        // Setup listener for response
        const responseListener = (message) => {
            if (message.type === 'execute_context_operations_response') {
                chrome.runtime.onMessage.removeListener(responseListener);

                if (message.data.error) {
                    reject(new Error(message.data.error));
                } else {
                    sidebarLogger.log('[Context Node] ✅ Backend response:', message.data);
                    sidebarLogger.log(`[Context Node] Success rate: ${message.data.successful}/${message.data.total_operations}`);
                    resolve(message.data);
                }
            }
        };

        chrome.runtime.onMessage.addListener(responseListener);

        // Send to background script — rag_search/semantic_search ops are intercepted
        // there and routed to Gluon v3 via MCP; all other ops go to the Rust backend
        chrome.runtime.sendMessage({
            action: 'execute_context_operations',
            payload: {
                operations: validatedOps,
                projectRoot: projectRoot
            }
        }).catch(error => {
            chrome.runtime.onMessage.removeListener(responseListener);
            sidebarLogger.error('[Context Node] ❌ Failed to send message:', error);
            reject(error);
        });

        // Timeout after 30 seconds
        setTimeout(() => {
            chrome.runtime.onMessage.removeListener(responseListener);
            reject(new Error('Timeout waiting for context operations response'));
        }, 30000);
    });
}

// ============================================================================
// CONTEXT FORMATTING: Convert Backend Response to Human-Readable Text
// ============================================================================

/**
 * Formats context response into human-readable text for AI
 *
 * @param {object} contextResponse - Response from backend
 * @returns {string} - Formatted text to inject into AI Studio
 */
/**
 * Formats context response into human-readable text for AI (Surgical Context)
 *
 * @param {object} contextResponse - Response from backend
 * @returns {string} - Formatted text to be saved as .txt file and attached to AI Studio
 */
function formatContextForModel(contextResponse) {
    const { request_id, items, successful, failed, total_operations } = contextResponse;

    let formatted = `━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🔍 GLUON CONTEXT REPORT (ID: ${request_id})
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Stats: ${successful} success, ${failed} failed.
`;

    // Process each item
    for (const item of items) {
        switch (item.type) {
            case 'symbol_content':
                formatted += formatSymbolContent(item);
                break;
            case 'file_content':
                formatted += formatFileContent(item);
                break;
            case 'rag_result':
                formatted += formatRagResult(item);
                break;
            case 'error':
                formatted += formatError(item);
                break;
            default:
                formatted += `⚠️ Unknown result type: ${item.type}\n`;
        }
    }

    formatted += `
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✅ CONTEXT LOADED.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
👉 **INSTRUCTION**:
1. Analyze the code above.
2. If you have enough to solve the User Task -> Provide the solution/code now.
3. If you need MORE context -> Reply with another @gluon:next_step JSON.

⚠️ **CRITICAL - STALE CONTEXT WARNING**:
If you provide code changes (SEARCH/REPLACE blocks), this context immediately becomes **STALE**.
AFTER the user applies your changes, you **MUST** issue a new \`@gluon:next_step\` request to reload these files and verify the changes were applied correctly.
DO NOT assume the code has changed until you reload it.
`;

    return formatted;
}

// ============================================================================
// FORMATTERS: Individual Item Types
// ============================================================================

function formatSymbolContent(item) {
    const { file_path, symbol_name, content } = item;

    // Detect language from file extension
    const ext = file_path.split('.').pop();
    const langMap = {
        'ts': 'typescript', 'tsx': 'typescript',
        'js': 'javascript', 'jsx': 'javascript',
        'py': 'python', 'rs': 'rust',
        'java': 'java', 'go': 'go', 'cpp': 'cpp'
    };
    const language = langMap[ext] || '';

    // Content from Rust already contains comments with file/symbol info (MVC)
    // We just wrap it in a code block
    return `
### 🧩 Symbol: \`${symbol_name}\` (${file_path})
\`\`\`${language}
${content}
\`\`\`
`;
}

function formatFileContent(item) {
    const { file_path, content, line_count } = item;

    const ext = file_path.split('.').pop();
    const langMap = {
        'ts': 'typescript',
        'tsx': 'typescript',
        'js': 'javascript',
        'jsx': 'javascript',
        'py': 'python',
        'rs': 'rust',
        'json': 'json',
        'md': 'markdown'
    };
    const language = langMap[ext] || '';

    return `### 📄 Full File: \`${file_path}\` (${line_count} lines)

\`\`\`${language}
${content}
\`\`\`

`;
}

function formatRagResult(item) {
    const { query, results } = item;

    let formatted = `
### 🧠 RAG Knowledge: "${query}"
`;

    if (!results || results.length === 0) {
        formatted += `(No relevant code found in Vector Database. Try 'file_symbol' if you know the file name.)\n`;
    } else {
        results.forEach((chunk, idx) => {
            // Chunks from backend usually come formatted as comment + code
            formatted += `
**Result ${idx + 1}:**
\`\`\`
${chunk.trim()}
\`\`\`
`;
        });
    }

    return formatted;
}

function formatError(item) {
    const { operation, error } = item;

    return `### ❌ Error in Operation: \`${operation}\`

**Error**: ${error}

💡 **Suggestion**: Check the Repo Skeleton for available symbols and file paths.

`;
}

// ============================================================================
// AI STUDIO INJECTION: Send Message to Content Script
// ============================================================================

/**
 * Creates a .txt file from context and attaches it to AI Studio input
 * (Alternative to pasting text - uses file attachment like Generate Context)
 *
 * With auto-cleanup: deletes previous context file (except the first one)
 *
 * @param {string} formattedText - Text to save as file
 * @param {string} filename - Name for the context file
 * @param {boolean} isFirstContext - Whether this is the initial complete context
 * @returns {Promise<boolean>} - Success status
 */
async function attachContextFileToAIStudio(formattedText, filename = 'gluon_context.txt', isFirstContext = false) {
    sidebarLogger.log('[Context Node] Creating context file for attachment...');

    try {
        // Load history from storage
        await loadContextHistory();

        // [ENTERPRISE] AUTO-CLEANUP LOGIC (Pre-Upload)
        // Ensure we delete the old turn BEFORE uploading the new one to avoid confusion
        if (CONFIG.AUTO_CLEANUP_ENABLED && !isFirstContext) {
            // Delete the previous context file (but NOT the first one)
            if (contextHistory.previousContextFile) {
                const previousFilename = contextHistory.previousContextFile;

                // Check if it's not the protected first context
                const isProtectedFirst = CONFIG.KEEP_FIRST_CONTEXT &&
                                        previousFilename === contextHistory.firstContextFile;

                if (!isProtectedFirst) {
                    sidebarLogger.log(`[Context Cleanup] 🧹 Deleting previous context turn: ${previousFilename}`);
                    // Await here ensures we clean up before adding new context
                    await deleteContextFile(previousFilename);
                } else {
                    sidebarLogger.log(`[Context Cleanup] 🔒 Keeping first context (protected): ${previousFilename}`);
                }
            }
        }

        // [ENTERPRISE] Send file data to background script
        // The file is now treated as the "Active Code Context"
        chrome.runtime.sendMessage({
            action: 'inject_file_to_gemini',
            payload: {
                filename: filename,
                content: formattedText,
                type: 'text/plain'
            }
        });

        // Update history tracking
        if (isFirstContext || !contextHistory.firstContextFile) {
            // This is the first complete context - never delete it
            contextHistory.firstContextFile = filename;
            sidebarLogger.log(`[Context Cleanup] 📌 Marked as first context: ${filename}`);
        }

        // Shift the history: current → previous, new → current
        contextHistory.previousContextFile = contextHistory.currentContextFile;
        contextHistory.currentContextFile = filename;

        // Save updated history
        await saveContextHistory();

        sidebarLogger.log('[Context Node] ✅ Context file attachment request sent');
        sidebarLogger.log(`[Context Cleanup] History: First=${contextHistory.firstContextFile}, Previous=${contextHistory.previousContextFile}, Current=${contextHistory.currentContextFile}`);

        return true;
    } catch (error) {
        sidebarLogger.error('[Context Node] ❌ Failed to attach context file:', error);
        return false;
    }
}

/**
 * Sends the formatted context to the Content Script for injection into the page.
 * Sidebar cannot access DOM directly, so we use messaging.
 *
 * @param {string} formattedText - Text to inject
 * @param {boolean} autoSend - Whether to automatically click Send button
 * @returns {Promise<boolean>} - Success status
 * @deprecated Use attachContextFileToAIStudio instead for G-RAG context
 */
async function injectContextIntoAIStudio(formattedText, autoSend = false) {
    sidebarLogger.log('[Context Node] Sending context to Content Script for injection...');

    return new Promise((resolve) => {
        chrome.tabs.query({ active: true, currentWindow: true }, function(tabs) {
            if (!tabs || !tabs[0] || !tabs[0].id) {
                sidebarLogger.error('[Context Node] ❌ No active tab found.');
                resolve(false);
                return;
            }

            chrome.tabs.sendMessage(tabs[0].id, {
                action: "paste_to_chat",
                text: formattedText,
                autoSend: autoSend
            }, (response) => {
                if (chrome.runtime.lastError) {
                    // This often happens if content script isn't loaded on the page
                    sidebarLogger.error('[Context Node] ❌ IPC Error (Content Script missing?):', chrome.runtime.lastError.message);
                    resolve(false);
                } else if (response && response.success) {
                    sidebarLogger.log('[Context Node] ✅ Content Script confirmed injection.');
                    resolve(true);
                } else {
                    sidebarLogger.warn('[Context Node] ⚠️ Content Script returned failure or no response.');
                    resolve(false);
                }
            });
        });
    });
}

// ============================================================================
// HIGH-LEVEL WORKFLOW: Handle @gluon:next_step Request
// ============================================================================

/**
 * Main handler for @gluon:next_step requests from AI
 * Executes the G-RAG Loop: Parse -> Retrieve -> Inject -> Auto-Send
 *
 * @param {object} nextStepData - Parsed JSON from model response
 * @param {string} projectRoot - Current project root path
 * @returns {Promise<object>} - Result object
 */
async function handleNextStepRequest(payload, projectRootOrMap) {
    sidebarLogger.log('[Context Node] 🧠 Handling @gluon:next_step request');

    try {
        // [FIX] Extract the actual data block.
        // The payload might be the raw parser result { next_step: { ... } } OR the direct data.
        const nextStepData = payload.next_step || payload;

        const action = nextStepData.action || 'continue';

        // 1. Check for Final Answer
        if (action === 'final_answer') {
            sidebarLogger.log('[Context Node] ✅ Model indicates task completion (final_answer). Loop ends.');
            return {
                success: true,
                action: 'final_answer',
                message: 'Model has completed the task'
            };
        }

        // 2. Determine if we're using multi-project mode
        const isMultiProject = projectRootOrMap instanceof Map;

        if (isMultiProject) {
            sidebarLogger.log('[Context Node] 🔀 Multi-project mode enabled');

            // Execute operations for each project and merge results
            const allResults = {
                items: [],
                successful: 0,
                failed: 0,
                total_operations: 0
            };

            for (const [projectRoot, operations] of projectRootOrMap.entries()) {
                sidebarLogger.log(`[Context Node] Executing ${operations.length} operations for project: ${projectRoot}`);

                try {
                    const contextResponse = await executeContextOperations(operations, projectRoot);

                    // Merge results
                    allResults.items.push(...(contextResponse.items || []));
                    allResults.successful += contextResponse.successful || 0;
                    allResults.failed += contextResponse.failed || 0;
                    allResults.total_operations += contextResponse.total_operations || 0;
                } catch (err) {
                    sidebarLogger.error(`[Context Node] ❌ Error executing operations for ${projectRoot}:`, err);
                    allResults.failed += operations.length;
                    allResults.total_operations += operations.length;
                }
            }

            // 4. Format the Report (Surgical Context)
            const formattedReport = formatContextForModel(allResults);

            // 5. Attach Context as .txt File
            if (CONFIG.AUTO_INJECT && CONFIG.USE_FILE_ATTACHMENT) {
                const timestamp = new Date().toISOString().replace(/[:.]/g, '-').split('T').join('_').slice(0, -5);
                const filename = `gluon_rag_context_${timestamp}.txt`;
                await attachContextFileToAIStudio(formattedReport, filename);
            }

            sidebarLogger.log(`[Context Node] ✅ Multi-project execution complete. Success: ${allResults.successful}, Failed: ${allResults.failed}`);

            return {
                success: true,
                action: action,
                contextResponse: allResults,
                formatted: formattedReport
            };
        }

        // LEGACY SINGLE-PROJECT MODE (backward compatibility)
        sidebarLogger.log('[Context Node] 📁 Single-project mode');
        const projectRoot = projectRootOrMap; // It's a string

        // 2. Validate Operations
        // Support multiple formats:
        // - Direct array: context_ops: [{type: "rag_search", ...}, ...]
        // - Nested array: context_ops: { load: [{type: "rag_search", ...}] }
        // - Single operation object: context_ops: { rag_search: {query: "...", top_k: 10} }
        let operations = [];
        const contextOps = nextStepData.context_ops;

        sidebarLogger.log('[Context Node] Raw context_ops:', JSON.stringify(contextOps));
        sidebarLogger.log('[Context Node] context_ops type:', typeof contextOps, Array.isArray(contextOps) ? '(array)' : '');

        if (Array.isArray(contextOps)) {
            // Format 1: Direct array
            operations = contextOps;
        } else if (contextOps && Array.isArray(contextOps.load)) {
            // Format 2: Nested array
            operations = contextOps.load;
        } else if (contextOps && typeof contextOps === 'object' && !contextOps.load) {
            // Format 3: Single operation as object { rag_search: {...}, file_symbol: {...} }
            // Convert to array format
            // Note: Exclude objects that have a 'load' property (Format 2) to avoid duplicates
            for (const [opType, opData] of Object.entries(contextOps)) {
                if (opData && typeof opData === 'object') {
                    operations.push({
                        type: opType,
                        ...opData
                    });
                }
            }
            sidebarLogger.log('[Context Node] Converted single-op format to array:', operations);
        } else {
             sidebarLogger.warn('[Context Node] context_ops is missing or invalid format:', nextStepData);
             sidebarLogger.log('Payload structure:', payload);
        }

        if (operations.length === 0) {
            if (action !== 'final_answer') {
                 sidebarLogger.warn('[Context Node] No operations found, but action is continue.');
                 // Optional: We could return early here if we strictly require operations
            }
        }

        /*
        if (!contextOps || !Array.isArray(contextOps.load)) {
            // Soft failure: If model messes up JSON structure, guide it back
            const errorMsg = "PROTOCOL ERROR: 'context_ops.load' must be an array. Please retry.";
            if (CONFIG.AUTO_INJECT) {
                await injectContextIntoAIStudio(`SYSTEM: ${errorMsg}`, false);
            }
            throw new Error(errorMsg);
        }
        */

        // 3. Execute Operations (Call Rust Backend)
        // Note: executeContextOperations handles the message passing to background -> rust
        const contextResponse = await executeContextOperations(operations, projectRoot);

        // 4. Format the Report (Surgical Context)
        const formattedReport = formatContextForModel(contextResponse);

        // 5. Attach Context as .txt File (instead of pasting text)
        if (CONFIG.AUTO_INJECT) {
            // Generate filename with timestamp
            const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, -5);
            const filename = `gluon_rag_context_${timestamp}.txt`;

            const attached = await attachContextFileToAIStudio(formattedReport, filename);

            if (!attached) {
                throw new Error('Failed to attach context file to AI Studio.');
            }

            sidebarLogger.success('[Context Node] 📎 Context file attached. Returning control to Model.');
        }

        return {
            success: true,
            action: 'context_loaded',
            contextResponse: contextResponse,
            formatted: formattedReport
        };

    } catch (error) {
        sidebarLogger.error('[Context Node] ❌ Critical Error in G-RAG Loop:', error);
        return {
            success: false,
            error: error.message
        };
    }
}

// ============================================================================
// EXPORTS
// ============================================================================

export {
    executeContextOperations,
    formatContextForModel,
    attachContextFileToAIStudio,
    injectContextIntoAIStudio, // Deprecated - kept for backwards compatibility
    handleNextStepRequest,
    CONFIG,
    // Context cleanup utilities
    resetContextHistory,
    getContextHistory,
    loadContextHistory,
    saveContextHistory
};