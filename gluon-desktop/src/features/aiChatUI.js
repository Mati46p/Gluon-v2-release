// ============================================================================
// AI CHAT UI MODULE
// ============================================================================

const AiChatUI = (function() {
  'use strict';

  // ============================================================================
  // STATE
  // ============================================================================

  const state = {
    providers: [],
    sessions: [],
    currentSession: null,
    messages: [],
    apiKeyStatuses: {},
    streamingMessage: null,
    isLoading: false,
    modelSettings: {
      temperature: 0.7,
      maxTokens: 4096,
      topP: 0.9,
      frequencyPenalty: 0.0,
      presencePenalty: 0.0,
      model: ''
    }
  };

  // ============================================================================
  // INITIALIZATION
  // ============================================================================

  async function initialize() {
    console.log('[AiChat] Initializing AI Chat UI...');

    // Set up event listeners
    setupEventListeners();

    // Show welcome screen initially
    showWelcomeScreen();

    // Load initial data
    await loadProviders();
    await loadApiKeyStatuses();
    await loadSessions();

    console.log('[AiChat] Initialized successfully');
  }

  function setupEventListeners() {
    // New Chat button
    const newChatBtn = document.getElementById('new-chat-btn');
    if (newChatBtn) {
      newChatBtn.addEventListener('click', openNewChatModal);
    }

    // Provider filter
    const providerFilter = document.getElementById('chat-provider-filter');
    if (providerFilter) {
      providerFilter.addEventListener('change', handleProviderFilterChange);
    }

    // Send message button
    const sendBtn = document.getElementById('send-message-btn');
    if (sendBtn) {
      sendBtn.addEventListener('click', handleSendMessage);
    }

    // Chat input (Enter to send)
    const chatInput = document.getElementById('chat-input');
    if (chatInput) {
      chatInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault();
          handleSendMessage();
        }
      });
    }

    // Session action buttons
    const exportBtn = document.getElementById('export-chat-btn');
    const deleteBtn = document.getElementById('delete-chat-btn');
    const pinBtn = document.getElementById('pin-session-btn');
    const renameBtn = document.getElementById('rename-session-btn');

    if (exportBtn) exportBtn.addEventListener('click', exportCurrentSession);
    if (deleteBtn) deleteBtn.addEventListener('click', deleteCurrentSession);
    if (pinBtn) pinBtn.addEventListener('click', togglePinCurrentSession);
    if (renameBtn) renameBtn.addEventListener('click', renameCurrentSession);

    // API Key Modal
    const apiKeyModalCancel = document.getElementById('api-key-modal-cancel');
    const apiKeyModalSave = document.getElementById('api-key-modal-save');
    if (apiKeyModalCancel) {
      apiKeyModalCancel.addEventListener('click', closeApiKeyModal);
    }
    if (apiKeyModalSave) {
      apiKeyModalSave.addEventListener('click', handleSaveApiKey);
    }

    // New Chat Modal
    const newChatModalCancel = document.getElementById('new-chat-modal-cancel');
    const newChatModalCreate = document.getElementById('new-chat-modal-create');
    if (newChatModalCancel) {
      newChatModalCancel.addEventListener('click', closeNewChatModal);
    }
    if (newChatModalCreate) {
      newChatModalCreate.addEventListener('click', handleCreateSession);
    }

    // Listen for Tauri events (streaming)
    if (window.__TAURI__) {
      window.__TAURI__.event.listen('chat_stream_chunk', handleStreamChunk);
      window.__TAURI__.event.listen('chat_message_added', handleMessageAdded);
    }

    // Model settings controls
    setupModelSettingsListeners();
  }

  function setupModelSettingsListeners() {
    console.log('[AiChat] Setting up model settings listeners...');

    // Check if elements exist
    const modelPanel = document.querySelector('.model-settings-panel');
    console.log('[AiChat] Model settings panel found:', !!modelPanel);

    // Temperature slider
    const tempSlider = document.getElementById('temperature-slider');
    const tempValue = document.getElementById('temperature-value');
    console.log('[AiChat] Temperature slider found:', !!tempSlider);
    if (tempSlider && tempValue) {
      tempSlider.addEventListener('input', (e) => {
        const value = parseFloat(e.target.value);
        tempValue.textContent = value.toFixed(1);
        state.modelSettings.temperature = value;
      });
    }

    // Top P slider
    const topPSlider = document.getElementById('top-p-slider');
    const topPValue = document.getElementById('top-p-value');
    if (topPSlider && topPValue) {
      topPSlider.addEventListener('input', (e) => {
        const value = parseFloat(e.target.value);
        topPValue.textContent = value.toFixed(2);
        state.modelSettings.topP = value;
      });
    }

    // Frequency Penalty slider
    const freqSlider = document.getElementById('frequency-penalty-slider');
    const freqValue = document.getElementById('frequency-penalty-value');
    if (freqSlider && freqValue) {
      freqSlider.addEventListener('input', (e) => {
        const value = parseFloat(e.target.value);
        freqValue.textContent = value.toFixed(1);
        state.modelSettings.frequencyPenalty = value;
      });
    }

    // Presence Penalty slider
    const presSlider = document.getElementById('presence-penalty-slider');
    const presValue = document.getElementById('presence-penalty-value');
    if (presSlider && presValue) {
      presSlider.addEventListener('input', (e) => {
        const value = parseFloat(e.target.value);
        presValue.textContent = value.toFixed(1);
        state.modelSettings.presencePenalty = value;
      });
    }

    // Max Tokens input
    const maxTokensInput = document.getElementById('max-tokens-input');
    if (maxTokensInput) {
      maxTokensInput.addEventListener('change', (e) => {
        state.modelSettings.maxTokens = parseInt(e.target.value);
      });
    }

    // Model select
    const modelSelect = document.getElementById('model-select');
    if (modelSelect) {
      modelSelect.addEventListener('change', (e) => {
        state.modelSettings.model = e.target.value;
      });
    }

    // Save settings button
    const saveBtn = document.getElementById('save-model-settings-btn');
    if (saveBtn) {
      saveBtn.addEventListener('click', handleSaveModelSettings);
    }
  }

  // ============================================================================
  // PROVIDERS
  // ============================================================================

  async function loadProviders() {
    try {
      state.providers = await window.__TAURI__.invoke('get_ai_providers');
      console.log('[AiChat] Loaded providers:', state.providers);
      renderProviderFilter();
      renderProviderOptions();
    } catch (error) {
      console.error('[AiChat] Failed to load providers:', error);
      showToast('Failed to load AI providers', 'error');
    }
  }

  function renderProviderFilter() {
    const filterSelect = document.getElementById('chat-provider-filter');
    if (!filterSelect) return;

    filterSelect.innerHTML = '<option value="">All Providers</option>';
    state.providers.forEach(provider => {
      const option = document.createElement('option');
      option.value = provider.id;
      option.textContent = provider.name;
      filterSelect.appendChild(option);
    });
  }

  function renderProviderOptions() {
    const newChatSelect = document.getElementById('new-chat-provider');
    if (!newChatSelect) return;

    newChatSelect.innerHTML = '<option value="">Select provider...</option>';
    state.providers.forEach(provider => {
      const option = document.createElement('option');
      option.value = provider.id;
      option.textContent = `${getProviderIcon(provider.providerType)} ${provider.name}`;
      newChatSelect.appendChild(option);
    });
  }

  function getProviderIcon(providerType) {
    const icons = {
      'gemini': '🔮',
      'claude': '🧠',
      'gpt4': '🤖',
      'vscode': '💻'
    };
    return icons[providerType] || '💬';
  }

  // ============================================================================
  // API KEYS
  // ============================================================================

  async function loadApiKeyStatuses() {
    const providerTypes = ['gemini', 'claude', 'gpt4', 'vscode'];

    for (const type of providerTypes) {
      try {
        const status = await window.__TAURI__.invoke('get_api_key_status', {
          providerType: type
        });
        state.apiKeyStatuses[type] = status;
      } catch (error) {
        console.error(`[AiChat] Failed to load API key status for ${type}:`, error);
      }
    }

    renderApiKeysList();
  }

  function renderApiKeysList() {
    const apiKeysList = document.getElementById('api-keys-list');
    if (!apiKeysList) return;

    apiKeysList.innerHTML = '';

    state.providers.forEach(provider => {
      const status = state.apiKeyStatuses[provider.providerType];
      const item = document.createElement('div');
      item.className = 'api-key-item';
      item.innerHTML = `
        <div class="api-key-provider">
          <span class="provider-icon">${getProviderIcon(provider.providerType)}</span>
          <span>${provider.name}</span>
        </div>
        <span class="api-key-status ${status?.isConfigured ? 'configured' : 'not-set'}">
          ${status?.isConfigured ? '✓ Set' : '⚙️ Not Set'}
        </span>
      `;
      item.addEventListener('click', () => openApiKeyModal(provider.providerType, provider.name));
      apiKeysList.appendChild(item);
    });
  }

  function openApiKeyModal(providerType, providerName) {
    const modal = document.getElementById('api-key-modal');
    const providerNameEl = document.getElementById('api-key-modal-provider-name');
    const input = document.getElementById('api-key-input');

    if (!modal || !providerNameEl || !input) return;

    providerNameEl.textContent = providerName;
    modal.dataset.providerType = providerType;
    input.value = '';
    modal.style.display = 'flex';
  }

  function closeApiKeyModal() {
    const modal = document.getElementById('api-key-modal');
    if (modal) {
      modal.style.display = 'none';
    }
  }

  async function handleSaveApiKey() {
    const modal = document.getElementById('api-key-modal');
    const input = document.getElementById('api-key-input');

    if (!modal || !input) return;

    const providerType = modal.dataset.providerType;
    const apiKey = input.value.trim();

    if (!apiKey) {
      showToast('Please enter an API key', 'error');
      return;
    }

    try {
      await window.__TAURI__.invoke('set_ai_api_key', {
        payload: { providerType, apiKey }
      });

      showToast('API key saved successfully', 'success');
      closeApiKeyModal();
      await loadApiKeyStatuses();
    } catch (error) {
      console.error('[AiChat] Failed to save API key:', error);
      showToast(`Failed to save API key: ${error}`, 'error');
    }
  }

  // ============================================================================
  // SESSIONS
  // ============================================================================

  async function loadSessions(providerId = null) {
    try {
      state.sessions = await window.__TAURI__.invoke('get_chat_sessions', {
        providerId
      });
      console.log('[AiChat] Loaded sessions:', state.sessions);
      renderSessionsList();
    } catch (error) {
      console.error('[AiChat] Failed to load sessions:', error);
      showToast('Failed to load chat sessions', 'error');
    }
  }

  function renderSessionsList() {
    const sessionsList = document.getElementById('chat-sessions-list');
    if (!sessionsList) return;

    sessionsList.innerHTML = '';

    if (state.sessions.length === 0) {
      sessionsList.innerHTML = '<p class="placeholder">No conversations yet. Start a new chat!</p>';
      return;
    }

    state.sessions.forEach(session => {
      const item = document.createElement('div');
      item.className = 'chat-session-item';
      if (state.currentSession && state.currentSession.id === session.id) {
        item.classList.add('active');
      }

      const date = new Date(session.updatedAt).toLocaleDateString();

      item.innerHTML = `
        <div class="session-title">
          <span>${session.title}</span>
          ${session.isPinned ? '<span class="session-pin-icon">📌</span>' : ''}
        </div>
        <div class="session-meta">
          <span class="session-provider">${getProviderIcon(session.providerType)} ${session.providerName}</span>
          <span class="session-date">${date}</span>
        </div>
      `;

      item.addEventListener('click', () => selectSession(session));
      sessionsList.appendChild(item);
    });
  }

  async function selectSession(session) {
    state.currentSession = session;
    renderSessionsList();
    await loadMessages(session.id);
    loadModelSettings(session.id);
    updateSessionStats();
    showChatArea();
  }

  function handleProviderFilterChange(event) {
    const providerId = event.target.value ? parseInt(event.target.value) : null;
    loadSessions(providerId);
  }

  function openNewChatModal() {
    const modal = document.getElementById('new-chat-modal');
    const titleInput = document.getElementById('new-chat-title');
    const providerSelect = document.getElementById('new-chat-provider');

    if (!modal || !titleInput || !providerSelect) return;

    titleInput.value = '';
    providerSelect.value = '';
    modal.style.display = 'flex';
  }

  function closeNewChatModal() {
    const modal = document.getElementById('new-chat-modal');
    if (modal) {
      modal.style.display = 'none';
    }
  }

  async function handleCreateSession() {
    const providerSelect = document.getElementById('new-chat-provider');
    const titleInput = document.getElementById('new-chat-title');

    if (!providerSelect || !titleInput) return;

    const providerId = parseInt(providerSelect.value);
    const title = titleInput.value.trim() || null;

    if (!providerId) {
      showToast('Please select a provider', 'error');
      return;
    }

    try {
      const newSession = await window.__TAURI__.invoke('create_chat_session', {
        payload: { providerId, title }
      });

      console.log('[AiChat] Created session:', newSession);
      showToast('Chat session created', 'success');
      closeNewChatModal();

      await loadSessions();

      // Auto-select the new session
      const fullSession = state.sessions.find(s => s.id === newSession.id);
      if (fullSession) {
        await selectSession(fullSession);
      }
    } catch (error) {
      console.error('[AiChat] Failed to create session:', error);
      showToast(`Failed to create session: ${error}`, 'error');
    }
  }

  async function deleteCurrentSession() {
    if (!state.currentSession) return;

    const confirmed = confirm(`Delete "${state.currentSession.title}"?`);
    if (!confirmed) return;

    try {
      await window.__TAURI__.invoke('delete_chat_session', {
        sessionId: state.currentSession.id
      });

      showToast('Session deleted', 'success');
      state.currentSession = null;
      state.messages = [];
      await loadSessions();
      showWelcomeScreen();
    } catch (error) {
      console.error('[AiChat] Failed to delete session:', error);
      showToast(`Failed to delete session: ${error}`, 'error');
    }
  }

  async function togglePinCurrentSession() {
    if (!state.currentSession) return;

    try {
      const newStatus = await window.__TAURI__.invoke('toggle_session_pin', {
        sessionId: state.currentSession.id
      });

      state.currentSession.isPinned = newStatus;
      await loadSessions();
      showToast(newStatus ? 'Session pinned' : 'Session unpinned', 'success');
    } catch (error) {
      console.error('[AiChat] Failed to toggle pin:', error);
      showToast(`Failed to toggle pin: ${error}`, 'error');
    }
  }

  async function renameCurrentSession() {
    if (!state.currentSession) return;

    const newTitle = prompt('New title:', state.currentSession.title);
    if (!newTitle || newTitle === state.currentSession.title) return;

    try {
      await window.__TAURI__.invoke('rename_chat_session', {
        sessionId: state.currentSession.id,
        newTitle
      });

      state.currentSession.title = newTitle;
      await loadSessions();
      updateChatHeader();
      showToast('Session renamed', 'success');
    } catch (error) {
      console.error('[AiChat] Failed to rename session:', error);
      showToast(`Failed to rename session: ${error}`, 'error');
    }
  }

  async function exportCurrentSession() {
    if (!state.currentSession) return;

    try {
      const markdown = await window.__TAURI__.invoke('export_chat_session', {
        sessionId: state.currentSession.id
      });

      // Create download link
      const blob = new Blob([markdown], { type: 'text/markdown' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${state.currentSession.title.replace(/[^a-z0-9]/gi, '_')}.md`;
      a.click();
      URL.revokeObjectURL(url);

      showToast('Chat exported', 'success');
    } catch (error) {
      console.error('[AiChat] Failed to export session:', error);
      showToast(`Failed to export session: ${error}`, 'error');
    }
  }

  // ============================================================================
  // MESSAGES
  // ============================================================================

  async function loadMessages(sessionId) {
    try {
      state.messages = await window.__TAURI__.invoke('get_chat_messages', {
        sessionId
      });
      console.log('[AiChat] Loaded messages:', state.messages);
      renderMessages();
    } catch (error) {
      console.error('[AiChat] Failed to load messages:', error);
      showToast('Failed to load messages', 'error');
    }
  }

  function renderMessages() {
    const container = document.getElementById('chat-messages-container');
    if (!container) return;

    container.innerHTML = '';

    state.messages.forEach(message => {
      const messageEl = createMessageElement(message);
      container.appendChild(messageEl);
    });

    // Scroll to bottom
    container.scrollTop = container.scrollHeight;
  }

  function createMessageElement(message) {
    const div = document.createElement('div');
    div.className = `chat-message ${message.role}`;
    div.dataset.messageId = message.id;

    const avatar = message.role === 'user' ? '👤' : '🤖';
    const time = new Date(message.createdAt).toLocaleTimeString();

    // Action buttons (shown on hover)
    const actionButtons = message.role === 'assistant' ? `
      <div class="message-actions">
        <button class="message-action-btn copy-message-btn" title="Copy message">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
          </svg>
        </button>
      </div>
    ` : '';

    div.innerHTML = `
      ${actionButtons}
      <div class="message-avatar">${avatar}</div>
      <div class="message-content">
        <div class="message-bubble">${formatMessageContent(message.content)}</div>
        <div class="message-meta">
          <span>${time}</span>
          ${message.tokenCount > 0 ? `<span>${message.tokenCount} tokens</span>` : ''}
        </div>
      </div>
    `;

    return div;
  }

  function formatMessageContent(content) {
    if (!content) return '';

    // Configure marked.js if available
    if (window.marked) {
      marked.setOptions({
        highlight: function(code, lang) {
          if (window.hljs && lang && hljs.getLanguage(lang)) {
            try {
              return hljs.highlight(code, { language: lang }).value;
            } catch (err) {
              console.error('Highlight error:', err);
            }
          }
          return escapeHtml(code);
        },
        breaks: true,
        gfm: true
      });

      try {
        const html = marked.parse(content);

        // Add copy buttons to code blocks
        return addCopyButtonsToCodeBlocks(html);
      } catch (err) {
        console.error('Marked parsing error:', err);
        return escapeHtml(content);
      }
    }

    // Fallback to simple formatting if marked.js not available
    return simpleFallbackFormatting(content);
  }

  function addCopyButtonsToCodeBlocks(html) {
    // Create a temporary container to parse HTML
    const temp = document.createElement('div');
    temp.innerHTML = html;

    // Find all <pre><code> blocks and add copy buttons
    const codeBlocks = temp.querySelectorAll('pre > code');
    codeBlocks.forEach((codeEl, index) => {
      const pre = codeEl.parentElement;
      const wrapper = document.createElement('div');
      wrapper.className = 'code-block-wrapper';

      const header = document.createElement('div');
      header.className = 'code-block-header';

      // Detect language from class
      const langClass = Array.from(codeEl.classList).find(c => c.startsWith('language-'));
      const lang = langClass ? langClass.replace('language-', '') : 'text';

      header.innerHTML = `
        <span class="code-lang">${lang}</span>
        <button class="code-copy-btn" data-code-index="${index}" title="Copy code">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
          </svg>
          Copy
        </button>
      `;

      // Store code content as data attribute
      pre.dataset.codeContent = codeEl.textContent;

      wrapper.appendChild(header);
      pre.parentNode.insertBefore(wrapper, pre);
      wrapper.appendChild(pre);
    });

    return temp.innerHTML;
  }

  function simpleFallbackFormatting(content) {
    let formatted = content;

    // Code blocks
    formatted = formatted.replace(/```(\w+)?\n([\s\S]*?)```/g, (match, lang, code) => {
      return `<pre><code class="language-${lang || 'text'}">${escapeHtml(code.trim())}</code></pre>`;
    });

    // Inline code
    formatted = formatted.replace(/`([^`]+)`/g, '<code>$1</code>');

    // Bold
    formatted = formatted.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');

    // Italic
    formatted = formatted.replace(/\*([^*]+)\*/g, '<em>$1</em>');

    // Line breaks
    formatted = formatted.replace(/\n/g, '<br>');

    return formatted;
  }

  function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  // Handle copy button clicks with event delegation
  document.addEventListener('click', function(e) {
    // Copy code block button
    if (e.target.closest('.code-copy-btn')) {
      const btn = e.target.closest('.code-copy-btn');
      const wrapper = btn.closest('.code-block-wrapper');
      const pre = wrapper.querySelector('pre');
      const code = pre.dataset.codeContent;

      if (code) {
        navigator.clipboard.writeText(code).then(() => {
          // Visual feedback
          btn.innerHTML = `
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="20 6 9 17 4 12"></polyline>
            </svg>
            Copied!
          `;
          btn.classList.add('copied');

          setTimeout(() => {
            btn.innerHTML = `
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
              </svg>
              Copy
            `;
            btn.classList.remove('copied');
          }, 2000);
        }).catch(err => {
          console.error('Failed to copy:', err);
          showToast('Failed to copy code', 'error');
        });
      }
    }

    // Copy entire message button
    if (e.target.closest('.copy-message-btn')) {
      const btn = e.target.closest('.copy-message-btn');
      const messageEl = btn.closest('.chat-message');
      const messageId = messageEl.dataset.messageId;
      const message = state.messages.find(m => m.id == messageId);

      if (message) {
        navigator.clipboard.writeText(message.content).then(() => {
          showToast('Message copied', 'success');
          btn.innerHTML = `
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="20 6 9 17 4 12"></polyline>
            </svg>
          `;

          setTimeout(() => {
            btn.innerHTML = `
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
              </svg>
            `;
          }, 2000);
        }).catch(err => {
          console.error('Failed to copy message:', err);
          showToast('Failed to copy message', 'error');
        });
      }
    }
  });

  async function handleSendMessage() {
    if (!state.currentSession) {
      showToast('Please select or create a chat session', 'warning');
      return;
    }

    const input = document.getElementById('chat-input');
    const sendBtn = document.getElementById('send-message-btn');

    if (!input || !sendBtn) return;

    const content = input.value.trim();
    if (!content) return;

    // Disable input
    input.disabled = true;
    sendBtn.disabled = true;
    state.isLoading = true;

    try {
      // Send message with model settings (will trigger streaming events)
      await window.__TAURI__.invoke('send_chat_message', {
        payload: {
          sessionId: state.currentSession.id,
          content,
          modelSettings: state.modelSettings
        }
      });

      // Clear input
      input.value = '';
      input.style.height = 'auto';

    } catch (error) {
      console.error('[AiChat] Failed to send message:', error);
      showToast(`Failed to send message: ${error}`, 'error');

      // Re-enable input
      input.disabled = false;
      sendBtn.disabled = false;
      state.isLoading = false;
    }
  }

  function handleMessageAdded(event) {
    const message = event.payload;
    console.log('[AiChat] Message added:', message);

    // Add to state
    state.messages.push(message);

    // Render
    const container = document.getElementById('chat-messages-container');
    if (container) {
      const messageEl = createMessageElement(message);
      container.appendChild(messageEl);
      container.scrollTop = container.scrollHeight;
    }

    // If it's an assistant message, prepare for streaming
    if (message.role === 'assistant') {
      state.streamingMessage = {
        id: message.id,
        content: ''
      };
      createStreamingMessageElement();
    }
  }

  function createStreamingMessageElement() {
    const container = document.getElementById('chat-messages-container');
    if (!container) return;

    const div = document.createElement('div');
    div.className = 'chat-message assistant';
    div.id = 'streaming-message';

    div.innerHTML = `
      <div class="message-avatar">🤖</div>
      <div class="message-content">
        <div class="message-bubble message-streaming"></div>
        <div class="message-meta">
          <span>Streaming...</span>
        </div>
      </div>
    `;

    container.appendChild(div);
    container.scrollTop = container.scrollHeight;
  }

  function handleStreamChunk(event) {
    const chunk = event.payload;
    console.log('[AiChat] Stream chunk:', chunk);

    if (!state.streamingMessage) {
      state.streamingMessage = {
        id: chunk.messageId,
        content: ''
      };
      createStreamingMessageElement();
    }

    // Accumulate content
    state.streamingMessage.content += chunk.content;

    // Update UI
    const streamingEl = document.getElementById('streaming-message');
    if (streamingEl) {
      const bubble = streamingEl.querySelector('.message-bubble');
      if (bubble) {
        bubble.innerHTML = formatMessageContent(state.streamingMessage.content);

        if (chunk.isFinal) {
          bubble.classList.remove('message-streaming');
          const meta = streamingEl.querySelector('.message-meta span');
          if (meta) {
            meta.textContent = new Date().toLocaleTimeString();
          }

          // Reset state
          state.streamingMessage = null;
          streamingEl.id = '';

          // Re-enable input
          const input = document.getElementById('chat-input');
          const sendBtn = document.getElementById('send-message-btn');
          if (input) input.disabled = false;
          if (sendBtn) sendBtn.disabled = false;
          state.isLoading = false;

          // Reload session to update stats
          if (state.currentSession) {
            loadSessions().then(() => {
              const updated = state.sessions.find(s => s.id === state.currentSession.id);
              if (updated) {
                state.currentSession = updated;
                updateSessionStats();
              }
            });
          }
        }
      }
    }

    // Scroll to bottom
    const container = document.getElementById('chat-messages-container');
    if (container) {
      container.scrollTop = container.scrollHeight;
    }
  }

  // ============================================================================
  // UI HELPERS
  // ============================================================================

  function showWelcomeScreen() {
    const chatMessages = document.getElementById('chat-messages-container');
    const titleEl = document.getElementById('current-chat-title');
    const providerBadge = document.getElementById('current-chat-provider');
    const statsSection = document.getElementById('session-stats-section');

    if (titleEl) titleEl.textContent = 'Select a conversation';
    if (providerBadge) providerBadge.textContent = '';
    if (statsSection) statsSection.style.display = 'none';

    if (chatMessages) {
      chatMessages.innerHTML = `
        <div class="chat-welcome">
          <img src="assets/icons/icon_256.png" alt="Gluon" class="chat-welcome-logo">
          <h3>Welcome to Gluon AI Chat</h3>
          <p>Select a conversation or start a new one to begin</p>
        </div>
      `;
    }

    // Disable action buttons
    disableSessionActionButtons();
  }

  function disableSessionActionButtons() {
    const exportBtn = document.getElementById('export-chat-btn');
    const deleteBtn = document.getElementById('delete-chat-btn');
    const pinBtn = document.getElementById('pin-session-btn');
    const renameBtn = document.getElementById('rename-session-btn');
    const chatInput = document.getElementById('chat-input');
    const sendBtn = document.getElementById('send-message-btn');

    if (exportBtn) exportBtn.disabled = true;
    if (deleteBtn) deleteBtn.disabled = true;
    if (pinBtn) pinBtn.disabled = true;
    if (renameBtn) renameBtn.disabled = true;
    if (chatInput) chatInput.disabled = true;
    if (sendBtn) sendBtn.disabled = true;

    // Disable model settings controls
    disableModelSettingsControls();
  }

  function disableModelSettingsControls() {
    const controls = [
      'model-select',
      'temperature-slider',
      'max-tokens-input',
      'top-p-slider',
      'frequency-penalty-slider',
      'presence-penalty-slider',
      'save-model-settings-btn'
    ];

    controls.forEach(id => {
      const element = document.getElementById(id);
      if (element) element.disabled = true;
    });
  }

  function showChatArea() {
    updateChatHeader();
  }

  function updateChatHeader() {
    const titleEl = document.getElementById('current-chat-title');
    const providerBadge = document.getElementById('current-chat-provider');

    if (titleEl && state.currentSession) {
      titleEl.textContent = state.currentSession.title;
    }

    if (providerBadge && state.currentSession) {
      providerBadge.textContent = `${getProviderIcon(state.currentSession.providerType)} ${state.currentSession.providerName}`;
    }
  }

  function updateSessionStats() {
    if (!state.currentSession) return;

    const messagesCountEl = document.getElementById('stat-message-count');
    const tokensUsedEl = document.getElementById('stat-token-total');
    const createdAtEl = document.getElementById('stat-created-at');
    const statsSection = document.getElementById('session-stats-section');

    if (statsSection) {
      statsSection.style.display = 'block';
    }

    if (messagesCountEl) {
      messagesCountEl.textContent = state.currentSession.messageCount || state.messages.length;
    }

    if (tokensUsedEl) {
      tokensUsedEl.textContent = `~${state.currentSession.tokenUsageTotal}`;
    }

    if (createdAtEl) {
      const date = new Date(state.currentSession.createdAt);
      createdAtEl.textContent = date.toLocaleDateString();
    }

    // Enable action buttons
    enableSessionActionButtons();

    // Update token progress bar
    updateTokenProgressBar();
  }

  function enableSessionActionButtons() {
    const exportBtn = document.getElementById('export-chat-btn');
    const deleteBtn = document.getElementById('delete-chat-btn');
    const pinBtn = document.getElementById('pin-session-btn');
    const renameBtn = document.getElementById('rename-session-btn');
    const chatInput = document.getElementById('chat-input');
    const sendBtn = document.getElementById('send-message-btn');

    if (exportBtn) exportBtn.disabled = false;
    if (deleteBtn) deleteBtn.disabled = false;
    if (pinBtn) pinBtn.disabled = false;
    if (renameBtn) renameBtn.disabled = false;
    if (chatInput) chatInput.disabled = false;
    if (sendBtn) sendBtn.disabled = false;

    // Enable model settings controls
    enableModelSettingsControls();
  }

  function enableModelSettingsControls() {
    const controls = [
      'model-select',
      'temperature-slider',
      'max-tokens-input',
      'top-p-slider',
      'frequency-penalty-slider',
      'presence-penalty-slider',
      'save-model-settings-btn'
    ];

    controls.forEach(id => {
      const element = document.getElementById(id);
      if (element) element.disabled = false;
    });

    // Load available models for the current provider
    loadAvailableModels();
  }

  function updateTokenProgressBar() {
    const progressFill = document.querySelector('.token-progress-fill');
    if (!progressFill || !state.currentSession) return;

    // Assume max 100k tokens for visualization
    const maxTokens = 100000;
    const percentage = Math.min((state.currentSession.tokenUsageTotal / maxTokens) * 100, 100);
    progressFill.style.width = `${percentage}%`;
  }

  function showToast(message, type = 'info') {
    if (window.showToast) {
      window.showToast(message, type);
    } else {
      console.log(`[Toast ${type}] ${message}`);
    }
  }

  // ============================================================================
  // MODEL SETTINGS
  // ============================================================================

  function loadAvailableModels() {
    if (!state.currentSession) return;

    const modelSelect = document.getElementById('model-select');
    if (!modelSelect) return;

    // Clear existing options
    modelSelect.innerHTML = '';

    // Get provider type
    const providerType = state.currentSession.providerType;

    // Define available models per provider
    const modelsByProvider = {
      'gemini': [
        { value: 'gemini-1.5-pro', label: 'Gemini 1.5 Pro' },
        { value: 'gemini-1.5-flash', label: 'Gemini 1.5 Flash' },
        { value: 'gemini-pro', label: 'Gemini Pro' }
      ],
      'claude': [
        { value: 'claude-3-5-sonnet-20241022', label: 'Claude 3.5 Sonnet' },
        { value: 'claude-3-opus-20240229', label: 'Claude 3 Opus' },
        { value: 'claude-3-sonnet-20240229', label: 'Claude 3 Sonnet' },
        { value: 'claude-3-haiku-20240307', label: 'Claude 3 Haiku' }
      ],
      'gpt4': [
        { value: 'gpt-4-turbo-preview', label: 'GPT-4 Turbo' },
        { value: 'gpt-4', label: 'GPT-4' },
        { value: 'gpt-3.5-turbo', label: 'GPT-3.5 Turbo' }
      ],
      'vscode': [
        { value: 'default', label: 'VS Code Default Model' }
      ]
    };

    const models = modelsByProvider[providerType] || [];

    models.forEach(model => {
      const option = document.createElement('option');
      option.value = model.value;
      option.textContent = model.label;
      modelSelect.appendChild(option);
    });

    // Set first model as default if none selected
    if (models.length > 0 && !state.modelSettings.model) {
      state.modelSettings.model = models[0].value;
      modelSelect.value = models[0].value;
    }
  }

  async function handleSaveModelSettings() {
    if (!state.currentSession) {
      showToast('No active session', 'warning');
      return;
    }

    try {
      // In a real implementation, this would save to backend
      // For now, we'll just store in localStorage and show success
      const settings = {
        sessionId: state.currentSession.id,
        ...state.modelSettings
      };

      localStorage.setItem(
        `model-settings-${state.currentSession.id}`,
        JSON.stringify(settings)
      );

      console.log('[AiChat] Saved model settings:', settings);
      showToast('Model settings saved', 'success');

      // If Tauri is available, send to backend
      if (window.__TAURI__) {
        try {
          await window.__TAURI__.invoke('save_model_settings', {
            sessionId: state.currentSession.id,
            settings: state.modelSettings
          });
        } catch (error) {
          console.warn('[AiChat] Backend save not available:', error);
        }
      }
    } catch (error) {
      console.error('[AiChat] Failed to save model settings:', error);
      showToast('Failed to save settings', 'error');
    }
  }

  function loadModelSettings(sessionId) {
    try {
      const saved = localStorage.getItem(`model-settings-${sessionId}`);
      if (saved) {
        const settings = JSON.parse(saved);

        // Apply settings to state
        state.modelSettings = {
          temperature: settings.temperature || 0.7,
          maxTokens: settings.maxTokens || 4096,
          topP: settings.topP || 0.9,
          frequencyPenalty: settings.frequencyPenalty || 0.0,
          presencePenalty: settings.presencePenalty || 0.0,
          model: settings.model || ''
        };

        // Update UI controls
        updateModelSettingsUI();
      }
    } catch (error) {
      console.error('[AiChat] Failed to load model settings:', error);
    }
  }

  function updateModelSettingsUI() {
    const settings = state.modelSettings;

    // Temperature
    const tempSlider = document.getElementById('temperature-slider');
    const tempValue = document.getElementById('temperature-value');
    if (tempSlider) tempSlider.value = settings.temperature;
    if (tempValue) tempValue.textContent = settings.temperature.toFixed(1);

    // Top P
    const topPSlider = document.getElementById('top-p-slider');
    const topPValue = document.getElementById('top-p-value');
    if (topPSlider) topPSlider.value = settings.topP;
    if (topPValue) topPValue.textContent = settings.topP.toFixed(2);

    // Frequency Penalty
    const freqSlider = document.getElementById('frequency-penalty-slider');
    const freqValue = document.getElementById('frequency-penalty-value');
    if (freqSlider) freqSlider.value = settings.frequencyPenalty;
    if (freqValue) freqValue.textContent = settings.frequencyPenalty.toFixed(1);

    // Presence Penalty
    const presSlider = document.getElementById('presence-penalty-slider');
    const presValue = document.getElementById('presence-penalty-value');
    if (presSlider) presSlider.value = settings.presencePenalty;
    if (presValue) presValue.textContent = settings.presencePenalty.toFixed(1);

    // Max Tokens
    const maxTokensInput = document.getElementById('max-tokens-input');
    if (maxTokensInput) maxTokensInput.value = settings.maxTokens;

    // Model
    const modelSelect = document.getElementById('model-select');
    if (modelSelect && settings.model) {
      modelSelect.value = settings.model;
    }
  }

  // ============================================================================
  // PUBLIC API
  // ============================================================================

  return {
    initialize,
    deleteCurrentSession,
    togglePinCurrentSession,
    renameCurrentSession,
    exportCurrentSession
  };

})();

// Auto-initialize when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => {
    // Initialization will be called from main.js
  });
} else {
  // DOM already loaded
  // Initialization will be called from main.js
}

// Export to global scope
window.AiChatUI = AiChatUI;
