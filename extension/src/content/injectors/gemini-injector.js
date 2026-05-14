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

// ============================================================================
// Gemini/AI Studio & Claude File Injector
// ============================================================================

logger.log('Universal file injector loaded');

// Detect current platform
const PLATFORM = detectPlatform();

function detectPlatform() {
  const hostname = window.location.hostname;
  if (hostname.includes('claude.ai')) return 'CLAUDE';
  if (hostname.includes('gemini.google.com') || hostname.includes('aistudio.google.com')) return 'GEMINI';
  return 'UNKNOWN';
}

// Listen for file upload requests from background script
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.action === 'upload_file_to_gemini') {
    logger.log('Received file upload request:', message.file);
    uploadFileToAI(message.file)
      .then(() => sendResponse({ success: true }))
      .catch(err => sendResponse({ success: false, error: err.message }));
    return true;
  }
  if (message.action === 'upload_binary_file_to_ai') {
      logger.log('Received binary file upload request. Payload:', message.payload); 
      
      try {
        const { filename, mimeType, base64Content } = message.payload; // <-- ZMIANA TUTAJ
        let byteCharacters;

        // DODATKOWY BLOK TRY...CATCH WOKÓŁ KRYTYCZNEJ OPERACJI
        try {
          logger.log('Step 1: Decoding Base64...'); 
          byteCharacters = atob(base64Content); // <-- ZMIANA TUTAJ
          logger.log('Step 1 SUCCESS. Decoded length:', byteCharacters.length); 
        } catch (e) {
          logger.error('FATAL: atob() failed!', e); 
          // Rzuć błąd dalej, aby został złapany przez zewnętrzny catch
          throw new Error('Base64 decoding failed. The data might be corrupt.');
        }
        
        logger.log('Step 2: Creating byte array...'); 
        const byteNumbers = new Array(byteCharacters.length);
        for (let i = 0; i < byteCharacters.length; i++) {
          byteNumbers[i] = byteCharacters.charCodeAt(i);
        }
        const byteArray = new Uint8Array(byteNumbers);
        logger.log('Step 2 SUCCESS.'); 
        
        logger.log('Step 3: Creating Blob and File...'); 
        const blob = new Blob([byteArray], { type: mimeType }); // <-- ZMIANA TUTAJ
        const file = new File([blob], filename, { type: mimeType }); // <-- ZMIANA TUTAJ
        logger.log('Step 3 SUCCESS. File object created:', file); 

        uploadFileToAI({
          filename: file.name,
          content: file,
          type: file.type
        })
        .then(() => sendResponse({ success: true }))
        .catch(err => sendResponse({ success: false, error: err.message }));

      } catch (err) {
        logger.error('Error converting Base64 to File:', err);
        sendResponse({ success: false, error: 'Failed to process binary file data.' });
      }

      return true;
  }
  if (message.action === 'paste_prompt_to_input') {
    logger.log('Received paste prompt request');
    pasteIntoBestInput(message.payload)
      .then(() => sendResponse({ success: true }))
      .catch(err => sendResponse({ success: false, error: err.message }));
    return true; // Keep the message channel open for async response
  }
  if (message.action === 'paste_to_chat') {
    logger.log('Received paste_to_chat request (G-Interactive Context)');
    pasteContextToChat(message.text, message.autoSend)
      .then(() => sendResponse({ success: true }))
      .catch(err => sendResponse({ success: false, error: err.message }));
    return true; // Keep the message channel open for async response
  }
  if (message.action === 'delete_attachment') {
    logger.log('Received delete_attachment request:', message.payload);
    deleteAttachmentFromUI(message.payload.filename)
      .then(() => sendResponse({ success: true }))
      .catch(err => sendResponse({ success: false, error: err.message }));
    return true; // Keep the message channel open for async response
  }
  // [ENTERPRISE] New Action: Delete entire turn context
  if (message.action === 'delete_context_turn') {
    logger.log('Received delete_context_turn request:', message.payload);
    const cleaner = new ContextCleaner();
    cleaner.removeContextTurn(message.payload.filename)
      .then(() => sendResponse({ success: true }))
      .catch(err => sendResponse({ success: false, error: err.message }));
    return true;
  }
});

/**
 * [ENTERPRISE] Context Cleaner Class
 * Handles surgical removal of chat turns based on file content
 */
class ContextCleaner {
  constructor() {
    this.retryCount = 0;
    this.maxRetries = 5;
  }

