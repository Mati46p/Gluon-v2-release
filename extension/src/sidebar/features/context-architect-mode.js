/**
 * CONTEXT ARCHITECT MODE
 *
 * Moduł zarządzający trybem "Context Architect" - zaawansowany workflow
 * gdzie model AI sam żąda precyzyjnych fragmentów kodu zamiast otrzymywać
 * gigantyczny dump wszystkich plików.
 *
 * Workflow:
 * 1. Użytkownik wybiera projekt i klika "Context Architect Mode"
 * 2. System generuje Repo Skeleton (lekka mapa projektu)
 * 3. System przygotowuje System Prompt + Skeleton + User Task
 * 4. Użytkownik kopiuje i wkleja do AI Studio
 * 5. Model analizuje i żąda kontekstu przez @gluon:next_step
 * 6. Extension automatycznie obsługuje żądania
 */

import { sidebarLogger } from '../../common/logger.js';
import { generateContextArchitectPrompt } from '../utils/prompt-generator.js';
import { showStatusMessage } from '../management/stateManagement.js';

/**
 * Pobiera Repo Skeleton z backendu Rust
 *
 * @param {string} projectPath - Ścieżka do projektu
 * @returns {Promise<string>} - Repo Skeleton jako tekst
 */
async function fetchRepoSkeleton(projectPath) {
    sidebarLogger.log('[Context Architect] Fetching repo skeleton for:', projectPath);

    try {
        const response = await chrome.runtime.sendMessage({
            action: 'get_repo_skeleton',
            payload: {
                projectPath: projectPath
            }
        });

        if (!response) {
            throw new Error('No response from background script');
        }

        if (response.error) {
            throw new Error(response.error);
        }

        sidebarLogger.log('[Context Architect] ✅ Skeleton received (length:', response.length, 'chars)');
        return response;

    } catch (error) {
        sidebarLogger.error('[Context Architect] ❌ Failed to fetch skeleton:', error);
        throw error;
    }
}

/**
 * Główna funkcja aktywująca Context Architect Mode
 *
 * @param {string} projectPath - Ścieżka do wybranego projektu
 * @param {string} userTask - Zadanie użytkownika (opcjonalne)
 * @param {string} language - Język ('en' lub 'pl')
 * @returns {Promise<string>} - Gotowy prompt do skopiowania
 */
async function activateContextArchitectMode(projectPath, userTask = '', language = 'pl') {
    sidebarLogger.log('[Context Architect] Activating mode for project:', projectPath);

    try {
        showStatusMessage('🔄 Generating Repo Skeleton...', 'info');

        // Krok 1: Pobierz Repo Skeleton
        const repoSkeleton = await fetchRepoSkeleton(projectPath);

        if (!repoSkeleton || repoSkeleton.trim().length === 0) {
            throw new Error('Repo Skeleton is empty. Project may not be indexed.');
        }

        showStatusMessage('✅ Skeleton generated!', 'success');

        // Krok 2: Wygeneruj pełny prompt
        sidebarLogger.log('[Context Architect] Generating Context Architect prompt...');

        const fullPrompt = generateContextArchitectPrompt(
            repoSkeleton,
            userTask,
            language
        );

        sidebarLogger.log('[Context Architect] ✅ Prompt generated (length:', fullPrompt.length, 'chars)');

        // Krok 3: Skopiuj do schowka
        await navigator.clipboard.writeText(fullPrompt);

        showStatusMessage('✅ Context Architect Prompt copied to clipboard!', 'success');

        return fullPrompt;

    } catch (error) {
        sidebarLogger.error('[Context Architect] ❌ Failed:', error);
        showStatusMessage(`❌ Failed: ${error.message}`, 'error');
        throw error;
    }
}

/**
 * Wyświetla preview Repo Skeleton w modal/popup
 *
 * @param {string} skeleton - Repo Skeleton do wyświetlenia
 */
function showSkeletonPreview(skeleton) {
    // TODO: Implementacja modal preview (FAZA 3.1)
    sidebarLogger.log('[Context Architect] Skeleton preview:', skeleton.substring(0, 500) + '...');
    console.log('=== REPO SKELETON PREVIEW ===\n', skeleton);
}

/**
 * Eksportuje Repo Skeleton do pliku
 *
 * @param {string} skeleton - Repo Skeleton
 * @param {string} projectName - Nazwa projektu
 */
async function exportSkeletonToFile(skeleton, projectName) {
    const filename = `${projectName}_skeleton_${Date.now()}.txt`;

    try {
        const blob = new Blob([skeleton], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);

        const a = document.createElement('a');
        a.href = url;
        a.download = filename;
        a.click();

        URL.revokeObjectURL(url);

        showStatusMessage(`✅ Skeleton exported as ${filename}`, 'success');
    } catch (error) {
        sidebarLogger.error('[Context Architect] Export failed:', error);
        showStatusMessage('❌ Export failed', 'error');
    }
}

export {
    fetchRepoSkeleton,
    activateContextArchitectMode,
    showSkeletonPreview,
    exportSkeletonToFile
};
