console.log('[Gluon Detector] Manual Mode Loaded (v4.0).');

const SCAN_INTERVAL_MS = 1000;
let processingTimer = null;

/**
 * Prosty algorytm Diff (implementacja O(ND) uproszczona do JavaScript)
 * Generuje zunifikowaną listę zmian: Context, Added, Removed.
 */
function computeUnifiedDiff(oldText, newText) {
    const oldLines = oldText.split('\n').filter(l => l !== '');
    const newLines = newText.split('\n').filter(l => l !== '');
    
    // Prosta macierz LCS (Longest Common Subsequence) dla linii
    const matrix = Array(oldLines.length + 1).fill(null).map(() => Array(newLines.length + 1).fill(0));

    for (let i = 1; i <= oldLines.length; i++) {
        for (let j = 1; j <= newLines.length; j++) {
            if (oldLines[i - 1].trim() === newLines[j - 1].trim()) {
                matrix[i][j] = matrix[i - 1][j - 1] + 1;
            } else {
                matrix[i][j] = Math.max(matrix[i - 1][j], matrix[i][j - 1]);
            }
        }
    }

    // Backtracking w celu zbudowania diffa
    const diff = [];
    let i = oldLines.length;
    let j = newLines.length;

    while (i > 0 || j > 0) {
        if (i > 0 && j > 0 && oldLines[i - 1].trim() === newLines[j - 1].trim()) {
            diff.unshift({ type: 'context', content: oldLines[i - 1] });
            i--; j--;
        } else if (j > 0 && (i === 0 || matrix[i][j - 1] >= matrix[i - 1][j])) {
            diff.unshift({ type: 'added', content: newLines[j - 1] });
            j--;
        } else if (i > 0 && (j === 0 || matrix[i][j - 1] < matrix[i - 1][j])) {
            diff.unshift({ type: 'removed', content: oldLines[i - 1] });
            i--;
        }
    }
    return diff;
}

/**
 * Parsuje tekst G-Protocol i przygotowuje dane do Diffa
 */