  async removeContextTurn(filename) {
    logger.log(`%c[Cleaner] 🧹 Starting removal sequence for: ${filename}`, 'color: #f0f; font-weight: bold');

    // 1. Find the turn containing the file
    const targetTurn = this.findTurnByFilename(filename);
    if (!targetTurn) {
      logger.warn(`[Cleaner] ⚠️ Turn with file "${filename}" not found. It might be already deleted or name mismatch.`);
      // We return success to not block the new upload flow
      return true;
    }

    logger.log('[Cleaner] 🎯 Target turn identified. Looking for menu button...');

    // 2. Open the menu (Three dots)
    // Expanded selectors for various AI Studio versions
    const menuBtn = targetTurn.querySelector([
        'button[iconname="more_vert"]', 
        'button.mat-mdc-menu-trigger',
        'button[aria-label*="More"]',
        'button[aria-label*="Więcej"]',
        '.ms-chat-turn-options button',
        'button[data-test-id="chat-turn-menu-button"]'
    ].join(', '));

    if (!menuBtn) {
      logger.error('[Cleaner] ❌ Menu button (more_vert) not found in target turn.');
      // Attempt to debug what IS there
      console.dir(targetTurn);
      throw new Error('Menu button not found in target turn');
    }

    // Ensure we scroll to it so clicks register
    menuBtn.scrollIntoView({ block: 'center', behavior: 'instant' });
    await new Promise(r => setTimeout(r, 100)); // Stabilization

    menuBtn.click();
    logger.log('[Cleaner] 🖱️ Menu button clicked. Waiting for overlay...');

    // 3. Wait for Angular Overlay (cdk-overlay-pane)
    const overlay = await this.waitForOverlay();
    if (!overlay) {
      logger.error('[Cleaner] ❌ Menu overlay did not appear after click.');
      throw new Error('Menu overlay did not appear');
    }

    logger.log('[Cleaner] 👁️ Overlay found. Searching for Delete option...');

    // 4. Find and Click Delete
    const deleteBtn = this.findDeleteButtonInOverlay(overlay);
    if (!deleteBtn) {
      logger.error('[Cleaner] ❌ "Delete" option not found in menu overlay.');
      // Close menu to clean up UI if we can't find delete
      document.body.click(); 
      throw new Error('Delete button not found in menu');
    }

    logger.log('[Cleaner] 🗑️ "Delete" button found. Clicking...');
    deleteBtn.click();
    
    // 5. Verify Removal
    logger.log('[Cleaner] ⏳ Waiting for DOM element removal...');
    await this.waitForRemoval(targetTurn);
    logger.log('[Cleaner] ✅ Turn successfully removed from DOM.');
    
    return true;
  }

  findTurnByFilename(filename) {
    // Strategy: Look for file chips/names, then traverse up to ms-chat-turn
    const fileElements = document.querySelectorAll('.name, [title], .file-name');

    for (const el of fileElements) {
      const text = el.textContent || el.title || '';
      if (text.includes(filename)) {
        // Traverse up to find the main turn container
        const turn = el.closest('ms-chat-turn');
        if (turn) {
          logger.log('[Cleaner] Target turn identified:', turn);
          return turn;
        }
      }
    }
    return null;
  }

  async waitForOverlay() {
    return new Promise(resolve => {
      let attempts = 0;
      const interval = setInterval(() => {
        attempts++;
        // Angular Material creates .cdk-overlay-pane at the end of body
        const overlay = document.querySelector('.cdk-overlay-pane');
        // Check if it's visible/has content
        if (overlay && overlay.children.length > 0) {
          clearInterval(interval);
          resolve(overlay);
        }
        if (attempts > 10) { // 2 seconds timeout
          clearInterval(interval);
          resolve(null);
        }
      }, 200);
    });
  }

  findDeleteButtonInOverlay(overlay) {
    // Strategy: Find button containing text "Delete" or icon "delete"
    // Expanded selectors for various UI versions
    const buttons = overlay.querySelectorAll('button, .mat-mdc-menu-item, [role="menuitem"]');

    for (const btn of buttons) {
      const text = (btn.textContent || '').toLowerCase();
      const ariaLabel = (btn.getAttribute('aria-label') || '').toLowerCase();

      // Check inner icon text (Google Symbols often use ligatures like "delete")
      const icon = btn.querySelector('.material-symbols-outlined, .google-symbols, mat-icon');
      const iconText = icon ? (icon.textContent || '').toLowerCase() : '';

      if (
          text.includes('delete') || 
          text.includes('usuń') || 
          text.includes('remove') ||
          ariaLabel.includes('delete') || 
          ariaLabel.includes('usuń') ||
          iconText.includes('delete') ||
          iconText === 'delete_forever' // Common ID for delete icon
      ) {
        return btn;
      }
    }
    return null;
  }

