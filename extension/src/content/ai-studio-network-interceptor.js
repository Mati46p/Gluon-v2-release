// AI Studio Network Interceptor - API Response Capture
// Intercepts fetch/XHR to capture AI responses from Google AI Studio API

(function() {
    'use strict';

    console.log('[AIStudio Interceptor] 🚀 Initializing network interceptor...');

    // Store original functions before any other script can modify them
    const originalFetch = window.fetch;
    const originalXHROpen = XMLHttpRequest.prototype.open;
    const originalXHRSend = XMLHttpRequest.prototype.send;

    // AI Studio API patterns to intercept
    const API_PATTERNS = [
        /generativelanguage\.googleapis\.com/,
        /makersuite.*\.google\.com/,
        /alkalimakersuite.*\.google\.com/,
        /alkalimakersuite.*\.clients6\.google\.com/,
        /aistudio\.google\.com.*api/,
        /GenerateContent/,
        /StreamGenerateContent/,
        /UpdatePrompt/
    ];

    function shouldInterceptURL(url) {
        return API_PATTERNS.some(pattern => pattern.test(url));
    }

    // ==================== FETCH INTERCEPTOR ====================
    window.fetch = function(...args) {
        const url = args[0]?.url || args[0];
        const shouldIntercept = shouldInterceptURL(url);

        if (shouldIntercept) {
            console.log('[AIStudio Interceptor] 🔍 Intercepting fetch:', url);
        }

        // Call original fetch
        const fetchPromise = originalFetch.apply(this, args);

        // If we should intercept, clone and process response
        if (shouldIntercept) {
            return fetchPromise.then(async response => {
                // Clone response so we don't consume the original
                const clonedResponse = response.clone();

                try {
                    // Check if it's a streaming response
                    const contentType = clonedResponse.headers.get('content-type') || '';

                    if (contentType.includes('text/event-stream') || contentType.includes('stream')) {
                        console.log('[AIStudio Interceptor] 📡 Detected streaming response');
                        handleStreamingResponse(clonedResponse, url);
                    } else {
                        // Regular JSON response
                        const data = await clonedResponse.json();
                        console.log('[AIStudio Interceptor] 📦 Captured API response:', data);
                        processAPIResponse(data, url);
                    }
                } catch (error) {
                    console.error('[AIStudio Interceptor] ❌ Error processing response:', error);
                }

                // Return original response for the page
                return response;
            });
        }

        return fetchPromise;
    };

    // ==================== XHR INTERCEPTOR ====================
    XMLHttpRequest.prototype.open = function(method, url, ...rest) {
        this._gluonURL = url;
        this._gluonMethod = method;
        this._gluonShouldIntercept = shouldInterceptURL(url);

        if (this._gluonShouldIntercept) {
            console.log('[AIStudio Interceptor] 🔍 Intercepting XHR:', method, url);
        }

        return originalXHROpen.apply(this, [method, url, ...rest]);
    };

    XMLHttpRequest.prototype.send = function(...args) {
        if (this._gluonShouldIntercept) {
            // Capture response when it loads
            this.addEventListener('load', function() {
                try {
                    const data = JSON.parse(this.responseText);
                    const url = this._gluonURL;

                    console.log('[AIStudio Interceptor] 📦 Captured XHR response from:', url);
                    console.log('[AIStudio Interceptor] 📊 Response structure:', {
                        type: Array.isArray(data) ? 'Array' : typeof data,
                        length: Array.isArray(data) ? data.length : 'N/A',
                        keys: typeof data === 'object' && !Array.isArray(data) ? Object.keys(data) : 'N/A'
                    });

                    // Log first 500 chars of response for debugging
                    const preview = JSON.stringify(data).substring(0, 500);
                    console.log('[AIStudio Interceptor] 📝 Response preview:', preview);

                    processAPIResponse(data, url);
                } catch (error) {
                    console.error('[AIStudio Interceptor] ❌ Error parsing XHR response:', error);
                }
            });
        }

        return originalXHRSend.apply(this, args);
    };

    // ==================== STREAMING RESPONSE HANDLER ====================
    async function handleStreamingResponse(response, url) {
        console.log('[AIStudio Interceptor] 🌊 Processing streaming response...');

        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let fullText = '';
        let buffer = '';

        try {
            while (true) {
                const { done, value } = await reader.read();

                if (done) {
                    console.log('[AIStudio Interceptor] ✅ Stream complete');
                    if (fullText) {
                        emitToContentScript({
                            type: 'ai_response_complete',
                            content: fullText,
                            url: url,
                            timestamp: Date.now()
                        });
                    }
                    break;
                }

                // Decode chunk
                buffer += decoder.decode(value, { stream: true });

                // Process complete lines (SSE format: "data: {...}\n\n")
                const lines = buffer.split('\n\n');
                buffer = lines.pop() || ''; // Keep incomplete line in buffer

                for (const line of lines) {
                    if (line.startsWith('data: ')) {
                        try {
                            const jsonData = JSON.parse(line.slice(6));

                            // Extract text from response structure
                            const text = extractTextFromResponse(jsonData);
                            if (text) {
                                fullText += text;
                                console.log('[AIStudio Interceptor] 📝 Chunk:', text.substring(0, 100));

                                // Emit partial update
                                emitToContentScript({
                                    type: 'ai_response_chunk',
                                    content: fullText,
                                    chunk: text,
                                    url: url,
                                    timestamp: Date.now()
                                });
                            }
                        } catch (e) {
                            // Not JSON, might be plain text chunk
                            console.log('[AIStudio Interceptor] 📝 Text chunk:', line);
                        }
                    }
                }
            }
        } catch (error) {
            console.error('[AIStudio Interceptor] ❌ Error reading stream:', error);
        }
    }

    // ==================== RESPONSE PROCESSOR ====================
    function processAPIResponse(data, url) {
        console.log('[AIStudio Interceptor] 🔧 Processing API response structure...');

        // Extract text content from various Google API response structures
        const text = extractTextFromResponse(data);

        if (text) {
            console.log('[AIStudio Interceptor] ✅ Extracted text length:', text.length);
            console.log('[AIStudio Interceptor] 📝 Preview:', text.substring(0, 200));

            emitToContentScript({
                type: 'ai_response_complete',
                content: text,
                url: url,
                timestamp: Date.now()
            });
        } else {
            console.warn('[AIStudio Interceptor] ⚠️ Could not extract text from response');
        }
    }

    // ==================== TEXT EXTRACTION ====================
    function extractTextFromResponse(data) {
        console.log('[AIStudio Interceptor] 🔍 Analyzing response structure...');
        console.log('[AIStudio Interceptor] 📊 Response type:', Array.isArray(data) ? 'Array' : typeof data);

        // Structure 1: Standard JSON - candidates[0].content.parts[0].text
        if (data?.candidates?.[0]?.content?.parts?.[0]?.text) {
            console.log('[AIStudio Interceptor] ✅ Matched Structure 1: Standard JSON');
            return data.candidates[0].content.parts[0].text;
        }

        // Structure 2: candidates[0].output
        if (data?.candidates?.[0]?.output) {
            console.log('[AIStudio Interceptor] ✅ Matched Structure 2: candidates[0].output');
            return data.candidates[0].output;
        }

        // Structure 3: text field directly
        if (data?.text) {
            console.log('[AIStudio Interceptor] ✅ Matched Structure 3: Direct text field');
            return data.text;
        }

        // Structure 4: content.parts[0].text
        if (data?.content?.parts?.[0]?.text) {
            console.log('[AIStudio Interceptor] ✅ Matched Structure 4: content.parts[0].text');
            return data.content.parts[0].text;
        }

        // Structure 5: Iterate through all candidates and combine
        if (data?.candidates && Array.isArray(data.candidates)) {
            const texts = data.candidates
                .map(c => c?.content?.parts?.[0]?.text || c?.output || '')
                .filter(t => t);

            if (texts.length > 0) {
                console.log('[AIStudio Interceptor] ✅ Matched Structure 5: Multiple candidates');
                return texts.join('\n\n');
            }
        }

        // Structure 6: Google Protobuf Array Format (AI Studio internal format)
        if (Array.isArray(data)) {
            console.log('[AIStudio Interceptor] 🔍 Detected array format (Protobuf), length:', data.length);

            // Try to find conversation in typical positions
            // Format: [status, ..., ..., config, metadata, ..., ..., ..., ..., ..., ..., ..., [], conversation]

            // Check position 13 (typical conversation location in UpdatePrompt)
            if (data[13] && Array.isArray(data[13])) {
                console.log('[AIStudio Interceptor] 🔍 Found array at position 13, checking for conversation...');
                const conversation = data[13];

                // Look for the last model response in conversation
                for (let i = conversation.length - 1; i >= 0; i--) {
                    const turnGroup = conversation[i];
                    if (!Array.isArray(turnGroup)) continue;

                    // Each turnGroup contains multiple turns
                    for (let j = turnGroup.length - 1; j >= 0; j--) {
                        const turn = turnGroup[j];
                        if (!Array.isArray(turn)) continue;

                        const text = turn[0];
                        const role = turn[8]; // Index 8 is the role ("user" or "model")

                        if (role === 'model' && typeof text === 'string' && text.length > 0) {
                            console.log('[AIStudio Interceptor] ✅ Matched Structure 6: Protobuf array format');
                            console.log('[AIStudio Interceptor] 📝 Extracted model response from position [13][' + i + '][' + j + ']');
                            return text;
                        }
                    }
                }
            }

            // Try other array positions if 13 didn't work
            console.log('[AIStudio Interceptor] 🔍 Scanning all array positions...');
            for (let i = 0; i < data.length; i++) {
                const item = data[i];

                // Look for nested arrays that might contain conversation
                if (Array.isArray(item) && item.length > 0) {
                    for (let j = 0; j < item.length; j++) {
                        const subItem = item[j];
                        if (Array.isArray(subItem) && subItem.length > 0) {
                            const text = subItem[0];
                            const role = subItem[8];

                            if (role === 'model' && typeof text === 'string' && text.length > 50) {
                                console.log('[AIStudio Interceptor] ✅ Found model response at [' + i + '][' + j + ']');
                                return text;
                            }
                        }
                    }
                }
            }
        }

        console.warn('[AIStudio Interceptor] ❌ No matching structure found');
        return null;
    }

    // ==================== BRIDGE TO CONTENT SCRIPT ====================
    function emitToContentScript(data) {
        console.log('[AIStudio Interceptor] 📤 Emitting to content script:', data.type);

        window.dispatchEvent(new CustomEvent('GLUON_AI_RESPONSE', {
            detail: data
        }));
    }

    console.log('[AIStudio Interceptor] ✅ Network interceptor initialized');
    console.log('[AIStudio Interceptor] 🎯 Monitoring patterns:', API_PATTERNS);
})();
