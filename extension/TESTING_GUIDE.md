# 🧪 Extension V3 Testing Guide

## Prerequisites

### 1. Start Gluon Desktop
```bash
cd gluon-desktop/src-tauri
cargo run
```

**Verify:**
- ✅ WebSocket server running on `ws://127.0.0.1:8743`
- ✅ Console shows: `[Workflow] Loaded workflow state with X agents`
- ✅ No compilation errors

### 2. Load Extension in Chrome/Edge

**Chrome:**
1. Navigate to `chrome://extensions/`
2. Enable "Developer mode" (top right)
3. Click "Load unpacked"
4. Select `c:\Users\PC\Desktop\Gluon-v2\extension` folder

**Edge:**
1. Navigate to `edge://extensions/`
2. Enable "Developer mode" (bottom left)
3. Click "Load unpacked"
4. Select `c:\Users\PC\Desktop\Gluon-v2\extension` folder

**Verify:**
- ✅ Extension loaded without errors
- ✅ Click extension icon → Sidebar opens
- ✅ Sidebar shows "Connected ✓" status

---

## Test Plan

### Phase 1: Basic Connectivity ✅

#### Test 1.1: WebSocket Connection
1. Open Extension Sidebar (click extension icon)
2. Open Browser DevTools → Console tab
3. Look for connection logs

**Expected Output:**
```
WebSocket Connected to Desktop.
🔄 [Workflow V2] State sync received from Rust
```

**Pass Criteria:**
- ✅ Status shows "Connected ✓"
- ✅ No WebSocket errors in console

#### Test 1.2: Background.js Workflow Handler
1. Open Browser DevTools → Console
2. Execute test command:
```javascript
chrome.runtime.sendMessage({
  type: 'workflow_command',
  action: 'workflow_get_graph',
  payload: {}
}, (response) => {
  console.log('✅ Graph Response:', response);
});
```

**Expected Output:**
```
✅ Graph Response: {
  agents: [...],
  connections: [...]
}
```

**Pass Criteria:**
- ✅ Response received within 1 second
- ✅ No timeout errors
- ✅ `data` field contains graph structure

---

### Phase 2: Preset Loading 🔄

#### Test 2.1: Load Agent Presets from Rust
1. Open Sidebar → Workflow tab
2. Click "Add Agent" button (or open agent preset selector)
3. Open Browser DevTools → Console

**Expected Output:**
```
✅ Loaded 20+ agent presets from Rust
```

**Expected UI:**
- Modal opens showing agent preset cards
- Categories: Architecture, Frontend, Backend, etc.
- Each card shows icon, name, description, tags

**Pass Criteria:**
- ✅ At least 20 agent presets loaded
- ✅ No "Failed to load" errors
- ✅ Search bar functional
- ✅ Category filtering works

#### Test 2.2: Search Agent Presets
1. In Agent Preset Selector modal
2. Type "frontend" in search bar

**Expected Result:**
- Filtered list shows only agents matching "frontend"
- e.g., "Frontend Specialist", "React Developer", etc.

**Pass Criteria:**
- ✅ Search returns relevant results
- ✅ Instant filtering (< 200ms)

#### Test 2.3: Select Agent Preset
1. Click on any agent card (e.g., "Frontend Specialist")
2. Click "Add Agent" button

**Expected Result:**
- Modal closes
- New agent appears in workflow graph
- Agent has default system prompt from preset

**Pass Criteria:**
- ✅ Agent created successfully
- ✅ System prompt matches preset
- ✅ Agent appears in graph UI

---

### Phase 3: CRUD Operations 🔄

#### Test 3.1: Add Agent (Manual)
**Command:**
```javascript
// In browser console
chrome.runtime.sendMessage({
  type: 'workflow_command',
  action: 'workflow_add_agent',
  payload: {
    name: 'Test Agent',
    output_wrapper: null,
    agent_type: 'Normal',
    position: { x: 100, y: 100 }
  }
}, (response) => {
  console.log('✅ Agent Added:', response.data);
});
```