  async waitForRemoval(element) {
    return new Promise(resolve => {
      if (!element.isConnected) {
        resolve();
        return;
      }

      const observer = new MutationObserver(() => {
        if (!element.isConnected) {
          observer.disconnect();
          resolve();
        }
      });

      observer.observe(document.body, { childList: true, subtree: true });

      // Fallback timeout
      setTimeout(() => {
        observer.disconnect();
        resolve();
      }, 3000);
    });
  }
}

/**
 * Universal file upload for both Claude and Gemini
 */
async function uploadFileToAI(fileData) {
  try {
    // Check if content is already a File/Blob object or raw content
    const file = fileData.content instanceof File 
      ? fileData.content
      : new File([fileData.content], fileData.filename, { 
          type: fileData.type || 'text/plain' 
        });
    
    logger.log('File created/prepared:', { 
      name: file.name, 
      size: file.size, 
      type: file.type,
      platform: PLATFORM
    });

    if (PLATFORM === 'CLAUDE') {
      await uploadToClaude(file);
    } else if (PLATFORM === 'GEMINI') {
      await uploadToGemini(file);
    } else {
      throw new Error('Unsupported platform');
    }
    
    logger.log('File upload procedure finished.');
    // Notification logic is now inside the upload functions
    
  } catch (error) {
    logger.error('Upload failed:', error);
    showNotification('Failed to attach file: ' + error.message, 'error');
    throw error;
  }
}

/**
 * Upload file to Claude.ai
 */
async function uploadToClaude(file) {
  // Method 1: Try to find and click the file input
  const fileInput = document.querySelector('input[type="file"][multiple]') || 
                    document.querySelector('input[type="file"]');
  
  if (fileInput) {
    
    // Create DataTransfer with file
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(file);
    fileInput.files = dataTransfer.files;
    
    // Trigger change event
    fileInput.dispatchEvent(new Event('change', { bubbles: true }));
    
    await new Promise(resolve => setTimeout(resolve, 200));
    return true;
  }
  
  // Method 2: Try drag & drop on the chat input area
  const dropZone = findClaudeDropZone();
  if (dropZone) {
    logger.log('Using drag & drop for Claude');
    await simulateFileDrop(dropZone, file);
    return true;
  }
  
  throw new Error('Could not find upload mechanism for Claude');
}

/**
 * Upload file to Gemini/AI Studio
 */
async function uploadToGemini(file) {
  // --- Strategy 1: Paste into Input Area (Preferred for Chat) ---
  // Expanded selectors for various versions of Gemini/AI Studio/Claude-like UIs
  const inputSelectors = [
    'div[contenteditable="true"]',
    '[role="textbox"]',
    '.ql-editor',
    'textarea',
    'input[type="text"]',
    'rich-textarea'
  ];

  let inputArea = null;
  for (const selector of inputSelectors) {
    const el = document.querySelector(selector);
    if (el) {
      inputArea = el;
      break;
    }
  }

  if (inputArea) {
    logger.log('Detected input area via selector. Attempting to paste file...', inputArea);
    // Ensure focus before pasting
    inputArea.focus();
    const success = await simulateFilePaste(inputArea, file);
    if (success) {
      showNotification('File attached', 'success');
      return;
    }
    logger.warn('Paste failed or not verified. Trying drop strategy next...');
  }

  // --- Strategy 2: Drag & Drop (Fallback or for AI Studio) ---
  const dropZone = findGeminiDropZone();
  if (dropZone) {
    logger.log('Detected Drop Zone. Attempting to drop file...', dropZone);
    const success = await simulateFileDrop(dropZone, file);
    if (success) {
      showNotification('File attached', 'success');
      return;
    }
  }
  
  // --- Fallback Error ---
  throw new Error('Could not find a compatible input area for Gemini or AI Studio (selectors failed).');
}

/**
 * Find drop zone for Claude.ai
 */
