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
// GLUON COMMAND DETECTOR v3.1 - PRODUCTION FIX
// ============================================================================

logger.log('🚀 === COMMAND DETECTOR SCRIPT START ===');
logger.log('URL:', window.location.href);
logger.log('ReadyState:', document.readyState);

// ============================================================================
// MAIN CLASS DEFINITION (with isolated try-catch)
// ============================================================================

let GluonCommandDetector;

try {
  logger.log('Defining GluonCommandDetector class...');
  
  GluonCommandDetector = class {
    constructor() {
      logger.log('🏗️ Constructor called');
      this.observer = null;
      this.processedCodeBlocks = new WeakSet();
      this.processingTimeout = null;
      this.DEBOUNCE_DELAY = 300;
      
      this.JSON_PATTERNS = [
        /```json\s*(\{[\s\S]*?"@gluon:(?:select|[a-z_]+)"[\s\S]*?\})\s*```/g,
        /```\s*(\{[\s\S]*?"@gluon:(?:select|[a-z_]+)"[\s\S]*?\})\s*```/g,
        /\{[\s\S]*?"@gluon:(?:select|[a-z_]+)"[\s\S]*?\}/g
      ];
      
      logger.success('Constructor completed');
    }

    init() {
      logger.log('🎬 Starting command detector...');
      this.startObserving();
      
      // Aggressive scanning
      setTimeout(() => this.scanExistingContent(), 500);
      
      let scanCount = 0;
      const aggressiveScan = setInterval(() => {
        this.scanExistingContent();
        scanCount++;
        if (scanCount >= 5) {
          clearInterval(aggressiveScan);
          setInterval(() => this.scanExistingContent(), 5000);
        }
      }, 2000);
      
      logger.success('Init completed');
    }

    startObserving() {
      const targetNode = document.body;
      const config = { childList: true, subtree: true, characterData: true };

      this.observer = new MutationObserver((mutations) => {
        const hasTextChanges = mutations.some(mutation => {
          return mutation.type === 'characterData' ||
                (mutation.addedNodes.length > 0 && 
                  Array.from(mutation.addedNodes).some(node => 
                    !node.classList || !node.classList.contains('gluon-command-controls')
                  ));
        });
        
        if (hasTextChanges) {
          clearTimeout(this.processingTimeout);
          this.processingTimeout = setTimeout(() => {
            this.scanExistingContent();
          }, this.DEBOUNCE_DELAY);
        }
      });

      this.observer.observe(targetNode, config);
      logger.success('MutationObserver started');
    }

    scanExistingContent() {
      
      const selectors = [
        'pre', 'code', '[class*="code"]', '[class*="Code"]',
        '[class*="language"]', 'div[data-lexical-editor]',
        '.font-mono', '[style*="font-family: monospace"]'
      ];
      
      const allCodeBlocks = [];
      selectors.forEach(selector => {
        document.querySelectorAll(selector).forEach(block => {
          if (!allCodeBlocks.includes(block)) {
            allCodeBlocks.push(block);
          }
        });
      });
      
      const mainCodeBlocks = allCodeBlocks.filter(block => {
        if (block.tagName === 'CODE' && block.closest('pre')) return false;
        if (this.processedCodeBlocks.has(block)) return false;
        if (block.closest('.gluon-command-controls')) return false;
        if (block.previousElementSibling?.classList.contains('gluon-command-controls')) {
          this.processedCodeBlocks.add(block);
          return false;
        }
        return true;
      });
      
      mainCodeBlocks.forEach((block, index) => {
        this.processCodeBlock(block);
      });
    }

    processCodeBlock(codeBlock) {
      if (this.processedCodeBlocks.has(codeBlock)) return;
      if (codeBlock.previousElementSibling?.classList.contains('gluon-command-controls')) {
        this.processedCodeBlocks.add(codeBlock);
        return;
      }
      
      const text = codeBlock.textContent || codeBlock.innerText;
      
      // ⚠️ IGNORE RESPONSE FORMAT
      if (text.includes('@gluon:response')) {
        logger.log('⏭️ Skipping - this is a RESPONSE format (handled by response-detector.js)');
        return;
      }
      
      if (!text.includes('@gluon:')) {
        return;
      }
      
      logger.log('🎯 Found @gluon: marker, trying to parse...');
      
      for (const pattern of this.JSON_PATTERNS) {
        const matches = text.matchAll(pattern);
        
        for (const match of matches) {
          const jsonStr = match[1] || match[0];
          
          try {
            const cleanedJson = this.cleanJsonString(jsonStr);
            const parsed = JSON.parse(cleanedJson);
            
            logger.log('📦 Parsed JSON:', parsed);
            
            if (this.isValidGluonCommand(parsed)) {
              logger.success('Valid Gluon COMMAND detected!');
              const commandData = this.extractCommandData(parsed);
              this.addControlsToCodeBlock(codeBlock, commandData);
              return;
            } else {
              // logger.log('❌ Invalid command structure');
            }
          } catch (e) {
            // logger.log('❌ JSON parse error:', e.message);
          }
        }
      }
    }

    cleanJsonString(str) {
      return str
        .replace(/```json\s*/g, '')
        .replace(/```\s*/g, '')
        .trim();
    }

    isValidGluonCommand(parsed) {
      if (!parsed || typeof parsed !== 'object') return false;
      
      // REJECT response format
      if (parsed["@gluon:response"]) {
        logger.log('⛔ This is a RESPONSE, not a COMMAND');
        return false;
      }
      
      // NEW MULTI-PROJECT FORMAT: {"@gluon:backend": [...], "@gluon:frontend": [...]}
      const projectKeys = Object.keys(parsed).filter(key => 
        key.startsWith('@gluon:') && key !== '@gluon:select'
      );
      
      if (projectKeys.length > 0) {
        const allValid = projectKeys.every(key => 
          Array.isArray(parsed[key]) && parsed[key].length > 0
        );
        
        if (allValid) {
          logger.success('Valid NEW multi-project format:', projectKeys);
          return true;
        }
      }
      
      // LEGACY FORMAT: {"@gluon:select": [...]}
      if (parsed["@gluon:select"] && 
          Array.isArray(parsed["@gluon:select"]) &&
          parsed["@gluon:select"].length > 0) {
        logger.success('Valid LEGACY format');
        return true;
      }
      
      return false;
    }

    extractCommandData(parsed) {
      // Multi-project format
      const projectKeys = Object.keys(parsed).filter(key => 
        key.startsWith('@gluon:') && key !== '@gluon:select'
      );
      
      if (projectKeys.length > 0) {
        const result = {};
        projectKeys.forEach(key => {
          result[key] = parsed[key];
        });
        return result;
      }
      
      // Legacy format
      if (parsed["@gluon:select"]) {
        return parsed["@gluon:select"];
      }
      
      return null;
    }

    addControlsToCodeBlock(codeBlock, commandData) {
      logger.log('🎨 Adding controls to code block');
      
      let insertTarget = codeBlock;
      let insertParent = codeBlock.parentElement;
      
      if (codeBlock.tagName === 'CODE' && codeBlock.closest('pre')) {
        insertTarget = codeBlock.closest('pre');
        insertParent = insertTarget.parentElement;
      }
      
      if (!insertParent) {
        logger.error('No parent element found!');
        return;
      }
      
      if (insertTarget.previousElementSibling?.classList.contains('gluon-command-controls')) {
        logger.log('✓ Controls already exist');
        this.processedCodeBlocks.add(codeBlock);
        return;
      }
      
      const controls = this.createControls(commandData, this.hashCommand(commandData), codeBlock);
      
      try {
        insertParent.insertBefore(controls, insertTarget);
        logger.success('Controls inserted!');
        
        setTimeout(() => {
          if (document.body.contains(controls)) {
            this.processedCodeBlocks.add(codeBlock);
            logger.log('✓ Block marked as processed');
          }
        }, 100);
      } catch (e) {
        logger.error('Failed to insert controls:', e);
      }
    }

    createControls(commandData, hash, codeBlock) {
      let totalFiles = 0;
      let projectCount = 0;
      let isMultiProject = false;
      
      if (Array.isArray(commandData)) {
        totalFiles = commandData.length;
        projectCount = 1;
      } else if (typeof commandData === 'object') {
        isMultiProject = true;
        const projectKeys = Object.keys(commandData).filter(k => k.startsWith('@gluon:'));
        projectCount = projectKeys.length;
        projectKeys.forEach(key => {
          totalFiles += commandData[key].length;
        });
      }
      
      const wrapper = document.createElement('div');
      wrapper.className = 'gluon-command-controls';
      wrapper.dataset.commandHash = hash;
      
      const buttonText = isMultiProject 
        ? `⚛️ Apply Gluon Config (${totalFiles} files, ${projectCount} projects)`
        : `⚛️ Apply Gluon Config (${totalFiles} files)`;
      
      wrapper.innerHTML = `
        <button class="gluon-apply-btn" title="Use these files in Gluon">
          ${buttonText}
        </button>
      `;
      
      wrapper.style.cssText = `
        display: block !important;
        margin: 12px 0 !important;
        padding: 8px !important;
        background: linear-gradient(135deg, #1a1d2e, #0f1521) !important;
        border: 2px solid #00d4ff !important;
        border-radius: 8px !important;
        box-shadow: 0 0 20px rgba(0, 212, 255, 0.5) !important;
        position: relative !important;
        z-index: 999999 !important;
        visibility: visible !important;
        opacity: 1 !important;
      `;
      
      const button = wrapper.querySelector('.gluon-apply-btn');
      button.style.cssText = `
        display: block !important;
        width: 100% !important;
        background: linear-gradient(135deg, #00d4ff, #00ff88) !important;
        color: #0a0d1a !important;
        border: none !important;
        padding: 6px 12px !important;
        border-radius: 6px !important;
        font-size: 12px !important;
        font-weight: bold !important;
        cursor: pointer !important;
        box-shadow: 0 4px 12px rgba(0, 212, 255, 0.4) !important;
        font-family: system-ui, -apple-system, sans-serif !important;
        transition: all 0.2s ease !important;
        text-align: center !important;
      `;
      
      button.onmouseover = () => {
        button.style.transform = 'translateY(-2px)';
        button.style.boxShadow = '0 6px 20px rgba(0, 212, 255, 0.6)';
      };
      
      button.onmouseout = () => {
        button.style.transform = 'translateY(0)';
        button.style.boxShadow = '0 4px 12px rgba(0, 212, 255, 0.4)';
      };
      
      button.onclick = (e) => {
        e.stopPropagation();
        logger.log('Apply button clicked:', commandData);
        this.sendToSidebar(commandData);
        
        const successText = isMultiProject 
          ? `✓ Applied! ${totalFiles} files from ${projectCount} projects`
          : `✓ Applied! ${totalFiles} files selected`;
        
        button.innerHTML = successText;
        button.style.background = 'linear-gradient(135deg, #00ff88, #00d4ff)';
        
        setTimeout(() => {
          button.innerHTML = buttonText;
          button.style.background = 'linear-gradient(135deg, #00d4ff, #00ff88)';
        }, 2000);
        
        this.showNotification(`✓ ${totalFiles} files selected in Gluon!`, 'success');
      };
      
      return wrapper;
    }

    sendToSidebar(commandData) {
      logger.log('Sending to sidebar:', commandData);
      
      chrome.runtime.sendMessage({
        action: 'select_files',
        files: commandData,
        clearPrevious: true
      }, (response) => {
        if (chrome.runtime.lastError) {
          logger.error('Failed:', chrome.runtime.lastError);
          return;
        }
        
        if (response?.success) {
          const fileCount = Array.isArray(commandData) 
            ? commandData.length 
            : Object.values(commandData).reduce((sum, arr) => sum + arr.length, 0);
          
          this.showNotification(`✅ Selected ${fileCount} files`, 'success');
        }
      });
    }
    
    hashCommand(commandData) {
      let hashString = '';
      
      if (Array.isArray(commandData)) {
        hashString = commandData.slice().sort().join('|');
      } else if (typeof commandData === 'object') {
        const projectKeys = Object.keys(commandData)
          .filter(k => k.startsWith('@gluon:'))
          .sort();
        const entries = projectKeys
          .map(key => `${key}:${commandData[key].sort().join(',')}`)
          .join('|');
        hashString = entries;
      }
      
      return 'gluon-' + hashString.replace(/[^a-zA-Z0-9]/g, '-').substring(0, 50);
    }

    showNotification(message, type = 'info') {
      const notification = document.createElement('div');
      notification.textContent = message;
      
      const bgColors = {
        success: 'linear-gradient(135deg, #00d4ff, #00ff88)',
        error: 'linear-gradient(135deg, #ef4444, #dc2626)',
        info: 'linear-gradient(135deg, #6366f1, #8b5cf6)'
      };
      
      notification.style.cssText = `
        position: fixed;
        top: 24px;
        right: 24px;
        padding: 14px 24px;
        background: ${bgColors[type]};
        color: ${type === 'success' ? '#0a0d1a' : 'white'};
        border-radius: 12px;
        font-family: system-ui, -apple-system, sans-serif;
        font-size: 14px;
        font-weight: 600;
        z-index: 999999;
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
        animation: slideInFromRight 0.4s cubic-bezier(0.68, -0.55, 0.265, 1.55);
        cursor: pointer;
      `;
      
      document.body.appendChild(notification);
      
      notification.addEventListener('click', () => {
        notification.style.animation = 'slideOutToRight 0.3s ease-in';
        setTimeout(() => notification.remove(), 300);
      });
      
      setTimeout(() => {
        if (notification.parentElement) {
          notification.style.animation = 'slideOutToRight 0.3s ease-in';
          setTimeout(() => notification.remove(), 300);
        }
      }, 4000);
    }

    destroy() {
      if (this.observer) {
        this.observer.disconnect();
        this.observer = null;
      }
      clearTimeout(this.processingTimeout);
      document.querySelectorAll('.gluon-command-controls').forEach(ctrl => ctrl.remove());
      this.processedCodeBlocks = new WeakSet();
    }
  };
  
  logger.success('Class definition successful');
  
} catch (error) {
  logger.error('💥 CRITICAL ERROR in class definition:', error);
  logger.error('Stack:', error.stack);
}

