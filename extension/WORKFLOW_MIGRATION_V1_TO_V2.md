# 🔄 Workflow Manager Migration: V1 → V2 (Thin Client)

## 📊 Comparison Overview

| Feature | V1 (Old - 2838 lines) | V2 (New - ~650 lines) | Change |
|---------|----------------------|----------------------|--------|
| **Architecture** | Fat Client | **Thin Client** | ✅ Simplified |
| **Graph State** | Local (`this.graph`) | Cached (`this.cachedGraph`) | ✅ Read-only |
| **CRUD Operations** | Local mutations | **Rust API calls** | ✅ Delegated |
| **Routing Logic** | `parseGCodeMessage()` ~150 lines | **Removed** (Rust) | ✅ -150 lines |
| **Presets** | `preset-manager.js` import | **Rust API** | ✅ SSOT |
| **Workflow Tabs** | localStorage | **Rust SavedWorkflowConfig** | ✅ Persistent |
| **Real-time Sync** | Manual refresh | **WebSocket events** | ✅ Automatic |
| **LLM Execution** | N/A | **`workflow_execute_agent()`** | ✅ New feature |

---

## ❌ What Was Removed (V1 → V2)

### 1. **Local Graph State** (~100 lines)
```javascript
// ❌ V1 - Local state with mutations
class WorkflowManager {
    constructor() {
        this.graph = null; // Modifiable local state
    }

    async addAgent(name, outputWrapper, agentType) {
        // Mutate local graph
        const agent = { id: generateUUID(), name, ... };
        this.graph.agents.set(agent.id, agent);

        // Save to storage
        await this.saveToLocalStorage();

        return agent;
    }
}
```

```javascript
// ✅ V2 - Read-only cache + Rust API
class WorkflowManager {
    constructor() {
        this.cachedGraph = null; // READ-ONLY cache
    }

    async addAgent(name, outputWrapper, agentType, position) {
        // Delegate to Rust
        const agent = await this.sendToBackground('workflow_add_agent', {
            name, output_wrapper: outputWrapper, agent_type: agentType, position
        });

        // Graph auto-updates via WebSocket event
        return agent;
    }
}
```

**Lines saved:** ~100

---

### 2. **parseGCodeMessage() - Routing Parser** (~150 lines)
```javascript
// ❌ V1 - Complex G-code parsing in JS
parseGCodeMessage(content) {
    const lines = content.split('\n');
    const routes = new Map();
    let currentTarget = null;
    let currentContent = [];

    for (const line of lines) {
        if (line.startsWith('>>>> TARGET:')) {
            // Save previous target
            if (currentTarget) {
                routes.set(currentTarget, currentContent.join('\n'));
            }

            // Parse new target
            currentTarget = line.substring('>>>> TARGET:'.length).trim();
            currentContent = [];
        } else if (currentTarget) {
            currentContent.push(line);
        }
    }

    // ~150 lines of complex parsing logic...
    return routes;
}

// Usage in V1
const routes = this.parseGCodeMessage(agentResponse);
for (const [targetName, content] of routes) {
    await this.sendMessageToAgent(targetName, content);
}
```

```javascript
// ✅ V2 - REMOVED (handled by Rust)
// Routing now happens in Rust:
//   1. LLM generates JSON with routing
//   2. Rust parses + self-heals (max 3 retries)
//   3. Rust dispatches to target agents
//   4. Extension receives 'workflow-route-message' events for UI notifications

handleRoutingNotification(payload) {
    const { from_agent_name, to_agent_name, content } = payload;
    this.showNotification(`📨 ${from_agent_name} → ${to_agent_name}`, ...);
}
```

**Lines saved:** ~150

---

### 3. **Preset Manager Import** (~20 lines + entire preset-manager.js file)
```javascript
// ❌ V1 - Hardcoded JS presets
import presetManager from '../prompts/preset-manager.js';

class WorkflowManager {
    constructor() {
        this.presetManager = presetManager;
    }

    async init() {
        await this.presetManager.init();
        // Load hardcoded presets from JS
    }
}
```

```javascript
// ✅ V2 - Load from Rust SSOT
class WorkflowManager {
    constructor() {
        this.agentPresets = [];
        this.connectionPresets = [];
        this.workflowPresets = [];
    }

    async loadPresetsFromRust() {
        this.agentPresets = await this.sendToBackground('workflow_get_agent_presets');
        this.connectionPresets = await this.sendToBackground('workflow_get_connection_presets');
        this.workflowPresets = await this.sendToBackground('workflow_get_workflow_presets');
    }
}
```

**Lines saved:** ~20 (+ can delete entire `preset-manager.js` file later)

---