function findClaudeDropZone() {
  // Try to find the main chat container or textarea
  const selectors = [
    '[contenteditable="true"]', // Main input area
    '.ProseMirror', // Rich text editor
    'fieldset', // Input fieldset container
    '[data-testid="chat-input"]',
    'div[role="textbox"]'
  ];
  
  for (const selector of selectors) {
    const element = document.querySelector(selector);
    if (element) {
      return element;
    }
  }
  
  return null;
}

/**
 * Find drop zone for Gemini/AI Studio
 */
function findGeminiDropZone() {
  // Priority list of selectors for Drop Zones
  const selectors = [
    // Gemini specific
    'div[file-drop-zone]',
    'div[xapfileselectordropzone]',
    'input-area-v2',
    
    // AI Studio specific
    '[msfiledragdrop]',
    'ms-prompt-input-wrapper',
    '.attachment-wrapper',
    
    // Generic Editors (often accept drops)
    '.ql-editor[contenteditable="true"]',
    'div[contenteditable="true"]',
    '[role="textbox"]',
    'textarea',
    
    // Broad containers as last resort
    '.input-area',
    'main' 
  ];

  for (const selector of selectors) {
    const el = document.querySelector(selector);
    if (el) {
      logger.log(`Drop zone found via selector: ${selector}`);
      
      // Special handling for AI Studio textarea wrapper
      if (selector === '[msfilecopypaste]') {
         return el.closest('.prompt-input-wrapper-container') || el.parentElement;
      }
      
      return el;
    }
  }

  // Check for AI Studio specific attribute even if querySelector failed above (redundancy)
  const textarea = document.querySelector('[msfilecopypaste]');
  if (textarea) {
    logger.log('Using textarea parent as drop zone (AI Studio fallback)');
    return textarea.closest('.prompt-input-wrapper-container') || textarea.parentElement;
  }
  
  logger.warn('No drop zone found for Gemini/AI Studio');
  return null;
}

/**
 * Simulates pasting a file into a target element.
 * @param {HTMLElement} target The contenteditable element to paste into.
 * @param {File} file The file to be pasted.
 * @returns {Promise<boolean>} True if the attachment is verified, false otherwise.
 */
async function simulateFilePaste(target, file) {
  logger.log('--- Starting simulateFilePaste ---');
  
  const dataTransfer = new DataTransfer();
  dataTransfer.items.add(file);

  try {
    logger.log('Dispatching paste event...');
    target.dispatchEvent(new ClipboardEvent('paste', {
      bubbles: true,
      cancelable: true,
      clipboardData: dataTransfer
    }));
    logger.log('Paste event dispatched.');

    // --- VERIFICATION ---
    logger.log('Waiting 1500ms for UI to update after paste...');
    await new Promise(resolve => setTimeout(resolve, 1500));
    
    logger.log('--- Starting Verification ---');
    // Updated selector list for modern Gemini UI & AI Studio (Angular Material)
    // Added ms-file-chip, mat-chip-row, and .mat-mdc-chip for newer AI Studio versions
    const selectors = [
      '.attachment-list-item',
      'file-chip',
      'ms-file-chip',          // AI Studio specific
      'mat-chip',
      'mat-chip-row',
      '.mat-mdc-chip',         // New Angular Material
      '[aria-label*="Attachment:"]',
      '[aria-label*="załącznik"]',
      '.upload-chip-container',
      '.bg-token-surface-variant' // Generic container often used for files
    ];

    const fileChip = document.querySelector(selectors.join(', '));

    if (fileChip) {
        logger.log('VERIFIED: Found file chip element:', fileChip);
        logger.log('SUCCESS: A file chip element was found in the UI.');
        return true;
    } 
    
    // --- AI Studio Specific Fallback ---
    // AI Studio (Angular) often processes the file internally without immediately 
    // rendering a standard DOM element we can query easily, or structure changed.
    // If we targeted an AI Studio component successfully, assume success to prevent double-upload.
    const isAIStudio = target.hasAttribute('msfilecopypaste') || 
                       target.closest('ms-prompt-input-wrapper') ||
                       document.querySelector('ms-app-layout');

    if (isAIStudio) {
        logger.log('AI STUDIO DETECTED: Visual verification failed, but event was dispatched to a valid AI Studio input.');
        logger.log('ASSUMING SUCCESS to prevent double-uploading.');
        return true;
    }

    // Standard Fallback check
    const filePreview = document.querySelector('div[class*="file-preview"]');
    if (filePreview) {
      logger.log('VERIFIED: Found file preview element as a fallback.');
      return true;
    }
    
    logger.log('FAILED: Did not find a file chip or preview element after paste.');
    return false;

  } catch (e) {
    logger.error('An error occurred during paste event dispatch:', e);
    return false;
  }
}

