/**
 * Gluon Apply System - Overlay UI (GitHub Style)
 * Displays code changes in a split view: File list on the left, Diff on the right.
 */

if (typeof logger === 'undefined') {
  var logger = {
    log: (...args) => console.log('[Apply System Overlay]', ...args),
    warn: (...args) => console.warn('[Apply System Overlay]', ...args),
    error: (...args) => console.error('[Apply System Overlay]', ...args),
  };
}

// Utility: Generate UUID v4
function generateUUID() {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
    const r = Math.random() * 16 | 0;
    const v = c === 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
}

// State Management
let overlayState = {
  changes: [],
  selectedChanges: new Set(),
  activeChangeIndex: 0,
  isVisible: false,
  overlayElement: null,
};

// Light UI State - Track changes by request_id for batch display
let lightUIState = {
  changesByRequest: {}, // { request_id: [change1, change2, ...] }
};

/**
 * Create the overlay HTML structure
 */
function createOverlayHTML() {
  const overlay = document.createElement('div');
  overlay.id = 'gluon-apply-system-overlay';
  overlay.className = 'gluon-apply-overlay hidden';

  overlay.innerHTML = `
    <div class="gluon-pr-container">
      <!-- Header -->
      <div class="gluon-pr-header">
        <div class="gluon-pr-title">
          <svg height="24" viewBox="0 0 24 24" width="24" fill="currentColor" style="color: #c9d1d9; margin-right: 8px;">
            <path d="M21.03 5.72a.75.75 0 0 1 0 1.06l-11.5 11.5a.75.75 0 0 1-1.072-.012l-5.5-5.75a.75.75 0 1 1 1.084-1.036l4.97 5.195L19.97 5.72a.75.75 0 0 1 1.06 0Z"></path>
          </svg>
          <span>Review & Apply Changes</span>
        </div>
        <div class="gluon-pr-header-actions">
            <span class="gluon-stat" id="total-changes-badge">0 files</span>

            <!-- Dropdown with changes list -->
            <div class="gluon-header-btn-group">
              <button class="gluon-header-btn" id="changes-dropdown-btn" title="View all changes">
                <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M2 4a1 1 0 011-1h10a1 1 0 110 2H3a1 1 0 01-1-1zm0 4a1 1 0 011-1h10a1 1 0 110 2H3a1 1 0 01-1-1zm1 3a1 1 0 100 2h10a1 1 0 100-2H3z"/>
                </svg>
                <span>Changes</span>
                <span style="font-size: 10px; margin-left: 4px;">▼</span>
              </button>
              <div class="gluon-header-dropdown" id="changes-dropdown-menu">
                <!-- Populated dynamically -->
              </div>
            </div>

            <!-- Undo button -->
            <button class="gluon-header-btn" id="undo-all-btn" title="Undo all applied changes">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
                <path d="M3.5 2a.5.5 0 00-.5.5v5a.5.5 0 001 0V3.707l8.146 8.147a.5.5 0 00.708-.708L4.707 3H9.5a.5.5 0 000-1h-6z"/>
              </svg>
              <span>Undo All</span>
            </button>

            <!-- Report Bug button -->
            <button class="gluon-header-btn" id="report-bug-btn" title="Report an issue with Apply System">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
                <path d="M8 0a8 8 0 110 16A8 8 0 018 0zM7 5a1 1 0 112 0v3a1 1 0 11-2 0V5zm1 7a1 1 0 100-2 1 1 0 000 2z"/>
              </svg>
              <span>Report Bug</span>
            </button>

            <button class="gluon-close-btn" id="close-overlay">✕ Close</button>
        </div>
      </div>

      <!-- Body -->
      <div class="gluon-pr-body">

        <!-- LEFT: Sidebar -->
        <div class="gluon-pr-sidebar">
            <div class="gluon-sidebar-header">
                <h3>Files changed</h3>
                <div class="gluon-sidebar-tools">
                    <button id="select-all-btn">All</button>
                    <button id="deselect-all-btn">None</button>
                </div>
            </div>
            <div class="gluon-file-list" id="file-list-container">
                <!-- File list injected here -->
            </div>
        </div>

        <!-- RIGHT: Diff View -->
        <div class="gluon-pr-main">
            <div id="diff-viewer-container">
                <!-- Diff content will be injected here -->
            </div>

            <!-- Apply button moved to bottom of diff view -->
            <div class="gluon-apply-footer">
                <button id="apply-btn" class="gluon-btn-primary-large">
                    Apply Selected (<span id="selected-count">0</span>)
                </button>
            </div>
        </div>

      </div>
    </div>
  `;

  return overlay;
}

/**
 * Inject CSS Styles
 */
