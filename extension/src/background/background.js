import { backgroundLogger } from '../common/logger.js';

let ws = null;
let reconnectAttempts = 0;
const MAX_RECONNECT_ATTEMPTS = 5;
const RECONNECT_DELAY = 2000;
const pendingRequests = new Map();
const pendingFileAttachments = new Map();
const agentToTabMapping = new Map(); // Maps agent_id -> tab_id
const pendingFallbackContexts = new Map(); // Maps request_id -> tab_id for fallback context fetching
function pauseLicenseChecks() {}
function resumeLicenseChecks() {}

// Browser Watcher State
let watcherTabId = null;
const WATCHER_RULE_ID = 1000; // Constant rule ID for watcher
let isWatcherActive = false;

// ============================================================================
// Browser Watcher Lifecycle Safety
// ============================================================================

// Handle tab closures - cleanup DNR rules if recording tab is closed
chrome.tabs.onRemoved.addListener((tabId) => {
  if (tabId === watcherTabId && isWatcherActive) {
    backgroundLogger.warn(`[Watcher] Recording tab ${tabId} was closed. Cleaning up...`);
    cleanupWatcher();
  }
});

// Handle navigation away from recording page
chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (tabId === watcherTabId && isWatcherActive && changeInfo.url) {
    backgroundLogger.warn(`[Watcher] Recording tab ${tabId} navigated away. Cleaning up...`);
    cleanupWatcher();
  }
});

function cleanupWatcher() {
  backgroundLogger.log('[Watcher] Starting cleanup...');

  // Remove DNR rules
  if (isWatcherActive) {
    chrome.declarativeNetRequest.updateSessionRules({
      removeRuleIds: [WATCHER_RULE_ID]
    }).then(() => {
      backgroundLogger.log(`[Watcher] Removed DNR rule ${WATCHER_RULE_ID}`);
    }).catch(e => {
      backgroundLogger.error('[Watcher] Failed to remove DNR rule:', e);
    });
  }

  // Reset state
  watcherTabId = null;
  isWatcherActive = false;

  // Notify sidebar that recording stopped
  chrome.runtime.sendMessage({
    type: 'watcher_status_changed',
    isRecording: false
  }).catch(() => {
    // Sidebar might be closed, ignore error
  });

  chrome.runtime.sendMessage({
    type: 'error',
    message: 'Browser Watcher stopped: Recording tab was closed or navigated away'
  }).catch(() => {});

  backgroundLogger.log('[Watcher] Cleanup complete');
} 

