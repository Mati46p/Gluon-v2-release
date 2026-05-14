// Plik: extension/src/content/content.js
// Główny skrypt inicjalizujący

// 1. Zachowaj przycisk (zgodnie z Twoim plikiem)

function detectProvider() {
    const host = window.location.host;
    if (host.includes('claude.ai')) return 'claude';
    if (host.includes('chatgpt.com')) return 'chatgpt';
    if (host.includes('gemini.google') || host.includes('aistudio.google')) return 'gemini';
    return 'unknown';
}

function initGluon() {
    const provider = detectProvider();
    console.log(`[Gluon Init] Detected provider: ${provider}`);
    
    if (provider === 'unknown') return;

    // Czekamy na załadowanie skryptów detektorów
    let attempts = 0;
    const checkInterval = setInterval(() => {
        attempts++;
        if (window.detectors && window.detectors.applySystem) {
            clearInterval(checkInterval);
            window.detectors.applySystem(provider);
            console.log(`[Gluon Init] ✅ ApplySystem detector started successfully for ${provider}`);
        } else if (attempts > 50) { // 5 sekund (50 * 100ms)
            clearInterval(checkInterval);
            console.error('[Gluon Init] ❌ TIMEOUT: window.detectors.applySystem is missing! Check manifest.json and script loading order.');
            // Spróbujmy wypisać co mamy w window.detectors
            console.log('[Gluon Init] Current window.detectors:', window.detectors);
        }
    }, 100);
}

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initGluon);
} else {
    initGluon();
}

// ============================================================================
// MCP Status Notifications Handler
// ============================================================================

/**
 * Creates and displays a toast notification for MCP status
 */
function showMcpStatusNotification(status, isError = false) {
    // Create notification element
    const notification = document.createElement('div');
    notification.className = `gluon-mcp-notification ${isError ? 'error' : 'info'}`;
    notification.style.cssText = `
        position: fixed;
        bottom: 20px;
        right: 20px;
        max-width: 350px;
        padding: 12px 16px;
        background: ${isError ? '#ff4444' : '#4CAF50'};
        color: white;
        border-radius: 8px;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
        font-size: 13px;
        box-shadow: 0 2px 8px rgba(0,0,0,0.3);
        z-index: 999999;
        animation: slideIn 0.3s ease-out;
    `;
    notification.textContent = status;

    // Add animation styles
    const style = document.createElement('style');
    style.textContent = `
        @keyframes slideIn {
            from {
                transform: translateX(400px);
                opacity: 0;
            }
            to {
                transform: translateX(0);
                opacity: 1;
            }
        }
        .gluon-mcp-notification {
            transition: opacity 0.3s ease-out;
        }
        .gluon-mcp-notification.fading {
            opacity: 0;
        }
    `;
    if (!document.querySelector('style[data-gluon-mcp-styles]')) {
        style.setAttribute('data-gluon-mcp-styles', 'true');
        document.head.appendChild(style);
    }

    document.body.appendChild(notification);

    // Auto-fade after 4 seconds (or 2 seconds for errors)
    const fadeDuration = isError ? 2000 : 4000;
    setTimeout(() => {
        notification.classList.add('fading');
        setTimeout(() => notification.remove(), 300);
    }, fadeDuration);
}

// ============================================================================
// MCP Results Handler - Inject results from V3 back into the chat
// ============================================================================

/**
 * Listener for MCP status updates from background.js
 */
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
    if (request.action === 'mcp_status_update') {
        const { status, isError } = request.payload || {};
        console.log('[MCP] Status:', status, { isError });
        showMcpStatusNotification(status, isError);
        return; // Don't continue processing
    }
});

/**
 * Listener for MCP results from background.js
 * When sidebar detects MCP calls and executes them, this receives the formatted results
 */
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
    if (request.action === 'inject_mcp_results') {
        const { mcpResults, originalMessageId } = request.payload || {};

        if (!mcpResults || !Array.isArray(mcpResults) || mcpResults.length === 0) {
            console.log('[MCP] No results to inject');
            return;
        }

        console.log('[MCP] Received MCP results to inject:', mcpResults);

        // Format results into a prompt
        // V3 returns content as ToolContent array: { content: [{ type: "text", text: "..." }] }
        let resultPrompt = '[🔧 MCP Tool Results]\n\n';
        for (const result of mcpResults) {
            resultPrompt += `**Tool:** ${result.tool}\n`;
            if (result.success) {
                let resultText;
                if (result.result?.content && Array.isArray(result.result.content)) {
                    resultText = result.result.content
                        .filter(c => c.type === 'text')
                        .map(c => c.text)
                        .join('\n');
                } else if (typeof result.result === 'string') {
                    resultText = result.result;
                } else {
                    resultText = JSON.stringify(result.result, null, 2);
                }
                resultPrompt += `**Result:**\n\`\`\`\n${resultText}\n\`\`\`\n\n`;
            } else {
                resultPrompt += `**Error:** ${result.error}\n\n`;
            }
        }

        resultPrompt += '\n⏭️ Zastosuj powyższe wyniki do kontynuacji pracy nad zadaniem. Wygeneruj nową odpowiedź z @gluon:next_step.';

        // Try to paste and auto-send
        (async () => {
            try {
                // Import the injection function from gemini-injector (it's in the same content_scripts)
                if (typeof pasteContextToChat === 'function') {
                    await pasteContextToChat(resultPrompt, true); // auto-send = true
                    console.log('[MCP] ✅ Results injected and message sent');
                } else {
                    console.warn('[MCP] pasteContextToChat not available, falling back to manual paste');
                    // Fallback: just paste and user sends manually
                    const textarea = document.querySelector("div[contenteditable='true'], textarea");
                    if (textarea) {
                        textarea.focus();
                        textarea.textContent = resultPrompt;
                        console.log('[MCP] ⚠️ Results pasted (manual send required)');
                    }
                }
                sendResponse({ success: true });
            } catch (error) {
                console.error('[MCP] Error injecting results:', error);
                sendResponse({ success: false, error: error.message });
            }
        })();

        return true; // Keep channel open for async operations
    }
});