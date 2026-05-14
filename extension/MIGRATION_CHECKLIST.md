# ✅ Extension V3 → Rust SSOT Migration Checklist

## 📋 Overview
This checklist guides the migration of Extension V3 Workflow Manager from Fat Client (2838 lines) to Thin Client (650 lines) architecture using Rust as the Single Source of Truth.

---

## Phase 1: Backend Preparation ✅ COMPLETED

- [x] Extended `agent_workflow.rs` with new structures (Message, Agent fields, AgentStatus)
- [x] Implemented Preset Library in Rust (20+ agents, 8 connections, 10+ workflows)
- [x] Created `llm_inference.rs` with OpenAI/Claude/Custom API support
- [x] Created `workflow_execution.rs` with smart router and self-healing JSON parser
- [x] Created `debounced_save.rs` for 2-second delayed persistence
- [x] Added 10 new Tauri commands in `workflow_commands.rs`
- [x] Registered all commands in `main.rs`
- [x] Compiled successfully (88 warnings, 0 errors)

---

## Phase 2: Extension V3 Migration 🔄 IN PROGRESS

### Step 2.1: Background.js Update ✅ COMPLETED

**File:** `extension/src/background/background.js`

- [x] Added universal `workflow_command` handler (lines 1638-1667)
  - Routes all workflow-manager-v2.js commands to Rust
  - Handles timeout (30 seconds)
  - Stores async callbacks for response routing

- [x] Added response handlers in `ws.onmessage` switch:
  - [x] Preset operations (5 commands)
  - [x] Saved config operations (4 commands)
  - [x] Execution operations (4 commands)
  - [x] Real-time state sync event (`workflow_state_sync`)
  - [x] Routing notifications (`workflow_route_message`)

**Testing:**
```javascript
// In browser console (sidebar context):
chrome.runtime.sendMessage({
  type: 'workflow_command',
  action: 'workflow_get_agent_presets',
  payload: {}
}, (response) => {
  console.log('Presets:', response.data);
});
```

### Step 2.2: Replace workflow-manager.js ⏳ PENDING

**Backup old version:**
```bash
cd extension/src/features/workflows
mv workflow-manager.js workflow-manager-v1-backup.js
mv workflow-manager-v2.js workflow-manager.js
```

**Verification:**
- [ ] Check if any files import `workflow-manager.js`
- [ ] Verify import paths remain valid
- [ ] Test basic operations (add agent, remove agent, add connection)

**Files to check for imports:**
```bash
grep -r "workflow-manager" extension/src --include="*.js"
```

Expected files:
- `extension/src/features/workflows/workflow-manager.js` (the file itself)
- `extension/src/sidepanel/sidebar.js` (likely imports it)
- `extension/src/content/overlay.js` (possibly imports it)

### Step 2.3: Remove preset-manager.js Dependency ⏳ PENDING

**Files to update:**

1. **workflow-manager.js** (already done in V2)
   - [x] Removed `import presetManager from '../prompts/preset-manager.js'`
   - [x] Replaced with `loadPresetsFromRust()`

2. **Check other imports:**
```bash
grep -r "preset-manager" extension/src --include="*.js"
```

If no other files import it:
```bash
# Safe to delete
rm extension/src/features/prompts/preset-manager.js
```

If other files DO import it, update them to use Rust API:
```javascript
// ❌ OLD
import presetManager from './preset-manager.js';
const presets = await presetManager.getAgentPresets();

// ✅ NEW
const presets = await workflowManager.sendToBackground('workflow_get_agent_presets');
```

### Step 2.4: Update manifest.json (if needed) ⏳ PENDING

**File:** `extension/manifest.json`

Check if any changes are needed:
- [ ] Verify WebSocket permissions (`ws://127.0.0.1:8743`)
- [ ] Verify `declarativeNetRequest` permissions (for workflow features)
- [ ] Check if `preset-manager.js` is listed anywhere

### Step 2.5: Test Real-time Sync ⏳ PENDING

**Manual Test:**
1. Open Gluon Extension in 2 browser windows (Window A and B)
2. In Window A: Add a new agent
3. Expected: Window B automatically refreshes and shows the new agent
4. In Window B: Remove an agent
5. Expected: Window A automatically refreshes and agent disappears

**WebSocket Event Flow:**
```
1. Extension A: addAgent() → sendToBackground()
2. Background.js: Forwards to Rust via WebSocket
3. Rust: Saves to graph, broadcasts 'workflow_state_sync'
4. Background.js: Receives sync, broadcasts to all extension pages
5. Extension A + B: handleGraphSync() → renderWorkflow()
```

### Step 2.6: Test Preset Operations ⏳ PENDING

**Test Cases:**

