// AI Studio Agent - Response Parser and Message Injector
// Handles parsing AI responses and injecting messages into AI Studio input

console.log('[AIStudioAgent] 🚀 Script loaded on URL:', window.location.href);

class AIStudioAgent {
    constructor() {
        this.isMonitoring = false;
        this.lastResponseContent = null;
        this.inputSelector = 'rich-textarea[aria-label="Enter a prompt here"] .ql-editor';
        this.sendButtonSelector = 'button[aria-label="Send message"]';
        // Updated selectors for Google AI Studio 2024+ structure
        this.responseTurnSelector = 'ms-chat-turn[data-turn-role="Model"]';
        this.responseTextSelector = 'ms-text-chunk';
        this.tabId = null;
        this.init();
    }

    init() {
        console.log('[AIStudioAgent] 🚀🚀🚀 Initializing AI Studio agent integration...');
        console.log('[AIStudioAgent] 🌍 Current URL:', window.location.href);
        console.log('[AIStudioAgent] 📍 Hostname:', window.location.hostname);

        // Get current tab ID
        chrome.runtime.sendMessage({ action: 'get_tab_id' }, (response) => {
            this.tabId = response?.tabId || `tab_${Date.now()}_${Math.random()}`;
            console.log('[AIStudioAgent] 🆔 Tab ID:', this.tabId);
        });

        console.log('[AIStudioAgent] ⚙️ Setting up components...');
        this.setupMessageListener();
        this.injectNetworkInterceptor();
        this.setupInterceptorListener();
        this.startResponseMonitoring();
        console.log('[AIStudioAgent] ✅ Initialization complete!');
    }

    getStorageKey() {
        return `agent_pairing_${this.tabId}`;
    }

    injectNetworkInterceptor() {
        console.log('[AIStudioAgent] 💉 Injecting network interceptor into MAIN world...');

        // Inject interceptor script into MAIN world (not isolated)
        const script = document.createElement('script');
        script.src = chrome.runtime.getURL('src/content/ai-studio-network-interceptor.js');
        script.onload = () => {
            console.log('[AIStudioAgent] ✅ Network interceptor injected successfully');
            script.remove();
        };
        script.onerror = (error) => {
            console.error('[AIStudioAgent] ❌ Failed to inject network interceptor:', error);
        };
        (document.head || document.documentElement).appendChild(script);
    }

    setupInterceptorListener() {
        console.log('[AIStudioAgent] 👂 Setting up listener for interceptor events...');

        window.addEventListener('GLUON_AI_RESPONSE', async (event) => {
            const { type, content, url } = event.detail;

            console.log('[AIStudioAgent] 📥 Received from interceptor:', type);
            console.log('[AIStudioAgent] 📝 Content length:', content?.length || 0);
            console.log('[AIStudioAgent] 🔗 URL:', url);

            if (type === 'ai_response_complete' && content) {
                console.log('[AIStudioAgent] ✅ Complete response received from API');
                console.log('[AIStudioAgent] 📝 Preview:', content.substring(0, 200));

                // Update lastResponseContent to prevent duplicate processing
                this.lastResponseContent = content;

                // Send to workflow
                await this.sendResponseToWorkflow(content);
            } else if (type === 'ai_response_chunk' && content) {
                console.log('[AIStudioAgent] 📦 Chunk received, full content so far:', content.length, 'chars');

                // Update lastResponseContent with streaming content
                this.lastResponseContent = content;
            }
        });

        console.log('[AIStudioAgent] ✅ Interceptor listener ready');
    }

