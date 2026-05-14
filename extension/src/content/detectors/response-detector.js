// Content scripts DEV_MODE - set to false for production
if (typeof CONTENT_DEV_MODE === 'undefined') {
  var CONTENT_DEV_MODE = true; // Change to false to disable logs
}

// Shared logger for all Gluon content scripts
if (typeof logger === 'undefined') {
  var logger = {
    _log: (level, ...args) => {
      // Don't log anything if not in dev mode, except errors
      if (!CONTENT_DEV_MODE && level !== 'error') {
        return;
      }

      const prefix = '[Gluon Content]';
      switch (level) {
        case 'log':
        case 'success':
          console.log(prefix, ...args);
          break;
        case 'warn':
          console.warn(prefix, ...args);
          break;
        case 'error':
          console.error(prefix, ...args);
          break;
        default:
          console.log(prefix, ...args);
      }
    },
    log: function(...args) { this._log('log', ...args); },
    warn: function(...args) { this._log('warn', ...args); },
    error: function(...args) { this._log('error', ...args); },
    success: function(...args) { this._log('success', '✅', ...args); },
  };
}

logger.log('Response detector active.');

let observer;
let debounceTimeout;
const processedMessages = new WeakSet();
const overlayDataCache = new Map();
// [FIX] Cache for last sent text to prevent duplicate events
const lastSentContent = new Map(); // messageId -> contentHash
let gluonModeEnabled = true;

function injectDetectorStyles() {
    if (!document.getElementById('gluon-detector-global-styles')) {
        const style = document.createElement('style');
        style.id = 'gluon-detector-global-styles';
        style.textContent = `
            .gluon-hidden-source {
                display: none !important;
                visibility: hidden !important;
                height: 0 !important;
                overflow: hidden !important;
            }
            ms-code-block.gluon-hidden-source, pre.gluon-hidden-source {
                display: none !important;
            }
        `;
        document.head.appendChild(style);
    }
}

function startObserver() {
    injectDetectorStyles();
    const targetNode = document.body;
    const config = { childList: true, subtree: true, characterData: true, characterDataOldValue: false };

    observer = new MutationObserver((mutationsList) => {
        let shouldScan = false;
        for (const mutation of mutationsList) {
            if (mutation.type === 'characterData' || (mutation.type === 'childList' && mutation.addedNodes.length > 0)) {
                shouldScan = true;
                break;
            }
        }

        if (shouldScan) {
            clearTimeout(debounceTimeout);
            // Reduced debounce for faster overlay detection during streaming
            debounceTimeout = setTimeout(scanForNewMessages, 300);
        }
    });

    observer.observe(targetNode, config);
    logger.log('MutationObserver started for AI responses.');
    scanForNewMessages(); // Initial scan
}