1. **Load Agent Presets**
```javascript
const presets = await workflowManager.sendToBackground('workflow_get_agent_presets');
console.log(`Loaded ${presets.length} agent presets`);
// Expected: 20+ presets (Frontend, Backend, Database, etc.)
```

2. **Create Agent from Preset**
```javascript
const agent = await workflowManager.createAgentFromPreset('frontend_specialist', 'My Frontend Agent');
console.log('Created agent:', agent.id, agent.name);
// Expected: New agent appears in workflow graph
```

3. **Create Workflow from Preset**
```javascript
const result = await workflowManager.createWorkflowFromPreset('fullstack_feature');
console.log(`Created ${result.agents_created} agents and ${result.connections_created} connections`);
// Expected: 5+ agents with connections appear
```

### Step 2.7: Test Saved Configs ⏳ PENDING

**Test Cases:**

1. **Save Current Workflow**
```javascript
await workflowManager.saveWorkflowConfig('my-config-1', 'My First Workflow');
// Expected: Config saved to Rust storage (~/.gluon/workflows/my-config-1.json)
```

2. **Load Saved Configs**
```javascript
await workflowManager.loadSavedConfigsFromRust();
console.log('Saved configs:', workflowManager.savedConfigs);
// Expected: List of saved workflow configs
```

3. **Load Workflow from Config**
```javascript
await workflowManager.loadWorkflowConfig('my-config-1');
// Expected: Workflow graph restored from saved config
```

4. **Delete Workflow Config**
```javascript
await workflowManager.deleteWorkflowConfig('my-config-1');
// Expected: Config removed from storage
```

### Step 2.8: Test LLM Execution ⏳ PENDING

**Prerequisites:**
- Valid LLM API key configured (OpenAI or Claude)
- At least 2 agents connected (e.g., "Agent A" → "Agent B")

**Test Cases:**

1. **Execute Agent with User Message**
```javascript
const llmSettings = {
  provider: 'openai',  // or 'claude', 'custom'
  model: 'gpt-4o-mini',
  api_key: 'sk-...',
  temperature: 0.7,
  max_tokens: 1000
};

const result = await workflowManager.sendToBackground('workflow_execute_agent', {
  agent_id: 'agent-uuid-123',
  user_message: 'Create a login form in React',
  llm_settings: llmSettings
});

console.log('Agent response:', result.agent_response);
console.log('Routes:', result.routes);
// Expected: LLM generates response + routing JSON
```

2. **Verify Self-Healing JSON Parser**
```javascript
// Trigger execution with agent that returns malformed JSON
// Expected: Rust retries up to 3 times, fixes JSON, or returns error
```

3. **Verify Routing Notification**
```javascript
// Listen for routing events
chrome.runtime.onMessage.addListener((message) => {
  if (message.type === 'workflow-route-message') {
    console.log('📨 Route:', message.payload.from_agent_name, '→', message.payload.to_agent_name);
  }
});
```

---

## Phase 3: Desktop App Review ⏳ PENDING

**Decision Point:** Desktop App has duplicate workflow stub. Choose one:

### Option A: Remove Desktop App Stub (Recommended)
- Delete `gluon-desktop/src/ui/windows/workflow.rs` or comment out stub
- Use Extension V3 as the only UI
- Desktop App only exposes Rust SSOT backend via Tauri commands

### Option B: Implement Full Desktop UI
- Port workflow-manager-v2.js logic to Desktop App UI
- Maintain 2 UIs (Extension + Desktop)
- Both consume same Rust SSOT backend

**Files to Review:**
```bash
# Search for workflow stubs in Desktop App
grep -r "workflow" gluon-desktop/src --include="*.rs" | grep -i "window\|ui"
```

---

## Phase 4: Testing & Verification ⏳ PENDING

### Functional Tests

- [ ] **Add Agent**
  - Via UI button
  - Via preset
  - Verify WebSocket sync

- [ ] **Remove Agent**
  - Via UI button
  - Verify connections are also removed
  - Verify WebSocket sync

- [ ] **Update Agent**
  - Change name
  - Change system prompt
  - Verify updates persist

- [ ] **Add Connection**
  - Between 2 agents
  - Verify template is saved
  - Verify WebSocket sync

- [ ] **Remove Connection**
  - Verify connection disappears
  - Verify WebSocket sync

- [ ] **Toggle Auto-Forward**
  - Enable/disable
  - Verify state persists

- [ ] **Save/Load Workflow Config**
  - Save current workflow
  - Load saved workflow
  - Delete saved workflow

- [ ] **Execute Agent with LLM**
  - Send message to agent
  - Verify LLM response
  - Verify routing to target agents

### Performance Tests