    setupMessageListener() {
        console.log('[AIStudioAgent] 📡 Setting up message listener for agent_inject_message...');
        chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
            console.log('[AIStudioAgent] 📩 Message received, type:', message.type);
            if (message.type === 'agent_inject_message') {
                console.log('[AIStudioAgent] ✅ agent_inject_message received! Content length:', message.content?.length);
                console.log('[AIStudioAgent] Received message to inject:', message.content);
                this.injectMessage(message.content, message.from_agent, message.auto_submit);
                sendResponse({ success: true });
                return true;
            }
            // Don't return true for other message types - we don't handle them
            return false;
        });
        console.log('[AIStudioAgent] ✅ Message listener setup complete');
    }

    startResponseMonitoring() {
        if (this.isMonitoring) return;

        console.log('[AIStudioAgent] Starting response monitoring...');
        this.isMonitoring = true;

        // Use MutationObserver to detect new AI responses
        const observer = new MutationObserver((mutations) => {
            console.log('[AIStudioAgent] 🔄 MutationObserver triggered, mutations:', mutations.length);
            for (const mutation of mutations) {
                if (mutation.type === 'childList' && mutation.addedNodes.length > 0) {
                    console.log('[AIStudioAgent] 🆕 New nodes added:', mutation.addedNodes.length);
                    // Check if new response was added
                    this.checkForNewResponse();
                }
            }
        });

        // Observe the chat container for changes
        let retryCount = 0;
        const observeContainer = () => {
            retryCount++;
            console.log('[AIStudioAgent] 🔍 [CONTAINER_SEARCH] Attempt', retryCount, 'to find chat container...');

            // Expanded selectors for modern AI Studio (Angular Material)
            const chatContainer = document.querySelector([
                'ms-chat-scroller',           // New primary scroller
                'div[class*="chat-history"]', 
                'div[class*="conversation"]',
                'main',                       // Fallback
                'ms-app-layout'               // High level fallback
            ].join(', '));

            if (chatContainer) {
                console.log('[AIStudioAgent] ✅ [CONTAINER_SEARCH] Found container:', chatContainer.tagName, chatContainer.className);
                observer.observe(chatContainer, {
                    childList: true,
                    subtree: true
                });
                console.log('[AIStudioAgent] ✅ Observing chat container for responses');

                // Do an initial check
                console.log('[AIStudioAgent] 🔍 Performing initial response check...');
                this.checkForNewResponse();
            } else {
                console.warn('[AIStudioAgent] ⚠️ [CONTAINER_SEARCH] Container not found on attempt', retryCount);
                console.log('[AIStudioAgent] 🔍 [CONTAINER_SEARCH] Available main elements:', document.querySelectorAll('main').length);
                console.log('[AIStudioAgent] 🔍 [CONTAINER_SEARCH] Available body children:', document.body.children.length);

                // Retry if container not found yet (max 10 times)
                if (retryCount < 10) {
                    setTimeout(observeContainer, 1000);
                } else {
                    console.error('[AIStudioAgent] ❌ [CONTAINER_SEARCH] Failed to find container after 10 attempts');
                }
            }
        };

        observeContainer();
    }

    async checkForNewResponse() {
        try {
            // Find the latest AI response turn (Google AI Studio structure)
            const responseTurns = document.querySelectorAll(this.responseTurnSelector);
            console.log('[AIStudioAgent] 🔍 [RESPONSE_CHECK] Found', responseTurns.length, 'response turns');

            if (responseTurns.length === 0) {
                console.log('[AIStudioAgent] ⚠️ [RESPONSE_CHECK] No response turns found, selector:', this.responseTurnSelector);
                return;
            }

            const latestTurn = responseTurns[responseTurns.length - 1];
            const responseText = this.extractResponseText(latestTurn);

            // Check if this is a new response
            if (responseText) {
                console.log('[AIStudioAgent] 📝 [RESPONSE_CHECK] Response text length:', responseText.length);
                console.log('[AIStudioAgent] 📝 [RESPONSE_CHECK] Response text (first 150 chars):', responseText.substring(0, 150));
                console.log('[AIStudioAgent] 📝 [RESPONSE_CHECK] Last response content:', this.lastResponseContent ? 'Set' : 'Empty');
            }

            if (responseText && responseText !== this.lastResponseContent) {
                this.lastResponseContent = responseText;
                console.log('[AIStudioAgent] ✅ [RESPONSE_CHECK] NEW response detected!');
                console.log('[AIStudioAgent] 📤 [RESPONSE_CHECK] Sending to workflow...');

                // Send to workflow
                await this.sendResponseToWorkflow(responseText);
            } else if (!responseText) {
                console.log('[AIStudioAgent] ⚠️ [RESPONSE_CHECK] Response text is empty or null');
            } else {
                console.log('[AIStudioAgent] ℹ️ [RESPONSE_CHECK] Response unchanged (same as last)');
            }
        } catch (error) {
            console.error('[AIStudioAgent] ❌ [RESPONSE_CHECK] Error checking for response:', error);
        }
    }

    extractResponseText(element) {
        if (!element) return null;

        // Extract text from Google AI Studio ms-text-chunk elements
        const textChunks = element.querySelectorAll(this.responseTextSelector);

        if (textChunks.length > 0) {
            // Extract text from all ms-text-chunk elements
            const text = Array.from(textChunks)
                .map(chunk => {
                    // Get ms-cmark-node elements within chunk
                    const cmarkNodes = chunk.querySelectorAll('ms-cmark-node');
                    if (cmarkNodes.length > 0) {
                        return Array.from(cmarkNodes)
                            .map(node => node.textContent || '')
                            .join('\n');
                    }
                    return chunk.textContent || '';
                })
                .join('\n')
                .trim();

            if (text) return text;
        }

        // Fallback to direct text content
        return element.textContent?.trim() || null;
    }

    async sendResponseToWorkflow(content) {
        try {
            if (!this.tabId) {
                console.warn('[AIStudioAgent] Tab ID not set, skipping send');
                return;
            }

            // Check if agent is paired (using tab-specific key)
            const storageKey = this.getStorageKey();
            const pairingData = await chrome.storage.local.get(storageKey);
            if (!pairingData[storageKey] || !pairingData[storageKey].agent) {
                console.log('[AIStudioAgent] Not paired with agent on this tab, skipping send');
                return;
            }

            const agentId = pairingData[storageKey].agent.id;
            console.log('[AIStudioAgent] Sending response to workflow from agent:', agentId);

            // Send to background script with agentId (backend expects camelCase)
            chrome.runtime.sendMessage({
                action: 'agent_send_message',
                content: content,
                agentId: agentId  // Changed from agent_id to agentId
            });

            console.log('[AIStudioAgent] Response sent to workflow');
        } catch (error) {
            console.error('[AIStudioAgent] Failed to send response to workflow:', error);
        }
    }

    async injectMessage(content, fromAgent, autoSubmit = null) {
        console.log('[AIStudioAgent] 💉 [INJECT] Injecting message from agent:', fromAgent);
        console.log('[AIStudioAgent] 💉 [INJECT] Content length:', content.length);
        console.log('[AIStudioAgent] 💉 [INJECT] Content (first 200 chars):', content.substring(0, 200));
        console.log('[AIStudioAgent] 💉 [INJECT] autoSubmit:', autoSubmit);

        try {
            // Find input element
            console.log('[AIStudioAgent] 🔍 [INJECT] Looking for input element...');
            const inputElement = this.findInputElement();
            if (!inputElement) {
                console.error('[AIStudioAgent] ❌ [INJECT] Input element not found');
                this.showNotification(`Failed to inject message from ${fromAgent}: Input not found`, 'error');
                return;
            }
            console.log('[AIStudioAgent] ✅ [INJECT] Input element found');
            console.log('[AIStudioAgent] 🔍 [INJECT] Element type:', inputElement.tagName);

            // Clear existing content and insert new content
            console.log('[AIStudioAgent] 🧹 [INJECT] Clearing input content...');
            console.log('[AIStudioAgent] ✏️ [INJECT] Inserting new content...');

            // Handle different input types
            if (inputElement.tagName === 'TEXTAREA' || inputElement.tagName === 'INPUT') {
                // For textarea and input elements, use .value
                console.log('[AIStudioAgent] 📝 [INJECT] Using .value for textarea/input');
                inputElement.value = content;
            } else {
                // For contenteditable elements, use innerHTML or textContent
                console.log('[AIStudioAgent] 📝 [INJECT] Using innerHTML for contenteditable');
                inputElement.innerHTML = '';
                const textNode = document.createTextNode(content);
                inputElement.appendChild(textNode);
            }

            // Trigger input events
            console.log('[AIStudioAgent] 📤 [INJECT] Triggering input events...');
            inputElement.dispatchEvent(new Event('input', { bubbles: true }));
            inputElement.dispatchEvent(new Event('change', { bubbles: true }));

            // Focus the input
            console.log('[AIStudioAgent] 👁️ [INJECT] Focusing input element...');
            inputElement.focus();

            // Show notification
            console.log('[AIStudioAgent] 🔔 [INJECT] Showing notification...');
            this.showNotification(`Message received from ${fromAgent}`, 'success');

            // Determine if should auto-send
            // Priority: explicit autoSubmit parameter > stored setting
            let shouldAutoSend = autoSubmit;
            if (shouldAutoSend === null || shouldAutoSend === undefined) {
                console.log('[AIStudioAgent] 🔍 [INJECT] Checking auto-send setting...');
                shouldAutoSend = await this.shouldAutoSend();
            }
            console.log('[AIStudioAgent] 📋 [INJECT] Should auto-send:', shouldAutoSend);

            if (shouldAutoSend) {
                console.log('[AIStudioAgent] ⏱️ [INJECT] Scheduling auto-send in 500ms...');
                setTimeout(() => {
                    console.log('[AIStudioAgent] 📨 [INJECT] Clicking send button...');
                    this.clickSendButton();
                }, 500);
            } else {
                console.log('[AIStudioAgent] ⏸️ [INJECT] Auto-send disabled, message ready for manual submission');
            }

            console.log('[AIStudioAgent] ✅ [INJECT] Message injection completed successfully');

        } catch (error) {
            console.error('[AIStudioAgent] ❌ [INJECT] Error injecting message:', error);
            console.error('[AIStudioAgent] ❌ [INJECT] Stack:', error.stack);
            this.showNotification('Failed to inject message: ' + error.message, 'error');
        }
    }

    findInputElement() {
        // Try multiple selectors for AI Studio input
        const selectors = [
            'rich-textarea[aria-label="Enter a prompt here"] .ql-editor',
            '.ql-editor[contenteditable="true"]',
            '[contenteditable="true"][role="textbox"]',
            'div.input-area [contenteditable="true"]',
            '[contenteditable="true"]',
            'textarea',
            'input[type="text"]',
            // New AI Studio selectors (2024+)
            'ms-input-container [contenteditable="true"]',
            '.prompt-textarea [contenteditable="true"]',
            '[aria-label*="prompt" i] [contenteditable="true"]',
            '[aria-label*="enter" i] [contenteditable="true"]'
        ];

        console.log('[AIStudioAgent] 🔍 [FIND_INPUT] Searching for input element...');

        // Debug: List all contenteditable elements
        const allEditable = document.querySelectorAll('[contenteditable="true"]');
        console.log('[AIStudioAgent] 🔍 [FIND_INPUT] Found', allEditable.length, 'contenteditable elements:');
        allEditable.forEach((el, idx) => {
            console.log(`  [${idx}]:`, el.tagName, el.className, el.getAttribute('aria-label'));
        });

        for (const selector of selectors) {
            const element = document.querySelector(selector);
            if (element) {
                console.log('[AIStudioAgent] ✅ [FIND_INPUT] Found input element with selector:', selector);
                console.log('[AIStudioAgent] ✅ [FIND_INPUT] Element:', element);
                return element;
            }
        }

        console.error('[AIStudioAgent] ❌ [FIND_INPUT] No input element found with any selector');
        return null;
    }

    clickSendButton() {
        const selectors = [
            'button[aria-label="Send message"]',
            'button.send-button',
            'button[type="submit"]',
            'button:has(mat-icon:contains("send"))'
        ];

        console.log('[AIStudioAgent] 🔘 [CLICK_BUTTON] Searching for send button with selectors:', selectors);

        for (const selector of selectors) {
            const button = document.querySelector(selector);
            if (button) {
                console.log('[AIStudioAgent] 🔘 [CLICK_BUTTON] Found button with selector:', selector);
                console.log('[AIStudioAgent] 🔘 [CLICK_BUTTON] Button disabled?', button.disabled);

                if (!button.disabled) {
                    console.log('[AIStudioAgent] ✅ [CLICK_BUTTON] Clicking send button...');
                    button.click();
                    console.log('[AIStudioAgent] ✅ [CLICK_BUTTON] Send button clicked successfully');
                    return true;
                } else {
                    console.warn('[AIStudioAgent] ⚠️ [CLICK_BUTTON] Send button found but is disabled');
                }
            }
        }

        console.warn('[AIStudioAgent] ❌ [CLICK_BUTTON] Send button not found with any selector or all disabled');
        return false;
    }

    async shouldAutoSend() {
        try {
            const settings = await chrome.storage.local.get('agent_auto_send');
            return settings.agent_auto_send !== false; // Default: true
        } catch (error) {
            return true;
        }
    }

    showNotification(message, type = 'info') {
        // Create notification element
        const notification = document.createElement('div');
        notification.className = `gluon-agent-notification gluon-notification-${type}`;
        notification.innerHTML = `
            <style>
                .gluon-agent-notification {
                    position: fixed;
                    top: 80px;
                    right: 20px;
                    z-index: 999999;
                    background: rgba(10, 13, 26, 0.95);
                    border: 2px solid rgba(0, 212, 255, 0.4);
                    color: white;
                    padding: 12px 20px;
                    border-radius: 8px;
                    box-shadow: 0 0 20px rgba(0, 212, 255, 0.3), 0 4px 16px rgba(0, 0, 0, 0.5);
                    backdrop-filter: blur(10px);
                    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
                    font-size: 14px;
                    animation: slideInNotif 0.3s ease-out;
                    max-width: 300px;
                }

                .gluon-notification-error {
                    border-color: rgba(239, 68, 68, 0.5);
                    box-shadow: 0 0 20px rgba(239, 68, 68, 0.3), 0 4px 16px rgba(0, 0, 0, 0.5);
                }

                .gluon-notification-success {
                    border-color: rgba(0, 255, 136, 0.5);
                    box-shadow: 0 0 20px rgba(0, 255, 136, 0.3), 0 4px 16px rgba(0, 0, 0, 0.5);
                }

                @keyframes slideInNotif {
                    from {
                        transform: translateX(100%);
                        opacity: 0;
                    }
                    to {
                        transform: translateX(0);
                        opacity: 1;
                    }
                }

                @keyframes slideOutNotif {
                    from {
                        transform: translateX(0);
                        opacity: 1;
                    }
                    to {
                        transform: translateX(100%);
                        opacity: 0;
                    }
                }
            </style>
            <div>${message}</div>
        `;

        document.body.appendChild(notification);

        // Auto-remove after 3 seconds
        setTimeout(() => {
            notification.style.animation = 'slideOutNotif 0.3s ease-out';
            setTimeout(() => {
                notification.remove();
            }, 300);
        }, 3000);
    }

    // Helper: Extract markdown code blocks
    extractCodeBlocks(text) {
        const codeBlockRegex = /```(\w+)?\n([\s\S]*?)```/g;
        const blocks = [];
        let match;

        while ((match = codeBlockRegex.exec(text)) !== null) {
            blocks.push({
                language: match[1] || 'text',
                code: match[2].trim()
            });
        }

        return blocks;
    }

    // Helper: Clean response for sending to next agent
    cleanResponse(text) {
        // Remove excessive whitespace
        let cleaned = text.replace(/\n{3,}/g, '\n\n').trim();

        // Remove UI artifacts
        cleaned = cleaned.replace(/^(Copy|Edit|Regenerate)\n/gm, '');

        return cleaned;
    }
}

// Initialize when DOM is ready
console.log('[AIStudioAgent] 🎬 Initializing script, document.readyState:', document.readyState);
if (document.readyState === 'loading') {
    console.log('[AIStudioAgent] Document loading, waiting for DOMContentLoaded...');
    document.addEventListener('DOMContentLoaded', () => {
        console.log('[AIStudioAgent] DOMContentLoaded fired, creating AIStudioAgent instance...');
        window.aiStudioAgent = new AIStudioAgent();
    });
} else {
    console.log('[AIStudioAgent] Document ready, creating AIStudioAgent instance immediately...');
    window.aiStudioAgent = new AIStudioAgent();
}