function scanForNewMessages() {
    // Skip scanning if Gluon Mode is disabled
    if (!gluonModeEnabled) {
        return;
    }

    const selectors = [
        // Gemini/AI Studio
        'ms-chat-turn.model .turn-content',
        'div[data-turn-role="Model"] .turn-content',
        '.mat-expansion-panel-body', // Fix for AI Studio Prompts view
        '.run-response-text',        // Fix for AI Studio Run results

        // Claude.ai (specific to user-provided HTML)
        '.font-claude-response', 
        
        // Generic fallbacks
        '[data-is-assistant="true"]',
        '.font-claude-message',
        'div[class*="font-user-message"] + div',
        '.assistant-message',
        '.model-response-text',
        '[role="article"]',
        '.prose'
    ];
    
    document.querySelectorAll(selectors.join(', ')).forEach(messageNode => {
        // [IMPROVED FIX] Allow reprocessing for streaming responses that add new JSON blocks
        // Only skip if:
        // 1. Is INSIDE another overlay (prevent nesting)
        // 2. Is being processed RIGHT NOW (race condition protection)
        if (messageNode.closest('.gluon-response-overlay')) {
            return;
        }

        // [FIX] Prevent double-processing of nested selectors (e.g. .turn-content inside .model)
        // If a parent element is already marked as processed or has an ID, skip this child
        if (messageNode.parentElement &&
            (messageNode.parentElement.dataset.gluonMessageId || processedMessages.has(messageNode.parentElement))) {
             return;
        }

        // [IMPROVED] Check if container is CURRENTLY being processed (race condition protection)
        // But allow reprocessing after a short delay if new content was added
        const messageContainer = messageNode.closest('.chat-turn-container, ms-chat-turn, [data-turn-role]');
        if (messageContainer) {
            // Only skip if ACTIVELY processing (will be cleared after render completes)
            if (messageContainer.dataset.gluonProcessing === 'true') {
                return;
            }
        }

        // Prioritize extracting text from a code block
        let textToParse = '';
        
        // Strategy 1: Find a code block (pre > code) containing the marker
        const codeBlocks = messageNode.querySelectorAll('pre code');
        let foundInCodeBlock = false;
        let sourceElement = null;

        // Strategy 1: Find a generic <pre><code> block
        for (const block of codeBlocks) {
            const blockText = getTextFromCodeblock(block); // Use new function
            // [FIX] Support both Legacy (@gluon:response) and Interactive (@gluon:next_step) protocols
            // Also support new Structured Output markers
            if (blockText && (
                blockText.includes('@gluon:response') || 
                blockText.includes('@gluon:next_step') ||
                blockText.includes('"gluon_actions"') ||
                blockText.includes('gluon_actions')
            )) {
                textToParse = blockText;
                // Find the parent PRE to hide later
                // [FIX] Prefer ms-code-block if it wraps the pre, to hide the entire UI component
                sourceElement = block.closest('ms-code-block') || block.closest('pre') || block;
                logger.log('Strategy 1: Extracted text from a <pre><code> block.');
                foundInCodeBlock = true;
                break;
            }
        }

        // Strategy 2: Fallback to Gemini/AI Studio specific <ms-code-block>
        if (!foundInCodeBlock) {
            const geminiBlocks = messageNode.querySelectorAll('ms-code-block');
            for (const geminiBlock of geminiBlocks) {
                // Find the actual code element *inside* the component
                const innerCode = geminiBlock.querySelector('pre code');
                if (innerCode) {
                    const blockText = getTextFromCodeblock(innerCode); // Use new function
                    // [FIX] Support both Legacy (@gluon:response) and Interactive (@gluon:next_step) protocols
                    // Also support Structured Output
                    if (blockText && (
                        blockText.includes('@gluon:response') || 
                        blockText.includes('@gluon:next_step') ||
                        blockText.includes('"gluon_actions"')
                    )) {
                        textToParse = blockText;
                        // For Gemini, hide the whole ms-code-block component
                        sourceElement = geminiBlock;
                        logger.log('Strategy 2: Extracted text from <ms-code-block> (inner <code>).');
                        foundInCodeBlock = true;
                        break;
                    }
                }
            }
        }
        
        // Strategy 3: Fallback to the entire message node's text
        if (!foundInCodeBlock) {
            textToParse = getTextFromComplexNode(messageNode);
            // Spróbuj znaleźć precyzyjny akapit zawierający surowy JSON
            // AI Studio czasami wypluwa JSON w elementach <p>
            const textNodes = messageNode.querySelectorAll('p, div.model-response-text');
            for (const tn of textNodes) {
                const t = tn.textContent || '';
                if (t.includes('gluon_actions') || t.includes('thought_process')) {
                    sourceElement = tn;
                    break;
                }
            }
        }

        // [FIX] Check for ALL protocols: Legacy, Interactive, and STRUCTURED OUTPUT
        const hasLegacyMarker = textToParse && textToParse.includes('@gluon:response');
        const hasInteractiveMarker = textToParse && textToParse.includes('@gluon:next_step');
        // Detekcja nowego formatu JSON Schema (szukamy unikalnych kluczy)
        const hasStructuredMarker = textToParse && (
            textToParse.includes('"gluon_actions"') || 
            textToParse.includes('gluon_actions') || 
            textToParse.includes('"thought_process"')
        );

        if (hasLegacyMarker || hasInteractiveMarker || hasStructuredMarker) {
            let cleanedText = textToParse; 
            
            // Określ typ markera dla logów (priorytetyzacja Structured Output)
            let markerType = 'unknown';
            if (hasStructuredMarker) markerType = 'gluon_actions';
            else if (hasInteractiveMarker) markerType = '@gluon:next_step';
            else markerType = '@gluon:response';

            // [CRITICAL FIX] Ensure we grab the JSON content properly even if it's wrapped in HTML entities
            // Sometimes innerText decoding messes up quotes
            if (cleanedText.includes('&quot;')) {
                cleanedText = cleanedText.replace(/&quot;/g, '"');
            }

            logger.log(`🔍 [DETECTION] Found marker: ${markerType}`);
            logger.log('📝 [DETECTION] Raw text length:', textToParse.length);

            // ✅ Clean quote markers
            cleanedText = cleanedText.replace(/\[cite_start\]/g, '');

            // Remove system prompt artifacts if present
            const systemEnd = cleanedText.lastIndexOf('</gluon_system>');
            if (systemEnd !== -1) {
                cleanedText = cleanedText.substring(systemEnd + 15);
                logger.log('🧹 [DETECTION] Removed system prompt from detection');
            }

            // Re-verify after cleaning (Check for ANY valid marker)
            if (cleanedText.includes(markerType) || hasStructuredMarker) {

                logger.log('--- GLUON DEBUG START ---');
                logger.log('🎯 [DETECTION] Marker confirmed in cleaned text.');
                logger.log('📊 [DETECTION] Raw text to parse (first 200 chars):', textToParse.substring(0, 200));
                logger.log('📊 [DETECTION] Cleaned text to send (first 200 chars):', cleanedText.substring(0, 200));
                logger.log('📊 [DETECTION] Full cleaned text length:', cleanedText.length);
                logger.log('--- GLUON DEBUG END ---');

                // [IMPROVED] Don't use WeakSet - it prevents reprocessing of streaming content
                // Instead, rely on content hash checking below
                logger.log('✅ [DETECTION] Detected potential Gluon response. Sending to sidebar for parsing.');

                // [FIX] Stable Message ID and Content De-duplication
                // Generate UNIQUE ID for each new JSON block in the same message
                // This allows multiple overlays in one response
                let messageId = messageNode.dataset.gluonMessageId;

                // Check if content changed since last send (for THIS specific messageNode)
                // Use the entire text as hash to properly detect duplicates
                const currentHash = cleanedText;
                const lastHash = lastSentContent.get(messageNode);

                if (lastHash === currentHash) {
                    // Content hasn't changed, skip to prevent spam
                    logger.log('⏭️ [DETECTION] Content unchanged, skipping duplicate send');
                    return;
                }

                // Mark container as being processed to prevent race conditions during backend roundtrip
                if (messageContainer) {
                    messageContainer.dataset.gluonProcessing = 'true';
                }

                // Content is NEW or UPDATED - generate new ID for this specific JSON block
                messageId = `gluon-msg-${Date.now()}-${Math.random()}`;

                // Tag the MESSAGE NODE (container) for general tracking
                messageNode.dataset.gluonMessageId = messageId;

                // Tag the SOURCE ELEMENT (pre/code) so we can hide it later
                if (sourceElement) {
                    sourceElement.dataset.gluonSourceId = messageId;
                    sourceElement.classList.add('gluon-pending-hide'); // Marker class
                }

                logger.log(`🏷️ [DETECTION] Tagged element with NEW ID: ${messageId}`);

                lastSentContent.set(messageNode, currentHash);

                // [FIX] Auto-clear processing flag after 3 seconds to prevent permanent blocking
                // This ensures that even if overlay render fails, the flag will be cleared
                if (messageContainer) {
                    setTimeout(() => {
                        if (messageContainer.dataset.gluonProcessing === 'true') {
                            logger.log('⏰ [DETECTION] Auto-clearing stale processing flag');
                            delete messageContainer.dataset.gluonProcessing;
                        }
                    }, 3000);
                }

                // logger.log('📤 [DETECTION] Sending message to background script...');
                chrome.runtime.sendMessage({
                    action: 'gluon_response_detected',
                    payload: {
                        rawText: cleanedText, // Wyślij oczyszczony tekst
                        messageId: messageId,
                        hasSourceElement: !!sourceElement
                    }
                }, (response) => {
                    if (chrome.runtime.lastError) {
                         // Suppress common connection errors when sidebar is closed
                         const err = chrome.runtime.lastError.message;
                         if (!err.includes("Receiving end does not exist")) {
                            logger.error('❌ [DETECTION] BG Error:', err);
                         }
                    }
                    // [FIX] Clear processing flag after message sent (success or failure)
                    // This will be overridden by the timeout if it takes longer than 3s
                    if (messageContainer) {
                        setTimeout(() => {
                            delete messageContainer.dataset.gluonProcessing;
                        }, 500); // Small delay to prevent race conditions
                    }
                });
            }
        }
    });
}

chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
    if (request.action === 'render_gluon_overlay') {
        const { messageId, overlayHtml, overlayData } = request.payload;
        const messageNode = document.querySelector(`[data-gluon-message-id="${messageId}"]`);

        if (messageNode) {
            // Zapisz pełne dane w cache pod messageId
            if (overlayData) {
                overlayDataCache.set(messageId, overlayData);
                logger.log(`Cached overlay data for ${messageId}`, overlayData);
            }

            // [IMPROVED FIX] Allow multiple overlays in the same container
            // Each JSON response can have its own overlay displayed sequentially
            const messageContainer = messageNode.closest('.chat-turn-container, ms-chat-turn, [data-turn-role]');

            // Check if we are inside another overlay (prevent nesting)
            if (messageNode.closest('.gluon-response-overlay')) {
                logger.warn(`Aborting render: Target node ${messageId} is inside an existing overlay.`);
                sendResponse({success: false, error: 'Nested overlay prevented'});
                return;
            }

            // [NEW LOGIC] Instead of blocking if ANY overlay exists, we:
            // 1. Check if the EXACT SAME overlay data already exists (by comparing overlayData)
            // 2. If it's a NEW overlay (different data), allow it to render
            // 3. Only remove overlays with matching messageId (refresh scenario)

            const existingOverlays = messageNode.querySelectorAll('.gluon-response-overlay');
            let isDuplicate = false;

            existingOverlays.forEach(overlay => {
                const existingMessageId = overlay.dataset.messageId;

                // If same messageId, this is a refresh - remove old overlay
                if (existingMessageId === messageId) {
                    logger.log(`Refreshing existing overlay for ${messageId}`);
                    overlay.remove();
                } else {
                    // Different messageId - check if data is duplicate
                    const existingData = overlayDataCache.get(existingMessageId);
                    if (existingData && overlayData &&
                        JSON.stringify(existingData) === JSON.stringify(overlayData)) {
                        isDuplicate = true;
                        logger.warn(`Duplicate overlay data detected, skipping render for ${messageId}`);
                    } else if (overlayData && overlayData.responseType === 'structured_output') {
                        // Ochrona przed wieloma różnymi nakładkami w jednym messageNode dla Structured Output
                        // Jeżeli pojawiła się nowa wersja tej samej nakładki w trakcie strumieniowania
                        // to starą trzeba usunąć, a nie tworzyć nową.
                        logger.log(`Removing outdated streaming overlay: ${existingMessageId}`);
                        overlay.remove();
                    }
                }
            });

            if (isDuplicate) {
                // [FIX] Clear processing flag on duplicate
                const messageContainer = messageNode.closest('.chat-turn-container, ms-chat-turn, [data-turn-role]');
                if (messageContainer) {
                    delete messageContainer.dataset.gluonProcessing;
                }
                sendResponse({success: false, error: 'Duplicate overlay data'});
                return;
            }

            // Insert new overlay and tag it with messageId for tracking
            const tempDiv = document.createElement('div');
            tempDiv.innerHTML = overlayHtml;

            // [FIX] Handle multiple elements (e.g. style tags + overlay div)
            // Find the actual overlay container to tag with ID
            const overlayContainer = tempDiv.querySelector('.gluon-response-overlay');

            if (overlayContainer) {
                overlayContainer.dataset.messageId = messageId;

                // TRY TO FIND SOURCE ELEMENT TO HIDE IT
                // First try direct lookup by ID (most reliable)
                let sourceElement = messageNode.querySelector(`[data-gluon-source-id="${messageId}"]`);

                // Fallback: find any pending hide element
                if (!sourceElement) {
                     sourceElement = messageNode.querySelector('.gluon-pending-hide');
                }

                // Append ALL created elements (styles + div) to the message node
                const fragment = document.createDocumentFragment();
                while (tempDiv.firstChild) {
                    fragment.appendChild(tempDiv.firstChild);
                }

                // IF we found a source element, insert BEFORE it and HIDE it
                if (sourceElement) {
                    logger.log(`Injecting overlay BEFORE source element ${sourceElement.tagName}`);
                    sourceElement.parentNode.insertBefore(fragment, sourceElement);

                    // Hide the source element forcefully
                    sourceElement.setAttribute('style', 'display: none !important; visibility: hidden !important; height: 0 !important; overflow: hidden !important;');
                    sourceElement.classList.add('gluon-hidden-source');
                    sourceElement.classList.remove('gluon-pending-hide');
                } else {
                    // Fallback: Append to end of message (legacy behavior)
                    logger.log('Source element not found, appending to end of message');
                    messageNode.appendChild(fragment);
                }

                logger.log(`Overlay rendered for message ${messageId}.`);
            } else {
                logger.warn(`Overlay HTML generated but no .gluon-response-overlay found inside.`);
            }

            // [FIX] Clear processing flag after successful render
            if (messageContainer) {
                delete messageContainer.dataset.gluonProcessing;
            }

            // Przekaż ID do funkcji ustawiającej event listenery
            setupOverlayEventListeners(messageNode, messageId);
            sendResponse({success: true});
        } else {
            logger.error(`INJECTION FAILED: Could not find message node with ID: ${messageId}. The element likely disappeared from the DOM after being tagged.`);
            // [FIX] Clear processing flag on error - can't find messageContainer since messageNode is null
            // Flag will be auto-cleared by timeout in scanForNewMessages
            sendResponse({success: false, error: 'Message node not found'});
        }
    }
    return true;
});