/**
 * Simulates dropping a file onto a target element. (For AI Studio)
 * @param {HTMLElement} target The element to drop the file onto.
 * @param {File} file The file to be dropped.
 * @returns {Promise<boolean>} True if the attachment is verified, false otherwise.
 */
async function simulateFileDrop(target, file) {
  logger.log('--- Starting simulateFileDrop (AI Studio) ---');
  
  const dataTransfer = new DataTransfer();
  dataTransfer.items.add(file);

  try {
    target.dispatchEvent(new DragEvent('dragenter', { bubbles: true, cancelable: true, dataTransfer }));
    await new Promise(resolve => setTimeout(resolve, 200));
    
    target.dispatchEvent(new DragEvent('dragover', { bubbles: true, cancelable: true, dataTransfer }));
    await new Promise(resolve => setTimeout(resolve, 200));
    
    target.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer }));
    logger.log('Drop dispatched.');

    await new Promise(resolve => setTimeout(resolve, 1500));
    
    // Verification for AI Studio - check for chips instead of just wrapper
    // Reuse the same robust selectors as Paste strategy
    const selectors = [
      'ms-file-chip',
      'mat-chip',
      'mat-chip-row',
      '.mat-mdc-chip',
      '.attachment-wrapper .remove-button', // If a remove button exists, a file exists
      '.attachment-preview'
    ];
    
    const fileChip = document.querySelector(selectors.join(', '));
    const attachmentWrapper = document.querySelector('.attachment-wrapper');

    if (fileChip || (attachmentWrapper && attachmentWrapper.children.length > 0)) {
      logger.log('SUCCESS: AI Studio attachment verified.');
      return true;
    } 
    
    // AI Studio "Soft Success"
    const isAIStudio = target.hasAttribute('msfilecopypaste') || 
                       target.closest('ms-prompt-input-wrapper') ||
                       document.querySelector('ms-app-layout');

    if (isAIStudio) {
        logger.log('AI STUDIO DETECTED (DROP): Visual verification failed, but drop event dispatched.');
        logger.log('ASSUMING SUCCESS.');
        return true;
    }

    logger.log('FAILED: AI Studio attachment verification failed (no chips found).');
    return false;

  } catch (e) {
    logger.error('An error occurred during drag-drop event dispatch:', e);
    return false;
  }
}

/**
 * Show temporary notification
 */
function showNotification(message, type = 'info') {
  const notification = document.createElement('div');
  notification.textContent = message;
  notification.style.cssText = `
    position: fixed;
    top: 20px;
    right: 20px;
    padding: 12px 20px;
    background: ${type === 'success' ? '#10b981' : '#ef4444'};
    color: white;
    border-radius: 8px;
    font-family: 'Google Sans', system-ui, sans-serif;
    font-size: 14px;
    z-index: 999999;
    box-shadow: 0 4px 12px rgba(0,0,0,0.3);
    animation: slideIn 0.3s ease-out;
  `;
  
  const style = document.createElement('style');
  style.textContent = `
    @keyframes slideIn {
      from { transform: translateX(400px); opacity: 0; }
      to { transform: translateX(0); opacity: 1; }
    }
  `;
  document.head.appendChild(style);
  
  document.body.appendChild(notification);
  
  setTimeout(() => {
    notification.style.transition = 'opacity 0.3s, transform 0.3s';
    notification.style.opacity = '0';
    notification.style.transform = 'translateX(400px)';
    setTimeout(() => notification.remove(), 300);
  }, 3000);
}

/**
 * Pastes context text into chat input and optionally sends it
 * @param {string} textToPaste - The context text to paste
 * @param {boolean} autoSend - Whether to click send button after paste
 */