function injectOverlayStyles() {
  if (document.getElementById('gluon-apply-system-styles')) return;

  const style = document.createElement('style');
  style.id = 'gluon-apply-system-styles';
  style.textContent = `
    .gluon-apply-overlay {
      position: fixed;
      top: 0; left: 0; right: 0; bottom: 0;
      background: rgba(1, 4, 9, 0.85);
      backdrop-filter: blur(4px);
      z-index: 999999;
      display: flex;
      justify-content: center;
      align-items: center;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
      opacity: 0;
      transition: opacity 0.2s ease;
      pointer-events: none;
    }
    .gluon-apply-overlay:not(.hidden) {
      opacity: 1;
      pointer-events: auto;
    }

    .gluon-pr-container {
      width: 90vw;
      height: 85vh;
      max-width: 800px;
      max-height: 1000px;
      background: #0d1117;
      border: 1px solid #30363d;
      border-radius: 12px;
      display: flex;
      flex-direction: column;
      overflow: hidden;
      box-shadow: 0 0 40px rgba(0,0,0,0.6);
    }

    /* HEADER */
    .gluon-pr-header {
      background: #161b22;
      border-bottom: 1px solid #30363d;
      padding: 0 20px;
      display: flex;
      justify-content: space-between;
      align-items: center;
      height: 60px;
      flex-shrink: 0;
    }
    .gluon-pr-title {
      display: flex;
      align-items: center;
      font-size: 18px;
      font-weight: 600;
      color: #c9d1d9;
    }
    .gluon-stat {
      color: #8b949e;
      font-size: 14px;
      margin-right: 16px;
    }
    .gluon-close-btn {
      background: transparent;
      border: 1px solid #30363d;
      color: #c9d1d9;
      padding: 6px 12px;
      border-radius: 6px;
      cursor: pointer;
      font-size: 14px;
      transition: 0.2s;
    }
    .gluon-close-btn:hover { background: #21262d; color: white; }

    /* HEADER ACTIONS */
    .gluon-pr-header-actions {
      display: flex;
      align-items: center;
      gap: 12px;
    }
    .gluon-header-btn-group {
      position: relative;
      display: inline-block;
    }
    .gluon-header-btn {
      display: flex;
      align-items: center;
      gap: 6px;
      background: transparent;
      border: 1px solid #30363d;
      color: #c9d1d9;
      padding: 6px 12px;
      border-radius: 6px;
      cursor: pointer;
      font-size: 13px;
      font-weight: 500;
      transition: all 0.2s;
    }
    .gluon-header-btn:hover {
      background: #21262d;
      color: white;
      border-color: #8b949e;
    }
    .gluon-header-btn svg {
      flex-shrink: 0;
    }
    .gluon-header-btn#undo-all-btn:hover {
      background: rgba(218, 54, 51, 0.1);
      border-color: #da3633;
      color: #da3633;
    }
    .gluon-header-btn#report-bug-btn:hover {
      background: rgba(255, 191, 0, 0.1);
      border-color: #ffbf00;
      color: #ffbf00;
    }

    /* HEADER DROPDOWN MENU */
    .gluon-header-dropdown {
      position: absolute;
      top: calc(100% + 8px);
      right: 0;
      background: #161b22;
      border: 1px solid #30363d;
      border-radius: 8px;
      box-shadow: 0 8px 24px rgba(0,0,0,0.6);
      min-width: 320px;
      max-width: 500px;
      max-height: 400px;
      overflow-y: auto;
      display: none;
      z-index: 1000;
      padding: 8px 0;
    }
    .gluon-header-dropdown.visible {
      display: block;
    }
    .gluon-changes-dropdown-item {
      padding: 10px 16px;
      cursor: pointer;
      border-bottom: 1px solid rgba(48, 54, 61, 0.5);
      transition: background 0.15s;
      display: flex;
      align-items: center;
      gap: 10px;
    }
    .gluon-changes-dropdown-item:last-child {
      border-bottom: none;
    }
    .gluon-changes-dropdown-item:hover {
      background: rgba(33, 38, 45, 0.8);
    }
    .gluon-changes-dropdown-item .file-name {
      font-family: monospace;
      font-size: 12px;
      color: #c9d1d9;
      flex: 1;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .gluon-changes-dropdown-item .status-icon {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      flex-shrink: 0;
    }

    /* BODY */
    .gluon-pr-body {
      display: flex;
      flex: 1;
      overflow: hidden;
    }

    /* MAIN (DIFF) */
    .gluon-pr-main {
      flex: 1;
      display: flex;
      flex-direction: column;
      background: #0d1117;
      overflow: hidden;
    }

    #diff-viewer-container {
      flex: 1;
      overflow-y: auto;
      padding: 24px;
    }

    /* APPLY FOOTER (Bottom of diff view) */
    .gluon-apply-footer {
      padding: 16px 24px;
      border-top: 1px solid #30363d;
      background: #161b22;
      flex-shrink: 0;
    }

    .gluon-diff-card {
      border: 1px solid #30363d;
      border-radius: 6px;
      background: #0d1117;
      overflow: hidden;
      margin-bottom: 20px;
    }
    .gluon-diff-header {
      background: #161b22;
      padding: 12px 16px;
      border-bottom: 1px solid #30363d;
      font-family: monospace;
      color: #c9d1d9;
      font-size: 13px;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    .gluon-diff-path { font-weight: 600; }
    .gluon-diff-lines { color: #8b949e; }

    .gluon-code-block {
      margin: 0;
      padding: 16px;
      background: #0d1117;
      color: #e6edf3;
      font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
      font-size: 13px;
      line-height: 1.5;
      white-space: pre-wrap;
      overflow-x: auto;
    }

    .gluon-diff-row { display: block; width: 100%; }
    .gluon-diff-search { background: rgba(248, 81, 73, 0.1); border-left: 3px solid #f85149; padding-left: 10px; }
    .gluon-diff-replace { background: rgba(46, 160, 67, 0.1); border-left: 3px solid #3fb950; padding-left: 10px; }
    .gluon-diff-create { background: rgba(46, 160, 67, 0.1); border-left: 3px solid #3fb950; padding-left: 10px; }

    /* --- GLUON STATUS INDICATORS --- */
    .gluon-status-dot {
        width: 8px; height: 8px; 
        border-radius: 50%; 
        display: inline-block; 
        margin-right: 8px;
        flex-shrink: 0;
    }
    .status-pending { background: #8b949e; box-shadow: 0 0 4px rgba(139, 148, 158, 0.4); }
    .status-applying { background: #e3b341; animation: pulse 1s infinite; }
    .status-success { background: #238636; box-shadow: 0 0 4px rgba(35, 134, 54, 0.4); }
    .status-error { background: #da3633; }
    
    @keyframes pulse { 0% { opacity: 1; } 50% { opacity: 0.5; } 100% { opacity: 1; } }

    /* --- DEBUG BUTTON STYLES --- */
    .gluon-debug-btn {
        position: absolute;
        right: 8px;
        top: 50%;
            background: transparent;
            border: none;
            cursor: pointer;
            font-size: 16px;
            opacity: 0.4; /* ZMIANA: Zawsze lekko widoczny */
            transition: all 0.2s ease;
            padding: 4px;
            border-radius: 4px;
            z-index: 10;
            display: flex;
            align-items: center;
            justify-content: center;
        }

        /* Pełna widoczność przy hover lub błędzie */
        .gluon-file-item:hover .gluon-debug-btn,
        .gluon-debug-btn.always-visible,
        .gluon-debug-btn:hover {
            opacity: 1;
        }

        .gluon-debug-btn:hover {
            background: rgba(255, 255, 255, 0.1);
            transform: translateY(-50%) scale(1.2);
            color: #f85149 !important; /* Podświetl na czerwono przy hover */
        }

    /* --- SIDE-BY-SIDE SPLIT VIEW --- */
    .gluon-split-view {
        display: flex;
        gap: 0;
        border: 1px solid #30363d;
        border-radius: 6px;
        overflow: hidden;
        background: #0d1117;
        max-height: 600px;
        overflow-y: auto;
    }
    .gluon-split-pane {
        flex: 1;
        overflow-x: auto;
        min-width: 0; /* Zapobiega rozpychaniu flexa przez pre */
    }
    .gluon-split-pane:first-child { border-right: 1px solid #30363d; }
    
    .gluon-pane-header {
        padding: 6px 12px;
        background: #161b22;
        border-bottom: 1px solid #30363d;
        font-size: 11px;
        color: #8b949e;
        text-transform: uppercase;
        font-weight: 600;
        letter-spacing: 0.5px;
        position: sticky;
        top: 0;
    }

    /* --- DIFF LINES --- */
    .diff-row {
        display: flex;
        width: 100%;
    }
    .diff-num {
        width: 40px;
        min-width: 40px;
        text-align: right;
        padding-right: 8px;
        color: #6e7681;
        background: #0d1117;
        border-right: 1px solid #30363d;
        user-select: none;
        opacity: 0.7;
        font-size: 11px;
        line-height: 18px;
    }
    .diff-line {
        font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
        font-size: 12px;
        line-height: 18px;
        white-space: pre; 
        padding: 0 8px;
        min-height: 18px;
        color: #e6edf3;
        flex: 1;
        overflow-x: auto;
    }
    .diff-add { background: rgba(46, 160, 67, 0.15); }
    .diff-del { background: rgba(248, 81, 73, 0.15); opacity: 0.8; }
    .diff-ctx { color: #8b949e; } /* Context / placeholder lines */
    /* --- ACTION BUTTONS --- */
    .gluon-btn-small {
        padding: 4px 10px;
        font-size: 12px;
        font-weight: 600;
        border-radius: 4px;
        border: 1px solid rgba(240, 246, 252, 0.1);
        background: #21262d;
        color: #c9d1d9;
        cursor: pointer;
        transition: 0.2s;
        display: inline-flex;
        align-items: center;
        justify-content: center;
    }
    .gluon-btn-small:hover:not(:disabled) { background: #30363d; color: white; border-color: #8b949e; }
    .gluon-btn-small:disabled { opacity: 0.6; cursor: not-allowed; }

    .btn-success-state { color: #238636 !important; border-color: rgba(35, 134, 54, 0.4) !important; background: transparent !important; }
    /* --- DROPDOWN SPLIT BUTTON --- */
    .gluon-btn-group {
        display: inline-flex;
        align-items: center;
        margin-left: 8px;
        position: relative;
    }
    .gluon-btn-group .gluon-btn-small {
        margin: 0;
        border-top-right-radius: 0;
        border-bottom-right-radius: 0;
        border-right: none;
    }
    /* LIGHT UI SPECIFIC STYLES */
    .gluon-btn-group.light-ui {
        margin-left: 0; /* Container handles gap */
    }
    .gluon-btn-group.light-ui .gluon-apply-btn {
        margin: 0;
        border-top-right-radius: 0;
        border-bottom-right-radius: 0;
        border-right: none;
    }

    .gluon-dropdown-trigger {
        padding: 4px 6px;
        font-size: 10px;
        border-radius: 0 4px 4px 0;
        border: 1px solid rgba(240, 246, 252, 0.1);
        border-left: 1px solid rgba(255,255,255,0.1);
        background: #21262d;
        color: #c9d1d9;
        cursor: pointer;
        height: 28px; /* Match btn-small height approx */
        display: flex;
        align-items: center;
        justify-content: center;
    }
    /* Adjust trigger height for Light UI button which is usually taller (32px) */
    .gluon-btn-group.light-ui .gluon-dropdown-trigger {
        height: 32px; 
        background: #21262d;
        border-color: #30363d; 
    }
    /* --- GLUON COMPACT UI (Minimalist Icons) --- */
    .gluon-overlay-container {
        display: flex !important;
        flex-direction: row !important;
        align-items: center !important;
        justify-content: flex-start !important;
        gap: 10px !important;
        padding: 10px 14px !important;
        background: linear-gradient(135deg, #161b22 0%, #0d1117 100%) !important;
        border: 1px solid #30363d !important;
        border-radius: 8px !important;
        margin-top: 12px !important;
        overflow: visible !important;
        position: relative !important;
        z-index: 1000 !important;
        width: fit-content !important;
        max-width: 95% !important;
        box-shadow: 0 8px 24px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.05) !important;
        transition: all 0.2s ease !important;
    }

    .gluon-overlay-container:hover {
        box-shadow: 0 12px 32px rgba(0,0,0,0.6), 0 0 0 1px rgba(255,255,255,0.1) !important;
        transform: translateY(-1px) !important;
    }

    /* Minimalist Progress/Status */
    .gluon-status-compact {
        display: flex !important;
        align-items: center !important;
        gap: 6px !important;
        font-size: 13px !important;
        font-weight: 600 !important;
        color: #c9d1d9 !important;
        padding: 4px 10px !important;
        background: rgba(255,255,255,0.05) !important;
        border-radius: 6px !important;
        white-space: nowrap !important;
        letter-spacing: 0.3px !important;
    }
    .gluon-status-compact.success {
        color: #3fb950 !important;
        background: rgba(46, 160, 67, 0.15) !important;
        border: 1px solid rgba(46, 160, 67, 0.3) !important;
    }
    .gluon-status-compact.error {
        color: #f85149 !important;
        background: rgba(248, 81, 73, 0.15) !important;
        border: 1px solid rgba(248, 81, 73, 0.3) !important;
    }

    /* Minimalist Icon Buttons */
    .gluon-icon-btn {
        width: 32px !important;
        height: 32px !important;
        display: flex !important;
        align-items: center !important;
        justify-content: center !important;
        border-radius: 6px !important;
        border: 1px solid #30363d !important;
        background: rgba(33, 38, 45, 0.6) !important;
        color: #c9d1d9 !important;
        cursor: pointer !important;
        transition: all 0.15s ease !important;
        font-size: 16px !important;
        font-weight: 600 !important;
        padding: 0 !important;
        position: relative !important;
    }
    .gluon-icon-btn:hover {
        background: #30363d !important;
        border-color: #484f58 !important;
        color: #ffffff !important;
        transform: translateY(-1px) !important;
        box-shadow: 0 4px 8px rgba(0,0,0,0.3) !important;
    }
    .gluon-icon-btn:active {
        transform: translateY(0) !important;
    }
    .gluon-icon-btn.primary {
        color: #3fb950 !important;
        border-color: rgba(46, 160, 67, 0.3) !important;
    }
    .gluon-icon-btn.danger {
        color: #f85149 !important;
        border-color: rgba(248, 81, 73, 0.3) !important;
    }

    /* Toggle for Dropdown */
    .gluon-icon-btn.active {
        background: #388bfd !important;
        border-color: #388bfd !important;
        color: white !important;
        box-shadow: 0 0 0 3px rgba(56, 139, 253, 0.2) !important;
    }

    /* DROPDOWN MENU (Compact List) */
    .gluon-compact-dropdown {
        position: absolute !important;
        top: 100% !important;
        right: 0 !important;
        margin-top: 8px !important;
        background: #161b22 !important;
        border: 1px solid #30363d !important;
        border-radius: 8px !important;
        box-shadow: 0 12px 32px rgba(0,0,0,0.8), 0 0 0 1px rgba(255,255,255,0.1) !important;
        z-index: 2147483647 !important;
        min-width: 300px !important;
        max-width: 450px !important;
        display: none !important;
        flex-direction: column !important;
        padding: 0 !important;
        animation: slideDown 0.2s ease !important;
        overflow: hidden !important;
    }
    .gluon-compact-dropdown.visible { display: flex !important; }

    @keyframes slideDown {
        from {
            opacity: 0;
            transform: translateY(-10px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }
    /* Dropdown Header */
    .gluon-compact-dropdown > div:first-child {
        background: linear-gradient(135deg, #21262d 0%, #161b22 100%) !important;
        padding: 10px 14px !important;
        font-size: 11px !important;
        font-weight: 700 !important;
        letter-spacing: 0.5px !important;
        color: #8b949e !important;
        border-bottom: 1px solid #30363d !important;
        text-transform: uppercase !important;
    }

    /* --- BATCH TABLE STYLES (V3.2) --- */
    .gluon-batch-table {
        width: 100% !important;
        border-collapse: collapse !important;
        font-size: 12px !important;
        color: #c9d1d9 !important;
    }

    .gluon-batch-table th {
        text-align: left !important;
        padding: 8px 12px !important;
        color: #8b949e !important;
        font-weight: 600 !important;
        background: rgba(255,255,255,0.02) !important;
        border-bottom: 1px solid #30363d !important;
    }

    .gluon-batch-table td {
        padding: 8px 12px !important;
        border-bottom: 1px solid rgba(48, 54, 61, 0.5) !important;
        vertical-align: middle !important;
    }

    .gluon-batch-table tr:last-child td {
        border-bottom: none !important;
    }

    .gluon-batch-table tr:hover td {
        background: rgba(255,255,255,0.04) !important;
    }

    .gluon-col-status { width: 30px; text-align: center !important; }
    .gluon-col-id { width: 40px; font-family: monospace; color: #8b949e; }
    .gluon-col-line { font-family: monospace; color: #58a6ff; }
    .gluon-col-action { text-align: right !important; }

    /* Fix Dropdown Positioning */
    .gluon-overlay-container {
        /* Ensure stacking context for absolute dropdown */
        position: relative !important; 
    }
    .gluon-compact-dropdown {
        top: calc(100% + 5px) !important; /* Move it clearly below */
        right: 0 !important;
    }
    .gluon-change-item:hover {
        background: rgba(33, 38, 45, 0.6) !important;
    }
    .gluon-change-item:last-child { border-bottom: none !important; }

    .gluon-change-info {
        display: flex !important;
        flex-direction: column !important;
        gap: 4px !important;
        flex: 1 !important;
    }
    .gluon-change-info strong {
        font-weight: 600 !important;
        color: #e6edf3 !important;
        font-size: 13px !important;
    }
    .gluon-change-line {
        color: #8b949e !important;
        font-size: 11px !important;
        font-family: "SFMono-Regular", Consolas, monospace !important;
    }

    .gluon-item-action {
        background: rgba(33, 38, 45, 0.8) !important;
        border: 1px solid #30363d !important;
        color: #c9d1d9 !important;
        border-radius: 5px !important;
        padding: 5px 10px !important;
        font-size: 11px !important;
        font-weight: 600 !important;
        cursor: pointer !important;
        transition: all 0.15s ease !important;
        white-space: nowrap !important;
    }
    .gluon-item-action:hover {
        background: #da3633 !important;
        border-color: #da3633 !important;
        color: white !important;
        transform: scale(1.05) !important;
    }

    .gluon-item-action.redo-action {
        background: rgba(34, 134, 58, 0.8) !important;
        border-color: rgba(34, 134, 58, 0.5) !important;
        color: #3fb950 !important;
    }

    .gluon-item-action.redo-action:hover {
        background: #3fb950 !important;
        border-color: #3fb950 !important;
        color: white !important;
        transform: scale(1.05) !important;
    }

    .gluon-item-action.pending-action {
        background: rgba(79, 161, 246, 0.8) !important;
        border-color: rgba(79, 161, 246, 0.5) !important;
        color: #58a6ff !important;
    }

    .gluon-item-action.pending-action:hover {
        background: #58a6ff !important;
        border-color: #58a6ff !important;
        color: white !important;
        transform: scale(1.05) !important;
    }

    .gluon-dropdown-item {
        padding: 8px 12px;
        font-size: 12px;
        color: #c9d1d9;
        cursor: pointer;
        border: none;
        background: transparent;
        text-align: left;
        width: 100%;
    }
    .gluon-dropdown-item:hover { background: #388bfd; color: white; }
    .gluon-dropdown-divider { height: 1px; background: #30363d; margin: 4px 0; }
    .gluon-dropdown-item.danger:hover { background: #da3633; }

    /* Debug Button (Light UI) */
    .gluon-debug-btn-light {
        width: 32px !important;
        height: 32px !important;
        display: flex !important;
        align-items: center !important;
        justify-content: center !important;
        border-radius: 6px !important;
        border: 1px solid rgba(248, 81, 73, 0.4) !important;
        background: rgba(248, 81, 73, 0.1) !important;
        color: #f85149 !important;
        cursor: pointer !important;
        transition: all 0.15s ease !important;
        font-size: 16px !important;
        padding: 0 !important;
        opacity: 0.7 !important;
    }
    .gluon-debug-btn-light:hover {
        background: rgba(248, 81, 73, 0.2) !important;
        border-color: #f85149 !important;
        opacity: 1 !important;
        transform: translateY(-1px) scale(1.05) !important;
        box-shadow: 0 4px 12px rgba(248, 81, 73, 0.3) !important;
    }
    .gluon-debug-btn-light:active {
        transform: translateY(0) scale(1) !important;
    }

    /* SIDEBAR */
    .gluon-pr-sidebar {
      width: 320px;
      background: #161b22;
      border-right: 1px solid #30363d;
      display: flex;
      flex-direction: column;
      flex-shrink: 0;
    }
    .gluon-sidebar-header {
      padding: 16px;
      border-bottom: 1px solid #30363d;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    .gluon-sidebar-header h3 { margin: 0; color: #c9d1d9; font-size: 14px; font-weight: 600; }
    .gluon-sidebar-tools button {
        background: transparent; border: 1px solid #30363d; color: #8b949e;
        padding: 2px 8px; border-radius: 4px; cursor: pointer; font-size: 12px;
    }
    .gluon-sidebar-tools button:hover { color: #c9d1d9; border-color: #8b949e; }

    .gluon-file-list {
      flex: 1;
      overflow-y: auto;
      padding: 10px;
    }

    .gluon-file-item {
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 10px;
      border-radius: 6px;
      cursor: pointer;
      margin-bottom: 4px;
      border: 1px solid transparent;
      transition: 0.1s;
      position: relative; /* Ważne dla pozycjonowania przycisku */
      padding-right: 40px; /* Miejsce na przycisk debug */
    }
    .gluon-file-item:hover { background: #21262d; }
    .gluon-file-item.active { background: #21262d; border-color: #388bfd; box-shadow: inset -3px 0 0 #388bfd; }

    .gluon-checkbox {
      width: 16px; height: 16px;
      border: 2px solid #484f58; border-radius: 4px;
      display: flex; align-items: center; justify-content: center;
      flex-shrink: 0;
    }
    .gluon-checkbox.checked { background: #238636; border-color: #238636; }
    .gluon-checkbox svg { width: 12px; height: 12px; color: white; display: none; }
    .gluon-checkbox.checked svg { display: block; }

    .gluon-file-info { overflow: hidden; }
    .gluon-file-name { font-size: 13px; color: #c9d1d9; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .gluon-file-meta { font-size: 11px; color: #8b949e; margin-top: 2px; }

    /* Apply button (now in footer at bottom) */
    .gluon-btn-primary-large {
      width: 100%;
      background: #238636;
      color: white;
      border: 1px solid rgba(240, 246, 252, 0.1);
      padding: 10px;
      border-radius: 6px;
      font-weight: 600;
      font-size: 14px;
      cursor: pointer;
      transition: 0.2s;
    }
    .gluon-btn-primary-large:hover { background: #2ea043; }
    .gluon-btn-primary-large:disabled { background: #21262d; color: #8b949e; cursor: not-allowed; border-color: transparent; }

    /* ============================================================================
       ETAP 4: CONFIDENCE SCORE UI
       ============================================================================ */

    /* Confidence Badge */
    .confidence-badge {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 4px 10px;
      border-radius: 4px;
      font-size: 12px;
      font-weight: 600;
      cursor: help;
      transition: all 0.2s;
      border: 1px solid transparent;
    }

    /* High Confidence (≥90%) - Green */
    .confidence-high {
      background: rgba(46, 160, 67, 0.15);
      color: #3fb950;
      border-color: rgba(46, 160, 67, 0.3);
    }
    .confidence-high:hover {
      background: rgba(46, 160, 67, 0.25);
      border-color: #3fb950;
    }

    /* Medium Confidence (70-89%) - Yellow */
    .confidence-medium {
      background: rgba(227, 179, 65, 0.15);
      color: #e3b341;
      border-color: rgba(227, 179, 65, 0.3);
    }
    .confidence-medium:hover {
      background: rgba(227, 179, 65, 0.25);
      border-color: #e3b341;
    }

    /* Low Confidence (<70%) - Red */
    .confidence-low {
      background: rgba(248, 81, 73, 0.15);
      color: #f85149;
      border-color: rgba(248, 81, 73, 0.3);
    }
    .confidence-low:hover {
      background: rgba(248, 81, 73, 0.25);
      border-color: #f85149;
    }

    /* Confidence Indicator Dot */
    .confidence-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      display: inline-block;
    }
    .confidence-high .confidence-dot { background: #3fb950; box-shadow: 0 0 6px rgba(46, 160, 67, 0.6); }
    .confidence-medium .confidence-dot { background: #e3b341; box-shadow: 0 0 6px rgba(227, 179, 65, 0.6); }
    .confidence-low .confidence-dot { background: #f85149; box-shadow: 0 0 6px rgba(248, 81, 73, 0.6); }

    /* Confidence Tooltip */
    .confidence-tooltip {
      position: absolute;
      background: #21262d;
      border: 1px solid #30363d;
      border-radius: 6px;
      padding: 12px;
      min-width: 280px;
      box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
      z-index: 1000000;
      display: none;
      font-size: 12px;
      color: #c9d1d9;
    }
    .confidence-tooltip.visible { display: block; }

    .confidence-tooltip-header {
      font-weight: 600;
      font-size: 13px;
      margin-bottom: 10px;
      padding-bottom: 8px;
      border-bottom: 1px solid #30363d;
    }

    .confidence-tooltip-row {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin: 6px 0;
      padding: 4px 0;
    }

    .confidence-tooltip-label {
      color: #8b949e;
      font-size: 11px;
    }

    .confidence-tooltip-value {
      font-weight: 600;
      font-family: "SFMono-Regular", Consolas, monospace;
    }

    .confidence-tooltip-bar {
      height: 4px;
      background: #30363d;
      border-radius: 2px;
      margin-top: 4px;
      overflow: hidden;
    }

    .confidence-tooltip-bar-fill {
      height: 100%;
      border-radius: 2px;
      transition: width 0.3s ease;
    }

    .confidence-tooltip-method {
      margin-top: 8px;
      padding-top: 8px;
      border-top: 1px solid #30363d;
      color: #8b949e;
      font-size: 11px;
      font-style: italic;
    }

    /* Matcher Method Badge */
    .matcher-badge {
      display: inline-block;
      background: #161b22;
      border: 1px solid #30363d;
      color: #8b949e;
      padding: 2px 6px;
      border-radius: 3px;
      font-size: 10px;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      margin-left: 8px;
    }
    .matcher-badge.weighted-anchor { color: #58a6ff; border-color: rgba(88, 166, 255, 0.3); }
    .matcher-badge.anchor-points { color: #8b949e; }
    .matcher-badge.fuzzy-match { color: #e3b341; }

    /* ============================================================================
       PROGRESS BAR STYLES (Pulse System)
       ============================================================================ */

    .gluon-progress-container {
      margin-top: 8px;
      width: 100%;
      animation: fadeIn 0.3s ease-in;
    }

    @keyframes fadeIn {
      from { opacity: 0; transform: translateY(-5px); }
      to { opacity: 1; transform: translateY(0); }
    }

    .gluon-progress-bar-bg {
      width: 100%;
      height: 6px;
      background: #21262d;
      border-radius: 3px;
      overflow: hidden;
      box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.3);
    }

    .gluon-progress-bar-fill {
      height: 100%;
      background: linear-gradient(90deg, #2ea043, #3fb950);
      transition: width 0.4s cubic-bezier(0.4, 0.0, 0.2, 1);
      box-shadow: 0 0 8px rgba(46, 160, 67, 0.5);
      position: relative;
      overflow: hidden;
    }

    .gluon-progress-bar-fill::after {
      content: '';
      position: absolute;
      top: 0;
      left: 0;
      right: 0;
      bottom: 0;
      background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
      animation: shimmer 1.5s infinite;
    }

    @keyframes shimmer {
      0% { transform: translateX(-100%); }
      100% { transform: translateX(100%); }
    }

    .gluon-progress-text {
      font-size: 11px;
      color: #8b949e;
      margin-top: 4px;
      display: flex;
      align-items: center;
      gap: 4px;
      font-weight: 500;
    }

    .gluon-progress-text::before {
      content: '⚡';
      font-size: 10px;
      animation: pulse 1s infinite;
    }

    .gluon-progress-details {
      font-size: 10px;
      color: #6e7681;
      margin-left: 4px;
    }
  `;
  document.head.appendChild(style);
}