function connect() {
  if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
    backgroundLogger.log("WebSocket is already open or connecting.");
    broadcastStatus();
    return;
  }

  ws = new WebSocket('ws://127.0.0.1:8743');

  ws.onopen = () => {
    backgroundLogger.log("WebSocket Connected to Desktop.");
    reconnectAttempts = 0;
    broadcastStatus();

    // Load initial data after connection
    sendMessageToDesktop('list_embedding_models'); // Load RAG models for dropdown
    sendMessageToDesktop('get_local_ai_status'); // Get current RAG state

    chrome.runtime.sendMessage({ type: 'license_status_loaded', data: { status: 'VALID' } });
  };

  ws.onmessage = (event) => {
    // backgroundLogger.log('Background received from desktop:', event.data);
    try {
      const response = JSON.parse(event.data);

      // [DEBUG] Log workflow responses
      if (response.action && response.action.startsWith('workflow_')) {
        backgroundLogger.log('[Workflow V2] 🔍 RAW WebSocket response:', event.data);
      }

      // Check if this is a response to a pending tauri_command or workflow_command request
      if (response.request_id && pendingRequests.has(response.request_id)) {
        const pendingRequest = pendingRequests.get(response.request_id);
        clearTimeout(pendingRequest.timeout);
        pendingRequests.delete(response.request_id);
        // backgroundLogger.log(`🧹 Cleared timeout for request: ${response.request_id} (action: ${response.action})`);

        // If this has a sendResponse callback, send it back to the caller
        if (pendingRequest.sendResponse && pendingRequest.command) {
          backgroundLogger.log(`✅ Sending response for ${pendingRequest.command}:`, response.payload);

          if (response.action === 'error') {
            pendingRequest.sendResponse({ error: response.payload.error || response.payload });
          } else {
            // [V2 FIX] Workflow commands expect { data: payload } format
            backgroundLogger.log(`[Workflow V2] Sending response to extension:`, { data: response.payload });
            pendingRequest.sendResponse({ data: response.payload });
          }
        }

        // If this has resolve/reject callbacks (for Promise-based requests)
        if (pendingRequest.resolve && pendingRequest.reject) {
          if (response.action === 'error') {
            pendingRequest.reject(response.payload.error || response.payload);
          } else {
            pendingRequest.resolve(response.payload);
          }
        }
      }

      // Route response based on action
      switch (response.action) {
        case 'change_locations_resolved':
          chrome.runtime.sendMessage({
            type: 'change_locations_resolved',
            data: response.payload,
            request_id: response.request_id
          });
          break;
        case 'get_projects':
          chrome.runtime.sendMessage({ type: 'projects_loaded', data: response.payload, request_id: response.request_id });
          break;
        case 'get_file_trees':
          chrome.runtime.sendMessage({ type: 'file_trees_loaded', data: response.payload, request_id: response.request_id });
          break;
        case 'get_files_multi':
          chrome.runtime.sendMessage({ type: 'files_multi_content_loaded', data: response.payload, request_id: response.request_id });
          break;
        case 'context_generation_progress':
          // WAŻNE: To NIE jest response na request (nie ma request_id)
          // To jest event emitowany przez Tauri, więc NIE czyścimy timeout
          // Timeout zostanie wyczyszczony dopiero przy 'context_file_generated'
          chrome.runtime.sendMessage({
            type: 'context_generation_progress',
            data: response.payload
          });
          break;
        case 'context_file_generated':
          chrome.runtime.sendMessage({ type: 'context_file_generated', data: response.payload, request_id: response.request_id });
          break;
        case 'context_generation_cancelled':
          resumeLicenseChecks(); // Resume license checks after cancellation
          chrome.runtime.sendMessage({
            type: 'context_generation_cancelled',
            data: response.payload,
            request_id: response.request_id
          });
          break;
        case 'get_environments':
          chrome.runtime.sendMessage({
            type: 'environments_loaded',
            data: response.payload,
            request_id: response.request_id
          });
          break;
        case 'get_environment_for_project':
          backgroundLogger.log('Received project environment from backend:', response.payload);
          chrome.runtime.sendMessage({
            type: 'project_environment_loaded',
            data: response.payload,
            request_id: response.request_id
          });
          break;
          case 'get_local_ai_status':
          case 'toggle_local_ai':
            chrome.runtime.sendMessage({
              type: 'local_ai_status_update',
              data: response.payload,
              request_id: response.request_id
            });
            break;
          case 'list_embedding_models':
            backgroundLogger.log('📋 Received list_embedding_models response:', response.payload);
            chrome.runtime.sendMessage({
              type: 'embedding_models_list',
              data: response.payload,
              request_id: response.request_id
            });
            break;
          case 'set_embedding_model':
            backgroundLogger.log('🔄 Received set_embedding_model response:', response.payload);
            chrome.runtime.sendMessage({
              type: 'embedding_model_changed',
              success: response.payload?.status === 'success',
              model: response.payload?.model || response.payload,
              error: response.payload?.error,
              request_id: response.request_id
            });
            break;
          case 'switch_ai_model':
            chrome.runtime.sendMessage({
              type: 'status_update',
              status: response.payload?.message || 'Model changed successfully'
            });
            break;
          case 'status_update': // FIX: Obsługa generycznych update'ów statusu z backendu
            chrome.runtime.sendMessage({
                type: 'status_update',
                status: response.payload
            });
            break;
          case 'get_context_files_history':
          chrome.runtime.sendMessage({ 
            type: 'context_history_loaded', 
            data: response.payload, 
            request_id: response.request_id 
          });
          break;
        case 'context_file_content_loaded':
          // Pobierz filename z cache
          const filename = pendingFileAttachments.get(response.request_id) || 'context.txt';
          pendingFileAttachments.delete(response.request_id);
          
          // Przekaż do sidebara
          chrome.runtime.sendMessage({ 
            type: 'context_file_content_ready', 
            data: {
              filename: filename,
              content: response.payload.content
            },
            request_id: response.request_id 
          });
          break;
        // w funkcji ws.onmessage, w bloku switch
        case 'file_as_base64_loaded':
          backgroundLogger.log('Received binary file from desktop. Forwarding to content script.');
          // Forward binary file data to content script on active tab
          chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
            if (tabs && tabs.length > 0) {
              const activeTabId = tabs[0].id;
              backgroundLogger.log(`Found active tab with ID: ${activeTabId}. Sending message...`); // <-- DODAJ TEN LOG
              chrome.tabs.sendMessage(activeTabId, {
                action: 'upload_binary_file_to_ai',
                payload: response.payload
              }, (response) => {
                // Sprawdź, czy wystąpił błąd podczas wysyłania
                if (chrome.runtime.lastError) {
                  backgroundLogger.error('FATAL: Error sending message to content script:', chrome.runtime.lastError.message); // <-- DODAJ TEN LOG
                  chrome.runtime.sendMessage({ type: 'error', message: 'Could not communicate with the page. Please refresh the Gemini/Claude tab.' });
                  return;
                }
                // Sprawdź, czy content script odpowiedział z błędem
                if (response && !response.success) {
                    backgroundLogger.error('Content script responded with an error:', response.error); // <-- DODAJ TEN LOG
                } else {
                    backgroundLogger.log('Message successfully sent to content script.'); // <-- DODAJ TEN LOG
                }
              });
            } else {
              backgroundLogger.error('No active tab found to inject binary file.');
            }
          });
          break;
        case 'toggle_context_favorite':
          // Wyślij potwierdzenie do sidebara
          chrome.runtime.sendMessage({ 
            type: 'context_favorite_updated', 
            data: response.payload,
            request_id: response.request_id 
          });
          break;
          
        case 'rename_context_file':
          // Wyślij potwierdzenie do sidebara
          chrome.runtime.sendMessage({ 
            type: 'context_file_renamed', 
            data: response.payload,
            request_id: response.request_id 
          });
          break;

        case 'apply_code_changes':
          backgroundLogger.log('🚀 [Background] Received "apply_code_changes". Payload:', JSON.stringify(response.payload, null, 2));
          chrome.runtime.sendMessage({
            type: 'apply_code_changes_response',
            data: response.payload,
            request_id: response.request_id,
            success: true
          });
          break;
          case 'agentic_progress':
            // Agentic RAG progress updates
            backgroundLogger.log('🤖 [Agentic] Progress:', JSON.stringify(response.payload));
            chrome.runtime.sendMessage({
              type: 'agentic_progress',
              data: response.payload
            });
            break;
          // [FIX] Obsługa postępu ładowania AI (wcześniej "Unknown action")
          case 'ai_loading_progress':
            chrome.runtime.sendMessage({
                type: 'ai_loading_progress',
                data: response.payload
            });
            break;
          // [FIX] Obsługa postępu indeksowania (wcześniej "Unknown action")
          case 'indexing_progress':
            chrome.runtime.sendMessage({
                type: 'indexing_progress',
                data: response.payload
            });
            break;
          case 'apply_progress_update':
            // Pulse System: Forward real-time progress updates to CONTENT SCRIPTS
            backgroundLogger.log('⚡ [Pulse] Progress update:', JSON.stringify(response.payload));
            backgroundLogger.log('⚡ [Pulse] Full response:', JSON.stringify(response));
            backgroundLogger.log('⚡ [Pulse] changeId:', response.payload?.changeId || response.payload?.change_id);
            backgroundLogger.log('⚡ [Pulse] message:', response.payload?.message);
            backgroundLogger.log('⚡ [Pulse] progress:', response.payload?.progress);

            const sendToTab = (tabId) => {
                // Use callback to avoid "message channel closed" errors
                backgroundLogger.log('⚡ [Pulse] Sending to tab:', tabId);
                chrome.tabs.sendMessage(tabId, {
                  type: 'apply_progress_update',
                  data: response.payload
                }, (sendResponse) => {
                    // Suppress runtime.lastError to prevent console noise
                    if (chrome.runtime.lastError) {
                        backgroundLogger.warn(`⚡ [Pulse] Tab ${tabId} unavailable:`, chrome.runtime.lastError.message);
                    } else {
                        backgroundLogger.log(`⚡ [Pulse] Successfully sent to tab ${tabId}`);
                    }
                });
            };

            chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
              if (tabs && tabs.length > 0) {
                sendToTab(tabs[0].id);
              } else {
                // Fallback: Broadcast to all tabs
                chrome.tabs.query({}, (allTabs) => allTabs.forEach(t => sendToTab(t.id)));
              }
            });
            break;

          case 'change_status_update':
            // Forward change status updates (undo/redo) to CONTENT SCRIPTS (Active Tab)
            backgroundLogger.log('🔄 [Status Update] Change status changed:', response.payload);
            chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
              if (tabs && tabs.length > 0) {
                chrome.tabs.sendMessage(tabs[0].id, {
                  type: 'change_status_update',
                  data: response.payload
                }).catch(() => { /* Ignore if tab has no content script */ });
              }
            });
            break;

        case 'undo_change':
            backgroundLogger.log('✅ [Undo] Undo operation acknowledged:', response.payload);
            // Optionally forward confirmation to content script or sidebar
            chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
              if (tabs && tabs.length > 0) {
                chrome.tabs.sendMessage(tabs[0].id, {
                  type: 'undo_change_response',
                  data: response.payload
                }).catch(() => {});
              }
            });
            break;

        case 'redo_change':
            backgroundLogger.log('✅ [Redo] Redo operation acknowledged:', response.payload);
            // Optionally forward confirmation to content script or sidebar
            chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
              if (tabs && tabs.length > 0) {
                chrome.tabs.sendMessage(tabs[0].id, {
                  type: 'redo_change_response',
                  data: response.payload
                }).catch(() => {});
              }
            });
            break;

        case 'process_dom_stream_result':
          if (response.payload.success) {
            const count = response.payload.processed_files ? response.payload.processed_files.length : 0;
            backgroundLogger.log(`✅ Desktop successfully processed ${count} file(s).`);
            // Send completion notification (NOT status_update to avoid disconnection overlay)
            chrome.runtime.sendMessage({
                type: 'processing_complete',
                status: `Processed ${count} changes`
            });
          } else {
            backgroundLogger.warn('⚠️ Desktop processed stream but returned no success.');
          }
          break;

        case 'workflow_get_graph':
          // Send to extension pages (sidebar)
          chrome.runtime.sendMessage({
            type: 'workflow_response',
            action: 'workflow_get_graph',
            success: true,
            data: response.payload,
            request_id: response.request_id
          });

          // Also broadcast to content scripts (overlay)
          chrome.tabs.query({}, (tabs) => {
            tabs.forEach(tab => {
              chrome.tabs.sendMessage(tab.id, {
                type: 'workflow_response',
                action: 'workflow_get_graph',
                success: true,
                data: response.payload,
                request_id: response.request_id
              }).catch(() => {}); // Ignore errors for tabs without content scripts
            });
          });
          break;

        case 'workflow_add_agent':
          // Send to extension pages (sidebar)
          chrome.runtime.sendMessage({
            type: 'workflow_response',
            action: 'workflow_add_agent',
            success: true,
            data: response.payload,
            request_id: response.request_id
          });

          // Also broadcast to content scripts (overlay) so they can refresh
          chrome.tabs.query({}, (tabs) => {
            tabs.forEach(tab => {
              chrome.tabs.sendMessage(tab.id, {
                type: 'workflow_response',
                action: 'workflow_add_agent',
                success: true,
                data: response.payload,
                request_id: response.request_id
              }).catch(() => {});
            });
          });
          break;

        case 'workflow_remove_agent':
          chrome.runtime.sendMessage({
            type: 'workflow_response',
            action: 'workflow_remove_agent',
            success: true,
            request_id: response.request_id
          });
          break;

        case 'workflow_update_agent':
          chrome.runtime.sendMessage({
            type: 'workflow_response',
            action: 'workflow_update_agent',
            success: true,
            data: response.payload,
            request_id: response.request_id
          });
          break;

        case 'workflow_add_connection':
          chrome.runtime.sendMessage({
            type: 'workflow_response',
            action: 'workflow_add_connection',
            success: true,
            request_id: response.request_id
          });
          break;

        case 'workflow_remove_connection':
          chrome.runtime.sendMessage({
            type: 'workflow_response',
            action: 'workflow_remove_connection',
            success: true,
            request_id: response.request_id
          });
          break;

        case 'agent_register':
          // Forward pairing response to content script
          chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
            if (tabs[0]) {
              const tabId = tabs[0].id;
              const agentId = response.payload?.agent?.id;

              // Store agent_id -> tab_id mapping
              if (agentId) {
                agentToTabMapping.set(agentId, tabId);
                backgroundLogger.log(`[Agent Pairing] Mapped agent ${agentId} to tab ${tabId}`);
              }

              chrome.tabs.sendMessage(tabId, {
                type: 'agent_pair_response',
                success: response.payload?.success || false,
                agent: response.payload?.agent,
                error: response.payload?.error,
                request_id: response.request_id
              });
            }
          });
          break;

        case 'agent_message_received':
          // Agent received a message from workflow - inject it into AI Studio
          const targetAgentId = response.payload.target_agent_id;
          const targetTabId = agentToTabMapping.get(targetAgentId);

          backgroundLogger.log('💬 [BG] agent_message_received from desktop');
          backgroundLogger.log('💬 [BG] Target Agent ID:', targetAgentId);
          backgroundLogger.log('💬 [BG] Target Tab ID:', targetTabId);
          backgroundLogger.log('💬 [BG] Content length:', response.payload.content ? response.payload.content.length : 'No content');
          backgroundLogger.log('💬 [BG] Content (first 200 chars):', response.payload.content ? response.payload.content.substring(0, 200) : 'N/A');
          backgroundLogger.log('💬 [BG] From agent:', response.payload.from_agent);

          if (targetTabId) {
            backgroundLogger.log(`✅ [BG] Found tab mapping. Routing to tab ${targetTabId} for agent ${targetAgentId}`);
            chrome.tabs.sendMessage(targetTabId, {
              type: 'agent_inject_message',
              content: response.payload.content,
              from_agent: response.payload.from_agent,
              auto_submit: response.payload.auto_submit
            }, (messageResponse) => {
              if (chrome.runtime.lastError) {
                backgroundLogger.error(`❌ [BG] Failed to send to tab ${targetTabId}:`, chrome.runtime.lastError.message);
                // Clean up stale mapping
                backgroundLogger.log(`🗑️ [BG] Cleaning up stale mapping for agent ${targetAgentId}`);
                agentToTabMapping.delete(targetAgentId);
              } else {
                backgroundLogger.log(`✅ [BG] Message injected into tab ${targetTabId} successfully`);
              }
            });
          } else {
            backgroundLogger.warn(`❌ [BG] No tab found for agent ${targetAgentId}. Available mappings:`, Array.from(agentToTabMapping.entries()));
          }
          break;

        case 'agent_response':
          // Handle agent-to-agent message routing from desktop
          const responseTargetAgentId = response.payload?.target_agent_id || response.payload?.targetAgentId;

          // [FIX] Jeśli nie ma target_agent_id, to jest to tylko potwierdzenie (ACK) lub internal routing (np. do Agregatora)
          // Nie próbuj wysyłać tego do karty.
          if (!responseTargetAgentId) {
             backgroundLogger.log('📨 [BG] agent_response ACK received (no target redirection needed):', response.payload?.message);
             return; 
          }

          const responseTargetTabId = agentToTabMapping.get(responseTargetAgentId);

          backgroundLogger.log('📨 [BG] agent_response routing request');
          backgroundLogger.log('📨 [BG] Target Agent ID:', responseTargetAgentId);
          backgroundLogger.log('📨 [BG] Target Tab ID:', responseTargetTabId);

          if (responseTargetTabId) {
            backgroundLogger.log(`✅ [BG] Found tab mapping. Routing to tab ${responseTargetTabId} for agent ${responseTargetAgentId}`);
            chrome.tabs.sendMessage(responseTargetTabId, {
              type: 'agent_inject_message',
              content: response.payload.content,
              from_agent: response.payload?.from_agent || response.payload?.fromAgent,
              auto_submit: response.payload?.auto_submit || response.payload?.autoSubmit || false
            }, (messageResponse) => {
              if (chrome.runtime.lastError) {
                backgroundLogger.error(`❌ [BG] Failed to send to tab ${responseTargetTabId}:`, chrome.runtime.lastError.message);
                agentToTabMapping.delete(responseTargetAgentId);
              } else {
                backgroundLogger.log(`✅ [BG] Message injected into tab ${responseTargetTabId} successfully`);
              }
            });
          } else {
            // To może być poprawne, jeśli agent nie jest jeszcze połączony, ale logujemy jako info/warn
            backgroundLogger.log(`ℹ️ [BG] No active tab found for agent ${responseTargetAgentId} (might be disconnected or virtual).`);
          }
          break;

        case 'workflow_aggregator_update':
          // Nowy typ wiadomości dla Agregatora Raportów
          backgroundLogger.log('📊 [BG] Aggregator update received:', response.payload);
          chrome.runtime.sendMessage({
            type: 'workflow_aggregator_update',
            data: response.payload
          });
          break;

        case 'workflow_auto_apply_trigger':
          // Trigger dla Auto-Apply w Sidebarze
          backgroundLogger.log('⚡ [BG] Auto-Apply trigger received:', response.payload);
          chrome.runtime.sendMessage({
            type: 'workflow_auto_apply_trigger',
            data: response.payload
          });
          break;

        case 'agent_status_changed':
          // Notify content script about agent status change
          chrome.tabs.query({}, (tabs) => {
            tabs.forEach(tab => {
              chrome.tabs.sendMessage(tab.id, {
                type: 'agent_status_changed',
                status: response.payload.status
              }).catch(() => {});
            });
          });
          break;

        case 'workflow_set_auto_forward':
          chrome.runtime.sendMessage({
            type: 'workflow_response',
            action: 'workflow_set_auto_forward',
            success: true,
            request_id: response.request_id
          });
          break;

        // 🔥 SYNC 1:1 Forwarder
        case 'workflow_sync':
          chrome.runtime.sendMessage({
            type: 'workflow_sync',
            data: response.payload
          });
          break;

        // [V2] Workflow Preset Operations - Already handled by pendingRequest.sendResponse above (line 146)
        case 'workflow_get_agent_presets':
        case 'workflow_get_connection_presets':
        case 'workflow_get_workflow_presets':
        case 'workflow_create_agent_from_preset':
        case 'workflow_create_from_preset':
        case 'workflow_get_saved_configs':
        case 'workflow_save_config':
        case 'workflow_delete_saved_config':
        case 'workflow_load_config':
          backgroundLogger.log(`✅ [Workflow V2] Operation completed: ${response.action}`);
          // Response already sent via pendingRequest.sendResponse (line 146)
          break;


        // [V2] Workflow Execution Operations - Already handled by pendingRequest.sendResponse above (line 146)
        case 'workflow_execute_agent':
        case 'workflow_set_agent_status':
        case 'workflow_get_agent_history':
        case 'workflow_send_message_to_agent':
          backgroundLogger.log(`✅ [Workflow V2] Execution completed: ${response.action}`);
          // Response already sent via pendingRequest.sendResponse (line 146)
          break;

        // [V2] Real-time State Sync Event (WebSocket broadcast)
        case 'workflow_state_sync':
          backgroundLogger.log('🔄 [Workflow V2] State sync received from Rust');
          chrome.runtime.sendMessage({
            type: 'workflow-state-sync',  // Note: workflow-manager-v2.js expects this format
            payload: response.payload
          });
          break;

        // [V2] Routing Notification (when LLM routes message between agents)
        case 'workflow_route_message':
          backgroundLogger.log('📨 [Workflow V2] Route notification:', response.payload);
          chrome.runtime.sendMessage({
            type: 'workflow-route-message',
            payload: response.payload
          });
          break;

        // [G-INTERACTIVE] Context Operations Response
        case 'execute_context_operations':
          backgroundLogger.log(`✅ [execute_context_operations] Response for request ${response.request_id}`);
          backgroundLogger.log(`   Pending requests before: ${pendingRequests.has(response.request_id) ? 'YES' : 'NO'}`);
          // NOTE: Timeout is already cleared in lines 128-144 (generic pendingRequests logic)

          if (pendingFallbackContexts.has(response.request_id)) {
              const tabId = pendingFallbackContexts.get(response.request_id);
              pendingFallbackContexts.delete(response.request_id);

              backgroundLogger.log(`🧠 [BG Fallback] Formatting context response for tab ${tabId}`);
              const results = response.payload.results || [];
              let contentText = "🧠 GLUON CONTEXT FETCH RESULTS:\n\n";
              let successful = 0;
              let failed = 0;
              results.forEach(res => {
                 const opStr = res.operation?.path || res.operation?.symbol || res.operation?.query || res.operation?.type || 'Unknown';
                 if (res.success) {
                     contentText += `=== [${(res.operation?.type || 'UNKNOWN').toUpperCase()}] ${opStr} ===\n${res.content}\n\n`;
                     successful++;
                 } else {
                     contentText += `=== [${(res.operation?.type || 'UNKNOWN').toUpperCase()}] ${opStr} ===\nERROR: ${res.error}\n\n`;
                     failed++;
                 }
              });

              chrome.tabs.sendMessage(tabId, {
                 action: 'upload_file_to_gemini',
                 file: {
                    filename: 'gluon_context.txt',
                    content: contentText,
                    type: 'text/plain'
                 }
              }, (res) => {
                  if (chrome.runtime.lastError) {
                      backgroundLogger.error('❌ [BG Fallback] Failed to inject file:', chrome.runtime.lastError.message);
                  } else {
                      backgroundLogger.log(`✅ [BG Fallback] Context file injected successfully (${successful} ok, ${failed} failed)`);
                  }
              });
              break;
          }

          chrome.runtime.sendMessage({
            type: 'execute_context_operations_response',
            data: response.payload,
            request_id: response.request_id
          });
          break;

        // [SYMBOL PICKER] Get file symbols response
        case 'get_file_symbols':
          backgroundLogger.log(`✅ [SymbolPicker] Response for request ${response.request_id}`);
          // Handled by pendingRequests.resolve/reject in lines 128-144
          break;

        // Google Drive integration responses
        case 'list_drive_files':
        case 'download_file_content':
        case 'get_drive_file_info':
        case 'is_google_logged_in':
        case 'has_google_credentials':
        case 'google_logout':
          // Forward Google Drive responses to sidebar
          backgroundLogger.log('📨 [Background] Received Google Drive response:', response.action);
          chrome.runtime.sendMessage({
            type: 'google_drive_response',
            action: response.action,
            data: response.payload,
            request_id: response.request_id
          });
          break;

        case 'error':
          resumeLicenseChecks();
          backgroundLogger.error('Error from desktop:', response.payload);
          chrome.runtime.sendMessage({ type: 'error', message: response.payload, request_id: response.request_id });
          break;
        default:
          backgroundLogger.warn('Unknown action from desktop:', response.action);
      }
    } catch (e) {
      backgroundLogger.error('Error parsing message from desktop:', e);
    }
  };

  ws.onerror = (error) => {
    backgroundLogger.error('Background WebSocket error:', error);
    ws = null;
    broadcastStatus();
  };

  ws.onclose = () => {
    backgroundLogger.log("WebSocket Disconnected from Desktop.");
    ws = null;
    broadcastStatus();
    if (reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
      reconnectAttempts++;
      const delay = RECONNECT_DELAY * Math.pow(2, reconnectAttempts - 1);
      backgroundLogger.log(`Attempting reconnect ${reconnectAttempts}/${MAX_RECONNECT_ATTEMPTS} in ${delay}ms`);
      setTimeout(() => connect(), delay);
    }
  };
}