function setupOverlayEventListeners(overlayParent, messageId) {
    const overlay = overlayParent.querySelector('.gluon-response-overlay');
    if (!overlay) return;

    // Toggle overlay expand/collapse when clicking header main area
    const headerMain = overlay.querySelector('.gluon-header-main');
    const toggleIcon = overlay.querySelector('.gluon-toggle-icon');

    if (headerMain && toggleIcon) {
        const toggleExpand = () => {
            overlay.classList.toggle('collapsed');
            overlay.classList.toggle('expanded');
            toggleIcon.textContent = overlay.classList.contains('collapsed') ? '▼' : '▲';
        };

        headerMain.addEventListener('click', toggleExpand);
        toggleIcon.addEventListener('click', toggleExpand);

        // Support keyboard navigation
        headerMain.addEventListener('keydown', (event) => {
            if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                toggleExpand();
            }
        });
    }

    // Użyj delegacji zdarzeń dla wszystkich przycisków wewnątrz nakładki
    overlay.addEventListener('click', (event) => {
        // Obsługa kliknięcia "JSON" w headerze (toggle original code block)
        if (event.target.closest('.gluon-btn-toggle-json')) {
            event.stopPropagation();

            // Get the cached overlay data to display JSON
            const cachedData = overlayDataCache.get(messageId);
            const overlayContent = overlay.querySelector('.gluon-overlay-expandable');
            let jsonContent = overlay.querySelector('.gluon-original-json-content');

            // Toggle state
            const shouldShowJson = !overlay.classList.contains('show-original');
            overlay.classList.toggle('show-original');

            if (!jsonContent) {
                // Create JSON display container (inside overlay, not outside)
                jsonContent = document.createElement('div');
                jsonContent.className = 'gluon-original-json-content';
                jsonContent.style.cssText = `
                    background: #0d1117;
                    border-radius: 0 0 6px 6px;
                    padding: 16px;
                    max-height: 400px;
                    overflow-y: auto;
                    font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
                    font-size: 12px;
                    line-height: 1.5;
                    color: #e6edf3;
                    white-space: pre-wrap;
                    word-break: break-word;
                `;

                // Insert at the end of overlay content, before actions
                const overlayActions = overlay.querySelector('.gluon-overlay-actions');
                if (overlayActions) {
                    overlay.insertBefore(jsonContent, overlayActions);
                } else {
                    overlay.appendChild(jsonContent);
                }
            }

            if (shouldShowJson) {
                // Show JSON
                if (overlayContent) overlayContent.style.display = 'none';
                jsonContent.style.display = 'block';

                if (cachedData) {
                    jsonContent.textContent = JSON.stringify(cachedData, null, 2);
                } else {
                    jsonContent.textContent = 'No cached data available';
                }
            } else {
                // Show overlay content
                if (overlayContent) overlayContent.style.display = 'block';
                jsonContent.style.display = 'none';
            }

            return;
        }

        const button = event.target.closest('button');
        if (!button) return;

        // Obsługa kliknięć przycisku "Find"
        if (button.classList.contains('gluon-btn-find')) {
            const filePath = button.dataset.filepath;
            const searchText = button.dataset.searchtext;

            if (filePath && searchText) {
                logger.log('Sending "find_in_editor" command:', { filePath, searchText });
                chrome.runtime.sendMessage({
                    action: 'find_in_editor',
                    payload: {
                        filePath,
                        searchText,
                    }
                });

                const originalText = button.textContent;
                button.textContent = '✔ Sent!';
                button.disabled = true;
                setTimeout(() => {
                    if (button) {
                       button.textContent = originalText;
                       button.disabled = false;
                    }
                }, 2000);
            }
        }

        // Obsługa przycisku "Apply Changes" (Structured Output)
        if (button.classList.contains('gluon-btn-apply-changes')) {
            const cachedData = overlayDataCache.get(messageId);
            if (!cachedData) {
                logger.error(`No cached data found for ${messageId}! Cannot apply changes.`);
                return;
            }

            const fileChanges = cachedData.file_changes || cachedData?.data?.file_changes;

            if (!fileChanges || fileChanges.length === 0) {
                logger.error('No file_changes found in cache');
                return;
            }

            logger.log('🔧 Apply Changes Button Clicked. Converting to ChangeQueueItem format...');

            // Konwersja do formatu ChangeQueueItem (zgodny z apply_code_changes)
            const changes = fileChanges.map((change, index) => ({
                id: `gsop-${Date.now()}-${index}`,
                batchId: `batch-${Date.now()}`,
                filePath: change.file_path,
                file_path: change.file_path,
                status: 'pending',
                format: 'SEARCH_REPLACE',
                oldCode: change.search_code || '',
                newCode: change.replace_code || '',
                lineStart: change.line_start || 0,
                lineEnd: change.line_end || 0,
                matching_data: change.matching_data || {}
            }));

            logger.log('Sending apply_code_changes with', changes.length, 'changes');

            // Wyślij do apply system
            chrome.runtime.sendMessage({
                action: 'apply_code_changes',
                payload: {
                    changes: changes,
                    selectedProjects: [] // Auto-detect w backend
                }
            });

            button.textContent = '⏳ Applying...';
            button.disabled = true;
            return;
        }

        // Obsługa przycisku "Load Context" (Structured Output)
        if (button.classList.contains('gluon-btn-load-context')) {
            const cachedData = overlayDataCache.get(messageId);
            if (!cachedData) {
                logger.error(`No cached data found for ${messageId}! Cannot load context.`);
                return;
            }

            const contextOps = cachedData.context_ops || cachedData?.data?.context_ops;

            if (!contextOps || !contextOps.load || contextOps.load.length === 0) {
                logger.error('No context_ops found in cache');
                return;
            }

            logger.log('🧠 Load Context Button Clicked. Sending execute_interactive_context...');

            // Struktura dla context-node.js
            const payload = {
                next_step: {
                    action: 'continue',
                    reasoning: cachedData.thought_process || cachedData?.data?.thought_process || 'Loading context...',
                    context_ops: contextOps
                }
            };

            chrome.runtime.sendMessage({
                action: 'execute_interactive_context',
                payload: payload
            });

            button.textContent = '⏳ Loading...';
            button.disabled = true;
            return;
        }

        // Obsługa przycisku "Apply" (Legacy + Interactive Mode)
        if (button.classList.contains('gluon-btn-apply')) {
            const cachedData = overlayDataCache.get(messageId);
            if (!cachedData) {
                logger.error(`No cached data found for ${messageId}! Cannot apply selection.`);
                return;
            }

            logger.log('Found cached data:', cachedData);

            // [FIX] DETECT INTERACTIVE MODE BUTTON
            if (button.classList.contains('interactive-btn')) {
                logger.log('🧠 Interactive Button Clicked. Sending execute_interactive_context...');

                // Extract next_step data from cached object
                // Struktura cache może być bezpośrednia lub zagnieżdżona w 'data'
                const interactiveData = cachedData.data || cachedData;

                chrome.runtime.sendMessage({
                    action: 'execute_interactive_context',
                    payload: {
                        ...interactiveData,
                        _overlayMessageId: messageId  // Pass overlay ID for result feedback
                    }
                });

                button.textContent = '⏳ Fetching...';
                button.disabled = true;
                return; // Stop execution here, don't run legacy logic
            }

            // LEGACY LOGIC (File Handoff / Auto Select)
            const payload = {
                files: cachedData.files || (cachedData.data ? cachedData.data.found : []) || [],
                responseType: cachedData.responseType,
                handoff: cachedData.handoff,
                prompt: cachedData.prompt,
            };

            logger.log('Final payload from CACHE:', payload);

            chrome.runtime.sendMessage({
                action: 'apply_gluon_selection',
                payload: payload
            });

            const originalText = button.textContent;
            button.textContent = '✔ Applied!';

            setTimeout(() => {
                if(button) {
                   button.textContent = originalText;
                }
            }, 2000);

            overlayDataCache.delete(messageId);
        }

        // Obsługa przycisku "Ignore"
        if (button.classList.contains('gluon-btn-ignore')) {
            // Restore hidden source element if exists
            const sourceElement = overlayParent.querySelector(`[data-gluon-source-id="${messageId}"]`);
            if (sourceElement) {
                sourceElement.style.display = ''; // Restore display
                sourceElement.classList.remove('gluon-hidden-source');
            }

            overlay.remove();
            overlayDataCache.delete(messageId);
        }
    });

    // Obsługa kliknięć na kafelki plików (znajdowanie w drzewie)
    overlay.querySelectorAll('.gluon-file-tile, .gluon-file-tile-missing').forEach(tile => {
        tile.addEventListener('click', (event) => {
            const filePath = tile.dataset.filepath;
            const projectPath = tile.dataset.project;
            const isMissing = tile.classList.contains('gluon-file-tile-missing');

            if (filePath) {
                logger.log('File tile clicked, searching in tree:', { filePath, projectPath, isMissing });

                // Send message to sidebar to search for this file
                chrome.runtime.sendMessage({
                    action: 'search_file_in_tree',
                    payload: {
                        filePath,
                        projectPath,
                        isMissing
                    }
                });

                // Visual feedback
                const originalBg = tile.style.background;
                tile.style.background = 'rgba(139, 92, 246, 0.2)';
                setTimeout(() => {
                    tile.style.background = originalBg;
                }, 1000);
            }
        });
    });
}