**Expected Result:**
- New agent created in Rust backend
- WebSocket broadcast: `workflow-state-sync` event
- UI auto-updates with new agent

**Pass Criteria:**
- ✅ Agent appears in graph
- ✅ Agent ID is UUID format
- ✅ No errors in console

#### Test 3.2: Remove Agent
**Command:**
```javascript
// Replace 'agent-id-here' with actual agent ID
chrome.runtime.sendMessage({
  type: 'workflow_command',
  action: 'workflow_remove_agent',
  payload: {
    agent_id: 'agent-id-here'
  }
}, (response) => {
  console.log('✅ Agent Removed:', response);
});
```

**Expected Result:**
- Agent removed from Rust backend
- WebSocket broadcast: `workflow-state-sync` event
- UI auto-updates (agent disappears)

**Pass Criteria:**
- ✅ Agent removed from graph
- ✅ Connected connections also removed
- ✅ No errors in console

#### Test 3.3: Add Connection
**Command:**
```javascript
// Replace IDs with actual agent IDs
chrome.runtime.sendMessage({
  type: 'workflow_command',
  action: 'workflow_add_connection',
  payload: {
    from_id: 'agent-1-id',
    to_id: 'agent-2-id',
    template: null
  }
}, (response) => {
  console.log('✅ Connection Added:', response);
});
```

**Expected Result:**
- Connection created in Rust backend
- WebSocket broadcast: `workflow-state-sync` event
- UI shows arrow from Agent 1 → Agent 2

**Pass Criteria:**
- ✅ Connection appears in graph
- ✅ Arrow connects correct agents
- ✅ No errors in console

---

### Phase 4: Real-time Sync 🔄

#### Test 4.1: Multi-Window Sync
**Setup:**
1. Open Extension in 2 browser windows (Window A and B)
2. Both should show same workflow graph

**Test Steps:**
1. In Window A: Add new agent
2. Wait 1 second
3. Check Window B

**Expected Result:**
- Window B automatically updates with new agent
- No manual refresh needed

**Pass Criteria:**
- ✅ Window B shows new agent within 1 second
- ✅ Console shows `workflow-state-sync` event
- ✅ Both windows show identical graph

#### Test 4.2: Connection Sync
1. In Window A: Add connection between 2 agents
2. Check Window B

**Expected Result:**
- Window B shows new connection
- Arrow appears in graph UI

**Pass Criteria:**
- ✅ Connection syncs to Window B
- ✅ Latency < 500ms

---

### Phase 5: Saved Configs 🔄

#### Test 5.1: Save Workflow Config
**Command:**
```javascript
chrome.runtime.sendMessage({
  type: 'workflow_command',
  action: 'workflow_save_config',
  payload: {
    id: 'test-config-1',
    name: 'My Test Workflow',
    workflow: { /* current graph state */ }
  }
}, (response) => {
  console.log('✅ Config Saved:', response);
});
```

**Expected Result:**
- Config saved to `~/.gluon/workflows/test-config-1.json`
- Success response returned

**Pass Criteria:**
- ✅ File created on disk
- ✅ No errors in console

#### Test 5.2: Load Saved Configs
**Command:**
```javascript
chrome.runtime.sendMessage({
  type: 'workflow_command',
  action: 'workflow_get_saved_configs',
  payload: {}
}, (response) => {
  console.log('✅ Saved Configs:', response.data);
});
```

**Expected Result:**
- Array of saved workflow configs
- Includes 'test-config-1' from previous test

**Pass Criteria:**
- ✅ Returns array of configs
- ✅ Each config has `id`, `name`, `created_at`

#### Test 5.3: Load Workflow from Config
**Command:**
```javascript
chrome.runtime.sendMessage({
  type: 'workflow_command',
  action: 'workflow_load_config',
  payload: {
    id: 'test-config-1'
  }
}, (response) => {
  console.log('✅ Config Loaded:', response.data);
});
```