// ============================================================================
// Logic
// ============================================================================

// ============================================================================
// ETAP 4: Confidence Score Helpers
// ============================================================================

/**
 * Get confidence level class and label based on confidence score
 * @param {number} confidence - Confidence score (0.0 - 1.0)
 * @returns {{level: string, label: string, class: string}}
 */
function getConfidenceLevel(confidence) {
  if (confidence >= 0.90) {
    return { level: 'high', label: 'High Confidence', class: 'confidence-high' };
  } else if (confidence >= 0.70) {
    return { level: 'medium', label: 'Medium Confidence', class: 'confidence-medium' };
  } else {
    return { level: 'low', label: 'Low Confidence', class: 'confidence-low' };
  }
}

/**
 * Get matcher method display name and class
 * @param {string} methodUsed - Matcher method enum value
 * @returns {{name: string, class: string}}
 */
function getMatcherMethodInfo(methodUsed) {
  const methods = {
    'WeightedAnchor': { name: 'Weighted Anchor', class: 'weighted-anchor' },
    'AnchorPoints': { name: 'Anchor Points', class: 'anchor-points' },
    'FuzzyMatch': { name: 'Fuzzy Match', class: 'fuzzy-match' },
    'BlockStructure': { name: 'Block Structure', class: 'block-structure' },
    'RegexPattern': { name: 'Regex Pattern', class: 'regex-pattern' },
    'ExactHash': { name: 'Exact Match', class: 'exact-hash' },
  };
  return methods[methodUsed] || { name: methodUsed || 'Unknown', class: '' };
}

/**
 * Create confidence badge HTML
 * @param {object} matchResult - Match result from backend (contains confidence, method_used, confidence_breakdown)
 * @returns {string} HTML string for confidence badge
 */
function createConfidenceBadge(matchResult) {
  if (!matchResult || typeof matchResult.confidence !== 'number') {
    return ''; // No confidence data available
  }

  const confidence = matchResult.confidence;
  const { label, class: levelClass } = getConfidenceLevel(confidence);
  const percentage = Math.round(confidence * 100);

  return `
    <div class="confidence-badge ${levelClass}" data-confidence="${confidence}" data-has-breakdown="${!!matchResult.confidence_breakdown}">
      <span class="confidence-dot"></span>
      <span>${percentage}% ${label}</span>
    </div>
  `;
}

/**
 * Create detailed confidence tooltip HTML
 * @param {object} matchResult - Match result with confidence_breakdown
 * @returns {string} HTML string for tooltip
 */
function createConfidenceTooltip(matchResult) {
  if (!matchResult) return '';

  const confidence = matchResult.confidence;
  const breakdown = matchResult.confidence_breakdown;
  const methodInfo = getMatcherMethodInfo(matchResult.method_used);

  let tooltipHTML = `
    <div class="confidence-tooltip-header">
      Match Confidence Breakdown
    </div>
  `;

  // If we have detailed breakdown (from Weighted Anchoring)
  if (breakdown) {
    const similarity = breakdown.similarity || 0;
    const tokenSim = breakdown.token_similarity || 0;
    const anchorQuality = breakdown.anchor_quality || 0;

    tooltipHTML += `
      <div class="confidence-tooltip-row">
        <span class="confidence-tooltip-label">Similarity Score</span>
        <span class="confidence-tooltip-value" style="color: ${similarity >= 0.90 ? '#3fb950' : (similarity >= 0.70 ? '#e3b341' : '#f85149')}">${Math.round(similarity * 100)}%</span>
      </div>
      <div class="confidence-tooltip-bar">
        <div class="confidence-tooltip-bar-fill" style="width: ${similarity * 100}%; background: ${similarity >= 0.90 ? '#3fb950' : (similarity >= 0.70 ? '#e3b341' : '#f85149')}"></div>
      </div>

      <div class="confidence-tooltip-row">
        <span class="confidence-tooltip-label">Token Similarity</span>
        <span class="confidence-tooltip-value" style="color: ${tokenSim >= 0.90 ? '#3fb950' : (tokenSim >= 0.70 ? '#e3b341' : '#f85149')}">${Math.round(tokenSim * 100)}%</span>
      </div>
      <div class="confidence-tooltip-bar">
        <div class="confidence-tooltip-bar-fill" style="width: ${tokenSim * 100}%; background: ${tokenSim >= 0.90 ? '#3fb950' : (tokenSim >= 0.70 ? '#e3b341' : '#f85149')}"></div>
      </div>

      <div class="confidence-tooltip-row">
        <span class="confidence-tooltip-label">Anchor Quality</span>
        <span class="confidence-tooltip-value" style="color: ${anchorQuality >= 0.90 ? '#3fb950' : (anchorQuality >= 0.70 ? '#e3b341' : '#f85149')}">${Math.round(anchorQuality * 100)}%</span>
      </div>
      <div class="confidence-tooltip-bar">
        <div class="confidence-tooltip-bar-fill" style="width: ${anchorQuality * 100}%; background: ${anchorQuality >= 0.90 ? '#3fb950' : (anchorQuality >= 0.70 ? '#e3b341' : '#f85149')}"></div>
      </div>

      <div class="confidence-tooltip-row" style="margin-top: 10px; padding-top: 10px; border-top: 1px solid #30363d;">
        <span class="confidence-tooltip-label">Overall Confidence</span>
        <span class="confidence-tooltip-value" style="color: ${confidence >= 0.90 ? '#3fb950' : (confidence >= 0.70 ? '#e3b341' : '#f85149')}; font-size: 14px;">${Math.round(confidence * 100)}%</span>
      </div>
    `;
  } else {
    // Simple confidence display (no breakdown available)
    tooltipHTML += `
      <div class="confidence-tooltip-row">
        <span class="confidence-tooltip-label">Confidence Score</span>
        <span class="confidence-tooltip-value" style="color: ${confidence >= 0.90 ? '#3fb950' : (confidence >= 0.70 ? '#e3b341' : '#f85149')}">${Math.round(confidence * 100)}%</span>
      </div>
      <div class="confidence-tooltip-bar">
        <div class="confidence-tooltip-bar-fill" style="width: ${confidence * 100}%; background: ${confidence >= 0.90 ? '#3fb950' : (confidence >= 0.70 ? '#e3b341' : '#f85149')}"></div>
      </div>
    `;
  }

  tooltipHTML += `
    <div class="confidence-tooltip-method">
      Matched using: <strong>${methodInfo.name}</strong>
    </div>
  `;

  return tooltipHTML;
}