/**
 * Extracts text content from a code block element (<pre><code> or inner <code>),
 * attempting to preserve line breaks even with complex DOM structures (like Gemini/AI Studio).
 * @param {HTMLElement} codeElement - The <code> element (or similar).
 * @returns {string} The reconstructed text content.
 */
function getTextFromCodeblock(codeElement) {
    if (!codeElement) return '';

    // UJEDNOLICENIE LOGIKI: Używamy tej samej, najbardziej niezawodnej implementacji
    // co w getTextFromComplexNode, aby zapewnić spójne i poprawne odtwarzanie tekstu.
    let lines = [];
    let currentLine = '';

    function traverse(node) {
        if (node.nodeType === Node.TEXT_NODE) {
            currentLine += node.textContent;
        } else if (node.nodeType === Node.ELEMENT_NODE) {
            // Ignoruj znane elementy UI, które nie powinny być częścią tekstu
            if (node.closest('.gluon-response-overlay, .actions-container, .turn-footer, button, mat-expansion-panel-header')) {
                return;
            }

            const tagName = node.tagName.toLowerCase();
            const displayStyle = window.getComputedStyle(node).display;

            // Traktuj elementy blokowe lub <br> jako potencjalne przełamanie linii
            if (displayStyle === 'block' || tagName === 'br' || tagName === 'div' || tagName === 'p' || tagName === 'pre') {
                // Wrzuć poprzednią linię, jeśli miała zawartość
                if (currentLine.trim()) {
                    lines.push(currentLine.trim()); // Przytnij białe znaki z końców linii
                }
                currentLine = ''; // Zresetuj dla bloku/przełamania

                // Przejdź przez dzieci *wewnątrz* bloku
                for (const child of node.childNodes) {
                    traverse(child);
                }

                // Wrzuć zawartość zebraną wewnątrz bloku, jeśli jakaś jest
                if (currentLine.trim()) {
                    lines.push(currentLine.trim());
                }
                currentLine = ''; // Zresetuj ponownie po bloku
            } else {
                // Elementy liniowe, po prostu kontynuuj przechodzenie przez dzieci
                for (const child of node.childNodes) {
                    traverse(child);
                }
            }
        }
    }

    for (const child of codeElement.childNodes) {
        traverse(child);
    }

    // Dodaj pozostały tekst z ostatniej linii
    if (currentLine.trim()) {
        lines.push(currentLine.trim());
    }

    // Połącz linie pojedynczym znakiem nowej linii
    return lines.join('\n');
}