function broadcastStatus() {
  let status = 'Disconnected';
  if (ws) {
    switch (ws.readyState) {
      case WebSocket.CONNECTING: status = 'Connecting...'; break;
      case WebSocket.OPEN: status = 'Connected ✓'; break;
      case WebSocket.CLOSING: status = 'Closing...'; break;
      case WebSocket.CLOSED: default: status = 'Disconnected'; break;
    }
  }
  backgroundLogger.log(`Broadcasting status: ${status}`);
  chrome.runtime.sendMessage({ type: 'status_update', status: status });
}

function sendMessageToDesktop(action, payload = {}) {
  //backgroundLogger.log('Attempting to send')${action}'... WS state: ${ws ? ws.readyState : 'null'}`);
  if (ws && ws.readyState === WebSocket.OPEN) {
    const message = {
      id: crypto.randomUUID(),
      action: action,
      payload: payload
    };
    
    // Setup timeout (longer for file generation and heavy operations)
    const timeoutDuration =
      action === 'generate_context_file' ? 180000 :  // 3 minuty
      action === 'get_file_trees' ? 60000 :  // 60 seconds for file tree operations
      action === 'get_files_multi' ? 60000 :  // 60 seconds for multi-file operations
      30000;  // 30 seconds default
    const timeout = setTimeout(() => {
      backgroundLogger.error(`Request ${message.id} timed out after ${timeoutDuration / 1000}s`);
      chrome.runtime.sendMessage({ type: 'error', message: 'Request timeout. The desktop app is not responding.', request_id: message.id });
      pendingRequests.delete(message.id);
    }, timeoutDuration);

    pendingRequests.set(message.id, { timeout });
    //backgroundLogger.log('Sending message object:'), message); 
    ws.send(JSON.stringify(message));
    // backgroundLogger.log(`Background sent '${action}' request to desktop with ID: ${message.id}`);
    return message.id; 
  } else {
    backgroundLogger.warn(`FAILED to send '${action}'. WebSocket is not open.`); 
    chrome.runtime.sendMessage({ type: 'error', message: 'Cannot send message, WebSocket is not open.' });
    return null;
  }
}

// ============================================================================
// Browser Watcher Functions
// ============================================================================

async function startWatcher(tabId) {
  try {
    backgroundLogger.log(`[Watcher] Starting for tab ${tabId}`);

    // Get tab info to validate URL
    const tab = await chrome.tabs.get(tabId);

    // Check if the URL is a protected/restricted URL that extensions cannot access
    const protectedUrlPatterns = [
      /^chrome:\/\//,
      /^chrome-extension:\/\//,
      /^edge:\/\//,
      /^about:/,
      /^devtools:\/\//,
      /^view-source:/,
      /^chrome-search:\/\//
    ];

    const isProtectedUrl = protectedUrlPatterns.some(pattern => pattern.test(tab.url));

    if (isProtectedUrl) {
      const errorMsg = `Cannot record on protected pages (${tab.url.split(':')[0]}://). Please navigate to a regular website.`;
      backgroundLogger.error(`[Watcher] ${errorMsg}`);

      chrome.runtime.sendMessage({
        type: 'error',
        message: errorMsg
      });

      return;
    }

    watcherTabId = tabId;

    // Remove CSP headers using declarativeNetRequest
    // Always try to remove existing rule first to avoid "not unique ID" errors
    await chrome.declarativeNetRequest.updateSessionRules({
      removeRuleIds: [WATCHER_RULE_ID],
      addRules: [{
        id: WATCHER_RULE_ID,
        priority: 1,
        action: {
          type: 'modifyHeaders',
          responseHeaders: [
            { header: 'content-security-policy', operation: 'remove' },
            { header: 'x-frame-options', operation: 'remove' }
          ]
        },
        condition: {
          tabIds: [tabId],
          resourceTypes: ['main_frame', 'sub_frame']
        }
      }]
    });

    backgroundLogger.log(`[Watcher] CSP removal rule ${WATCHER_RULE_ID} added for tab ${tabId}`);

    // Reload the tab to apply the new rules
    await chrome.tabs.reload(tabId);

    // Wait for the tab to finish loading, then inject the recorder script
    chrome.tabs.onUpdated.addListener(function injectListener(updatedTabId, changeInfo) {
      if (updatedTabId === tabId && changeInfo.status === 'complete') {
        chrome.tabs.onUpdated.removeListener(injectListener);

        // Inject web-recorder.js into MAIN world
        chrome.scripting.executeScript({
          target: { tabId: tabId },
          files: ['src/content/web-recorder.js'],
          world: 'MAIN'
        }).then(() => {
          backgroundLogger.log(`[Watcher] Web recorder injected into tab ${tabId}`);

          // Send start recording command
          chrome.scripting.executeScript({
            target: { tabId: tabId },
            func: () => {
              window.dispatchEvent(new Event('GLUON_WATCHER_START'));
            },
            world: 'MAIN'
          });

          isWatcherActive = true;

          // Notify sidebar
          chrome.runtime.sendMessage({
            type: 'watcher_status_changed',
            isRecording: true
          });

          backgroundLogger.log(`[Watcher] Recording started on tab ${tabId}`);
        }).catch(error => {
          backgroundLogger.error(`[Watcher] Failed to inject recorder:`, error);
          chrome.runtime.sendMessage({
            type: 'error',
            message: 'Failed to inject recorder script. The page may not support instrumentation.'
          });
        });
      }
    });

    return { success: true };
  } catch (error) {
    backgroundLogger.error(`[Watcher] Error starting watcher:`, error);
    return { success: false, error: error.message };
  }
}