/**
 * Show tooltip on hover
 */
function attachTooltipListeners() {
  let tooltip = document.querySelector('.confidence-tooltip');
  if (!tooltip) {
    tooltip = document.createElement('div');
    tooltip.className = 'confidence-tooltip';
    document.body.appendChild(tooltip);
  }

  document.addEventListener('mouseover', (e) => {
    const badge = e.target.closest('.confidence-badge');
    if (!badge) {
      tooltip.classList.remove('visible');
      return;
    }

    // Get change data
    const diffCard = badge.closest('.gluon-diff-card');
    if (!diffCard) return;

    const change = overlayState.changes[overlayState.activeChangeIndex];
    if (!change || !change.match_result) return;

    // Update tooltip content
    tooltip.innerHTML = createConfidenceTooltip(change.match_result);

    // Position tooltip
    const rect = badge.getBoundingClientRect();
    tooltip.style.top = `${rect.bottom + 10}px`;
    tooltip.style.left = `${rect.left}px`;
    tooltip.classList.add('visible');
  });

  document.addEventListener('mouseout', (e) => {
    if (!e.target.closest('.confidence-badge') && !e.relatedTarget?.closest('.confidence-tooltip')) {
      tooltip.classList.remove('visible');
    }
  });
}

function showOverlay(changes) {
  if (!overlayState.overlayElement) {
    injectOverlayStyles();
    overlayState.overlayElement = createOverlayHTML();
    document.body.appendChild(overlayState.overlayElement);
    attachEventListeners();
  }

  // Generate unique batch ID for this group of changes
  const batchId = generateUUID();

  // Ensure each change has a unique ID and batch ID
  // Also normalize file path fields (support both camelCase and snake_case)
  changes.forEach(change => {
    if (!change.id) {
      change.id = generateUUID();
    }
    if (!change.batchId) {
      change.batchId = batchId;
    }

    // Normalize file path fields: ensure both filePath (JS convention) and file_path (backend convention) exist
    if (change.file_path && !change.filePath) {
      change.filePath = change.file_path;
    } else if (change.filePath && !change.file_path) {
      change.file_path = change.filePath;
    } else if (!change.filePath && !change.file_path) {
      // If neither exists, this is an error condition, but set defaults to prevent crashes
      change.filePath = 'unknown';
      change.file_path = 'unknown';
      console.warn('Change missing file path:', change);
    }
  });

  overlayState.changes = changes;
  overlayState.selectedChanges = new Set(changes.map((_, idx) => idx));
  overlayState.activeChangeIndex = 0;

  renderSidebar();
  renderDiffView();

  // ETAP 4: Initialize tooltip listeners
  attachTooltipListeners();

  overlayState.overlayElement.classList.remove('hidden');
  overlayState.isVisible = true;

  document.getElementById('total-changes-badge').textContent = `${changes.length} files`;

  // Request real line numbers
  const requests = changes.map(c => ({
      filePath: c.filePath,
      searchContent: c.oldCode
  }));
  chrome.runtime.sendMessage({
      action: 'resolve_change_locations',
      payload: requests
  });
}

function hideOverlay() {
  if (overlayState.overlayElement) {
    overlayState.overlayElement.classList.add('hidden');
    overlayState.isVisible = false;
  }
}

/**
 * Renders the sidebar list
 */
function renderSidebar() {
    const container = document.getElementById('file-list-container');
    const countSpan = document.getElementById('selected-count');
    const applyBtn = document.getElementById('apply-btn');
    
    if (!container) return;

    container.innerHTML = overlayState.changes.map((change, idx) => {
        const isSelected = overlayState.selectedChanges.has(idx);
        const isActive = idx === overlayState.activeChangeIndex;
        const statusClass = `status-${change.status || 'pending'}`;

        // ETAP 4: Add confidence indicator to sidebar
        let confidenceIndicator = '';
        if (change.match_result && typeof change.match_result.confidence === 'number') {
            const { class: levelClass } = getConfidenceLevel(change.match_result.confidence);
            const dotColor = levelClass === 'confidence-high' ? '46, 160, 67' : (levelClass === 'confidence-medium' ? '227, 179, 65' : '248, 81, 73');
            confidenceIndicator = `<span class="confidence-dot" style="margin-left: 6px; box-shadow: 0 0 4px rgba(${dotColor}, 0.6); background: rgb(${dotColor});"></span>`;
        }

        // [DEBUG SYSTEM] Przycisk Debugowania (Black Box)
        // Jest dodawany do KAŻDEGO pliku na liście, ale domyślnie ukryty (pokazuje się po najechaniu myszką).
        // Dla błędów (czerwony status) jest widoczny zawsze.
        
        let isError = change.status === 'error' || (change.match_result && change.match_result.confidence < 0.7);
        let btnColor = isError ? '#f85149' : '#8b949e'; // Czerwony dla błędu, szary dla reszty
        let visibilityClass = isError ? 'always-visible' : '';

        let debugAction = `
            <button class="gluon-debug-btn ${visibilityClass}" 
                    title="Save Debug Snapshot (Zapisz stan błędu)" 
                    data-idx="${idx}" 
                    style="color:${btnColor};">
                🐞
            </button>
        `;

        // Pulse System: Add progress indicator when applying
        let progressIndicator = '';
        if (change.status === 'applying') {
            // Default values if not set
            const progressPercent = change.progressPercent || 0;
            const progressText = change.progressMessage || '⏳ Starting...';
            const progressDetails = change.progressDetails ? `<span class="gluon-progress-details">${escapeHTML(change.progressDetails)}</span>` : '';

            progressIndicator = `
                <div class="gluon-progress-container">
                    <div class="gluon-progress-bar-bg">
                        <div class="gluon-progress-bar-fill" style="width: ${progressPercent}%;"></div>
                    </div>
                    <div class="gluon-progress-text">
                        ${escapeHTML(progressText)} ${progressDetails}
                    </div>
                    <button class="gluon-progress-cancel-btn" data-change-id="${change.id}" title="Cancel this change">⏹️</button>
                </div>
            `;
        }

        return `
        <div class="gluon-file-item ${isActive ? 'active' : ''}" data-index="${idx}">
            <div class="gluon-checkbox ${isSelected ? 'checked' : ''}"
                 data-action="toggle" data-index="${idx}">
                <svg viewBox="0 0 16 16" fill="currentColor"><path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.75.75 0 0 1 1.06-1.06L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0z"></path></svg>
            </div>
            <div class="gluon-file-info">
                <div class="gluon-file-name" title="${change.filePath}">
                    <span class="gluon-status-dot ${statusClass}"></span>
                    ${change.filePath}
                    ${confidenceIndicator}
                </div>
                <div class="gluon-file-meta">${change.format} • ${change.id ? change.id.substr(0,6) : 'diff'}</div>
                ${progressIndicator}
            </div>
            ${debugAction}
        </div>
        `;
    }).join('');

    // Attach listeners for Debug Buttons
    container.querySelectorAll('.gluon-debug-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const idx = parseInt(btn.dataset.idx);
            saveDebugSnapshot(idx, btn);
        });
    });

    // Attach listeners for Cancel Buttons in progress indicators
    container.querySelectorAll('.gluon-progress-cancel-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const changeId = btn.dataset.changeId;
            cancelChange(changeId, btn);
        });
    });

    if (countSpan) countSpan.textContent = overlayState.selectedChanges.size;
    if (applyBtn) {
        // Update main button text based on selection
        const count = overlayState.selectedChanges.size;
        applyBtn.textContent = count > 0 ? `Apply Selected (${count})` : 'Apply Selected';
        applyBtn.disabled = count === 0;
    }
}

/**
 * Renders the main Diff View
 */
