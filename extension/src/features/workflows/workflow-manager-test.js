// Minimal test to see if module can load
console.log('[WorkflowManagerTest] 📦 Test module loading...');

class WorkflowManagerTest {
    constructor() {
        console.log('[WorkflowManagerTest] ✅ Constructor called');
    }
}

// Initialize immediately
console.log('[WorkflowManagerTest] 🚀 Creating instance...');
const testInstance = new WorkflowManagerTest();
window.workflowManagerTest = testInstance;
console.log('[WorkflowManagerTest] ✅ Instance attached to window');