/**
 * Extracts text content from a complex message node, attempting to reconstruct
 * line breaks based on block-level elements. Used as a fallback.
 * @param {HTMLElement} messageNode - The main message container element.
 * @returns {string} The reconstructed text content.
 */
function getTextFromComplexNode(messageNode) {
    if (!messageNode) return '';

    let lines = [];
    let currentLine = '';

    function traverse(node) {
        if (node.nodeType === Node.TEXT_NODE) {
            currentLine += node.textContent;
        } else if (node.nodeType === Node.ELEMENT_NODE) {
             // Ignore known UI elements that shouldn't be part of the text
            if (node.closest('.gluon-response-overlay, .actions-container, .turn-footer, button, mat-expansion-panel-header')) {
                return;
            }

            const tagName = node.tagName.toLowerCase();
             const displayStyle = window.getComputedStyle(node).display;

            // Treat block elements or <br> as potential line breaks
             if (displayStyle === 'block' || tagName === 'br' || tagName === 'div' || tagName === 'p' || tagName === 'pre') {
                 // Push previous line if it had content
                if (currentLine.trim()) {
                     lines.push(currentLine.trim()); // Trim whitespace from ends of lines
                }
                currentLine = ''; // Reset for the block/break

                // Traverse children *within* the block
                for (const child of node.childNodes) {
                    traverse(child);
                }

                // Push content gathered within the block if any
                if (currentLine.trim()) {
                    lines.push(currentLine.trim());
                }
                currentLine = ''; // Reset again after the block
            } else {
                 // Inline elements, just continue traversing children
                for (const child of node.childNodes) {
                    traverse(child);
                }
            }
        }
    }

    for (const child of messageNode.childNodes) {
        traverse(child);
    }

    // Add any remaining text on the last line
    if (currentLine.trim()) {
        lines.push(currentLine.trim());
    }

    // Join lines with a single newline character
    return lines.join('\n');
}