**Expected Result:**
- Current workflow replaced with saved config
- UI updates to show loaded graph

**Pass Criteria:**
- ✅ Graph restored from saved config
- ✅ All agents and connections loaded

---

### Phase 6: LLM Execution (Optional) ⚠️

**Prerequisites:**
- Valid LLM API key (OpenAI or Claude)
- At least 2 connected agents

#### Test 6.1: Execute Agent with LLM
**Command:**
```javascript
chrome.runtime.sendMessage({
  type: 'workflow_command',
  action: 'workflow_execute_agent',
  payload: {
    agent_id: 'agent-id-here',
    user_message: 'Create a login form in React',
    llm_settings: {
      provider: 'openai',
      model: 'gpt-4o-mini',
      api_key: 'sk-...',
      temperature: 0.7,
      max_tokens: 1000
    }
  }
}, (response) => {
  console.log('✅ LLM Response:', response.data);
});
```

**Expected Result:**
- Agent status changes to "Working"
- LLM generates response
- If routing targets exist, message sent to target agents
- Agent status changes to "Success"

**Pass Criteria:**
- ✅ Response received within 30 seconds
- ✅ `agent_response` contains LLM output
- ✅ `routes` contains routing information (if applicable)

#### Test 6.2: Routing Notification
**Setup:**
1. Agent A connected to Agent B
2. Execute Agent A with LLM

**Expected Result:**
- Console shows routing notification:
```
📨 [Workflow V2] Route notification: {
  from_agent_name: "Agent A",
  to_agent_name: "Agent B",
  content: "..."
}
```

**Pass Criteria:**
- ✅ Routing event received
- ✅ Contains `from_agent_name`, `to_agent_name`, `content`

---

## Troubleshooting

### Problem: "Desktop app not connected"
**Solution:**
1. Verify Gluon Desktop is running (`cargo run`)
2. Check WebSocket is on port 8743
3. Restart Extension (reload in chrome://extensions/)

### Problem: Presets don't load
**Solution:**
1. Open Rust logs: Check for `[Workflow] Loaded X agent presets`
2. Verify Tauri commands registered in `main.rs`
3. Check background.js console for errors

### Problem: Real-time sync not working
**Solution:**
1. Check background.js console for `workflow-state-sync` events
2. Verify `workflow_sync` case in `ws.onmessage` switch
3. Restart Gluon Desktop (WebSocket may be stuck)

### Problem: Timeout errors (30s)
**Solution:**
1. Increase timeout in background.js:
```javascript
}, 60000);  // Change from 30s to 60s
```
2. Check Rust logs for slow operations

---

## Success Criteria Summary

### Must Pass ✅
- [ ] WebSocket connection established
- [ ] Agent presets load from Rust (20+)
- [ ] Add agent works
- [ ] Remove agent works
- [ ] Add connection works
- [ ] Real-time sync works (2 windows)

### Should Pass 🔄
- [ ] Save workflow config
- [ ] Load workflow config
- [ ] Search agent presets
- [ ] Category filtering

### Optional ⚠️
- [ ] LLM execution (requires API key)
- [ ] Routing notifications

---

## Next Steps After Testing

1. **If all tests pass:**
   - Delete `workflow-manager-v1-backup.js`
   - Delete `preset-manager.js`
   - Create git commit

2. **If tests fail:**
   - Check background.js console logs
   - Check Rust console logs
   - Review error messages
   - Refer to Troubleshooting section

3. **Performance validation:**
   - Preset loading: < 500ms
   - Add agent: < 200ms
   - Real-time sync latency: < 500ms
   - WebSocket roundtrip: < 100ms

---

**Test Date:** _____________________
**Tester:** _____________________
**Result:** ☐ PASS  ☐ FAIL
**Notes:**