function renderDiffView() {
    const container = document.getElementById('diff-viewer-container');
    const change = overlayState.changes[overlayState.activeChangeIndex];
    
    if (!container || !change) return;

    // [GLUON UPDATE] Detect formats
    // LazyStitcher: New format where we only have newCode with markers
    // MarkdownOverwrite: Fallback where we overwrite file but don't have oldCode matched yet
    const isLazy = change.format === 'LazyStitcher';
    const isNewFile = change.format === 'CREATE' || ((!change.oldCode || change.oldCode === '') && !isLazy && change.newCode);
    const startLine = change.lineStart || 1;

    // Helper to render code lines with syntax highlighting for markers
    const renderLines = (text, type, startNum) => {
        if (!text) return '';
        return text.split('\n').map((line, i) => {
            const num = startNum + i;
            // Don't show line numbers for context placeholders or new files if desirable
            const numDisplay = (type === 'ctx' || (type === 'del' && isNewFile)) ? '' : num;
            
            // 1. Escape HTML entities first to prevent rendering issues
            let safeLine = escapeHTML(line) || ' '; 
            
            // 2. Special formatting for Lazy Markers in the 'add' (Proposed) column
            if (isLazy && type === 'add') {
                // Regex for various comment styles of "... existing code ..."
                const markerRegex = /^(\s*)(\/\/|#|<!--|\/\*)\s*\.\.\.\s*existing\s+code/i;
                if (markerRegex.test(line)) {
                    // Wrap in a span for styling (grey, italic)
                    safeLine = `<span style="color: #8b949e; font-style: italic;">${safeLine}</span>`;
                }
            }

            // Determine CSS class for the row background
            const rowClass = type === 'add' ? 'diff-add' : (type === 'del' ? 'diff-del' : 'diff-ctx');
            
            return `
                <div class="diff-row ${rowClass}">
                    <div class="diff-num">${numDisplay}</div>
                    <div class="diff-line">${safeLine}</div>
                </div>`;
        }).join('');
    };

    let leftContent = '';
    let rightContent = '';

    if (isNewFile) {
        // Case: Creating a new file
        leftContent = `<div class="diff-row diff-ctx"><div class="diff-num"> </div><div class="diff-line" style="color: #8b949e; font-style: italic;">// New file (creating)</div></div>`;
        rightContent = renderLines(change.newCode, 'add', 1);
    } else if (isLazy) {
        // Case: Lazy Rewrite (Old code is on disk, we only have the rewrite plan)
        leftContent = `
            <div class="diff-row diff-ctx" style="padding: 20px; display:flex; flex-direction:column; align-items:center; justify-content:center; color: #8b949e; height: 100%;">
                <div style="text-align: center;">
                    <div style="font-size: 24px; margin-bottom: 10px;">⚡️</div>
                    <strong>Lazy Rewrite Mode</strong><br>
                    <span style="font-size: 12px; opacity: 0.8;">Original content is on disk.</span><br>
                    <span style="font-size: 12px; opacity: 0.8;">Gluon will stitch the changes automatically.</span>
                </div>
            </div>`;
        rightContent = renderLines(change.newCode, 'add', startLine);
    } else {
        // Case: Standard Diff (Search/Replace or Unified Diff)
        leftContent = renderLines(change.oldCode, 'del', startLine);
        rightContent = renderLines(change.newCode, 'add', startLine);
    }
    // Logic for Apply/Undo buttons in the header (Split Button)
    let btnHtml = '';

    // Helper to create split button
    const createSplitBtn = (mainId, mainText, mainClass = '', actions = []) => {
        const actionItems = actions.map(a => {
            if (a.divider) return `<div class="gluon-dropdown-divider"></div>`;
            return `<button class="gluon-dropdown-item ${a.danger ? 'danger' : ''}" data-action="${a.id}">${a.label}</button>`;
        }).join('');

        return `
            <div class="gluon-btn-group">
                <button class="gluon-btn-small ${mainClass}" id="${mainId}">${mainText}</button>
                <button class="gluon-dropdown-trigger" id="${mainId}-dropdown">▼</button>
                <div class="gluon-dropdown-menu" id="${mainId}-menu">
                    ${actionItems}
                </div>
            </div>
        `;
    };

    if (change.status === 'success') {
        btnHtml = `
            <button class="gluon-btn-small btn-success-state" disabled>✔ Applied</button>
            ${createSplitBtn('single-undo-btn', '↶ Undo', '', [
                { id: 'smart-undo', label: 'Undo this & Keep others' },
                { id: 'full-undo', label: 'Undo Entire File (Revert)' }
            ])}
        `;
    } else if (change.status === 'undone') {
        btnHtml = `
            <button class="gluon-btn-small" style="background: #444; border-color: #666;" disabled>↶ Undone</button>
            ${createSplitBtn('single-redo-btn', '↷ Redo', '', [
                { id: 'redo', label: 'Redo this change' }
            ])}
        `;
    } else if (change.status === 'applying') {
        btnHtml = `<button class="gluon-btn-small" disabled>Applying...</button>`;
    } else if (change.status === 'undoing') {
        btnHtml = `<button class="gluon-btn-small" disabled>Undoing...</button>`;
    } else if (change.status === 'redoing') {
        btnHtml = `<button class="gluon-btn-small" disabled>Redoing...</button>`;
    } else if (change.status === 'error') {
        btnHtml = createSplitBtn('single-apply-btn', 'Retry Apply', 'status-error', [
             { id: 'apply', label: 'Retry Apply' },
             { divider: true },
             { id: 'skip', label: 'Skip/Discard', danger: true }
        ]);
    } else {
        btnHtml = createSplitBtn('single-apply-btn', '⚡ Apply', '', [
             { id: 'apply', label: 'Apply Change' },
             { divider: true },
             { id: 'skip', label: 'Skip/Discard', danger: true }
        ]);
    }

    // ETAP 4: Generate confidence badge if match_result is available
    const confidenceBadgeHTML = change.match_result ? createConfidenceBadge(change.match_result) : '';
    const methodInfo = change.match_result ? getMatcherMethodInfo(change.match_result.method_used) : null;

    // ETAP 4: Generate warning banner for low confidence matches
    let warningBanner = '';
    if (change.match_result && change.match_result.confidence < 0.70) {
        warningBanner = `
            <div style="background: rgba(248, 81, 73, 0.1); border: 1px solid rgba(248, 81, 73, 0.3); border-radius: 6px; padding: 12px; margin-bottom: 16px; display: flex; align-items: start; gap: 10px;">
                <svg width="20" height="20" viewBox="0 0 16 16" fill="currentColor" style="color: #f85149; flex-shrink: 0; margin-top: 2px;">
                    <path d="M6.457 1.047c.659-1.234 2.427-1.234 3.086 0l6.082 11.378A1.75 1.75 0 0 1 14.082 15H1.918a1.75 1.75 0 0 1-1.543-2.575Zm1.763.707a.25.25 0 0 0-.44 0L1.698 13.132a.25.25 0 0 0 .22.368h12.164a.25.25 0 0 0 .22-.368Zm.53 3.996v2.5a.75.75 0 0 1-1.5 0v-2.5a.75.75 0 0 1 1.5 0ZM9 11a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z"></path>
                </svg>
                <div style="flex: 1;">
                    <div style="color: #f85149; font-weight: 600; font-size: 13px; margin-bottom: 4px;">⚠ Low Confidence Match</div>
                    <div style="color: #c9d1d9; font-size: 12px; line-height: 1.5;">
                        This match has low confidence (${Math.round(change.match_result.confidence * 100)}%). Please review carefully before applying.
                        ${change.match_result.details ? `<br><span style="color: #8b949e; font-size: 11px;">${escapeHTML(change.match_result.details)}</span>` : ''}
                    </div>
                </div>
            </div>
        `;
    } else if (change.match_result && change.match_result.confidence < 0.90) {
        warningBanner = `
            <div style="background: rgba(227, 179, 65, 0.1); border: 1px solid rgba(227, 179, 65, 0.3); border-radius: 6px; padding: 12px; margin-bottom: 16px; display: flex; align-items: start; gap: 10px;">
                <svg width="20" height="20" viewBox="0 0 16 16" fill="currentColor" style="color: #e3b341; flex-shrink: 0; margin-top: 2px;">
                    <path d="M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm8-6.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM6.5 7.75A.75.75 0 0 1 7.25 7h1a.75.75 0 0 1 .75.75v2.75h.25a.75.75 0 0 1 0 1.5h-2a.75.75 0 0 1 0-1.5h.25v-2h-.25a.75.75 0 0 1-.75-.75ZM8 6a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"></path>
                </svg>
                <div style="flex: 1;">
                    <div style="color: #e3b341; font-weight: 600; font-size: 13px; margin-bottom: 4px;">ℹ Medium Confidence Match</div>
                    <div style="color: #c9d1d9; font-size: 12px; line-height: 1.5;">
                        This match has medium confidence (${Math.round(change.match_result.confidence * 100)}%). Review the changes to ensure accuracy.
                    </div>
                </div>
            </div>
        `;
    }

    // Render the full card structure
    container.innerHTML = `
        ${warningBanner}
        <div class="gluon-diff-card">
            <div class="gluon-diff-header">
                <div style="display:flex; align-items:center; gap: 10px;">
                    <span class="gluon-diff-path">${escapeHTML(change.filePath)}</span>
                    <span class="gluon-diff-lines" style="margin-left:10px;">
                        ${change.format}
                        ${startLine > 1 ? '(approx. line ' + startLine + ')' : ''}
                    </span>
                    ${methodInfo ? `<span class="matcher-badge ${methodInfo.class}">${methodInfo.name}</span>` : ''}
                </div>
                <div style="display:flex; align-items:center; gap: 10px;">
                    ${confidenceBadgeHTML}
                    ${btnHtml}
                </div>
            </div>
            <div class="gluon-diff-content">
                <div class="gluon-split-view">
                    <div class="gluon-split-pane">
                        <div class="gluon-pane-header">Original / Context</div>
                        ${leftContent}
                    </div>
                    <div class="gluon-split-pane">
                        <div class="gluon-pane-header">Proposed Change</div>
                        ${rightContent}
                    </div>
                </div>
            </div>
        </div>
    `;
        // Attach event listeners to Apply/Undo/Redo buttons & Dropdowns
        const setupSplitBtn = (baseId, primaryAction) => {
            const btn = document.getElementById(baseId);
            const trigger = document.getElementById(`${baseId}-dropdown`);
            const menu = document.getElementById(`${baseId}-menu`);

            if (btn) btn.addEventListener('click', primaryAction);

            if (trigger && menu) {
                trigger.addEventListener('click', (e) => {
                    e.stopPropagation();
                    // Close others
                    document.querySelectorAll('.gluon-dropdown-menu').forEach(m => {
                        if (m !== menu) m.classList.remove('visible');
                    });
                    menu.classList.toggle('visible');
                });

                menu.addEventListener('click', (e) => {
                    const action = e.target.dataset.action;
                    if (!action) return;

                    menu.classList.remove('visible');
                    const idx = overlayState.activeChangeIndex;

                    if (action === 'smart-undo') handleSmartUndo(idx);
                    else if (action === 'full-undo') undoSingleChange(idx); // Legacy revert
                    else if (action === 'redo') redoSingleChange(idx);
                    else if (action === 'apply') applySingleChange(idx);
                    else if (action === 'skip') {
                        const change = overlayState.changes[idx];
                        if (change) change.status = 'skipped'; // New status
                        renderSidebar();
                        renderDiffView();
                    }
                });
            }
        };

        // Close dropdowns on global click
        document.addEventListener('click', () => {
            document.querySelectorAll('.gluon-dropdown-menu').forEach(m => m.classList.remove('visible'));
        });

        setupSplitBtn('single-apply-btn', () => applySingleChange(overlayState.activeChangeIndex));
        setupSplitBtn('single-undo-btn', () => handleSmartUndo(overlayState.activeChangeIndex)); // Default to Smart Undo
        setupSplitBtn('single-redo-btn', () => redoSingleChange(overlayState.activeChangeIndex));
    }
    /**
     * SMART UNDO: 
     * If multiple changes are applied to the same file, undoing one involves:
     * 1. Reverting the file (Backend undo).
     * 2. Re-applying the *other* changes automatically.
     */
    async function handleSmartUndo(index) {
        const changeToRemove = overlayState.changes[index];
        if (!changeToRemove || changeToRemove.status !== 'success') return;

        const targetPath = changeToRemove.filePath || changeToRemove.file_path;

        // Find subsequent applied changes for this file (to warn user)
        const subsequentChanges = overlayState.changes.filter((c, idx) => 
            idx > index && 
            (c.filePath === targetPath || c.file_path === targetPath) && 
            c.status === 'success'
        );

        if (subsequentChanges.length > 0) {
            const confirmMsg = `There are ${subsequentChanges.length} changes applied AFTER this one.\n` +
                               `Gluon will revert to the state before this change, then automatically attempt to re-apply the later changes.\n\n` + 
                               `Proceed?`;
            if (!confirm(confirmMsg)) return;
        }

        // Backend is the single source of truth. 
        // We just trigger the undo_change command. The backend handles the rollback 
        // and the re-application loop internally to ensure atomicity.
        await undoSingleChange(index);
    }

    /**
     * Applies a single change asynchronously
     */
    async function cancelChange(changeId, btnElement) {
        const change = overlayState.changes.find(c => c.id === changeId);
        if (!change) return;

        btnElement.disabled = true;
        btnElement.innerHTML = '⏳';

        try {
            // Send cancel request to backend
            await chrome.runtime.sendMessage({
                action: 'cancel_change',
                payload: {
                    change_id: changeId
                }
            });

            // Update UI to show cancelled state
            change.status = 'cancelled';
            change.progressMessage = 'Cancelled';
            change.progressPercent = 0;

            renderSidebar();
            renderDiffView();

            console.log(`[Gluon] Change ${changeId} cancelled`);
        } catch (error) {
            console.error('[Gluon Cancel] Failed:', error);
            btnElement.disabled = false;
            btnElement.innerHTML = '⏹️';
        }
    }

    async function saveDebugSnapshot(index, btnElement) {
    const change = overlayState.changes[index];
    if (!change || !change.id) return;

    btnElement.innerHTML = '⏳';
    btnElement.disabled = true;

    try {
        // Grab HTML from diff viewer if active, else just placeholder
        const htmlSnippet = document.getElementById('diff-viewer-container')?.innerHTML || "<div>Snapshot</div>";

        await chrome.runtime.sendMessage({
            action: 'create_debug_snapshot',
            payload: {
                change_id: change.id,
                error_msg: change.error_message || "Manual Debug Report",
                html_snippet: htmlSnippet
            }
        });

        btnElement.innerHTML = '✅';
        btnElement.title = "Snapshot Saved!";
        console.log(`[Gluon Debug] Snapshot saved for ${change.id}`);
    } catch (error) {
        console.error('[Gluon Debug] Failed:', error);
        btnElement.innerHTML = '❌';
        btnElement.disabled = false;
    }
}

async function applySingleChange(index) {
    const change = overlayState.changes[index];
    if (!change || change.status === 'success' || change.status === 'applying') return;

    // 1. Update UI to 'applying'
    change.status = 'applying';
    renderSidebar();
    renderDiffView();

    try {
        // 2. Get selected projects from storage
        const { selectedProjects } = await chrome.storage.local.get('selectedProjects');

        // 3. Send message to background -> desktop
        await chrome.runtime.sendMessage({
            action: 'apply_code_changes',
            payload: {
                changes: [change],
                selectedProjects: selectedProjects || []
            }
        });

        // 4. Optimistic success (Backend sends errors via separate channel if fails)
        change.status = 'success';
        
        // 4. Auto-deselect applied change
        overlayState.selectedChanges.delete(index);

    } catch (error) {
        console.error('Apply failed:', error);
        change.status = 'error';
    }

    // 5. Refresh UI
    renderSidebar();
    // Only re-render diff view if we are still looking at the same change
    if (overlayState.activeChangeIndex === index) {
        renderDiffView();
    }
}

/**
 * Undoes a single change
 */
async function undoSingleChange(index) {
    const change = overlayState.changes[index];
    if (!change || change.status !== 'success') return;

    // Ensure change has an ID
    if (!change.id) {
        console.error('Cannot undo change without ID:', change);
        return;
    }

    // 1. Update UI to 'undoing'
    change.status = 'undoing';
    renderSidebar();
    renderDiffView();

    try {
        // 2. Send undo request to background -> desktop -> VS Code
        // Support both filePath (camelCase) and file_path (snake_case) from backend
        const filePath = change.filePath || change.file_path || 'unknown';

        await chrome.runtime.sendMessage({
            action: 'undo_change',
            payload: {
                changeId: change.id,
                filePath: filePath
            }
        });

        // 3. Update status to 'undone'
        change.status = 'undone';

    } catch (error) {
        console.error('Undo failed:', error);
        change.status = 'success'; // Revert status on error
    }

    // 4. Refresh UI
    renderSidebar();
    if (overlayState.activeChangeIndex === index) {
        renderDiffView();
    }
}

/**
 * Redoes a single change
 */
async function redoSingleChange(index) {
    const change = overlayState.changes[index];
    if (!change || change.status !== 'undone') return;

    // Ensure change has an ID
    if (!change.id) {
        console.error('Cannot redo change without ID:', change);
        return;
    }

    // 1. Update UI to 'redoing'
    change.status = 'redoing';
    renderSidebar();
    renderDiffView();

    try {
        // 2. Send redo request to background -> desktop -> VS Code
        // Support both filePath (camelCase) and file_path (snake_case) from backend
        const filePath = change.filePath || change.file_path || 'unknown';

        await chrome.runtime.sendMessage({
            action: 'redo_change',
            payload: {
                changeId: change.id,
                filePath: filePath
            }
        });

        // 3. Update status to 'success'
        change.status = 'success';

    } catch (error) {
        console.error('Redo failed:', error);
        change.status = 'undone'; // Revert status on error
    }

    // 4. Refresh UI
    renderSidebar();
    if (overlayState.activeChangeIndex === index) {
        renderDiffView();
    }
}

/**
 * Applies all selected changes sequentially (one by one)
 * allowing UI updates in between.
 */
async function applySelectedChanges() {
    const selectedIndices = Array.from(overlayState.selectedChanges).sort((a, b) => a - b);
    if (selectedIndices.length === 0) return;

    const btn = document.getElementById('apply-btn');
    const originalText = btn.textContent;
    btn.disabled = true;

    for (let i = 0; i < selectedIndices.length; i++) {
        const idx = selectedIndices[i];
        btn.textContent = `Applying ${i + 1}/${selectedIndices.length}...`;

        await applySingleChange(idx);

        // Small delay to allow UI breath and backend processing
        await new Promise(resolve => setTimeout(resolve, 100));
    }

    btn.textContent = '✔ Done';
    setTimeout(() => {
        btn.disabled = false;
        renderSidebar(); // Refresh text based on remaining selection
    }, 1000);
}

function updateMainApplyButton() {
    const countSpan = document.getElementById('selected-count');
    const applyBtn = document.getElementById('apply-btn');
    if (countSpan) countSpan.textContent = overlayState.selectedChanges.size;
    if (applyBtn) applyBtn.disabled = overlayState.selectedChanges.size === 0;
}

function attachEventListeners() {
    const el = overlayState.overlayElement;

    // Close buttons
    el.querySelector('#close-overlay').addEventListener('click', hideOverlay);
    
    // Sidebar delegation
    el.querySelector('#file-list-container').addEventListener('click', (e) => {
        const item = e.target.closest('.gluon-file-item');
        if (!item) return;
        
        const idx = parseInt(item.dataset.index);
        
        // Check if checkbox was clicked
        if (e.target.closest('.gluon-checkbox')) {
            if (overlayState.selectedChanges.has(idx)) {
                overlayState.selectedChanges.delete(idx);
            } else {
                overlayState.selectedChanges.add(idx);
            }
            renderSidebar();
            // Don't switch view on checkbox click
            return; 
        }
        
        // Switch active view
        overlayState.activeChangeIndex = idx;
        renderSidebar(); // Update active class
        renderDiffView();
    });

    // Selection buttons
    el.querySelector('#select-all-btn').addEventListener('click', () => {
        overlayState.selectedChanges = new Set(overlayState.changes.map((_, i) => i));
        renderSidebar();
    });
    
    el.querySelector('#deselect-all-btn').addEventListener('click', () => {
        overlayState.selectedChanges.clear();
        renderSidebar();
    });

    // Apply
    const applyBtn = el.querySelector('#apply-btn');
    // Remove old listener if needed (though overlay is usually recreated)
    applyBtn.replaceWith(applyBtn.cloneNode(true));
    el.querySelector('#apply-btn').addEventListener('click', applySelectedChanges);

    // Changes dropdown
    const changesDropdownBtn = el.querySelector('#changes-dropdown-btn');
    const changesDropdownMenu = el.querySelector('#changes-dropdown-menu');
    if (changesDropdownBtn && changesDropdownMenu) {
        changesDropdownBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            const isVisible = changesDropdownMenu.classList.contains('visible');
            changesDropdownMenu.classList.toggle('visible');

            if (!isVisible) {
                // Populate dropdown with changes list
                populateChangesDropdown();

                // Close dropdown when clicking outside
                setTimeout(() => {
                    document.addEventListener('click', closeChangesDropdown);
                }, 0);
            }
        });
    }

    // Undo All button
    const undoAllBtn = el.querySelector('#undo-all-btn');
    if (undoAllBtn) {
        undoAllBtn.addEventListener('click', async () => {
            const appliedChanges = overlayState.changes.filter(c => c.status === 'success');
            if (appliedChanges.length === 0) {
                alert('No changes to undo. All changes are either pending or already undone.');
                return;
            }

            if (confirm(`Undo all ${appliedChanges.length} applied changes?`)) {
                undoAllBtn.disabled = true;
                undoAllBtn.innerHTML = '<span>Undoing...</span>';

                for (const change of appliedChanges) {
                    if (change.id) {
                        try {
                            await new Promise((resolve) => {
                                chrome.runtime.sendMessage({
                                    action: 'undo_change',
                                    change_id: change.id,
                                    file_path: change.filePath
                                }, () => setTimeout(resolve, 100));
                            });
                        } catch (error) {
                            console.error('Failed to undo change:', change.id, error);
                        }
                    }
                }

                undoAllBtn.disabled = false;
                undoAllBtn.innerHTML = '<svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M3.5 2a.5.5 0 00-.5.5v5a.5.5 0 001 0V3.707l8.146 8.147a.5.5 0 00.708-.708L4.707 3H9.5a.5.5 0 000-1h-6z"/></svg><span>Undo All</span>';
            }
        });
    }

    // Report Bug button
    const reportBugBtn = el.querySelector('#report-bug-btn');
    if (reportBugBtn) {
        reportBugBtn.addEventListener('click', () => {
            // Collect diagnostic information
            const diagnosticInfo = {
                totalChanges: overlayState.changes.length,
                selectedChanges: overlayState.selectedChanges.size,
                appliedChanges: overlayState.changes.filter(c => c.status === 'success').length,
                failedChanges: overlayState.changes.filter(c => c.status === 'error').length,
                changes: overlayState.changes.map(c => ({
                    filePath: c.filePath,
                    status: c.status,
                    operation: c.operation,
                    confidence: c.match_result?.confidence,
                    error: c.error_message
                }))
            };

            // Copy to clipboard
            const reportText = `## Gluon Apply System Bug Report

**Issue Description:**
[Please describe the issue you encountered]

**Diagnostic Information:**
\`\`\`json
${JSON.stringify(diagnosticInfo, null, 2)}
\`\`\`

**Steps to Reproduce:**
1. [Step 1]
2. [Step 2]
3. [Step 3]

**Expected Behavior:**
[What you expected to happen]

**Actual Behavior:**
[What actually happened]

**Browser:** ${navigator.userAgent}
**Extension Version:** [Check chrome://extensions]
`;

            navigator.clipboard.writeText(reportText).then(() => {
                alert('Bug report template copied to clipboard!\n\nPlease paste it into a GitHub issue or email it to the development team.');
            }).catch(() => {
                // Fallback: show in a modal
                const reportWindow = window.open('', 'Bug Report', 'width=600,height=800');
                reportWindow.document.write(`<pre>${reportText}</pre>`);
            });
        });
    }

    // Escape Key
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && overlayState.isVisible) hideOverlay();
    });
}