async function stopWatcher(tabId) {
  try {
    backgroundLogger.log(`[Watcher] Stopping for tab ${tabId}`);

    // If watcher isn't active, just cleanup and return
    if (!isWatcherActive) {
      backgroundLogger.log(`[Watcher] Watcher not active, skipping script injection`);
      return { success: true };
    }

    // Get tab info to check if URL is accessible
    const tab = await chrome.tabs.get(tabId).catch(() => null);
    if (!tab) {
      backgroundLogger.warn(`[Watcher] Tab ${tabId} no longer exists`);
      return { success: true };
    }

    // Check if the URL is a protected URL
    const protectedUrlPatterns = [
      /^chrome:\/\//,
      /^chrome-extension:\/\//,
      /^edge:\/\//,
      /^about:/,
      /^devtools:\/\//,
      /^view-source:/,
      /^chrome-search:\/\//
    ];

    const isProtectedUrl = protectedUrlPatterns.some(pattern => pattern.test(tab.url));
    if (isProtectedUrl) {
      backgroundLogger.warn(`[Watcher] Cannot inject into protected URL, skipping data collection`);
      return { success: true };
    }

    // Inject a bridge function that will listen for postMessage and forward to extension
    await chrome.scripting.executeScript({
      target: { tabId: tabId },
      func: () => {
        // Set up listener for postMessage from MAIN world
        window.addEventListener('message', (event) => {
          if (event.data && event.data.type === 'GLUON_WATCHER_DATA') {
            // Forward to background script
            chrome.runtime.sendMessage({
              type: 'watcher_data_from_page',
              data: event.data
            });
          }
        }, { once: true }); // Only listen once
      },
      world: 'ISOLATED' // This runs in content script context
    });

    // Now trigger the flush in MAIN world
    await chrome.scripting.executeScript({
      target: { tabId: tabId },
      func: () => {
        window.dispatchEvent(new Event('GLUON_WATCHER_FLUSH'));
      },
      world: 'MAIN'
    });

    backgroundLogger.log(`[Watcher] Flush command sent, waiting for data...`);

    // Return immediately - the data will be handled by the watcher_data_from_page message handler
    return { success: true };
  } catch (error) {
    backgroundLogger.error(`[Watcher] Error stopping watcher:`, error);
    return { success: false, error: error.message };
  } finally {
    // Use centralized cleanup function
    cleanupWatcher();
  }
}

function formatWatcherLogs(events) {
  if (!events || events.length === 0) {
    return '# Browser Watcher Log\n\nNo events recorded.\n';
  }

  let output = '# Browser Watcher Log - AI Analysis Ready\n\n';
  output += `## Session Summary\n`;
  output += `- Total Events: ${events.length}\n`;
  output += `- Generated: ${new Date().toISOString()}\n`;

  // Count event types
  const consoleCount = events.filter(e => e.type === 'console').length;
  const networkCount = events.filter(e => e.type === 'fetch' || e.type === 'xhr').length;
  const errorCount = events.filter(e => e.type === 'console' && e.level === 'error').length;

  output += `- Console Events: ${consoleCount} (${errorCount} errors)\n`;
  output += `- Network Events: ${networkCount}\n\n`;
  output += '=' .repeat(80) + '\n\n';
  output += '## Event Timeline\n\n';

  events.forEach((event, index) => {
    const timestamp = new Date(event.timestamp).toLocaleTimeString();
    const eventNum = String(index + 1).padStart(4, '0');

    if (event.type === 'console') {
      const level = event.level.toUpperCase().padEnd(5);
      output += `[${eventNum}] [${timestamp}] [CONSOLE] [${level}]\n`;
      output += `${'-'.repeat(80)}\n`;
      output += `${event.message}\n`;
      if (event.url) {
        output += `Source: ${event.url}\n`;
      }
      output += '\n';
    } else if (event.type === 'fetch' || event.type === 'xhr') {
      const networkType = event.type === 'fetch' ? 'FETCH' : 'XHR';
      const statusIndicator = event.status >= 400 ? '❌' : event.status >= 300 ? '⚠️' : '✅';

      output += `[${eventNum}] [${timestamp}] [NETWORK] [${networkType}]\n`;
      output += `${'-'.repeat(80)}\n`;
      output += `${event.method} ${event.url}\n`;

      if (event.status) {
        output += `Status: ${statusIndicator} ${event.status} ${event.statusText || ''}\n`;
      }

      if (event.error) {
        output += `Error: ❌ ${event.error}\n`;
      }

      if (event.duration !== undefined) {
        output += `Duration: ${event.duration}ms\n`;
      }

      output += '\n';
    }
  });

  output += '=' .repeat(80) + '\n';
  output += '## Analysis Instructions\n\n';
  output += 'This log contains browser console outputs and network requests captured during user interaction.\n';
  output += 'Look for:\n';
  output += '- Error patterns and their root causes\n';
  output += '- Failed network requests (4xx, 5xx status codes)\n';
  output += '- Performance issues (slow network requests)\n';
  output += '- Console warnings that might indicate bugs\n';

  return output;
}

// ============================================================================
// MCP (Model Context Protocol) Handler - Integration with Gluon V3
// ============================================================================

/**
 * Sends MCP status update to sidebar and active tabs
 * @param {string} status - Status message (e.g., "Starting semantic_search...")
 * @param {string} toolName - Tool name for context
 * @param {boolean} isError - Whether this is an error status
 */
function broadcastMcpStatus(status, toolName = '', isError = false) {
    backgroundLogger.log(`[MCP Status] ${status}`, { tool: toolName, error: isError });

    // Send to sidebar
    chrome.runtime.sendMessage({
        type: 'mcp_status_update',
        payload: { status, toolName, isError, timestamp: Date.now() }
    }).catch(() => {
        // Sidebar might be closed, ignore error
    });

    // Send to all active tabs (for badge/visual feedback)
    chrome.tabs.query({}, (tabs) => {
        tabs.forEach(tab => {
            chrome.tabs.sendMessage(tab.id, {
                action: 'mcp_status_update',
                payload: { status, toolName, isError, timestamp: Date.now() }
            }).catch(() => {
                // Tab might not have content script
            });
        });
    });
}

/**
 * Executes MCP tool calls on the Gluon V3 server (localhost:3001)
 * @param {Array} mcpCalls - Array of MCP tool calls: [{tool: "name", args: {...}}, ...]
 * @returns {Promise<Array>} - Array of results from each tool call
 */
async function executeMcpCalls(mcpCalls) {
    if (!mcpCalls || !Array.isArray(mcpCalls) || mcpCalls.length === 0) {
        backgroundLogger.log('[MCP] No MCP calls to execute');
        return [];
    }

    const results = [];

    for (let i = 0; i < mcpCalls.length; i++) {
        const call = mcpCalls[i];
        try {
            const { tool, args } = call;
            if (!tool) {
                backgroundLogger.warn('[MCP] Skipping call with missing tool name:', call);
                continue;
            }

            // Broadcast starting status
            const totalTools = mcpCalls.length;
            const currentNum = i + 1;
            broadcastMcpStatus(`[${currentNum}/${totalTools}] Starting ${tool}...`, tool, false);
            backgroundLogger.log(`[MCP] Executing tool: ${tool}`, args);

            // Build JSON-RPC 2.0 request
            const mcpRequest = {
                jsonrpc: "2.0",
                method: "tools/call",
                params: {
                    name: tool,
                    arguments: args || {}
                },
                id: Date.now()
            };

            // Send to V3 server on localhost:3001
            // Note: fetch() doesn't support timeout natively — use AbortController
            // Timeout: 90 seconds (some tools like analyze_cpg need time for large codebases)
            const abortController = new AbortController();
            const TOOL_TIMEOUT_MS = 90000; // 90 seconds
            const timeoutId = setTimeout(() => abortController.abort(), TOOL_TIMEOUT_MS);

            let response;
            try {
                response = await fetch('http://localhost:3001/mcp', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(mcpRequest),
                    signal: abortController.signal
                });
            } finally {
                clearTimeout(timeoutId);
            }

            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            const data = await response.json();
            backgroundLogger.log(`[MCP] Tool result for ${tool}:`, data);

            // V3 returns JSON-RPC errors in data.error (protocol errors)
            // AND tool-level errors as data.result.isError === true (MCP spec)
            const toolResult = data.result;
            const isToolError = toolResult?.isError === true;
            const isProtocolError = !!data.error;

            if (isProtocolError || isToolError) {
                const errorMsg = isProtocolError
                    ? (data.error?.message || 'Protocol error')
                    : (toolResult?.content?.[0]?.text || 'Tool error');
                broadcastMcpStatus(`[${currentNum}/${totalTools}] ❌ ${tool} failed: ${errorMsg}`, tool, true);
            } else {
                broadcastMcpStatus(`[${currentNum}/${totalTools}] ✅ ${tool} completed`, tool, false);
            }

            results.push({
                tool: tool,
                success: !isProtocolError && !isToolError,
                result: toolResult || data,
                error: isProtocolError
                    ? (data.error?.message || 'Protocol error')
                    : (isToolError ? (toolResult?.content?.[0]?.text || 'Tool error') : null)
            });

        } catch (error) {
            const currentNum = i + 1;
            const totalTools = mcpCalls.length;
            const errorMsg = error.message.includes('abort')
                ? `timeout after 90 seconds`
                : error.message;

            broadcastMcpStatus(`[${currentNum}/${totalTools}] ❌ ${call.tool} error: ${errorMsg}`, call.tool, true);
            backgroundLogger.error(`[MCP] Error executing ${call.tool}:`, error);
            results.push({
                tool: call.tool,
                success: false,
                error: errorMsg || String(error)
            });
        }
    }

    backgroundLogger.log('[MCP] All tool calls completed:', results);
    return results;
}

/**
 * Checks if parsed JSON contains MCP calls that need to be executed
 * @param {Object} jsonObject - Parsed JSON from model response
 * @returns {Array|null} - Array of MCP calls or null if none exist
 */
function extractMcpCalls(jsonObject) {
    if (!jsonObject) return null;

    // Format G-Interactive: @gluon:next_step.context_ops.mcp_calls
    const nextStep = jsonObject['@gluon:next_step'];
    if (nextStep?.context_ops?.mcp_calls && Array.isArray(nextStep.context_ops.mcp_calls)) {
        return nextStep.context_ops.mcp_calls;
    }

    // Format G-SOP: gluon_actions.context_ops.mcp_calls
    const contextOps = jsonObject.gluon_actions?.context_ops;
    if (contextOps?.mcp_calls && Array.isArray(contextOps.mcp_calls)) {
        return contextOps.mcp_calls;
    }

    return null;
}