async function pasteContextToChat(textToPaste, autoSend = false) {
  const selectors = [
    "div.ql-editor[contenteditable='true']",       // Gemini's rich text editor
    "div[role='textbox'][contenteditable='true']", // Another common Gemini pattern
    ".ProseMirror[contenteditable='true']",       // Claude's editor
    "div[contenteditable='true']",               // Generic fallback
    'textarea',
    'input[type="text"]'
  ];

  let inputElement = null;
  for (const selector of selectors) {
    inputElement = document.querySelector(selector);
    if (inputElement) {
      break;
    }
  }

  if (!inputElement) {
    throw new Error("Could not find a suitable input field on the page.");
  }

  // Focus and paste the content
  inputElement.focus();

  if (inputElement.isContentEditable) {
    // For contenteditable, use innerHTML/textContent or paste event
    const lines = textToPaste.split('\n');
    const formattedHtml = lines.map(line => `<div>${line || '<br>'}</div>`).join('');

    const dataTransfer = new DataTransfer();
    dataTransfer.setData('text/html', formattedHtml);
    dataTransfer.setData('text/plain', textToPaste);

    const pasteEvent = new ClipboardEvent('paste', {
        clipboardData: dataTransfer,
        bubbles: true,
        cancelable: true,
    });

    inputElement.dispatchEvent(pasteEvent);
  } else {
    // Fallback for simple textarea/input elements
    inputElement.value = textToPaste;
    inputElement.dispatchEvent(new Event('input', { bubbles: true, cancelable: true }));
  }

  logger.log('Context pasted into chat input');

  // Auto-send if requested
  if (autoSend) {
    await new Promise(resolve => setTimeout(resolve, 300)); // Wait for UI to update

    const sendButtonSelectors = [
      'button[aria-label*="Send"]',
      'button[title*="Send"]',
      'button.send-button',
      'button[type="submit"]',
      '.send-prompt-button',
      'button[aria-label*="Wyślij"]' // Polish version
    ];

    let sendButton = null;
    for (const selector of sendButtonSelectors) {
      sendButton = document.querySelector(selector);
      if (sendButton && !sendButton.disabled) {
        break;
      }
    }

    if (sendButton) {
      logger.log('Auto-sending message...');
      sendButton.click();
    } else {
      logger.warn('Send button not found or disabled - message not sent automatically');
    }
  }
}

/**
 * Finds the best input field on the page and pastes text into it.
 * @param {string} textToPaste - The full prompt text.
 */