// Helper function to populate changes dropdown
function populateChangesDropdown() {
    const menu = document.querySelector('#changes-dropdown-menu');
    if (!menu) return;

    menu.innerHTML = '';

    if (overlayState.changes.length === 0) {
        menu.innerHTML = '<div style="padding: 16px; text-align: center; color: #8b949e;">No changes to display</div>';
        return;
    }

    overlayState.changes.forEach((change, idx) => {
        const item = document.createElement('div');
        item.className = 'gluon-changes-dropdown-item';
        item.dataset.index = idx;

        const statusIcon = document.createElement('div');
        statusIcon.className = 'status-icon';
        if (change.status === 'success') statusIcon.style.background = '#238636';
        else if (change.status === 'error') statusIcon.style.background = '#da3633';
        else if (change.status === 'applying') statusIcon.style.background = '#e3b341';
        else statusIcon.style.background = '#8b949e';

        const fileName = document.createElement('div');
        fileName.className = 'file-name';
        fileName.textContent = change.filePath || 'Unknown file';

        item.appendChild(statusIcon);
        item.appendChild(fileName);

        item.addEventListener('click', () => {
            overlayState.activeChangeIndex = idx;
            renderSidebar();
            renderDiffView();
            menu.classList.remove('visible');
        });

        menu.appendChild(item);
    });
}

// Helper function to close changes dropdown
function closeChangesDropdown(e) {
    const menu = document.querySelector('#changes-dropdown-menu');
    const btn = document.querySelector('#changes-dropdown-btn');

    if (menu && !menu.contains(e.target) && !btn.contains(e.target)) {
        menu.classList.remove('visible');
        document.removeEventListener('click', closeChangesDropdown);
    }
}