/**
 * Formats MCP results into a prompt to send back to the model
 * @param {Array} mcpResults - Results from executeMcpCalls
 * @returns {string} - Formatted prompt for the model
 */
function formatMcpResultsAsPrompt(mcpResults) {
    let prompt = '[🔧 MCP Tool Results]\n\n';

    for (const result of mcpResults) {
        prompt += `**Tool:** ${result.tool}\n`;
        if (result.success) {
            // V3 returns JSON-RPC result: { content: [{ type: "text", text: "..." }] }
            let resultText;
            if (result.result?.content && Array.isArray(result.result.content)) {
                // Extract text from MCP ToolContent array
                resultText = result.result.content
                    .filter(c => c.type === 'text')
                    .map(c => c.text)
                    .join('\n');
            } else if (typeof result.result === 'string') {
                resultText = result.result;
            } else {
                resultText = JSON.stringify(result.result, null, 2);
            }
            prompt += `**Result:**\n\`\`\`\n${resultText}\n\`\`\`\n\n`;
        } else {
            prompt += `**Error:** ${result.error}\n\n`;
        }
    }

    prompt += '\nZastosuj powyższe wyniki do kontynuacji pracy nad zadaniem.';
    return prompt;
}

// ============================================================================
// Fallback Overlay Generator (Background Mode)
// ============================================================================
function generateAndSendFallbackOverlay(payload, tabId) {
    try {
        const rawText = payload.rawText;

        let jsonObject;
        const trimmedText = rawText.trim();
        if (trimmedText.startsWith('{')) {
            jsonObject = JSON.parse(trimmedText);
        } else {
            const jsonMatch = rawText.match(/```(?:json)?\s*([\s\S]+?)\s*```/);
            if (jsonMatch) jsonObject = JSON.parse(jsonMatch[1]);
            else {
                const deepMatch = rawText.match(/(\{[\s\S]*\})/);
                if (deepMatch) jsonObject = JSON.parse(deepMatch[1]);
            }
        }

        // Support both G-SOP (gluon_actions) and G-Interactive (@gluon:next_step) formats
        const nextStep = jsonObject['@gluon:next_step'];
        const isGInteractive = !!nextStep;
        const isGSop = !!jsonObject.gluon_actions || !!jsonObject.thought_process;

        if (!jsonObject || (!isGSop && !isGInteractive)) {
            return;
        }

        const overlayData = {
            file_changes: jsonObject.gluon_actions?.file_changes || [],
            context_ops: jsonObject.gluon_actions?.context_ops || nextStep?.context_ops || null,
            thought_process: jsonObject.thought_process || nextStep?.reasoning,
            user_message: jsonObject.user_message || (isGInteractive ? `[G-Interactive] action: ${nextStep?.action}` : null)
        };

        const escapeHTML = (str) => (str || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
        const formatMessage = (str) => escapeHTML(str).replace(/\n/g, '<br>');

        const generateSmartDiffPreview = (search, replace) => {
            if (!search || !replace) return '<div style="color: #6e7681; font-style: italic;">No preview available</div>';

            const searchLines = search.split('\n');
            const replaceLines = replace.split('\n');

            let html = '<div class="gluon-diff-container" style="border: 1px solid #30363d; border-radius: 6px; background: #0d1117; max-height: 400px; overflow: auto; width: 100%;">';

            searchLines.forEach((line, i) => {
                const lineNum = i + 1;
                html += `
                    <div class="diff-row" style="display: table; width: 100%; table-layout: fixed; border-collapse: collapse;">
                        <div class="diff-num" style="display: table-cell; width: 50px; text-align: right; padding-right: 8px; color: #6e7681; background: rgba(248, 81, 73, 0.15); border-right: 1px solid #30363d; user-select: none; opacity: 0.7; font-size: 11px; line-height: 18px; vertical-align: top;">${lineNum}</div>
                        <div class="diff-line" style="display: table-cell; font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; font-size: 12px; line-height: 18px; white-space: pre-wrap; word-break: break-all; padding: 0 8px; color: #ff7b72; background: rgba(248, 81, 73, 0.15);"><span style="user-select: none; margin-right: 8px; opacity: 0.8;">-</span>${escapeHTML(line)}</div>
                    </div>`;
            });

            replaceLines.forEach((line, i) => {
                const lineNum = i + 1;
                html += `
                    <div class="diff-row" style="display: table; width: 100%; table-layout: fixed; border-collapse: collapse;">
                        <div class="diff-num" style="display: table-cell; width: 50px; text-align: right; padding-right: 8px; color: #6e7681; background: rgba(46, 160, 67, 0.15); border-right: 1px solid #30363d; user-select: none; opacity: 0.7; font-size: 11px; line-height: 18px; vertical-align: top;">${lineNum}</div>
                        <div class="diff-line" style="display: table-cell; font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; font-size: 12px; line-height: 18px; white-space: pre-wrap; word-break: break-all; padding: 0 8px; color: #3fb950; background: rgba(46, 160, 67, 0.15);"><span style="user-select: none; margin-right: 8px; opacity: 0.8;">+</span>${escapeHTML(line)}</div>
                    </div>`;
            });

            html += '</div>';
            return html;
        };

        const changesCount = overlayData.file_changes.length;
        const contextOpsList = overlayData.context_ops?.load || (Array.isArray(overlayData.context_ops) ? overlayData.context_ops : []);
        const opsCount = contextOpsList.length;

        let actionsHtml = '';
        if (changesCount > 0) {
            const fileCards = overlayData.file_changes.map((c, index) => `
                <div class="gluon-change-card" style="margin-bottom: 12px; background: #161b22; border: 1px solid #30363d; border-radius: 6px; overflow: hidden; max-width: 100%;">
                    <div class="change-header" style="padding: 8px 12px; background: #21262d; border-bottom: 1px solid #30363d; display: flex; align-items: center; justify-content: space-between;">
                        <div style="display: flex; align-items: center; gap: 8px; color: #e6edf3; font-size: 14px; font-weight: 600;">
                            <span>📝</span>
                            <span style="font-family: monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 400px;" title="${escapeHTML(c.file_path)}">${escapeHTML(c.file_path)}</span>
                        </div>
                        <span style="font-size: 12px; color: #8b949e; white-space: nowrap;">Modification #${index + 1}</span>
                    </div>
                    <div class="change-body" style="padding: 0; overflow: hidden; max-width: 100%;">
                        ${generateSmartDiffPreview(c.search_code, c.replace_code)}
                    </div>
                </div>
            `).join('');
            actionsHtml += `<div style="margin-top: 16px; border-left: 3px solid #00ff88; background: rgba(0, 255, 136, 0.05); padding: 12px; border-radius: 0 4px 4px 0;">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
                    <strong style="color: #00ff88; font-size: 15px;">⚡ Code Modifications (${changesCount})</strong>
                </div>
                ${fileCards}
            </div>`;
        }
        if (opsCount > 0) {
            actionsHtml += `<div style="margin-top: 10px; border-left: 3px solid #00d4ff; background: rgba(0, 212, 255, 0.05); padding: 12px; border-radius: 0 4px 4px 0;">
                <strong style="color: #00d4ff; font-size: 15px;">🧠 Context Operations (${opsCount})</strong>
                <ul style="margin: 5px 0 10px 15px; font-size: 14px; color: #cbd5e1;">
                    ${contextOpsList.map(op => {
                        const desc = op.path || op.symbol || op.query || 'unknown';
                        return `<li>📥 ${op.type}: ${escapeHTML(desc)}</li>`;
                    }).join('')}
                </ul>
            </div>`;
        }

        const html = `
        <style>
            .gluon-response-overlay.structured-output + pre,
            pre:has(+ .gluon-response-overlay.structured-output),
            pre:has(> .gluon-response-overlay.structured-output) {
                display: none !important;
            }
            .gluon-response-overlay.structured-output.show-original + pre,
            pre:has(+ .gluon-response-overlay.structured-output.show-original),
            pre:has(> .gluon-response-overlay.structured-output.show-original) {
                display: block !important;
            }
            .gluon-response-overlay.structured-output.show-original .gluon-overlay-expandable {
                display: none !important;
            }
            .gluon-response-overlay.structured-output {
                margin-top: -10px;
            }
        </style>
        <div class='gluon-response-overlay structured-output expanded'>
            <div class='gluon-overlay-header' style="background: linear-gradient(135deg, #1a2238, #242c44); padding: 10px; border-bottom: 1px solid #334155; display: flex; align-items: center; justify-content: space-between;">
                <div style="display: flex; align-items: center; gap: 8px;" class="gluon-header-main">
                    <span style="font-size: 16px;">🤖</span>
                    <span style="font-weight: 600; color: #e2e8f0;">Gluon Actions (Background Mode)</span>
                </div>
                <div style="display: flex; align-items: center; gap: 8px;">
                    <button class="gluon-btn-toggle-json" style="background: rgba(255,255,255,0.1); border: 1px solid rgba(255,255,255,0.2); color: #94a3b8; padding: 4px 8px; border-radius: 4px; cursor: pointer; font-size: 11px; transition: all 0.2s;" title="Show/Hide Original JSON">
                        📋 JSON
                    </button>
                    <span class="gluon-toggle-icon" style="cursor: pointer; color: #94a3b8;">▲</span>
                </div>
            </div>
            <div class='gluon-overlay-expandable' style="padding: 12px; background: #0d1117;">
                <div style="padding: 12px 0; font-size: 15px; line-height: 1.6; color: #e2e8f0; border-bottom: 1px solid #334155;">
                    ${formatMessage(overlayData.user_message)}
                </div>
                ${actionsHtml}
            </div>
            <div class='overlay-actions' style="padding: 10px; background: #161b22; border-top: 1px solid #30363d; display: flex; justify-content: flex-end; gap: 8px;">
                ${changesCount > 0 ? `
                <button class='gluon-btn-apply-changes' style="background: #238636; border: 1px solid rgba(240,246,252,0.1); color: white; padding: 6px 12px; border-radius: 6px; font-weight: 600; cursor: pointer;">
                    ⚡ Apply Changes
                </button>` : ''}
                ${opsCount > 0 ? `
                <button class='gluon-btn-load-context' style="background: #1f6feb; border: 1px solid rgba(240,246,252,0.1); color: white; padding: 6px 12px; border-radius: 6px; font-weight: 600; cursor: pointer;">
                    🧠 Load Context
                </button>` : ''}
                <button class='gluon-btn-ignore' style="background: transparent; border: 1px solid #30363d; color: #8b949e; padding: 6px 12px; border-radius: 6px; font-weight: 600; cursor: pointer;">
                    Dismiss
                </button>
            </div>
        </div>`;

        chrome.tabs.sendMessage(tabId, {
            action: 'render_gluon_overlay',
            payload: {
                messageId: payload.messageId,
                overlayHtml: html,
                overlayData: overlayData
            }
        });

    } catch (e) {
        backgroundLogger.error('Error generating fallback overlay', e);
    }
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  let requestId;
  switch (message.action) {
    case 'get_tab_id':
      // Return the tab ID to content scripts
      if (sender.tab && sender.tab.id) {
        sendResponse({ tabId: sender.tab.id });
      } else {
        sendResponse({ tabId: null });
      }
      return true; // Keep channel open for async response

    case 'get_projects':
      requestId = sendMessageToDesktop('get_projects');
      break;

    case 'process_dom_stream':
      backgroundLogger.log('📨 [Background] Received process_dom_stream from content script.');
      backgroundLogger.log(`   Provider: ${message.provider}, HTML length: ${message.html?.length}`);

      // [FIX] Używamy ID wygenerowanego przez frontend (jeśli podano), aby spiąć pętlę zdarzeń
      const customId = message.requestId || crypto.randomUUID();

      if (ws && ws.readyState === WebSocket.OPEN) {
          const msgObj = {
              id: customId,
              action: 'process_dom_stream',
              payload: {
                  html: message.html,
                  provider: message.provider
              }
          };

          // Ustawiamy timeout dla tego konkretnego ID
          const timeout = setTimeout(() => {
             pendingRequests.delete(customId);
          }, 30000);
          pendingRequests.set(customId, { timeout });

          ws.send(JSON.stringify(msgObj));
          requestId = customId; // Dla sendResponse

          backgroundLogger.log(`   ✅ Forwarded to Desktop with Linked Request ID: ${requestId}`);
      } else {
          backgroundLogger.error('   ❌ Failed to forward to Desktop (WebSocket closed)');
          requestId = null;
      }
      break;

    case 'cancel_processing':
      backgroundLogger.log('📨 [Background] Received cancel_processing request.');
      backgroundLogger.log(`   Request ID to cancel: ${message.requestId}`);

      if (ws && ws.readyState === WebSocket.OPEN) {
          const cancelMsg = {
              id: crypto.randomUUID(),
              action: 'cancel_processing',
              payload: {
                  request_id: message.requestId
              }
          };

          ws.send(JSON.stringify(cancelMsg));
          backgroundLogger.log(`   ✅ Sent cancel request to Desktop for: ${message.requestId}`);
          sendResponse({ success: true });
      } else {
          backgroundLogger.error('   ❌ Failed to send cancel (WebSocket closed)');
          sendResponse({ success: false, error: 'WebSocket not connected' });
      }
      return true; // Keep channel open for async response

    case 'select_files':
      // Przekaż komendę do sidebar (z content script)
      chrome.runtime.sendMessage({
        type: 'gluon_command_detected',
        action: 'select_files',
        files: message.files,
        clearPrevious: message.clearPrevious !== false
      });
      sendResponse({ success: true });
      break;
      
    case 'get_file_trees':
      requestId = sendMessageToDesktop('get_file_trees', { paths: message.payload.paths });
      break;
    case 'get_files_multi':
      requestId = sendMessageToDesktop('get_files_multi', { projects: message.payload.projects });
      break;
    case 'generate_context_file':
      requestId = sendMessageToDesktop('generate_context_file', message.payload);
      if (sendResponse) {
        sendResponse({ request_id: requestId });
      }
      break;
    case 'save_semantic_map':
      requestId = sendMessageToDesktop('save_semantic_map', {
        filename: message.payload.filename,
        content: message.payload.content,
        projectRoot: message.payload.projectRoot
      });
      pendingRequests.set(requestId, { sendResponse, command: 'save_semantic_map' });
      return true; // Keep sendResponse alive for async response
    case 'cancel_context_generation':
      requestId = sendMessageToDesktop('cancel_context_generation', {
        request_id: message.payload.request_id
      });
      break;
    case 'get_environments':
      requestId = sendMessageToDesktop('get_environments');
      break;
    case 'get_environment_for_project':
      requestId = sendMessageToDesktop('get_environment_for_project', { path: message.payload.path });
      break;
    case 'toggle_local_ai':
      requestId = sendMessageToDesktop('toggle_local_ai', {
        enabled: message.payload.enabled,
        skip_auto_index: message.payload.skip_auto_index
      });
      break;

    case 'get_local_ai_status':
      requestId = sendMessageToDesktop('get_local_ai_status');
      break;

    case 'trigger_indexing':
      requestId = sendMessageToDesktop('trigger_indexing', {
        selectedFiles: message.payload.selectedFiles
      });
      break;

    case 'list_embedding_models':
      requestId = sendMessageToDesktop('list_embedding_models');
      break;

    case 'set_embedding_model':
      requestId = sendMessageToDesktop('set_embedding_model', { model_filename: message.payload.model_filename });
      break;

    case 'switch_ai_model':
      requestId = sendMessageToDesktop('switch_ai_model', { model: message.payload.model });
      break;
      case 'get_context_files_history':
      requestId = sendMessageToDesktop('get_context_files_history', message.payload || {});
      break;
    case 'get_context_file_content':
      requestId = sendMessageToDesktop('get_context_file_content', { filepath: message.payload.filepath });
      // Zapamiętaj filename dla późniejszego użycia
      if (requestId) {
        pendingFileAttachments.set(requestId, message.payload.filename);
      }
      break;
    case 'get_binary_file_for_upload':
      backgroundLogger.log('Received "get_binary_file_for_upload". Forwarding to desktop...', message.payload); // <-- DODAJ TEN LOG
      requestId = sendMessageToDesktop('get_file_as_base64', { filepath: message.payload.filepath });
      break;
    case 'toggle_context_favorite':
      requestId = sendMessageToDesktop('toggle_context_favorite', {
        filepath: message.payload.filepath,
        favorite: message.payload.favorite
      });
      break;
      
    case 'rename_context_file':
      requestId = sendMessageToDesktop('rename_context_file', {
        filepath: message.payload.filepath,
        newName: message.payload.newName
      });
      break;

    case 'inject_file_to_gemini':
      // Forward file data to content script on active tab
      chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
        if (!tabs || tabs.length === 0) {
          chrome.runtime.sendMessage({
            type: 'error',
            message: 'No active tab found'
          });
          return;
        }

        const activeTab = tabs[0];

        // Check if we're on supported AI platform
        const supportedPlatforms = [
          'gemini.google.com',
          'aistudio.google.com',
          'claude.ai'
        ];

        const isSupported = supportedPlatforms.some(platform =>
          activeTab.url?.includes(platform)
        );

        if (!isSupported) {
          chrome.runtime.sendMessage({
            type: 'error',
            message: 'Please navigate to Claude, Gemini, or AI Studio first'
          });
          return;
        }

        // Send message to content script
        chrome.tabs.sendMessage(activeTab.id, {
          action: 'upload_file_to_gemini',
          file: message.payload
        }, (response) => {
          if (chrome.runtime.lastError) {
            backgroundLogger.error('Failed to inject file:', chrome.runtime.lastError);
            chrome.runtime.sendMessage({ 
              type: 'error', 
              message: 'Content script not loaded. Please refresh the AI Studio page.' 
            });
          } else if (response && !response.success) {
            chrome.runtime.sendMessage({ 
              type: 'error', 
              message: response.error 
            });
          }
        });
      });
      return true; // Keep channel open for async chrome.tabs.query

    case 'delete_attachment':
      // Forward delete request to content script on active tab
      chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
        if (!tabs || tabs.length === 0) {
          backgroundLogger.error('No active tab found for delete_attachment');
          return;
        }

        const activeTab = tabs[0];

        // Send message to content script
        chrome.tabs.sendMessage(activeTab.id, {
          action: 'delete_attachment',
          payload: message.payload
        }, (response) => {
          if (chrome.runtime.lastError) {
            backgroundLogger.error('Failed to delete attachment:', chrome.runtime.lastError);
          } else if (response && response.success) {
            backgroundLogger.log('✅ Attachment deleted successfully');
          }
        });
      });
      return true; // Keep channel open for async chrome.tabs.query

    case 'resolve_change_locations':
      requestId = sendMessageToDesktop('resolve_change_locations', message.payload);
      break;
    case 'create_debug_snapshot':
      // Forward debug request to desktop via WebSocket (Fire & Forget)
      if (ws && ws.readyState === WebSocket.OPEN) {
          const msgObj = {
              id: crypto.randomUUID(),
              action: 'create_debug_snapshot',
              payload: message.payload
          };
          ws.send(JSON.stringify(msgObj));
          backgroundLogger.log(`[Debug] Sent snapshot request for ${message.payload.change_id}`);
      } else {
          backgroundLogger.error('[Debug] WS Disconnected, cannot save snapshot');
      }
      break;
    case 'paste_prompt':
      chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
        if (tabs && tabs.length > 0) {
          const activeTab = tabs[0];
          
          // CRITICAL STEP: First, ensure the tab is focused before sending the message.
          chrome.tabs.update(activeTab.id, { active: true }, () => {
            // After focusing the tab, give it a moment, then send the message to paste.
            setTimeout(() => {
              chrome.tabs.sendMessage(activeTab.id, {
                action: 'paste_prompt_to_input',
                payload: message.payload
              }, (response) => {
                if (chrome.runtime.lastError) {
                  backgroundLogger.error('Paste prompt error:', chrome.runtime.lastError.message);
                  chrome.runtime.sendMessage({ type: 'error', message: 'Failed to communicate with the page. Please refresh.' });
                } else if (response && !response.success) {
                  chrome.runtime.sendMessage({ type: 'error', message: response.error });
                }
              });
            }, 150); // A small delay to ensure focus is registered by the browser.
          });
        } else {
          chrome.runtime.sendMessage({ type: 'error', message: 'No active tab found.' });
        }
      });
      return true; // Keep channel open for async chrome.tabs.query
    case 'gluon_response_detected':
      // Forward from content script to sidebar
      // [FIX] Store sender tab ID for overlay routing
      const detectionTabId = sender?.tab?.id;
      backgroundLogger.log(`📨 [BG] gluon_response_detected from tab ${detectionTabId}`);

      // [NEW] Check for MCP calls before sending to sidebar
      (async () => {
        try {
          const rawText = message.payload.rawText;
          let jsonObject;
          const trimmedText = rawText.trim();

          // Parse JSON from response
          if (trimmedText.startsWith('{')) {
            jsonObject = JSON.parse(trimmedText);
          } else {
            const jsonMatch = rawText.match(/```(?:json)?\s*([\s\S]+?)\s*```/);
            if (jsonMatch) jsonObject = JSON.parse(jsonMatch[1]);
            else {
              const deepMatch = rawText.match(/(\{[\s\S]*\})/);
              if (deepMatch) jsonObject = JSON.parse(deepMatch[1]);
            }
          }

          // Extract and execute MCP calls if present
          const mcpCalls = extractMcpCalls(jsonObject);
          if (mcpCalls && mcpCalls.length > 0) {
            backgroundLogger.log(`[MCP] Detected ${mcpCalls.length} MCP calls. Executing...`);

            const mcpResults = await executeMcpCalls(mcpCalls);

            // Attach MCP results to payload for sidebar
            message.payload.mcpResults = mcpResults;
            message.payload.hasMcpCalls = true;

            backgroundLogger.log('[MCP] Results attached to payload');
          }
        } catch (error) {
          backgroundLogger.warn('[MCP] Error checking/executing MCP calls:', error);
          // Continue anyway - MCP is optional
        }

        // Add tab ID to payload so sidebar knows where to send the overlay
        const payloadWithTab = {
          ...message.payload,
          sourceTabId: detectionTabId
        };

        chrome.runtime.sendMessage({ type: 'gluon_response_detected', payload: payloadWithTab }, (response) => {
          if (chrome.runtime.lastError) {
            // [FIX] Ignore "Receiving end does not exist" error when Sidebar is closed
            const errMsg = chrome.runtime.lastError.message;
            if (!errMsg.includes("Receiving end does not exist") && !errMsg.includes("The message port closed")) {
               backgroundLogger.error('❌ [BG] Failed to send to sidebar:', errMsg);
            } else {
               // Sidebar is closed! Generate fallback overlay.
               backgroundLogger.warn('⚠️ [BG] Sidebar is closed! Generating fallback overlay for G-SOP.');
               generateAndSendFallbackOverlay(payloadWithTab, detectionTabId);
            }
          }
        });
      })();
      break;
    case 'render_gluon_overlay':
      // Forward from sidebar to content script
      // [FIX] Use sourceTabId from payload instead of active tab
      const targetTabId = message.payload?.sourceTabId;

      if (targetTabId) {
        backgroundLogger.log(`📤 [BG] Sending overlay to tab ${targetTabId}`);
        chrome.tabs.sendMessage(targetTabId, { action: 'render_gluon_overlay', payload: message.payload }, (response) => {
          if (chrome.runtime.lastError) {
            backgroundLogger.warn(`⚠️ [BG] Failed to send overlay to tab ${targetTabId}:`, chrome.runtime.lastError.message);
          } else {
            backgroundLogger.log(`✅ [BG] Overlay sent to tab ${targetTabId}`);
          }
        });
      } else {
        // Fallback to active tab if sourceTabId is not available
        backgroundLogger.warn('⚠️ [BG] No sourceTabId, falling back to active tab');
        chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
          if (tabs[0]) {
            chrome.tabs.sendMessage(tabs[0].id, { action: 'render_gluon_overlay', payload: message.payload });
          }
        });
      }
      return true; // Keep channel open for async chrome.tabs.query
    case 'apply_gluon_selection':
       // Forward from content script to sidebar
      chrome.runtime.sendMessage({ type: 'apply_gluon_selection', payload: message.payload });
      break;
    case 'execute_interactive_context':
       // [FIX] Forward interactive context request to sidebar
       backgroundLogger.log('🧠 [BG] Forwarding execute_interactive_context to sidebar');
       // Inject source tab ID so sidebar can route result feedback back to the correct tab
       chrome.runtime.sendMessage({ type: 'execute_interactive_context', payload: { ...message.payload, _sourceTabId: sender.tab?.id } }, (response) => {
           if (chrome.runtime.lastError) {
               const errMsg = chrome.runtime.lastError.message;
               if (errMsg.includes("Receiving end does not exist") || errMsg.includes("The message port closed")) {
                   backgroundLogger.warn('⚠️ [BG] Sidebar is closed! Using Fallback Context Fetcher.');

                   const contextOps = message.payload?.next_step?.context_ops || message.payload?.context_ops;
                   let opsToLoad = [];
                   if (contextOps && Array.isArray(contextOps.load)) {
                       opsToLoad = contextOps.load;
                   } else if (Array.isArray(contextOps)) {
                       opsToLoad = contextOps;
                   }

                   if (opsToLoad.length > 0) {
                       const requestId = sendMessageToDesktop('execute_context_operations', {
                           operations: opsToLoad,
                           projectRoot: null
                       });

                       if (requestId && sender && sender.tab) {
                           pendingFallbackContexts.set(requestId, sender.tab.id);
                           backgroundLogger.log(`✅ [BG] Fallback context request sent to desktop: ${requestId}`);
                       } else {
                           backgroundLogger.error('❌ [BG] Failed to send fallback context request to desktop.');
                       }
                   } else {
                       backgroundLogger.warn('⚠️ [BG] No load operations found for fallback context fetch.');
                   }
               }
           }
       });
       sendResponse({ success: true }); // Prevent "Port closed" error
       break;

    case 'notify_interactive_context_done':
       // Route context loading result back to the originating content script tab
       if (message.payload?.sourceTabId) {
           chrome.tabs.sendMessage(message.payload.sourceTabId, {
               action: 'interactive_context_done',
               payload: message.payload
           }, () => { if (chrome.runtime.lastError) {} }); // Suppress errors
       }
       break;

    // [NEW] Interactive Context Handlers (Sidebar -> Rust)
    case 'get_precise_context':
       requestId = sendMessageToDesktop('get_precise_context', message.payload);
       // Store callback to resolve the Promise in sidebar
       pendingRequests.set(requestId, { sendResponse, command: 'get_precise_context' }); 
       return true; // Keep channel open for async response

    case 'rag_search':
       requestId = sendMessageToDesktop('rag_search', { query: message.query });
       pendingRequests.set(requestId, { sendResponse, command: 'rag_search' });
       return true;

    // [G-INTERACTIVE] Context Node - Execute batch context operations
    case 'execute_context_operations':
       backgroundLogger.log('🧠 [BG] Executing context operations:', message.payload);
       backgroundLogger.log('🧠 [BG] Operations count:', message.payload.operations?.length);
       backgroundLogger.log('🧠 [BG] Operations detail:', JSON.stringify(message.payload.operations));
       backgroundLogger.log('🧠 [BG] Project root:', message.payload.projectRoot);

       (async () => {
         const allOps = message.payload.operations || [];
         const ragOps = allOps.filter(op => op.type === 'rag_search' || op.type === 'semantic_search');
         const rustOps = allOps.filter(op => op.type !== 'rag_search' && op.type !== 'semantic_search');

         // Items accumulated from all sources (MCP v3 + Rust backend)
         const mergedItems = [];
         let successful = 0;
         let failed = 0;
         const fakeRequestId = `ctx_${Date.now()}`;

         // --- Route rag_search / semantic_search → Gluon v3 MCP ---
         if (ragOps.length > 0) {
           backgroundLogger.log(`[BG] Routing ${ragOps.length} search op(s) to Gluon v3 via MCP`);
           const mcpCalls = ragOps.map(op => ({
             tool: 'semantic_search',
             args: { query: op.query, top_k: op.top_k ?? 5 }
           }));

           const mcpResults = await executeMcpCalls(mcpCalls);

           mcpResults.forEach((mcpRes, idx) => {
             const query = ragOps[idx].query;
             if (mcpRes.success) {
               // v3 returns: result.content[0].text = JSON string with { results: [{content, file_path, line_range, ...}] }
               let chunks = [];
               try {
                 const rawText = mcpRes.result?.content?.[0]?.text;
                 const parsed = rawText ? JSON.parse(rawText) : null;
                 if (parsed?.results) {
                   chunks = parsed.results.map(r =>
                     `// ${r.file_path} (lines ${r.line_range})\n${r.content}`
                   );
                 }
               } catch (e) {
                 backgroundLogger.warn('[BG] Failed to parse semantic_search result:', e);
                 // Fallback: pass raw text as single chunk
                 const rawText = mcpRes.result?.content?.[0]?.text;
                 if (rawText) chunks = [rawText];
               }
               mergedItems.push({ type: 'rag_result', query, results: chunks });
               successful++;
             } else {
               mergedItems.push({ type: 'error', operation: 'rag_search', error: mcpRes.error || 'MCP v3 semantic_search failed' });
               failed++;
             }
           });
         }

         // --- Route remaining ops → Rust backend (v2) ---
         if (rustOps.length > 0) {
           backgroundLogger.log(`[BG] Routing ${rustOps.length} op(s) to Rust backend`);
           try {
             const rustResult = await new Promise((resolve, reject) => {
               const rid = sendMessageToDesktop('execute_context_operations', {
                 operations: rustOps,
                 projectRoot: message.payload.projectRoot || null
               });

               const listener = (msg) => {
                 if (msg.type === 'execute_context_operations_response' && msg.request_id === rid) {
                   chrome.runtime.onMessage.removeListener(listener);
                   if (msg.data.error) reject(new Error(msg.data.error));
                   else resolve(msg.data);
                 }
               };
               chrome.runtime.onMessage.addListener(listener);
               setTimeout(() => {
                 chrome.runtime.onMessage.removeListener(listener);
                 reject(new Error('Timeout waiting for Rust backend'));
               }, 30000);
             });

             mergedItems.push(...(rustResult.items || []));
             successful += rustResult.successful || 0;
             failed += rustResult.failed || 0;
           } catch (err) {
             backgroundLogger.error('[BG] Rust backend error:', err);
             rustOps.forEach(op => {
               mergedItems.push({ type: 'error', operation: op.type, error: err.message });
               failed++;
             });
           }
         }

         // Broadcast merged response (same format context-node.js expects)
         chrome.runtime.sendMessage({
           type: 'execute_context_operations_response',
           data: {
             request_id: fakeRequestId,
             items: mergedItems,
             successful,
             failed,
             total_operations: allOps.length
           }
         });
       })();
       break;

    // [SYMBOL PICKER] Get file symbols for preview UI
    case 'get_file_symbols':
       if (!ws || ws.readyState !== WebSocket.OPEN) {
         backgroundLogger.error('👁️ [SymbolPicker] Desktop app not connected');
         sendResponse({ success: false, error: 'Desktop app not connected' });
         return true;
       }

       backgroundLogger.log('👁️ [SymbolPicker] Requesting symbols for:', message.payload.file_path);

       const sidebarRequestId = message.request_id; // Capture sidebar's request ID
       requestId = sendMessageToDesktop('get_file_symbols', {
         file_path: message.payload.file_path,
         project_root: message.payload.project_root
       });

       // Setup timeout (15 seconds for symbol fetching)
       const symbolTimeout = setTimeout(() => {
         if (pendingRequests.has(requestId)) {
           backgroundLogger.error(`⏱️ [SymbolPicker] Request ${requestId} timed out after 15s`);
           pendingRequests.delete(requestId);
           chrome.runtime.sendMessage({
             type: 'get_file_symbols_response',
             request_id: sidebarRequestId,
             success: false,
             error: 'Timeout fetching symbols (15s)'
           });
         }
       }, 15000);

       // Store callback for async response
       pendingRequests.set(requestId, {
         action: 'get_file_symbols',
         sidebarRequestId: sidebarRequestId, // Store sidebar request ID
         timeout: symbolTimeout, // Store timeout reference so it can be cleared
         resolve: (data) => {
           backgroundLogger.log('👁️ [SymbolPicker] Received symbols:', data?.length || 0);
           chrome.runtime.sendMessage({
             type: 'get_file_symbols_response',
             request_id: sidebarRequestId, // Use sidebar's request ID
             success: true,
             symbols: data
           });
         },
         reject: (error) => {
           backgroundLogger.error('👁️ [SymbolPicker] Error:', error);
           chrome.runtime.sendMessage({
             type: 'get_file_symbols_response',
             request_id: sidebarRequestId, // Use sidebar's request ID
             success: false,
             error: error
           });
         }
       });

       return true; // Keep channel open for async response

    // [G-INTERACTIVE] Get Repo Skeleton - Lightweight project map
    case 'get_repo_skeleton':
       backgroundLogger.log('📂 [BG] Getting repo skeleton for:', message.payload.projectPath);
       requestId = sendMessageToDesktop('get_repo_skeleton', {
         projectPath: message.payload.projectPath
       });
       pendingRequests.set(requestId, { sendResponse, command: 'get_repo_skeleton' });
       return true; // Keep channel open for async response

    case 'search_file_in_tree':
      // Forward from content script to sidebar
      chrome.runtime.sendMessage({ type: 'search_file_in_tree', payload: message.payload });
      break;
    case 'init_connection':
      if (!ws || ws.readyState === WebSocket.CLOSED) {
        connect();
      } else {
        broadcastStatus();
      }
      setTimeout(() => {
        sendMessageToDesktop('get_projects');
        sendMessageToDesktop('get_environments');
      }, 250);
      break;
    case 'find_in_editor':
      backgroundLogger.log('Received "find_in_editor". Forwarding to desktop.', message.payload);
      requestId = sendMessageToDesktop('find_in_editor', {
          filePath: message.payload.filePath,
          searchText: message.payload.searchText,
      });
      break;

    case 'apply_code_changes':
      backgroundLogger.log('Received "apply_code_changes" from Apply System overlay. Forwarding to desktop...', message.payload);
      requestId = sendMessageToDesktop('apply_code_changes', {
          changes: message.payload.changes,
          selectedProjects: message.payload.selectedProjects || []
      });
      break;

    case 'undo_change':
      backgroundLogger.log('Received "undo_change" request. Forwarding to desktop...', message.payload);
      requestId = sendMessageToDesktop('undo_change', {
          changeId: message.payload.changeId,
          filePath: message.payload.filePath
      });
      break;

    case 'redo_change':
      backgroundLogger.log('Received "redo_change" request. Forwarding to desktop...', message.payload);
      requestId = sendMessageToDesktop('redo_change', {
          changeId: message.payload.changeId,
          filePath: message.payload.filePath
      });
      break;

    case 'gluon_mode_changed':
      // Broadcast to all content scripts (all tabs)
      chrome.tabs.query({}, (tabs) => {
        tabs.forEach(tab => {
          chrome.tabs.sendMessage(tab.id, {
            action: 'gluon_mode_changed',
            enabled: message.enabled
          }).catch(() => {
            // Ignore errors for tabs without content scripts
          });
        });
      });
      backgroundLogger.log(`Gluon mode changed: ${message.enabled}`);
      break;

    // Workflow Management Actions
    case 'workflow_get_graph':
      requestId = sendMessageToDesktop('workflow_get_graph');
      break;

    case 'workflow_add_agent':
      requestId = sendMessageToDesktop('workflow_add_agent', {
        name: message.name,
        output_wrapper: message.output_wrapper,
        agent_type: message.agent_type,
        position: message.position
      });
      break;

    case 'workflow_remove_agent':
      requestId = sendMessageToDesktop('workflow_remove_agent', {
        agent_id: message.agent_id
      });
      break;

    case 'workflow_update_agent':
      requestId = sendMessageToDesktop('workflow_update_agent', {
        agent_id: message.agent_id,
        name: message.name,
        output_wrapper: message.output_wrapper,
        system_prompt: message.system_prompt
      });
      break;

    case 'workflow_add_connection':
      requestId = sendMessageToDesktop('workflow_add_connection', {
        from_id: message.from_id,
        to_id: message.to_id,
        template: message.template
      });
      break;

    case 'workflow_remove_connection':
      requestId = sendMessageToDesktop('workflow_remove_connection', {
        from_id: message.from_id,
        to_id: message.to_id
      });
      break;

    // [FIX] Obsługa Auto-Apply (Frontend -> Backend)
    case 'workflow_auto_apply':
      requestId = sendMessageToDesktop('workflow_auto_apply', {
        agent_id: message.agent_id,
        content: message.content
      });
      break;

    case 'workflow_clear_auto_apply_queue':
        requestId = sendMessageToDesktop('workflow_clear_auto_apply_queue', {
            agent_id: message.agent_id
        });
        break;

    case 'workflow_set_auto_forward':
      requestId = sendMessageToDesktop('workflow_set_auto_forward', {
        enabled: message.enabled
      });
      break;

    case 'workflow_save_config':
      requestId = sendMessageToDesktop('workflow_save_config', {
        id: message.id,
        name: message.name,
        workflow: message.workflow
      });
      break;

    case 'workflow_get_saved_configs':
      requestId = sendMessageToDesktop('workflow_get_saved_configs');
      break;

    case 'workflow_delete_saved_config':
      requestId = sendMessageToDesktop('workflow_delete_saved_config', {
        id: message.id
      });
      break;

    // Agent Pairing Actions
    case 'agent_pair':
      requestId = sendMessageToDesktop('agent_register', {
        pairingCode: message.pairing_code
      });
      break;

    case 'agent_unpair':
      requestId = sendMessageToDesktop('agent_unregister');
      break;

    case 'agent_send_message':
      backgroundLogger.log('🔄 [BG] agent_send_message action received from AI Studio agent');
      backgroundLogger.log('🔄 [BG] Agent ID:', message.agentId);
      backgroundLogger.log('🔄 [BG] Content length:', message.content ? message.content.length : 'No content');
      backgroundLogger.log('🔄 [BG] Content (first 200 chars):', message.content ? message.content.substring(0, 200) : 'N/A');
      backgroundLogger.log('🔄 [BG] Sending to desktop via WebSocket...');

      requestId = sendMessageToDesktop('agent_response', {
        content: message.content,
        agentId: message.agentId  // Changed to camelCase for Rust backend
      });

      if (requestId) {
        backgroundLogger.log('✅ [BG] Message queued for desktop. Request ID:', requestId);
      } else {
        backgroundLogger.error('❌ [BG] Failed to send message to desktop (WebSocket not open?)');
      }
      break;

    case 'execute_mcp_tool': {
      const { tool, args } = message.payload || {};
      if (!tool) {
        sendResponse({ success: false, error: 'Missing tool name' });
        break;
      }

      (async () => {
        try {
          broadcastMcpStatus(`Starting ${tool}...`, tool, false);
          const mcpResults = await executeMcpCalls([{ tool, args: args || {} }]);

          // Wyślij wyniki z powrotem do sidebaru (nie do taba)
          sendResponse({ success: true, results: mcpResults });
        } catch (error) {
          backgroundLogger.error('[MCP Tools] Error:', error);
          sendResponse({ success: false, error: error.message });
        }
      })();

      return true; // keep message channel open for async sendResponse
    }

    case 'start_watcher':
      // Start Browser Watcher on active tab
      chrome.tabs.query({ active: true, currentWindow: true }, async (tabs) => {
        if (tabs && tabs.length > 0) {
          const result = await startWatcher(tabs[0].id);
          sendResponse(result);
        } else {
          sendResponse({ success: false, error: 'No active tab found' });
        }
      });
      return true;

    case 'stop_watcher':
      // Stop Browser Watcher
      if (watcherTabId) {
        stopWatcher(watcherTabId).then(result => {
          sendResponse(result);
        });
      } else {
        sendResponse({ success: false, error: 'No active watcher session' });
      }
      return true;

    case 'watcher_data_from_page':
      // Received data from the injected script via postMessage
      const { events, count } = message.data;
      backgroundLogger.log(`[Watcher] Received ${count} events from page`);

      // Format the logs into a readable text file
      const logText = formatWatcherLogs(events);
      const filename = `browser_logs_${Date.now()}.txt`;

      // Send to sidebar to attach as virtual file
      chrome.runtime.sendMessage({
        type: 'watcher_logs_ready',
        filename: filename,
        content: logText,
        eventCount: count
      });
      break;

    case 'tauri_command':
      // Handle Tauri commands for Google Drive integration
      // These commands need to be forwarded to the desktop app via WebSocket
      backgroundLogger.log('📨 [Background] Received tauri_command:', message.command);

      if (ws && ws.readyState === WebSocket.OPEN) {
        const requestId = crypto.randomUUID();
        const tauriMsg = {
          id: requestId,
          action: 'tauri_invoke',
          payload: {
            command: message.command,
            args: message.args || {}
          }
        };

        // Store the sendResponse callback so we can call it when the response arrives
        const timeout = setTimeout(() => {
          if (pendingRequests.has(requestId)) {
            backgroundLogger.error(`Tauri command ${message.command} timed out`);
            pendingRequests.delete(requestId);
            sendResponse({ error: 'Request timeout' });
          }
        }, 60000);  // 60 seconds (increased from 15s)

        pendingRequests.set(requestId, {
          timeout,
          sendResponse,
          command: message.command
        });

        ws.send(JSON.stringify(tauriMsg));
        backgroundLogger.log('✅ Forwarded Tauri command to desktop:', message.command);
      } else {
        backgroundLogger.error('❌ Cannot send Tauri command - WebSocket not connected');
        sendResponse({ error: 'Desktop app not connected' });
      }
      return true;

    case 'workflow_command':
      // Universal handler for workflow-manager-v2.js (Thin Client)
      // Routes all workflow commands to Rust backend
      backgroundLogger.log(`🔄 [Workflow V2] Received command: ${message.workflow_action}`);

      if (ws && ws.readyState === WebSocket.OPEN) {
        const workflowRequestId = crypto.randomUUID();
        const workflowMsg = {
          id: workflowRequestId,
          action: message.workflow_action,  // Forward nested workflow_action (e.g., 'workflow_get_agent_presets')
          payload: message.payload || {}
        };

        // Timeout for workflow commands
        const workflowTimeout = setTimeout(() => {
          if (pendingRequests.has(workflowRequestId)) {
            backgroundLogger.error(`⏱️ [Workflow V2] Command ${message.workflow_action} timed out`);
            pendingRequests.delete(workflowRequestId);
            sendResponse({ error: `Workflow command timeout: ${message.workflow_action}` });
          }
        }, 30000);  // 30 seconds

        // Store callback for async response
        pendingRequests.set(workflowRequestId, {
          timeout: workflowTimeout,
          sendResponse,
          command: message.workflow_action
        });

        ws.send(JSON.stringify(workflowMsg));
        backgroundLogger.log(`✅ [Workflow V2] Forwarded to Rust: ${message.workflow_action}`);
      } else {
        backgroundLogger.error('❌ [Workflow V2] Cannot send - WebSocket not connected');
        sendResponse({ error: 'Desktop app not connected. Please start Gluon Desktop.' });
      }
      return true;

    default:
      backgroundLogger.warn('Unknown action from extension:', message.action);
  }
  sendResponse(requestId);
  return true;
});

connect();

chrome.action.onClicked.addListener((tab) => {
  chrome.sidePanel.open({ windowId: tab.windowId });
});

setInterval(() => {
  if (ws && ws.readyState === WebSocket.OPEN) {
    // backgroundLogger.log('Connection alive');
  }
}, 30000);