async function pasteIntoBestInput(textToPaste) {
  const selectors = [
    "div.ql-editor[contenteditable='true']",       // Gemini's rich text editor
    "div[role='textbox'][contenteditable='true']", // Another common Gemini pattern
    ".ProseMirror[contenteditable='true']",       // Claude's editor
    "div[contenteditable='true']",               // Generic fallback
    'textarea',
    'input[type="text"]'
  ];

  let inputElement = null;
  for (const selector of selectors) {
    inputElement = document.querySelector(selector);
    if (inputElement) {
      break;
    }
  }

  if (!inputElement) {
    throw new Error("Could not find a suitable input field on the page.");
  }

  // 1. Set the content
  inputElement.focus(); // Ensure the element is active first.

  if (inputElement.isContentEditable) {
    if (PLATFORM === 'CLAUDE') {
        // For Claude's ProseMirror, we must simulate a paste event with HTML content
        // to correctly render line breaks, as direct .innerHTML/.textContent is ignored.
        logger.log('Simulating HTML paste event for Claude.');
        
        // For Claude's ProseMirror, we must simulate a paste event with HTML content
        // to correctly render line breaks. Each line is wrapped in a <div> to treat it
        // as a separate paragraph block, mimicking how rich text is copied from other editors.
        logger.log('Simulating structured HTML paste event for Claude.');

        const lines = textToPaste.split('\n');
        // Wrap each line in a <div>. For empty lines, insert a <br> to preserve the space.
        // This ensures tags like <gluon_system> and section breaks are rendered correctly.
        const formattedHtml = lines.map(line => `<div>${line || '<br>'}</div>`).join('');

        const dataTransfer = new DataTransfer();
        dataTransfer.setData('text/html', formattedHtml);
        dataTransfer.setData('text/plain', textToPaste); // Also provide plain text fallback.

        const pasteEvent = new ClipboardEvent('paste', {
            clipboardData: dataTransfer,
            bubbles: true,
            cancelable: true,
        });

        inputElement.dispatchEvent(pasteEvent);
    } else if (PLATFORM === 'GEMINI') { // This is where Gemini logic goes
        logger.log('Applying Gemini-specific pasting logic with hidden tags and formatting.');
        
        const lines = textToPaste.split('\n');
        let inSystemBlock = false;

        const finalHtml = lines.map(line => {
            const trimmedLine = line.trim();
            
            if (trimmedLine === '<gluon_system>') {
                inSystemBlock = true;
                return `<div style="display: none;">${line}</div>`;
            }
            if (trimmedLine === '</gluon_system>') {
                const result = `<div style="display: none;">${line}</div>`;
                inSystemBlock = false;
                return result;
            }
            if (trimmedLine === '<gluon_user>' || trimmedLine === '</gluon_user>') {
                return `<div style="display: none;">${line}</div>`;
            }
            if (inSystemBlock) {
                return `<div style="display: none;">${line || '<br>'}</div>`;
            }
            return `<div>${line || '<br>'}</div>`;
        }).join('');
        
        // FINAL STRATEGY: Use the modern Clipboard API to write to the clipboard,
        // then execute the native 'paste' command. The background script is now responsible
        // for focusing the document before this code is called.
        try {
            logger.log('Using Clipboard API to write HTML and then executing "paste" command.');
            
            // Focus the specific input element one last time to be safe.
            inputElement.focus();
            
            // Create blobs for the Clipboard API
            const htmlBlob = new Blob([finalHtml], { type: 'text/html' });
            const textBlob = new Blob([textToPaste], { type: 'text/plain' });

            // Create a ClipboardItem with both HTML and plain text versions
            const clipboardItem = new ClipboardItem({
                'text/html': htmlBlob,
                'text/plain': textBlob
            });

            // Write the item to the system clipboard
            await navigator.clipboard.write([clipboardItem]);

            // Now, command the document to paste from the clipboard
            const success = document.execCommand('paste');
            if (!success) {
                throw new Error('document.execCommand("paste") was unsuccessful.');
            }
        } catch (error) {
            logger.error('Critical paste failure:', error);
            throw new Error('Failed to paste prompt. Use Ctrl + V in model input.');
        }
    }
  } else {
    // Fallback for simple textarea/input elements.
    inputElement.value = textToPaste;
    inputElement.dispatchEvent(new Event('input', { bubbles: true, cancelable: true }));
  }

  // 3. Set the cursor position inside <gluon_user> tag
  await new Promise(resolve => setTimeout(resolve, 50)); // Small delay for DOM update

  const content = (inputElement.textContent || inputElement.value) || '';
  const userTagStart = content.indexOf('<gluon_user>');
  
  if (userTagStart !== -1) {
    const userTagEnd = content.indexOf('>', userTagStart) + 1;
    
    try {
      if (inputElement.isContentEditable) {
        const selection = window.getSelection();
        const range = document.createRange();
        let textNode = null;
        let charCount = 0;
        
        function findTextNode(element) {
            for (const node of element.childNodes) {
                if (node.nodeType === Node.TEXT_NODE) {
                    if (charCount + node.length >= userTagEnd) {
                        textNode = node;
                        return true;
                    }
                    charCount += node.length;
                } else {
                    if (findTextNode(node)) return true;
                }
            }
            return false;
        }

        if (findTextNode(inputElement)) {
            range.setStart(textNode, userTagEnd - charCount);
            range.collapse(true);
            selection.removeAllRanges();
            selection.addRange(range);
        }
      } else {
        // Fallback for textarea/input
        inputElement.selectionStart = inputElement.selectionEnd = userTagEnd;
      }
    } catch (error) {
      logger.warn('Failed to set cursor position:', error);
      // Graceful degradation - just focus the input
      inputElement.focus();
    }
  }
  logger.log('Prompt pasted and cursor positioned.');
}

/**
 * Deletes an attachment from the AI Studio UI by filename
 * Finds the file chip and simulates clicking the delete button
 * @param {string} filename - Name of the file to delete
 */
