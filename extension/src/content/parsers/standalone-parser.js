// Standalone Parser for Content Script
// Parsuje Gluon responses BEZ zależności od sidebara

/**
 * Parsuje tekst AI na overlay data
 * @param {string} rawText - Surowy tekst z AI
 * @returns {object} - { success: bool, type: string, data: object, error?: string }
 */
export function parseGluonResponseStandalone(rawText) {
    console.log('[Gluon Parser] Starting standalone parsing...');

    let jsonObject;

    try {
        const trimmedText = rawText.trim();

        // Attempt 1: Direct Parse
        if (trimmedText.startsWith('{')) {
            try {
                jsonObject = JSON.parse(trimmedText);
            } catch (e) {
                // Attempt 2: Fuzzy JSON (remove trailing garbage)
                const lastBrace = trimmedText.lastIndexOf('}');
                if (lastBrace > 0) {
                    const cleanJson = trimmedText.substring(0, lastBrace + 1);
                    jsonObject = JSON.parse(cleanJson);
                } else {
                    throw e;
                }
            }
        } else {
            // Attempt 3: Markdown Block Extraction
            const jsonMatch = rawText.match(/```(?:json)?\s*([\s\S]+?)\s*```/);
            if (jsonMatch) {
                jsonObject = JSON.parse(jsonMatch[1]);
            } else {
                // Attempt 4: Deep Search
                const deepMatch = rawText.match(/(\{[\s\S]*\})/);
                if (deepMatch) {
                    jsonObject = JSON.parse(deepMatch[1]);
                } else {
                    throw new Error("No JSON structure found");
                }
            }
        }
    } catch (error) {
        console.error('[Gluon Parser] JSON parse failed:', error);
        return {
            success: false,
            type: 'format_error',
            error: `Failed to parse JSON: ${error.message}`
        };
    }

    // Detect response type

    // 1. Structured Output (G-SOP)
    if (jsonObject.gluon_actions || jsonObject.thought_process || jsonObject.user_message) {
        console.log('[Gluon Parser] ✅ Detected G-SOP (Structured Output)');

        return {
            success: true,
            type: 'structured_output',
            data: {
                responseType: 'structured_output',
                structuredData: {
                    user_message: jsonObject.user_message || '',
                    thought_process: jsonObject.thought_process || jsonObject.reasoning || '',
                    reasoning: jsonObject.reasoning || jsonObject.thought_process || '',
                    file_changes: jsonObject.gluon_actions?.file_changes || [],
                    context_ops: jsonObject.gluon_actions?.context_ops || { load: [] }
                }
            }
        };
    }

    // 2. Interactive Context (@gluon:next_step)
    if (jsonObject.next_step) {
        console.log('[Gluon Parser] ✅ Detected Interactive Context');

        return {
            success: true,
            type: 'interactive_context',
            data: jsonObject
        };
    }

    // 3. Legacy Auto-Select / Context Handoff
    if (jsonObject['@gluon:response'] || jsonObject['@gluon:files']) {
        console.log('[Gluon Parser] ✅ Detected Legacy Gluon Response');

        const responseType = jsonObject['@gluon:response'] || 'auto_select';

        return {
            success: true,
            type: responseType,
            data: {
                responseType: responseType,
                found: [], // Simplified - no file validation in content script
                reasoning: jsonObject['@gluon:reasoning'] || '',
                handoff: jsonObject['@gluon:handoff'] || null,
                prompt: jsonObject['@gluon:prompt'] || null
            }
        };
    }

    // 4. Unknown format
    console.warn('[Gluon Parser] ⚠️ Unknown JSON format');
    return {
        success: false,
        type: 'unknown_format',
        error: 'Unrecognized Gluon response format'
    };
}

/**
 * Escape HTML dla bezpieczeństwa
 */
export function escapeHTML(str) {
    if (!str) return '';
    return String(str)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#039;');
}