- [ ] **DebouncedSave Timing**
  - Make 10 rapid changes
  - Verify only 1 save occurs after 2 seconds
  - Check disk I/O logs

- [ ] **WebSocket Latency**
  - Measure time from action to UI update
  - Expected: < 100ms for local operations

- [ ] **Preset Loading Speed**
  - Measure time to load 20+ agent presets
  - Expected: < 500ms

### Error Handling Tests

- [ ] **WebSocket Disconnection**
  - Stop Gluon Desktop
  - Try to add agent
  - Expected: Error notification "Desktop app not connected"

- [ ] **Timeout Handling**
  - Simulate slow response (> 30s)
  - Expected: Timeout error notification

- [ ] **Invalid JSON from LLM**
  - Trigger self-healing parser
  - Expected: Max 3 retries, then error

- [ ] **Missing Agent ID**
  - Try to execute non-existent agent
  - Expected: Error "Agent not found"

---

## Phase 5: Cleanup & Documentation ⏳ PENDING

### Code Cleanup

- [ ] Delete `workflow-manager-v1-backup.js` (after verification)
- [ ] Delete `preset-manager.js` (if no other imports)
- [ ] Remove unused localStorage code (old tabs logic)
- [ ] Clean up console.log statements

### Documentation

- [ ] Update README with V2 architecture diagram
- [ ] Document new Tauri commands (already in WORKFLOW_MIGRATION_GUIDE.md)
- [ ] Create user guide for preset library
- [ ] Document LLM execution workflow

### Git Commit

```bash
git add extension/src/background/background.js
git add extension/src/features/workflows/workflow-manager.js
git add extension/WORKFLOW_MIGRATION_V1_TO_V2.md
git add extension/MIGRATION_CHECKLIST.md
git commit -m "feat: Migrate Extension V3 to Thin Client (Rust SSOT)

- Updated background.js with universal workflow_command handler
- Replaced workflow-manager.js with V2 (650 lines, -77%)
- Removed local state management (READ-ONLY cache)
- Removed parseGCodeMessage() routing logic (-150 lines)
- Removed preset-manager.js dependency
- Added WebSocket real-time sync
- Delegated all CRUD to Rust backend
- Documented migration in WORKFLOW_MIGRATION_V1_TO_V2.md

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## 🎯 Success Criteria

✅ All tests pass
✅ WebSocket real-time sync works
✅ No localStorage race conditions
✅ Preset library loads from Rust
✅ LLM execution works with smart routing
✅ Code reduced by 77% (2838 → 650 lines)
✅ No compilation errors
✅ Documentation complete

---

## 🐛 Troubleshooting Guide

### Problem: "Desktop app not connected"
**Solution:** Start Gluon Desktop, verify WebSocket on port 8743

### Problem: State doesn't sync between windows
**Solution:** Check background.js console for `workflow_state_sync` events

### Problem: Presets don't load
**Solution:** Verify Rust commands are registered in main.rs:
```rust
.invoke_handler(tauri::generate_handler![
    workflow_get_agent_presets,
    workflow_get_connection_presets,
    workflow_get_workflow_presets,
    // ... other commands
])
```

### Problem: LLM execution fails
**Solution:** Check LLM API key, verify model name, check Rust logs:
```bash
# In gluon-desktop directory
cargo run 2>&1 | grep -i "llm\|workflow"
```

### Problem: Timeout errors (30s)
**Solution:** Increase timeout in background.js line 1657:
```javascript
}, 60000);  // Increase from 30s to 60s
```

---

## 📊 Migration Impact

| Metric | Before (V1) | After (V2) | Change |
|--------|-------------|------------|--------|
| Lines of Code | 2838 | 650 | **-77%** |
| Local State | Mutable | READ-ONLY | **Simplified** |
| Persistence | localStorage | Rust DebouncedSave | **Reliable** |
| Sync | Manual refresh | WebSocket events | **Real-time** |
| Presets | JS hardcoded | Rust SSOT | **Maintainable** |
| Routing | parseGCodeMessage() | Rust Smart Router | **Self-healing** |
| LLM Execution | None | Full support | **New Feature** |

---

## 🚀 Next Steps After Migration

1. **Monitor Production**: Watch for WebSocket errors, timeout issues
2. **Gather Feedback**: User testing on preset library, LLM execution
3. **Optimize**: Profile DebouncedSave, reduce WebSocket payload size
4. **Extend**: Add more presets, improve self-healing parser
5. **Document**: Create video tutorials for workflow builder

---

**Migration Status:** Phase 2 (Step 2.1 Complete) ✅
**Next Action:** Replace workflow-manager.js with V2 version
**Estimated Time Remaining:** 2-4 hours (testing + verification)