### 4. **Workflow Tabs (localStorage)** (~200 lines)
```javascript
// ❌ V1 - localStorage with manual serialization
class WorkflowManager {
    constructor() {
        this.workflowTabs = [];
        this.activeTabId = null;
        this.nextTabId = 1;
    }

    async loadTabsFromStorage() {
        const stored = localStorage.getItem('gluon_workflow_tabs');
        if (stored) {
            this.workflowTabs = JSON.parse(stored);
        }
    }

    async saveTabsToStorage() {
        localStorage.setItem('gluon_workflow_tabs', JSON.stringify(this.workflowTabs));
    }

    createNewTab() {
        const tab = {
            id: `tab-${this.nextTabId++}`,
            name: `Workflow ${this.nextTabId}`,
            workflow: this.serializeGraph(),
            createdAt: Date.now()
        };
        this.workflowTabs.push(tab);
        await this.saveTabsToStorage();
    }

    // ~200 lines of tab management...
}
```

```javascript
// ✅ V2 - Use Rust SavedWorkflowConfig
class WorkflowManager {
    constructor() {
        this.savedConfigs = []; // Loaded from Rust
        this.activeTabId = null;
    }

    async loadSavedConfigsFromRust() {
        this.savedConfigs = await this.sendToBackground('workflow_get_saved_configs');
    }

    async saveWorkflowConfig(id, name) {
        const config = await this.sendToBackground('workflow_save_config', {
            id: id || `config-${Date.now()}`,
            name,
            workflow: this.cachedGraph
        });

        await this.loadSavedConfigsFromRust(); // Refresh list
    }

    async deleteWorkflowConfig(id) {
        await this.sendToBackground('workflow_delete_saved_config', { id });
        await this.loadSavedConfigsFromRust();
    }
}
```

**Lines saved:** ~200

---

### 5. **Manual Graph Refresh Logic** (~50 lines)
```javascript
// ❌ V1 - Manual refresh on every change
async addAgent(...) {
    // ... mutations ...
    await this.saveToLocalStorage();
    this.renderWorkflow(); // Manual re-render
}

async refreshWorkflow() {
    // Fetch from backend
    const graph = await this.fetchGraphFromBackend();
    this.graph = graph;
    this.renderWorkflow();
}
```

```javascript
// ✅ V2 - Automatic WebSocket sync
subscribeToBackendUpdates() {
    chrome.runtime.onMessage.addListener((message) => {
        if (message.type === 'workflow-state-sync') {
            this.handleGraphSync(message.payload);
        }
    });
}

handleGraphSync(graphData) {
    this.cachedGraph = graphData;
    this.renderWorkflow(); // Automatic re-render on backend change
}
```

**Lines saved:** ~50

---

## ✅ What Was Added (V2 New Features)

### 1. **WebSocket Real-time Sync** (~50 lines)
```javascript
subscribeToBackendUpdates() {
    chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
        if (message.type === 'workflow-state-sync') {
            console.log('🔄 Received state sync from Rust');
            this.handleGraphSync(message.payload);
        }

        if (message.type === 'workflow-route-message') {
            console.log('📨 Received routing event:', message.payload);
            this.handleRoutingNotification(message.payload);
        }
    });
}
```

### 2. **Unified sendToBackground() API** (~30 lines)
```javascript
async sendToBackground(action, payload = {}) {
    return new Promise((resolve, reject) => {
        chrome.runtime.sendMessage(
            { type: 'workflow_command', action, payload },
            (response) => {
                if (chrome.runtime.lastError) {
                    reject(chrome.runtime.lastError);
                    return;
                }
                if (response?.error) {
                    reject(new Error(response.error));
                    return;
                }
                resolve(response?.data);
            }
        );
    });
}
```

### 3. **Preset Operations** (~50 lines)
```javascript
async createAgentFromPreset(presetId, customName, position) {
    const agent = await this.sendToBackground('workflow_create_agent_from_preset', {
        preset_id: presetId,
        custom_name: customName || null,
        position: position || null
    });

    this.showNotification('Agent Created', `Created from preset: ${presetId}`, 'success');
    return agent;
}

async createWorkflowFromPreset(presetId) {
    const result = await this.sendToBackground('workflow_create_from_preset', {
        preset_id: presetId
    });

    this.showNotification(
        'Workflow Created',
        `Created ${result.agents_created} agents and ${result.connections_created} connections`,
        'success'
    );
}
```

---

## 📉 Lines of Code Comparison

| Component | V1 Lines | V2 Lines | Reduction |
|-----------|----------|----------|-----------|
| Graph state management | ~100 | ~20 | **-80%** |
| CRUD operations | ~300 | ~150 | **-50%** |
| Routing (parseGCodeMessage) | ~150 | ~10 (notification only) | **-93%** |
| Preset management | ~20 + preset-manager.js | ~50 | **Simplified** |
| Workflow tabs | ~200 | ~50 | **-75%** |
| Graph refresh logic | ~50 | ~30 (WebSocket) | **-40%** |
| **TOTAL** | **~2838** | **~650** | **-77%** |