// ============================================================================
// STYLES (separate try-catch)
// ============================================================================

try {
  if (!document.getElementById('gluon-detector-styles')) {
    const style = document.createElement('style');
    style.id = 'gluon-detector-styles';
    style.textContent = `
      @keyframes slideInFromRight {
        from { transform: translateX(400px); opacity: 0; }
        to { transform: translateX(0); opacity: 1; }
      }
      
      @keyframes slideOutToRight {
        from { transform: translateX(0); opacity: 1; }
        to { transform: translateX(400px); opacity: 0; }
      }
      
      .gluon-command-controls {
        display: block !important;
        visibility: visible !important;
        opacity: 1 !important;
      }
      
      .gluon-apply-btn {
        display: block !important;
        visibility: visible !important;
        opacity: 1 !important;
      }
    `;
    document.head.appendChild(style);
    logger.success('Styles injected');
  }
} catch (error) {
  logger.error('Failed to inject styles:', error);
}

// ============================================================================
// INITIALIZATION (separate try-catch)
// ============================================================================

let detector;

function initDetector() {
  try {
    if (!document.body) {
      logger.log('Waiting for body...');
      setTimeout(initDetector, 100);
      return;
    }
    
    if (!GluonCommandDetector) {
      logger.error('GluonCommandDetector class not defined!');
      return;
    }
    
    logger.log('🎬 Creating detector instance...');
    detector = new GluonCommandDetector();
    
    logger.log('🎬 Initializing detector...');
    detector.init();
    
    logger.log('🎬 Exposing to window...');
    window.__gluonDetector = detector;
    
    logger.success('Detector fully initialized and exposed!');
    logger.log('Test with: window.__gluonDetector');
    
  } catch (error) {
    logger.error('💥 CRITICAL ERROR during initialization:', error);
    logger.error('Stack:', error.stack);
  }
}

try {
  if (document.readyState === 'loading') {
    logger.log('Waiting for DOMContentLoaded...');
    document.addEventListener('DOMContentLoaded', initDetector);
  } else {
    logger.log('DOM ready, initializing immediately...');
    initDetector();
  }

  window.addEventListener('beforeunload', () => {
    if (detector) detector.destroy();
  });
  
} catch (error) {
  logger.error('💥 CRITICAL ERROR in event listeners:', error);
}

logger.success('=== COMMAND DETECTOR SCRIPT END ===');