async function deleteAttachmentFromUI(filename) {
  logger.log(`[Delete Attachment] Looking for file: ${filename}`);

  try {
    // Strategy 1: Find file chip by exact filename match
    // AI Studio uses ms-file-chip elements with name span inside
    const fileChips = document.querySelectorAll('ms-file-chip, mat-chip, mat-chip-row, .mat-mdc-chip, file-chip');

    let targetChip = null;

    for (const chip of fileChips) {
      // Check if this chip contains the filename
      const nameElement = chip.querySelector('.name, [title], span[class*="name"]');
      const chipText = chip.textContent || chip.innerText;

      if ((nameElement && nameElement.textContent.includes(filename)) ||
          chipText.includes(filename)) {
        targetChip = chip;
        logger.log('[Delete Attachment] Found target chip:', chip);
        break;
      }
    }

    if (!targetChip) {
      // Strategy 2: Fallback - find by partial match or aria-label
      const allChips = document.querySelectorAll('[aria-label*="' + filename + '"], [title*="' + filename + '"]');
      if (allChips.length > 0) {
        targetChip = allChips[0];
        logger.log('[Delete Attachment] Found via aria-label/title:', targetChip);
      }
    }

    if (!targetChip) {
      logger.warn(`[Delete Attachment] File chip not found for: ${filename}`);
      throw new Error(`Attachment "${filename}" not found in UI`);
    }

    // Find the "more options" button (three dots) or direct delete button
    const moreButton = targetChip.querySelector('button[aria-label*="More"], button[aria-label*="options"], .mat-mdc-menu-trigger, button.ms-button-icon, button[iconname="more_vert"]');
    const directDeleteButton = targetChip.querySelector('button[aria-label*="Delete"], button[aria-label*="Remove"], button[aria-label*="delete"]');

    if (directDeleteButton) {
      // If there's a direct delete button, click it
      logger.log('[Delete Attachment] Clicking direct delete button');
      directDeleteButton.click();
      await new Promise(resolve => setTimeout(resolve, 500));

      // Confirm deletion if modal appears
      await confirmDeletionModal();

      logger.log('[Delete Attachment] ✅ File deleted successfully');
      showNotification(`Removed: ${filename}`, 'success');
      return true;
    }

    if (moreButton) {
      // Click the more options button to open menu
      logger.log('[Delete Attachment] Clicking more options button');
      moreButton.click();

      // Wait for menu to appear
      await new Promise(resolve => setTimeout(resolve, 300));

      // Find and click the Delete option in the menu
      const deleteMenuItems = document.querySelectorAll(
        'button[mat-menu-item], .mat-mdc-menu-item, button[role="menuitem"]'
      );

      let deleteButton = null;
      for (const item of deleteMenuItems) {
        const itemText = item.textContent.toLowerCase();
        const itemIcon = item.querySelector('.material-symbols-outlined, mat-icon');
        const iconText = itemIcon ? itemIcon.textContent.toLowerCase() : '';

        if (itemText.includes('delete') || itemText.includes('usuń') ||
            itemText.includes('remove') || iconText.includes('delete')) {
          deleteButton = item;
          logger.log('[Delete Attachment] Found delete menu item:', item);
          break;
        }
      }

      if (deleteButton) {
        logger.log('[Delete Attachment] Clicking delete in menu');
        deleteButton.click();

        await new Promise(resolve => setTimeout(resolve, 500));

        // Confirm deletion if modal appears
        await confirmDeletionModal();

        logger.log('[Delete Attachment] ✅ File deleted successfully');
        showNotification(`Removed: ${filename}`, 'success');
        return true;
      } else {
        throw new Error('Delete option not found in menu');
      }
    }

    // Strategy 3: Try to find a close/remove button directly on the chip
    const closeButton = targetChip.querySelector('button[aria-label*="close"], button[aria-label*="remove"], .remove-button, button.close-button');
    if (closeButton) {
      logger.log('[Delete Attachment] Clicking close button on chip');
      closeButton.click();
      await new Promise(resolve => setTimeout(resolve, 500));
      logger.log('[Delete Attachment] ✅ File deleted successfully');
      showNotification(`Removed: ${filename}`, 'success');
      return true;
    }

    throw new Error('No delete mechanism found for this attachment');

  } catch (error) {
    logger.error('[Delete Attachment] Failed:', error);
    throw error;
  }
}

/**
 * Confirms deletion if a modal/dialog appears
 */
async function confirmDeletionModal() {
  await new Promise(resolve => setTimeout(resolve, 200));

  // Look for confirmation dialog
  const confirmButtons = document.querySelectorAll(
    'button[mat-dialog-close], button[aria-label*="Confirm"], button[aria-label*="Yes"], ' +
    'button.confirm-button, button[class*="confirm"]'
  );

  for (const button of confirmButtons) {
    const buttonText = button.textContent.toLowerCase();
    if (buttonText.includes('delete') || buttonText.includes('confirm') ||
        buttonText.includes('yes') || buttonText.includes('ok') ||
        buttonText.includes('usuń') || buttonText.includes('potwierdź')) {
      logger.log('[Delete Attachment] Clicking confirmation button');
      button.click();
      await new Promise(resolve => setTimeout(resolve, 300));
      break;
    }
  }
}