function parseGProtocolContent(text) {
    const lines = text.split('\n');
    let searchBlock = [];
    let replaceBlock = [];
    let state = 'context';

    // Regexy
    const searchStart = /^(╔═══════ SEARCH|<<<<<<< SEARCH)/;
    const replaceStart = /^(╠═══════ REPLACE|=======)/;
    const endBlock = /^(╚═══════ END|>>>>>>> REPLACE)/;

    // Nazwa pliku
    const fileMatch = text.match(/(\/\/|#|\/\*)\s*(?:File|Plik):\s*(.*?)(?:\s*\*\/)?$/m);
    const filePath = fileMatch ? fileMatch[2].trim() : 'Unknown File';

    console.log('[Gluon Parser] Detected file:', filePath);

    for (let line of lines) {
        if (searchStart.test(line)) { state = 'search'; continue; }
        if (replaceStart.test(line)) { state = 'replace'; continue; }
        if (endBlock.test(line)) { state = 'context'; continue; }

        if (state === 'search') searchBlock.push(line);
        else if (state === 'replace') replaceBlock.push(line);
    }

    // Zwróć null tylko jeśli nie znaleziono żadnego z markerów (nie jest to diff)
    const hasMarkers = text.includes('SEARCH') && (text.includes('REPLACE') || text.includes('======='));
    if (!hasMarkers) {
        console.log('[Gluon Parser] No SEARCH/REPLACE markers found');
        return null;
    }

    console.log('[Gluon Parser] Found markers, searchBlock:', searchBlock.length, 'replaceBlock:', replaceBlock.length);

    // Jeśli mamy markery ale puste bloki, to znaczy że AI jeszcze generuje - zwróć pusty diff
    if (searchBlock.length === 0 && replaceBlock.length === 0) {
        console.log('[Gluon Parser] Empty blocks, AI still generating');
        return { filePath, lines: [] };
    }

    // Uruchom algorytm diffowania na blokach Search i Replace
    const diffLines = computeUnifiedDiff(searchBlock.join('\n'), replaceBlock.join('\n'));
    console.log('[Gluon Parser] Diff computed, lines:', diffLines.length);

    return { filePath, lines: diffLines };
}

function startApplySystemDetector(provider) {
  console.log(`[Gluon Detector] 🟢 ACTIVATED for provider: ${provider}`);

  const observer = new MutationObserver(() => {
    if (!processingTimer) {
        processingTimer = setTimeout(() => {
            scanAndInjectButtons(provider);
            processingTimer = null;
        }, SCAN_INTERVAL_MS);
    }
  });

  observer.observe(document.body, { childList: true, subtree: true });
  scanAndInjectButtons(provider);
}

function scanAndInjectButtons(provider) {
  if (provider === 'gemini') {
    const turns = Array.from(document.querySelectorAll('ms-chat-turn'));

    turns.forEach(turn => {
        const isModel = turn.classList.contains('model') ||
                        turn.getAttribute('role') === 'model' ||
                        turn.innerHTML.includes('data-turn-role="Model"');

        if (!isModel) return;

        // Szukaj ms-code-block ORAZ zwykłych pre (fallback)
        const blocks = Array.from(turn.querySelectorAll('ms-code-block, pre'));

        blocks.forEach(block => {
            // Unikaj duplikatów: Jeśli to pre wewnątrz ms-code-block, ignoruj
            if (block.tagName === 'PRE' && block.closest('ms-code-block')) return;

            // Sprawdź czy ten konkretny blok był już przetwarzany
            if (block.dataset.gluonProcessed === 'true') {
                return; 
            }

            // Dodatkowe zabezpieczenie: sprawdź czy bezpośrednio przed blokiem jest overlay
            const prevSibling = block.previousElementSibling;
            if (prevSibling && (
                prevSibling.classList.contains('gluon-inline-diff-container') || 
                prevSibling.classList.contains('gluon-overlay-container')
            )) {
                block.dataset.gluonProcessed = 'true';
                return;
            }

            const text = block.innerText || "";
            
            if (text.includes('@gluon:next_step') || text.includes('@gluon:response')) {
                return;
            }

            // Agresywna detekcja: wystarczy ścieżka pliku LUB marker diff
            const hasFileMarker = /(\/\/|#|\/\*)\s*(?:File|Plik):/i.test(text);
            const hasDiffMarker = text.includes('SEARCH') && text.includes('REPLACE');
            
            if (hasFileMarker || hasDiffMarker) {
                injectOverlay(block, provider);
            }
        });
    });
  }
}

function injectOverlay(codeBlockElement, provider) {
    // Oznaczamy blok jako przetworzony
    codeBlockElement.dataset.gluonProcessed = 'true';

    // --- ELEMENTY WSPÓLNE UI ---
    const statusPanel = document.createElement('div');
    statusPanel.className = 'gluon-status-panel hidden';
    statusPanel.innerHTML = `
        <div class="gluon-status-text">Ready</div>
        <div class="gluon-progress-track">
            <div class="gluon-progress-fill"></div>
        </div>
    `;

    const statusText = statusPanel.querySelector('.gluon-status-text');
    const progressFill = statusPanel.querySelector('.gluon-progress-fill');

    const btn = document.createElement('button');
    btn.className = 'gluon-apply-btn';
    btn.innerHTML = '⚡ Apply';

    const stopBtn = document.createElement('button');
    stopBtn.className = 'gluon-stop-btn hidden';
    stopBtn.innerHTML = '⏹️';
    stopBtn.title = 'Cancel processing';

    // Kontenery
    const diffContainer = document.createElement('div');
    diffContainer.className = 'gluon-inline-diff-container';
    
    const legacyOverlay = document.createElement('div');
    legacyOverlay.className = 'gluon-overlay-container';

    // Flaga streamingu
    let isStreaming = true;
    let refreshInterval = null;
    let currentStartLine = 1; // Przechowuje wykryty numer linii

    // Funkcja aktualizująca Diffa
    const updateOverlay = () => {
        const rawText = codeBlockElement.innerText;
        const diffData = parseGProtocolContent(rawText);

        // Debug: sprawdź co zwraca parser
        if (!diffData) {
            console.log('[Gluon] parseGProtocolContent returned null for:', rawText.substring(0, 200));
        }

        // Sprawdź czy zakończono generowanie (END marker)
        const isFinished = rawText.includes('╚═══════ END') || rawText.includes('>>>>>>> REPLACE');
        
        if (isFinished && isStreaming) {
            isStreaming = false;
            if (refreshInterval) clearInterval(refreshInterval);
            console.log('[Gluon] Stream finished. Fetching real line numbers...');
            
            // Pobierz prawdziwe numery linii dopiero po zakończeniu
            if (diffData) resolveLineNumbers(diffData);
        }

        if (diffData) {
            // --- TRYB DIFF ---
            // Jeśli wcześniej był legacyOverlay (fallback), usuń go i przełącz na diff
            if (legacyOverlay.parentNode) {
                legacyOverlay.remove();
                console.log('[Gluon] Switching from fallback to diff mode');
            }

            if (!diffContainer.parentNode) {
                // Pierwsze wstrzyknięcie kontenera Diff
                const header = document.createElement('div');
                header.className = 'gluon-diff-header';
                
                const title = document.createElement('div');
                title.className = 'gluon-diff-title';
                
                const actions = document.createElement('div');
                actions.className = 'gluon-diff-actions';
                
                const toggleBtn = document.createElement('button');
                toggleBtn.className = 'gluon-toggle-raw-btn';
                toggleBtn.textContent = 'Show Raw Code';
                
                const content = document.createElement('div');
                content.className = 'gluon-diff-content';

                actions.append(toggleBtn, btn, stopBtn);
                header.append(title, actions);
                diffContainer.append(header, content, statusPanel);
                statusPanel.style.margin = "8px";

                // Toggle Logic
                let isRawVisible = false;
                toggleBtn.onclick = () => {
                    isRawVisible = !isRawVisible;
                    if (isRawVisible) {
                        codeBlockElement.classList.remove('gluon-code-hidden');
                        content.style.display = 'none';
                        toggleBtn.textContent = 'Show Diff View';
                    } else {
                        codeBlockElement.classList.add('gluon-code-hidden');
                        content.style.display = 'block';
                        toggleBtn.textContent = 'Show Raw Code';
                    }
                };

                codeBlockElement.classList.add('gluon-code-hidden');
                codeBlockElement.parentNode.insertBefore(diffContainer, codeBlockElement);
            }

            // Aktualizacja treści (przy każdym ticku)
            const titleEl = diffContainer.querySelector('.gluon-diff-title');
            const contentEl = diffContainer.querySelector('.gluon-diff-content');
            
            // Update Tytułu
            const lineInfo = currentStartLine > 1 ? ` <span style="opacity:0.5; font-weight:400; margin-left:8px;">(L${currentStartLine})</span>` : '';
            titleEl.innerHTML = `<span style="opacity:0.6">📄</span> ${diffData.filePath}${lineInfo}`;

            // Update Linii
            contentEl.innerHTML = ''; 
            let lineNumLeft = currentStartLine;
            let lineNumRight = currentStartLine;

            diffData.lines.forEach(line => {
                const div = document.createElement('div');
                div.className = `gluon-diff-line ${line.type}`;
                
                const lineNum = document.createElement('div');
                lineNum.className = 'gluon-line-number';
                
                const displayNum = currentStartLine === 1 ? (isFinished ? '?' : '...') : null;

                if (line.type === 'context') {
                    lineNum.textContent = displayNum || lineNumRight++;
                    if (currentStartLine !== 1) lineNumLeft++;
                } else if (line.type === 'removed') {
                    lineNum.textContent = displayNum || lineNumLeft++;
                } else if (line.type === 'added') {
                    lineNum.textContent = currentStartLine === 1 ? '+' : lineNumRight++;
                }

                const lineContent = document.createElement('div');
                lineContent.className = 'gluon-line-content';
                const prefix = line.type === 'removed' ? '- ' : (line.type === 'added' ? '+ ' : '  ');
                lineContent.textContent = prefix + line.content;

                div.append(lineNum, lineContent);
                contentEl.appendChild(div);
            });

        } else {
            // --- TRYB FALLBACK (Brak Diffa / Zwykły kod) ---
            // WAŻNE: Nie twórz fallback overlay podczas streamingu, poczekaj aż AI skończy
            if (!isStreaming && !legacyOverlay.parentNode) {
                console.log('[Gluon] Creating fallback overlay (no diff detected after stream ended)');

                // Tworzymy prosty layout z przyciskiem Apply
                // Ukrywamy statusPanel jako domyślny (pokaże się po kliknięciu Apply)
                statusPanel.classList.add('hidden');

                // Kontener dla przycisków
                const actionsContainer = document.createElement('div');
                actionsContainer.style.cssText = 'display: flex; align-items: center; gap: 10px; justify-content: flex-end;';
                actionsContainer.appendChild(btn);
                actionsContainer.appendChild(stopBtn);

                legacyOverlay.appendChild(actionsContainer);
                legacyOverlay.appendChild(statusPanel);

                codeBlockElement.parentNode.insertBefore(legacyOverlay, codeBlockElement.nextSibling);
            }
        }
    };

    // Funkcja pytająca backend o linię
    const resolveLineNumbers = (diffData) => {
        const searchBlockContent = diffData.lines
            .filter(l => l.type === 'context' || l.type === 'removed')
            .map(l => l.content)
            .join('\n');

        if (searchBlockContent.trim()) {
            chrome.runtime.sendMessage({
                action: 'resolve_change_locations',
                payload: [{
                    filePath: diffData.filePath,
                    searchContent: searchBlockContent
                }]
            });

            const locationListener = (msg) => {
                if (msg.type === 'change_locations_resolved' && msg.data) {
                    const match = msg.data.find(m => 
                        (m.filePath === diffData.filePath || m.filePath.endsWith(diffData.filePath)) &&
                        m.lineStart > 0
                    );
                    if (match) {
                        currentStartLine = match.lineStart;
                        updateOverlay(); // Odśwież UI z nowymi numerami
                        chrome.runtime.onMessage.removeListener(locationListener);
                    }
                }
            };
            chrome.runtime.onMessage.addListener(locationListener);
        }
    };

    // Uruchomienie początkowe
    updateOverlay();

    // Jeśli nie ma markera końcowego, uruchom interwał
    const rawText = codeBlockElement.innerText;
    if (!rawText.includes('╚═══════ END') && !rawText.includes('>>>>>>> REPLACE')) {
        refreshInterval = setInterval(updateOverlay, 1000);
    } else {
        // Jeśli od razu znaleźliśmy END (np. przy przeładowaniu strony), pobierz linie od razu
        const initialData = parseGProtocolContent(rawText);
        if (initialData) resolveLineNumbers(initialData);
    }

    // --- LOGIKA OBSŁUGI ZDARZEŃ (WSPÓLNA) ---
    // (Reszta funkcji bez zmian - updateState, btn.onclick, stopBtn.onclick)
    let activeRequestId = null;

    const updateState = (step, message, progress) => {
        statusPanel.classList.remove('hidden');
        statusText.textContent = message;
        statusText.className = 'gluon-status-text active';
        progressFill.style.width = `${progress}%`;

        const icons = {
            'queued': '⏳', 'validating': '🔒', 'snapshotting': '📸',
            'matching': '🔍', 'safetycheck': '🛡️', 'writing': '💾',
            'notifying': '🔔', 'success': '✅', 'failed': '❌'
        };
        const icon = icons[step] || '⚙️';
        statusText.innerHTML = `<span class="gluon-step-icon">${icon}</span> ${message}`;

        if (step === 'success' || step === 'failed') {
            stopBtn.classList.add('hidden');
            btn.classList.remove('processing');
        } else {
            stopBtn.classList.remove('hidden');
        }

        if (step === 'success') {
            statusText.className = 'gluon-status-text success';
            btn.innerHTML = 'Applied';
            btn.classList.add('success');
        } else if (step === 'failed') {
            statusText.className = 'gluon-status-text error';
            progressFill.style.background = '#da3633';
        }
    };

    btn.onclick = async () => {
        const htmlContent = codeBlockElement.innerHTML;
        const requestId = `req-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
        activeRequestId = requestId;
        
        if (diffContainer) diffContainer.dataset.requestId = requestId;
        else legacyOverlay.dataset.requestId = requestId;

        console.log(`[Gluon] Apply Clicked. ID: ${requestId}`);

        btn.disabled = true;
        btn.innerHTML = 'Processing...';
        btn.classList.add('processing');
        statusPanel.classList.remove('hidden');
        updateState('queued', 'Starting...', 5);

        const messageListener = (msg) => {
            if (msg.type === 'apply_progress_update' && msg.data && msg.data.requestId === requestId) {
                const { step, message, progress, details } = msg.data;
                const displayMsg = details ? `${message} (${details})` : message;
                updateState(step, displayMsg, progress);

                if (step === 'success' || step === 'failed') {
                    chrome.runtime.onMessage.removeListener(messageListener);
                    activeRequestId = null;
                    if (step === 'failed') {
                        btn.disabled = false;
                        btn.innerHTML = 'Retry';
                        btn.classList.remove('processing');
                    }
                }
            }
        };
        chrome.runtime.onMessage.addListener(messageListener);

        try {
            await sendToRust(htmlContent, provider, requestId);
        } catch (error) {
            console.error('[Gluon Apply Error]', error);
            updateState('failed', error.message || 'Connection failed', 100);
            chrome.runtime.onMessage.removeListener(messageListener);
            activeRequestId = null;
            btn.disabled = false;
            btn.innerHTML = 'Retry';
            btn.classList.remove('processing');
        }
    };

    stopBtn.onclick = async () => {
        if (!activeRequestId) return;
        stopBtn.disabled = true;
        stopBtn.innerHTML = '⏹️ Stopping...';
        try {
            await chrome.runtime.sendMessage({
                action: 'cancel_processing',
                requestId: activeRequestId
            });
            statusText.innerHTML = `<span class="gluon-step-icon">❌</span> Cancelled`;
            stopBtn.classList.add('hidden');
            btn.disabled = false;
            btn.innerHTML = 'Retry';
            btn.classList.remove('processing');
            activeRequestId = null;
        } catch (error) {
            console.error('[Gluon Cancel Error]', error);
            stopBtn.disabled = false;
            stopBtn.innerHTML = '⏹️ Stop';
        }
    };
}

function sendToRust(htmlContent, provider, requestId) {
    return new Promise((resolve, reject) => {
        if (!chrome.runtime || !chrome.runtime.id) {
            reject(new Error('Extension context invalidated. Reload page.'));
            return;
        }

        try {
            chrome.runtime.sendMessage({
                action: 'process_dom_stream',
                html: htmlContent,
                provider: provider,
                requestId: requestId
            }, (response) => {
                if (chrome.runtime.lastError) {
                    reject(new Error(chrome.runtime.lastError.message));
                } else {
                    resolve(response);
                }
            });
        } catch (error) {
            reject(error);
        }
    });
}

window.detectors = window.detectors || {};
window.detectors.applySystem = startApplySystemDetector;