import { sidebarLogger } from '../../common/logger.js';

/**
 * Calculates the Levenshtein distance between two strings.
 * @param {string} a The first string.
 * @param {string} b The second string.
 * @returns {number} The Levenshtein distance.
 */
function levenshteinDistance(a, b) {
    const matrix = Array(b.length + 1).fill(null).map(() => Array(a.length + 1).fill(null));
    for (let i = 0; i <= a.length; i += 1) { matrix[0][i] = i; }
    for (let j = 0; j <= b.length; j += 1) { matrix[j][0] = j; }
    for (let j = 1; j <= b.length; j += 1) {
        for (let i = 1; i <= a.length; i += 1) {
            const indicator = a[i - 1] === b[j - 1] ? 0 : 1;
            matrix[j][i] = Math.min(
                matrix[j][i - 1] + 1,      // deletion
                matrix[j - 1][i] + 1,      // insertion
                matrix[j - 1][i - 1] + indicator, // substitution
            );
        }
    }
    return matrix[b.length][a.length];
}

/**
 * Main parsing pipeline for handling responses from the AI model.
 * @param {string} rawText The raw text from the AI's response.
 * @param {Array<object>} fileTreeData The current file tree data for validation.
 * @param {Array<object>} allAvailableProjects The complete list of all known projects.
 * @returns {object} A result object with status and parsed data.
 */