function escapeHTML(str) {
    if (!str) return '';
    return str.replace(/&/g, '&amp;')
              .replace(/</g, '&lt;')
              .replace(/>/g, '&gt;')
              .replace(/"/g, '&quot;')
              .replace(/'/g, '&#039;');
}
chrome.runtime.onMessage.addListener((message) => {
    // [GLUON SPY] Śledzenie wszystkich wiadomości wchodzących do nakładki
    if (message.type !== 'apply_progress_update') { // Ignoruj spam paska postępu
        console.log('%c[GLUON SPY] Incoming Message:', 'background: #222; color: #bada55', message);
    }

    // [FIX] Ensure styles are loaded for Light UI operations
    if (message.type === 'apply_progress_update') {
        injectOverlayStyles();
    }

    if (message.type === 'change_locations_resolved' && overlayState.isVisible) {
        const locations = message.data;
        let updated = false;

        locations.forEach(loc => {
            const change = overlayState.changes.find(c =>
                c.filePath === loc.filePath || c.filePath.endsWith(loc.filePath)
            );

            if (change && loc.lineStart) {
                change.lineStart = loc.lineStart;
                updated = true;
            }
        });

        if (updated) {
            renderDiffView();
        }
    }
    // Pulse System: Handle real-time progress updates
    if (message.type === 'apply_progress_update') {
        console.log('[DIAGNOSTIC B] Raw message received in Overlay:', message);
        console.log('[DIAGNOSTIC B] message.data content:', message.data);
        console.log('[DIAGNOSTIC B] message.data.progress:', message.data?.progress);
        console.log('[DIAGNOSTIC B] message.data.step:', message.data?.step);

        let progressData = message.data;

        // FIX: Backend sends JSON string, parse it if necessary
        if (typeof progressData === 'string') {
            try {
                progressData = JSON.parse(progressData);
            } catch (e) {
                console.error('[Gluon Overlay] Failed to parse progress JSON:', e);
                return;
            }
        }

        // [FIX] Normalize ALL fields (handle snake_case vs camelCase)
        // Ensure consistent access regardless of backend serialization behavior
        if (progressData.request_id && !progressData.requestId) progressData.requestId = progressData.request_id;
        if (progressData.change_id && !progressData.changeId) progressData.changeId = progressData.change_id;
        if (progressData.file_path && !progressData.filePath) progressData.filePath = progressData.file_path;
        
        const incomingRequestId = progressData.requestId;
        const incomingChangeId = progressData.changeId;

        // Track changes by request_id for batch display
        // Allow missing requestId if we can bind contextually
        if (incomingChangeId) {
            
            // Debug DOM state
            const allContainers = document.querySelectorAll('.gluon-overlay-container');

            // [FIX] Contextual Binding for missing RequestID
            // If backend didn't send requestId, try to find the active container
            let effectiveRequestId = incomingRequestId;
            if (!effectiveRequestId && allContainers.length > 0) {
                // Find the container that is currently "Processing"
                const activeContainer = Array.from(allContainers).find(c => 
                    c.querySelector('.gluon-status-text.active') || 
                    c.querySelector('.gluon-apply-btn.processing')
                );
                if (activeContainer) {
                    effectiveRequestId = activeContainer.dataset.requestId;
                }
            }

            // If we still don't have an ID, use a fallback
            const storageKey = effectiveRequestId || 'unknown_batch';

            if (!lightUIState.changesByRequest[storageKey]) {
                lightUIState.changesByRequest[storageKey] = [];
            }
            // Add or update change in the batch
            const existingIndex = lightUIState.changesByRequest[storageKey].findIndex(c => c.id === incomingChangeId);

            // Extract line number if available in details
            const extractedLine = progressData.details ? progressData.details.match(/Line (\d+)/)?.[1] : null;

            let changeData;

            if (existingIndex >= 0) {
                // Merge with existing data to preserve line_start and file_path if missing in this update
                const existing = lightUIState.changesByRequest[storageKey][existingIndex];
                changeData = {
                    id: incomingChangeId,
                    line_start: extractedLine || existing.line_start || '?',
                    file_path: progressData.filePath || existing.file_path,
                    type: progressData.step === 'success' ? 'applied' : 'change',
                    status: progressData.step
                };
                lightUIState.changesByRequest[storageKey][existingIndex] = changeData;
            } else {
                // Create new entry
                changeData = {
                    id: incomingChangeId,
                    line_start: extractedLine || '?',
                    file_path: progressData.filePath,
                    type: progressData.step === 'success' ? 'applied' : 'change',
                    status: progressData.step
                };
                lightUIState.changesByRequest[storageKey].push(changeData);
            }
        }

        // 1. Handle Full Overlay Logic (Sidebar)
        if (overlayState.isVisible) {
            const change = overlayState.changes.find(c => c.id === progressData.changeId);
            if (change) {
                change.progressStep = progressData.step;
                change.progressMessage = progressData.message;
                change.progressPercent = progressData.progress;
                change.progressDetails = progressData.details;

                // Store file_path from backend if provided (needed for undo/redo)
                if (progressData.filePath) {
                    change.file_path = progressData.filePath;
                    if (!change.filePath) {
                        change.filePath = progressData.filePath;
                    }
                }

                if (progressData.step === 'success') change.status = 'success';
                else if (progressData.step === 'failed') change.status = 'error';
                else change.status = 'applying';

                renderSidebar();
                renderDiffView();
            }
        }

        // 2. Handle Lightweight UI (Bottom Bar) - ROBUST IMPLEMENTATION
        // This should ALWAYS run, regardless of sidebar visibility
        try {
                const containers = Array.from(document.querySelectorAll('.gluon-overlay-container'));
                console.log('[DIAGNOSTIC C] Found containers:', containers.length);
                console.log('[DIAGNOSTIC C] incomingRequestId:', incomingRequestId);
                console.log('[DIAGNOSTIC C] incomingChangeId:', incomingChangeId);

                // [FIX] Visual heartbeat - show that messages ARE being received
                if (containers.length === 0 && document.body) {
                    console.log('[DIAGNOSTIC C] No containers found - messages arriving but UI not initialized!');
                    // Create a temporary debug indicator
                    let debugIndicator = document.getElementById('gluon-progress-debug');
                    if (!debugIndicator) {
                        debugIndicator = document.createElement('div');
                        debugIndicator.id = 'gluon-progress-debug';
                        debugIndicator.style.cssText = 'position:fixed;bottom:10px;right:10px;background:#10b981;color:white;padding:8px 12px;border-radius:6px;font-size:12px;z-index:999999;font-family:monospace;';
                        document.body.appendChild(debugIndicator);
                    }
                    debugIndicator.textContent = `📡 ${progressData.message || 'Processing...'} (${progressData.progress || 0}%)`;
                    // Auto-remove after success
                    if (progressData.step === 'success' || progressData.step === 'failed') {
                        setTimeout(() => debugIndicator.remove(), 3000);
                    }
                    return; // Skip further processing if no containers
                }

                let targetContainer = null;

                // [FIX] Use normalized variables & auto-binding
                // If backend didn't send requestId, try to find the active container
                let effectiveRequestId = incomingRequestId;
                if (!effectiveRequestId && containers.length > 0) {
                    const activeContainer = containers.find(c =>
                        c.querySelector('.gluon-status-text.active') ||
                        c.querySelector('.gluon-apply-btn.processing')
                    );
                    console.log('[DIAGNOSTIC C] activeContainer from fallback:', !!activeContainer);
                    if (activeContainer) {
                        effectiveRequestId = activeContainer.dataset.requestId;
                    }
                }

                const currentRequestId = effectiveRequestId;
                console.log('[DIAGNOSTIC C] currentRequestId after normalization:', currentRequestId);

                // Strategy A: Precise ID matching
                if (currentRequestId) {
                    targetContainer = containers.find(c => c.dataset.requestId === currentRequestId);
                    console.log('[DIAGNOSTIC C] Strategy A - targetContainer found:', !!targetContainer);
                }

                // Strategy B: Fallback Binding (Only for initial queue state)
                if (!targetContainer && progressData.step === 'queued') {
                    targetContainer = containers.find(c => !c.dataset.requestId && c.querySelector('.gluon-apply-btn:disabled'));
                    console.log('[DIAGNOSTIC C] Strategy B - targetContainer found:', !!targetContainer);
                    if (targetContainer && currentRequestId) {
                        targetContainer.dataset.requestId = currentRequestId;
                    }
                }

                // Strategy C: Last Resort (Promiscuous Mode)
                if (!targetContainer) {
                    const candidate = containers.find(c => c.querySelector('.gluon-apply-btn.processing'));
                    console.log('[DIAGNOSTIC C] Strategy C - candidate found:', !!candidate);
                    if (candidate) {
                        targetContainer = candidate;
                        if (!targetContainer.dataset.requestId && currentRequestId) {
                            targetContainer.dataset.requestId = currentRequestId;
                        }
                    }
                }

                console.log('[DIAGNOSTIC C] FINAL targetContainer:', !!targetContainer);

                if (targetContainer) {
                    console.log('[DIAGNOSTIC C] targetContainer found:', targetContainer.dataset.requestId);
                    console.log('[DIAGNOSTIC C] progressData.step:', progressData.step);
                    console.log('[DIAGNOSTIC C] progressData.progress:', progressData.progress);
                    console.log('[DIAGNOSTIC C] progressData.message:', progressData.message);

                    // Store both filePath and changeId for undo functionality
                    if (progressData.filePath) targetContainer.dataset.filePath = progressData.filePath;
                    if (incomingChangeId) targetContainer.dataset.changeId = incomingChangeId;

                    // --- UPDATE PROGRESS ---
                    if (progressData.step !== 'success' && progressData.step !== 'failed' && progressData.step !== 'error') {
                        const barFill = targetContainer.querySelector('.gluon-progress-fill');
                        const statusText = targetContainer.querySelector('.gluon-status-text');

                        console.log('[DIAGNOSTIC C] Updating progress bar. barFill:', !!barFill, 'statusText:', !!statusText);

                        if (barFill) {
                            const progressValue = progressData.progress || 0;
                            barFill.style.width = `${progressValue}%`;
                            console.log('[DIAGNOSTIC C] Progress bar updated to:', progressValue + '%');
                        }
                        if (statusText) {
                            statusText.innerHTML = `<span class="gluon-step-icon">⚡</span> ${progressData.message}`;
                            statusText.className = 'gluon-status-text active';
                            console.log('[DIAGNOSTIC C] Status text updated to:', progressData.message);
                        }
                    } else {
                        console.log('[DIAGNOSTIC C] Skipping progress update - step is:', progressData.step);
                    }
                    // --- SUCCESS STATE: FORCE UI REBUILD ---
                    if (progressData.step === 'success') {
                        injectOverlayStyles();

                        // [FIX] Robust File Path Resolution
                        // Try to find the file path from 1) Current Event, 2) Container Dataset, 3) Batch State
                        let resolvedFilePath = progressData.filePath;

                        if (!resolvedFilePath || resolvedFilePath === 'unknown') {
                            resolvedFilePath = targetContainer.dataset.filePath;
                        }

                        if ((!resolvedFilePath || resolvedFilePath === 'unknown') && currentRequestId) {
                            const batchItem = lightUIState.changesByRequest[currentRequestId]?.find(c => c.id === incomingChangeId);
                            if (batchItem && batchItem.file_path) {
                                resolvedFilePath = batchItem.file_path;
                            }
                        }

                        resolvedFilePath = resolvedFilePath || 'unknown';

                        // Persist resolved path back to dataset for future actions
                        if (resolvedFilePath !== 'unknown') {
                            targetContainer.dataset.filePath = resolvedFilePath;
                        }

                        // 1. Wipe existing controls
                        const oldPanel = targetContainer.querySelector('.gluon-status-panel');
                        const oldBtn = targetContainer.querySelector('.gluon-apply-btn');
                        const oldStop = targetContainer.querySelector('.gluon-stop-btn');
                        const oldTools = targetContainer.querySelector('.gluon-tools-container');
                        
                        if (oldPanel) oldPanel.remove();
                        if (oldBtn) oldBtn.remove();
                        if (oldStop) oldStop.remove();
                        if (oldTools) oldTools.remove();

                        // 2. Create Fresh Tools Container
                        const toolsContainer = document.createElement('div');
                        toolsContainer.className = 'gluon-tools-container';
                        toolsContainer.style.cssText = "display: flex; gap: 6px; align-items: center; margin-left: auto;";
                        
                        // 3. Status Badge (Batch Aware)
                        const batchKey = currentRequestId || 'unknown_batch';
                        const changesBatch = lightUIState.changesByRequest[batchKey] || 
                            [{ id: incomingChangeId || 'unknown', status: 'success', line_start: '?', type: 'applied' }];
                            
                        const appliedCount = changesBatch.filter(c => c.status === 'success').length;
                        const statusBadge = document.createElement('div');
                        statusBadge.className = 'gluon-status-compact success';
                        statusBadge.innerHTML = `<span style="font-size:14px; margin-right:4px">✅</span> Applied (${appliedCount})`;
                        toolsContainer.appendChild(statusBadge);

                        // 4. Undo Button (Batch Undo)
                        const undoBtn = document.createElement('button');
                        undoBtn.className = 'gluon-icon-btn danger';
                        undoBtn.title = "Undo All Changes in Batch";
                        undoBtn.innerHTML = '↶';
                        undoBtn.onclick = async (e) => {
                            e.stopPropagation();

                            // Filter successful changes to undo
                            const changesToUndo = changesBatch
                                .filter(c => c.status === 'success' || c.type === 'applied')
                                .reverse(); // LIFO order

                            if (changesToUndo.length === 0) {
                                // Fallback for single change if batch tracking failed
                                changesToUndo.push({ id: incomingChangeId, file_path: resolvedFilePath });
                            }

                            if(confirm(`Undo ${changesToUndo.length} changes from this batch?`)) {
                                undoBtn.disabled = true;
                                undoBtn.innerHTML = '⏳';

                                for (const change of changesToUndo) {
                                    await new Promise(resolve => {
                                        chrome.runtime.sendMessage({
                                            action: 'undo_change',
                                            payload: { 
                                                changeId: change.id, 
                                                filePath: change.file_path || resolvedFilePath 
                                            }
                                        }, () => setTimeout(resolve, 100)); // Small delay between undos
                                    });
                                }

                                undoBtn.disabled = false;
                                undoBtn.innerHTML = '↶';
                            }
                        };
                        toolsContainer.appendChild(undoBtn);

                        // 5. Menu Toggle
                        const menuBtn = document.createElement('button');
                        menuBtn.className = 'gluon-icon-btn';
                        menuBtn.innerHTML = '☰';
                        // Dropdown List
                        const dropdown = document.createElement('div');
                        dropdown.className = 'gluon-compact-dropdown';

                        // Generate Rows with precise Debug actions
                        let rowsHtml = '';
                        changesBatch.forEach((ch, i) => {
                            rowsHtml += `
                            <tr>
                                <td class="gluon-col-status">✔</td>
                                <td class="gluon-col-id">#${i+1}</td>
                                <td class="gluon-col-line">Line ${ch.line_start || '?'}</td>
                                <td class="gluon-col-action" style="display:flex; justify-content:flex-end; gap:6px; align-items:center;">
                                    <button class="gluon-item-action debug-action" data-change-id="${ch.id}" title="Save Snapshot for this change" style="min-width:28px; padding:5px; color:#f85149; border-color:rgba(248,81,73,0.3);">🐞</button>
                                    <button class="gluon-item-action undo-action" data-change-id="${ch.id}" data-file-path="${ch.file_path || resolvedFilePath}">Undo</button>
                                </td>
                            </tr>`;
                        });

                        dropdown.innerHTML = `
                            <div>BATCH DETAILS</div>
                            <div style="max-height: 200px; overflow-y: auto;">
                                <table class="gluon-batch-table">
                                    <tbody>${rowsHtml}</tbody>
                                </table>
                            </div>
                        `;

                        // Add event delegation for buttons (CSP-compliant)
                        dropdown.addEventListener('click', (e) => {
                            e.stopPropagation();

                            // 1. Handle Debug Action (Specific Change ID)
                            if (e.target.classList.contains('debug-action')) {
                                const btn = e.target;
                                const changeId = btn.getAttribute('data-change-id');
                                btn.innerHTML = '⏳';

                                console.log('[Gluon Debug] Creating snapshot for specific change:', changeId);
                                chrome.runtime.sendMessage({
                                    action: 'create_debug_snapshot',
                                    payload: {
                                        change_id: changeId,
                                        error_msg: `Manual Report from Batch Table (Change #${changeId.substring(0,6)})`,
                                        html_snippet: document.documentElement.outerHTML.substring(0, 100000)
                                    }
                                });

                                setTimeout(() => { btn.innerHTML = '✅'; }, 1000);
                                setTimeout(() => { btn.innerHTML = '🐞'; }, 3000);
                                return;
                            }

                            // 2. Handle Undo Action
                            if (e.target.classList.contains('undo-action')) {
                                const changeId = e.target.getAttribute('data-change-id');
                                const filePath = e.target.getAttribute('data-file-path');
                                chrome.runtime.sendMessage({
                                    action: 'undo_change',
                                    payload: { changeId: changeId, filePath: filePath }
                                });
                            }
                        });
                        
                        // Toggle Logic
                        menuBtn.onclick = (e) => {
                            e.stopPropagation();
                            const isVisible = dropdown.classList.contains('visible');
                            document.querySelectorAll('.gluon-compact-dropdown.visible').forEach(d => d.classList.remove('visible'));
                            
                            if (!isVisible) {
                                dropdown.classList.add('visible');
                                menuBtn.classList.add('active');
                                
                                const closeFn = (evt) => {
                                    if (!dropdown.contains(evt.target) && !menuBtn.contains(evt.target)) {
                                        dropdown.classList.remove('visible');
                                        menuBtn.classList.remove('active');
                                        document.removeEventListener('click', closeFn);
                                    }
                                };
                                setTimeout(() => document.addEventListener('click', closeFn), 10);
                            }
                        };

                        toolsContainer.appendChild(menuBtn);
                        toolsContainer.appendChild(dropdown);
                        
                        // 6. Append to Main Container
                        targetContainer.appendChild(toolsContainer);
                    }

                    // --- ERROR STATE ---
                    if (progressData.step === 'failed' || progressData.step === 'error') {
                        const statusText = targetContainer.querySelector('.gluon-status-text');
                        const actionBtn = targetContainer.querySelector('.gluon-apply-btn');

                        if (statusText) {
                            statusText.textContent = `Error: ${progressData.message}`;
                            statusText.className = 'gluon-status-text error';
                        }
                        if (actionBtn) {
                            actionBtn.disabled = false;
                            actionBtn.textContent = 'Retry';
                            actionBtn.classList.remove('processing');
                            actionBtn.classList.add('error');
                        }
                    }

                    // 3. Inject Debug Button (Biedronka) if missing - ONLY for this container
                    if (!targetContainer.querySelector('.gluon-debug-btn-light')) {
                 const debugBtn = document.createElement('button');
                 debugBtn.className = 'gluon-debug-btn-light';
                 debugBtn.innerHTML = '🐞';
                 debugBtn.title = 'Zgłoś błąd / Zapisz snapshot';
                 debugBtn.style.cssText = 'background:rgba(255,0,0,0.1) !important; border:1px solid #f85149 !important; border-radius:4px; cursor:pointer; font-size:16px; color:#f85149 !important; margin-left:8px; padding:2px 8px; pointer-events:auto;';
                 
                 debugBtn.onclick = (e) => {
                     e.preventDefault();
                     e.stopPropagation();
                     console.log('[Gluon Debug] Manual snapshot requested via Light UI');
                     debugBtn.innerHTML = '⏳';
                     chrome.runtime.sendMessage({
                        action: 'create_debug_snapshot',
                        payload: {
                            change_id: progressData.changeId || 'manual-snapshot-light',
                            error_msg: `Manual Report from Light UI: ${progressData.message}`,
                            html_snippet: document.documentElement.outerHTML.substring(0, 100000)
                        }
                    });
                    setTimeout(() => { debugBtn.innerHTML = '✅'; }, 1500);
                    setTimeout(() => { debugBtn.innerHTML = '🐞'; }, 4000);
                 };
                 
                 targetContainer.appendChild(debugBtn);
                    }
                }
            } catch (err) {
                console.error('[Gluon Overlay] Critical UI Error:', err);
            }
    }
    // Handle change status updates (undo/redo synchronization)
    if (message.type === 'change_status_update') {
        const statusData = message.data;
        // Normalize ID (handle both camelCase and snake_case)
        const targetId = statusData.changeId || statusData.change_id;

        console.log('[Status Update] 🟢 UI received update for:', targetId, 'Status:', statusData.status, 'Payload:', statusData);

        // 1. Update Full Overlay State
        if (overlayState && overlayState.changes) {
            const change = overlayState.changes.find(c => c.id === targetId);
            if (change) {
                change.status = statusData.status;
                if (overlayState.isVisible) {
                    renderSidebar();
                    if (overlayState.changes[overlayState.activeChangeIndex]?.id === targetId) {
                        renderDiffView();
                    }
                }
            }
        }

        // 2. Update Light UI (Bottom Bar) - PRECISE MATCHING
        if (!targetId) {
            console.error('[Status Update] ❌ Missing change ID in update payload:', statusData);
            return;
        }

        // Try to find container by change-id first, then fall back to searching all containers
        let targetContainer = document.querySelector(`.gluon-overlay-container[data-change-id="${targetId}"]`);

        // Fallback: search through all containers for matching changeId in their batch data
        if (!targetContainer) {
            console.log('[Status Update] Container not found by data-change-id, searching batches...');
            const allContainers = document.querySelectorAll('.gluon-overlay-container');
            for (const container of allContainers) {
                const requestId = container.dataset.requestId;
                if (requestId && lightUIState.changesByRequest[requestId]) {
                    const changeInBatch = lightUIState.changesByRequest[requestId].find(c => c.id === targetId);
                    if (changeInBatch) {
                        targetContainer = container;
                        // Store changeId and filePath for future lookups
                        container.dataset.changeId = targetId;
                        if (changeInBatch.file_path && !container.dataset.filePath) {
                            container.dataset.filePath = changeInBatch.file_path;
                        }
                        console.log('[Status Update] Found container via batch lookup, requestId:', requestId);
                        break;
                    }
                }
            }
        }

        if (targetContainer) {
            console.log('[Status Update] ✅ Found DOM container for update.');

            // Ensure filePath is stored (recover from batch data if missing)
            let resolvedFilePath = targetContainer.dataset.filePath;
            if (!resolvedFilePath) {
                const requestId = targetContainer.dataset.requestId;
                if (requestId && lightUIState.changesByRequest[requestId]) {
                    const changeInBatch = lightUIState.changesByRequest[requestId].find(c => c.id === targetId);
                    if (changeInBatch && changeInBatch.file_path) {
                        resolvedFilePath = changeInBatch.file_path;
                        targetContainer.dataset.filePath = resolvedFilePath;
                        console.log('[Status Update] Recovered filePath from batch:', resolvedFilePath);
                    }
                }
            }

            // Check if this is a BATCH container (has dropdown with table rows)
            const dropdown = targetContainer.querySelector('.gluon-compact-dropdown');
            const batchTable = dropdown?.querySelector('.gluon-batch-table');
            if (batchTable) {
                // This is a BATCH - find the specific change row and update it
                console.log('[Status Update] Updating individual change in batch:', targetId);

                // Find the button specifically by data attribute to be precise
                // This selector finds the button regardless of its current class (undo/redo/pending)
                const actionBtn = batchTable.querySelector(`button[data-change-id="${targetId}"].gluon-item-action`);

                if (actionBtn) {
                    const filePath = actionBtn.getAttribute('data-file-path') || resolvedFilePath || 'unknown';

                    // Remove old event listeners by cloning
                    const newBtn = actionBtn.cloneNode(true);
                    actionBtn.parentNode.replaceChild(newBtn, actionBtn);

                    // Update Status Icon in the same row
                    const row = newBtn.closest('tr');
                    const statusCell = row ? row.querySelector('.gluon-col-status') : null;

                    if (statusData.status === 'undone') {
                        console.log('[Status Update] UI -> Redo State');
                        newBtn.textContent = 'Redo';
                        newBtn.classList.remove('undo-action', 'pending-action');
                        newBtn.classList.add('redo-action');
                        if (statusCell) statusCell.textContent = '↩';

                        newBtn.onclick = (e) => {
                            e.stopPropagation();
                            newBtn.textContent = '⏳';
                            chrome.runtime.sendMessage({
                                action: 'redo_change',
                                payload: { changeId: targetId, filePath: filePath }
                            });
                        };
                    }
                    else if (statusData.status === 'success') {
                        console.log('[Status Update] UI -> Undo State');
                        newBtn.textContent = 'Undo';
                        newBtn.classList.remove('redo-action', 'pending-action');
                        newBtn.classList.add('undo-action');
                        if (statusCell) statusCell.textContent = '✔';

                        newBtn.onclick = (e) => {
                            e.stopPropagation();
                            newBtn.textContent = '⏳';
                            chrome.runtime.sendMessage({
                                action: 'undo_change',
                                payload: { changeId: targetId, filePath: filePath }
                            });
                        };
                    }
                    else if (statusData.status === 'applying') {
                        console.log('[Status Update] UI -> Applying State');
                        newBtn.textContent = '⏳';
                        newBtn.disabled = true;
                        if (statusCell) statusCell.textContent = '...';
                    }
                    else if (statusData.status === 'error') {
                        console.log('[Status Update] UI -> Error State');
                        newBtn.textContent = 'Retry';
                        newBtn.classList.remove('undo-action', 'redo-action');
                        newBtn.classList.add('pending-action'); // Use pending style for retry
                        newBtn.disabled = false;
                        if (statusCell) statusCell.textContent = '❌';

                        newBtn.onclick = (e) => {
                            e.stopPropagation();
                            newBtn.textContent = '⏳';
                            chrome.runtime.sendMessage({
                                action: 'apply_change', // Retry application
                                payload: { changeId: targetId, filePath: filePath }
                            });
                        };
                    }
                } else {
                    console.warn('[Status Update] Button not found for ID:', targetId);
                }
            } else {
                // This is a SINGLE change container - use original logic
                const actionBtn = targetContainer.querySelector('.gluon-apply-btn');
                const statusText = targetContainer.querySelector('.gluon-status-text');

                if (actionBtn && statusText) {
                    // Unlock button
                    actionBtn.disabled = false;
                    actionBtn.classList.remove('processing'); // Ensure spinner stops

                    if (statusData.status === 'undone') {
                        // Update UI to "Undone" state
                        statusText.innerHTML = '<span class="gluon-step-icon">↩</span> Undone';
                        statusText.className = 'gluon-status-text';
                        statusText.style.color = '#8b949e';

                        actionBtn.innerHTML = '<span>↷ Redo</span>';
                        actionBtn.style.background = '#238636';
                        actionBtn.style.borderColor = 'rgba(255,255,255,0.1)';
                        actionBtn.title = "Redo this change";

                        // Re-bind click for Redo
                        actionBtn.onclick = (e) => {
                            e.stopPropagation();
                            e.preventDefault();
                            actionBtn.textContent = 'Redoing...';
                            actionBtn.disabled = true;

                            const filePath = resolvedFilePath || 'unknown';
                            console.log('[Gluon UI] Requesting REDO for', targetId);

                            chrome.runtime.sendMessage({
                                action: 'redo_change',
                                payload: { changeId: targetId, filePath: filePath }
                            });
                        };
                    }
                    else if (statusData.status === 'success') {
                        // Update UI to "Applied" state (Undo available)
                        statusText.innerHTML = '<span class="gluon-step-icon">⚡</span> Applied';
                        statusText.className = 'gluon-status-text success';
                        statusText.removeAttribute('style');

                        actionBtn.innerHTML = '<span>↶ Undo</span>';
                        actionBtn.style.background = '#21262d';
                        actionBtn.style.borderColor = '#8b949e';
                        actionBtn.title = "Undo this change";

                        // Re-bind click for Undo
                        actionBtn.onclick = (e) => {
                            e.stopPropagation();
                            e.preventDefault();
                            actionBtn.textContent = 'Undoing...';
                            actionBtn.disabled = true;

                            const filePath = resolvedFilePath || 'unknown';
                            console.log('[Gluon UI] Requesting UNDO for', targetId);

                            chrome.runtime.sendMessage({
                                action: 'undo_change',
                                payload: { changeId: targetId, filePath: filePath }
                            });
                        };
                    }
                } else {
                    console.warn('[Status Update] Container found but missing inner elements.');
                }
            }
        } else {
            console.warn('[Status Update] ⚠️ Container NOT found for ID:', targetId);
        }
    }
});

// Export
window.GluonApplySystemOverlay = {
  show: showOverlay,
  hide: hideOverlay
};

logger.log('Apply System Overlay (GitHub Style) loaded.');