---

## 🚀 Migration Steps

### Step 1: Update background.js

Add handler for `workflow_command` messages:

```javascript
// extension/src/background/background.js

chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
    if (request.type === 'workflow_command') {
        // Forward to Tauri via WebSocket
        handleWorkflowCommand(request.action, request.payload)
            .then(data => sendResponse({ data }))
            .catch(error => sendResponse({ error: error.message }));

        return true; // Async response
    }
});

async function handleWorkflowCommand(action, payload) {
    // Send to WebSocket (port 8743)
    const ws = getWebSocketConnection();

    return new Promise((resolve, reject) => {
        const requestId = generateRequestId();

        ws.send(JSON.stringify({
            request_id: requestId,
            action,
            payload
        }));

        // Wait for response
        pendingRequests.set(requestId, { resolve, reject });
    });
}

// Listen for WebSocket events from Rust
ws.onmessage = (event) => {
    const message = JSON.parse(event.data);

    // Broadcast state syncs to all extension pages
    if (message.action === 'workflow_sync') {
        chrome.runtime.sendMessage({
            type: 'workflow-state-sync',
            payload: message.payload
        });
    }

    // Broadcast routing events
    if (message.action === 'workflow_route_message') {
        chrome.runtime.sendMessage({
            type: 'workflow-route-message',
            payload: message.payload
        });
    }
};
```

### Step 2: Replace workflow-manager.js

```bash
# Backup old version
mv extension/src/features/workflows/workflow-manager.js \
   extension/src/features/workflows/workflow-manager-v1-backup.js

# Use new version
mv extension/src/features/workflows/workflow-manager-v2.js \
   extension/src/features/workflows/workflow-manager.js
```

### Step 3: Update imports

```javascript
// Any file importing workflow-manager.js
import workflowManager from './features/workflows/workflow-manager.js';

// No changes needed - API is compatible
workflowManager.addAgent('Test Agent', null, 'Normal');
```

### Step 4: (Optional) Remove preset-manager.js

```bash
# After verifying V2 works
rm extension/src/features/prompts/preset-manager.js
```

---

## ✅ Benefits of V2

1. **🎯 Simplified:** 77% less code (2838 → 650 lines)
2. **🔄 Real-time:** Automatic sync via WebSocket
3. **📦 SSOT:** All state in Rust (no sync bugs)
4. **🧪 Testable:** Backend fully testable in Rust
5. **🚀 Faster:** No localStorage I/O on every change
6. **🔐 Type-safe:** Rust ensures data integrity
7. **🐛 Self-healing:** JSON parsing with retries
8. **📊 Scalable:** Backend can handle complex workflows

---

## 🧪 Testing Checklist

### V2 Manual Tests:

- [ ] Add agent via UI
- [ ] Remove agent
- [ ] Update agent (name, system prompt)
- [ ] Add connection between agents
- [ ] Remove connection
- [ ] Toggle auto-forward
- [ ] Create agent from preset
- [ ] Create workflow from preset (e.g., "fullstack_feature")
- [ ] Save workflow config
- [ ] Load workflow config
- [ ] Delete workflow config
- [ ] Verify real-time sync (open 2 tabs, change in one, see update in other)
- [ ] Verify routing notifications appear

---

## 🔍 Backward Compatibility

V2 maintains the same public API as V1:

```javascript
// ✅ All these still work
workflowManager.addAgent(name, wrapper, type);
workflowManager.removeAgent(id);
workflowManager.addConnection(fromId, toId);
workflowManager.refreshWorkflow();

// ✅ New methods available
workflowManager.createAgentFromPreset(presetId);
workflowManager.createWorkflowFromPreset(presetId);
workflowManager.saveWorkflowConfig(id, name);
```

---

## 📚 Next Steps

1. ✅ V2 implementation complete
2. ⏳ Update background.js WebSocket handler
3. ⏳ Test V2 in extension
4. ⏳ Remove V1 backup after verification
5. ⏳ (Optional) Remove `preset-manager.js`
6. ⏳ Update extension manifest if needed

---

## 🎉 Summary

**V1 → V2 Migration** transforms the Extension from a **Fat Client** (2838 lines) to a **Thin Client** (650 lines), delegating all business logic to the Rust SSOT backend. This results in:

- **77% less code** to maintain
- **Automatic real-time sync**
- **No localStorage race conditions**
- **Type-safe backend** (Rust)
- **Self-healing JSON parsing**
- **Preset library as SSOT**

The migration preserves API compatibility while unlocking powerful new features like LLM-based agent execution with smart routing.