function parseGluonResponse(rawText, fileTreeData, allAvailableProjects) {
    sidebarLogger.log('Starting parsing pipeline...');
    
    // 1. Check if this is a G-Protocol Code Patch (XML)
    // If so, we skip JSON parsing as this is handled by content script.
    if (rawText.includes('<gluon_patch>')) {
        sidebarLogger.log('Detected G-Protocol XML Patch. Ignoring JSON parsing in sidebar.');
        return { 
            status: 'ignored', 
            type: 'code_patch', 
            message: 'Code patch detected. Handled by content script.' 
        };
    }

    let jsonObject;
    try {
        // 1. STRUCTURED OUTPUT MODE (AI Studio Direct JSON)
        const trimmedText = rawText.trim();

        // Attempt 1: Direct Parse (Best Case)
        if (trimmedText.startsWith('{')) {
            try {
                jsonObject = JSON.parse(trimmedText);
            } catch (e) {
                // Attempt 2: Fuzzy JSON Extraction (if text has trailing garbage)
                // Finds the last closing brace that balances the first opening brace
                // Simple heuristic: Look for the last '}' in the string
                const lastBrace = trimmedText.lastIndexOf('}');
                if (lastBrace > 0 && lastBrace < trimmedText.length - 1) {
                    const cleanJson = trimmedText.substring(0, lastBrace + 1);
                    jsonObject = JSON.parse(cleanJson);
                } else {
                    throw e; // Re-throw if fuzzy fix didn't apply
                }
            }
        } else {
            // Attempt 3: Markdown Block Extraction
            const jsonMatch = rawText.match(/```(?:json)?\s*([\s\S]+?)\s*```/);
            if (jsonMatch) {
                jsonObject = JSON.parse(jsonMatch[1]);
            } else {
                // Attempt 4: Deep Search for JSON object
                const deepMatch = rawText.match(/(\{[\s\S]*\})/);
                if (deepMatch) {
                    jsonObject = JSON.parse(deepMatch[1]);
                } else {
                    throw new Error("No JSON structure found");
                }
            }
        }
    } catch (error) {
        // Jeśli nie udało się sparsować JSONa, traktujemy to jako zwykły tekst
        return { status: 'error', type: 'format', message: `JSON Parse Error: ${error.message}` };
    }

    // --- DETEKCJA STRUKTURY ---

    // 1. Nowy format G-SOP (Structured Output Protocol)
    if (jsonObject.gluon_actions || jsonObject.thought_process) {
        sidebarLogger.log('✅ Detected G-SOP (Structured Output)');

        const files = [];

        // Mapowanie file_changes na stary format oczekiwany przez resztę systemu
        if (jsonObject.gluon_actions && jsonObject.gluon_actions.file_changes) {
            jsonObject.gluon_actions.file_changes.forEach(change => {
                files.push({
                    path: change.file_path,
                    // Symulujemy format code patch dla reszty systemu
                    // W rzeczywistości content script odbierze to inaczej, ale sidebar potrzebuje listy plików
                    project: "unknown" // Zostanie uzupełnione w warstwie walidacji
                });
            });
        }

        // Normalizuj context_ops do formatu load array
        let contextOps = null;
        if (jsonObject.gluon_actions?.context_ops) {
            const rawOps = jsonObject.gluon_actions.context_ops;
            if (Array.isArray(rawOps)) {
                contextOps = { load: rawOps };
            } else if (rawOps.load && Array.isArray(rawOps.load)) {
                contextOps = rawOps;
            } else {
                // Format pojedynczego obiektu {type: "...", ...}
                contextOps = { load: [rawOps] };
            }
        }

        // Konstruujemy obiekt next_step dla kompatybilności wstecznej
        const nextStep = {
            action: "continue",
            reasoning: jsonObject.thought_process,
            user_message: jsonObject.user_message,
            file_changes: jsonObject.gluon_actions?.file_changes || [],
            context_ops: contextOps
        };

        return {
            status: 'success',
            type: 'structured_output',
            data: {
                responseType: 'structured_output',
                structuredData: nextStep,
                files: files, // Dla kompatybilności z walidacją plików
                // Przechowuj oryginalne dane dla overlay
                thought_process: jsonObject.thought_process,
                user_message: jsonObject.user_message,
                file_changes: jsonObject.gluon_actions?.file_changes || [],
                context_ops: contextOps
            }
        };
    }

    // 2. Legacy G-Interactive Protocol (dla wstecznej kompatybilności)
    if (jsonObject['@gluon:next_step']) {
        sidebarLogger.log('✅ Detected Legacy G-Interactive Protocol');
        return {
            status: 'success',
            type: 'interactive_context',
            data: {
                responseType: 'interactive_context',
                next_step: jsonObject['@gluon:next_step']
            }
        };
    }

    // Legacy Protocol Validation (Standard Handoff)
    const responseType = jsonObject['@gluon:response'];
    if (!responseType || !jsonObject['@gluon:files']) {
        return { status: 'error', type: 'format', message: 'Response is missing required keys: "@gluon:response" or "@gluon:files" (and is not a valid next_step).' };
    }
    sidebarLogger.log('Layer 3: Schema Validation ✓');

    // Layer 4: File Validation
    const allProjectFiles = new Map();
    
    const collectFiles = (nodes, paths = new Set()) => {
        for (const node of nodes) {
            if (node.nodeType === 'file') {
                paths.add(node.path.replace(/\\/g, '/'));
            } else if (node.children) {
                collectFiles(node.children, paths);
            }
        }
        return paths;
    };

    fileTreeData.forEach(project => {
        if (project.tree) {
            allProjectFiles.set(project.projectPath, collectFiles(project.tree));
        }
    });

    const validationResult = {
        found: [],
        notFound: [],
        suggestions: [],
    };

    const projectMapping = new Map();
    const projectsToMap = allAvailableProjects && allAvailableProjects.length > 0
        ? allAvailableProjects
        : fileTreeData.map(p => ({ path: p.projectPath }));

    projectsToMap.forEach((project) => {
        const projectPath = project.path || project.projectPath;
        if (!projectPath) return;
        const projectName = projectPath.split(/[\\/]/).pop() || projectPath;
        const sanitizedName = projectName.replace(/[^a-zA-Z0-9_-]/g, '_').toLowerCase();
        const gluonKey = `@gluon:${sanitizedName}`;
        
        projectMapping.set(gluonKey, projectPath);
        projectMapping.set(projectName, projectPath);
    });
    
    sidebarLogger.log('Project Mapping:', Object.fromEntries(projectMapping));

    for (const [gluonKey, fileList] of Object.entries(jsonObject['@gluon:files'])) {
        const projectPath = projectMapping.get(gluonKey);
        sidebarLogger.log(`Processing key: ${gluonKey}, ProjectPath: ${projectPath}`);
        
        if (!projectPath) {
            fileList.forEach(file => validationResult.notFound.push({ 
                project: gluonKey, 
                path: file, 
                reason: 'Unknown project key' 
            }));
            continue;
        }

        const availableFiles = allProjectFiles.get(projectPath);
        if (!availableFiles) {
            sidebarLogger.log(`❌ Project not loaded (not in fileTreeData): ${projectPath} (Key: ${gluonKey})`);
            fileList.forEach(filePath => {
                 validationResult.notFound.push({ project: projectPath, path: filePath, reason: 'Project not loaded' });
            });
            continue;
        }

        fileList.forEach(filePath => {
            const normalizedPath = filePath.replace(/\\/g, '/');
            if (availableFiles.has(normalizedPath)) {
                validationResult.found.push({ project: projectPath, path: normalizedPath });
            } else {
                let suggestion = null;
                let minDistance = 4; // Levenshtein distance threshold
                for (const availableFile of availableFiles) {
                    const distance = levenshteinDistance(normalizedPath, availableFile);
                    if (distance < minDistance) {
                        minDistance = distance;
                        suggestion = availableFile;
                    }
                }
                validationResult.notFound.push({ project: projectPath, path: normalizedPath, reason: 'File not found' });
                if (suggestion) {
                    validationResult.suggestions.push({ from: normalizedPath, to: suggestion });
                }
            }
        });
    }

    sidebarLogger.log('Layer 4: File Validation ✓', validationResult);

    const hasFound = validationResult.found.length > 0;
    const hasNotFound = validationResult.notFound.length > 0;

    const responseData = {
        files: validationResult.found,
        responseType: responseType
    };
    
    // Zaktualizowana logika do obsługi różnych typów odpowiedzi
    if (jsonObject['@gluon:reasoning']) {
        responseData.reasoning = jsonObject['@gluon:reasoning'];
    }
    if (jsonObject['@gluon:handoff']) {
        responseData.handoff = jsonObject['@gluon:handoff'];
    }

    if (!hasNotFound) {
        return { status: 'success', data: responseData };
    }

    if (hasFound && hasNotFound) {
        return {
            status: 'partial',
            data: {
                ...responseData,
                notFound: validationResult.notFound,
                suggestions: validationResult.suggestions
            }
        };
    }

    if (!hasFound && hasNotFound) {
        const allUnknownProjects = validationResult.notFound.every(
            file => file.reason === 'Unknown project key'
        );
        let errorMessage = 'None of the requested files could be found.';
        
        if (allUnknownProjects) {
            const unknownKeys = [...new Set(validationResult.notFound.map(f => f.project))].join('", "');
            errorMessage = `The following requested project(s) are not loaded or do not exist: "${unknownKeys}". Please select the correct projects in Gluon.`;
        } else if (validationResult.notFound.length > 0) {
            errorMessage = 'Some requested files were not found in the loaded projects.';
        }

        return { 
            status: 'error', 
            type: 'files', 
            message: errorMessage,
            data: validationResult 
        };
    }
    
    return { status: 'error', type: 'unknown', message: 'An unknown error occurred during parsing.', data: validationResult };
}

export { parseGluonResponse };