// Listen for Gluon Mode changes from sidebar
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
    if (request.action === 'gluon_mode_changed') {
        gluonModeEnabled = request.enabled;
        logger.log(`Gluon Mode ${request.enabled ? 'enabled' : 'disabled'}`);

        // Remove all existing overlays when disabled
        if (!request.enabled) {
            document.querySelectorAll('.gluon-response-overlay').forEach(overlay => {
                overlay.remove();
            });
        } else {
            // Re-scan when enabled
            scanForNewMessages();
        }

        sendResponse({ success: true });
        return true;
    }
});

// Listen for context loading result to update the overlay button
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
    if (request.action === 'interactive_context_done') {
        const { overlayMessageId, success, itemCount } = request.payload || {};
        if (!overlayMessageId) return;

        const overlay = document.querySelector(`.gluon-response-overlay[data-message-id="${overlayMessageId}"]`);
        if (overlay) {
            const btn = overlay.querySelector('.gluon-btn-apply.interactive-btn');
            if (btn) {
                if (success) {
                    btn.textContent = `✅ Loaded (${itemCount})`;
                    btn.style.background = '#238636';
                    btn.style.borderColor = '#2ea043';
                } else {
                    btn.textContent = '❌ Failed – retry';
                    btn.style.background = '#da3633';
                    btn.style.borderColor = '#f85149';
                    btn.disabled = false; // Allow retry on failure
                }
            }
        }

        sendResponse({ success: true });
    }
    return true;
});

// Initialize Gluon Mode state from storage
if (typeof chrome !== 'undefined' && chrome.storage) {
    chrome.storage.local.get({ gluonModeEnabled: true }, (data) => {
        gluonModeEnabled = data.gluonModeEnabled;
        logger.log(`Gluon Mode initialized: ${gluonModeEnabled}`);
    });
}

startObserver();