/**
 * Unit Tests for WorkflowManager Tab Management
 *
 * Tests cover:
 * - saveCurrentTabWorkflow() with different graph states
 * - createNewTab()
 * - switchToTab()
 * - closeTab()
 * - Tab state persistence
 * - Edge cases
 */

import { describe, test, expect, jest, beforeEach } from '@jest/globals';

// Mock PresetManager
const mockPresetManager = {
  init: jest.fn(() => Promise.resolve()),
  presets: { agents: [], connections: [], workflows: [] }
};

class TestableWorkflowManager {
  constructor() {
    this.graph = null;
    this.workflowTabs = [];
    this.activeTabId = null;
    this.nextTabId = 1;
    this.presetManager = mockPresetManager;
  }

  saveTabsToStorage() {
    const data = {
      tabs: this.workflowTabs,
      activeTabId: this.activeTabId,
      nextTabId: this.nextTabId
    };
    localStorage.setItem('gluon_workflow_tabs', JSON.stringify(data));
  }

  loadTabsFromStorage() {
    const stored = localStorage.getItem('gluon_workflow_tabs');
    if (stored) {
      try {
        const data = JSON.parse(stored);
        this.workflowTabs = data.tabs || [];
        this.activeTabId = data.activeTabId || null;
        this.nextTabId = data.nextTabId || 1;
      } catch (error) {
        this.workflowTabs = [];
      }
    }
  }

  createNewTab(name = null, makeActive = true) {
    const tabId = `tab-${this.nextTabId++}`;
    const tabName = name || `Workflow ${this.workflowTabs.length + 1}`;

    const newTab = {
      id: tabId,
      name: tabName,
      workflow: {
        agents: {},
        connections: [],
        auto_forward: false
      },
      createdAt: Date.now(),
      modifiedAt: Date.now()
    };

    this.workflowTabs.push(newTab);

    if (makeActive) {
      this.switchToTab(tabId);
    }

    this.saveTabsToStorage();
    return tabId;
  }

  switchToTab(tabId) {
    const tab = this.workflowTabs.find(t => t.id === tabId);
    if (!tab) {
      return false;
    }

    // Save current tab's workflow before switching
    if (this.activeTabId) {
      this.saveCurrentTabWorkflow();
    }

    this.activeTabId = tabId;
    this.graph = tab.workflow;
    this.saveTabsToStorage();
    return true;
  }

  saveCurrentTabWorkflow() {
    if (!this.activeTabId) return false;

    const tab = this.workflowTabs.find(t => t.id === this.activeTabId);
    if (tab && this.graph) {
      tab.workflow = JSON.parse(JSON.stringify(this.graph)); // Deep copy
      tab.modifiedAt = Date.now();
      this.saveTabsToStorage();
      return true;
    }
    return false;
  }

  closeTab(tabId) {
    const tabIndex = this.workflowTabs.findIndex(t => t.id === tabId);
    if (tabIndex === -1) return false;

    // Prevent closing the last tab
    if (this.workflowTabs.length === 1) {
      return false;
    }

    // Remove the tab
    this.workflowTabs.splice(tabIndex, 1);

    // If we closed the active tab, switch to another one
    if (this.activeTabId === tabId) {
      const newActiveIndex = tabIndex >= this.workflowTabs.length ? tabIndex - 1 : tabIndex;
      this.switchToTab(this.workflowTabs[newActiveIndex].id);
    }

    this.saveTabsToStorage();
    return true;
  }

  duplicateTab(tabId) {
    const tab = this.workflowTabs.find(t => t.id === tabId);
    if (!tab) return null;

    const newTabId = `tab-${this.nextTabId++}`;
    const duplicatedTab = {
      id: newTabId,
      name: `${tab.name} (kopia)`,
      workflow: JSON.parse(JSON.stringify(tab.workflow)),
      createdAt: Date.now(),
      modifiedAt: Date.now()
    };

    this.workflowTabs.push(duplicatedTab);
    this.saveTabsToStorage();
    return newTabId;
  }

  renameTab(tabId, newName) {
    const tab = this.workflowTabs.find(t => t.id === tabId);
    if (!tab) return false;

    tab.name = newName;
    tab.modifiedAt = Date.now();
    this.saveTabsToStorage();
    return true;
  }
}

describe('WorkflowManager - saveCurrentTabWorkflow()', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
  });

  describe('Basic functionality', () => {
    test('should save current graph to active tab', () => {
      const tabId = manager.createNewTab('Test Tab');

      manager.graph = {
        agents: {
          'agent-1': { id: 'agent-1', name: 'Agent 1' }
        },
        connections: [{ from: 'agent-1', to: 'agent-2' }],
        auto_forward: true
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result).toBe(true);
      const tab = manager.workflowTabs.find(t => t.id === tabId);
      expect(tab.workflow).toEqual(manager.graph);
    });

    test('should update modifiedAt timestamp', () => {
      const tabId = manager.createNewTab('Test Tab');
      const originalTime = manager.workflowTabs[0].modifiedAt;

      // Wait a bit to ensure timestamp changes
      jest.advanceTimersByTime(100);

      manager.graph = { agents: {}, connections: [], auto_forward: false };
      manager.saveCurrentTabWorkflow();

      const tab = manager.workflowTabs.find(t => t.id === tabId);
      expect(tab.modifiedAt).toBeGreaterThanOrEqual(originalTime);
    });

    test('should deep copy graph to prevent reference issues', () => {
      manager.createNewTab('Test Tab');

      const originalGraph = {
        agents: {
          'agent-1': { id: 'agent-1', name: 'Agent 1', config: { nested: { value: 1 } } }
        },
        connections: [{ from: 'agent-1', to: 'agent-2', config: { template: 'test' } }],
        auto_forward: false
      };

      manager.graph = originalGraph;
      manager.saveCurrentTabWorkflow();

      // Modify original graph
      originalGraph.agents['agent-1'].name = 'Modified';
      originalGraph.agents['agent-1'].config.nested.value = 999;
      originalGraph.connections[0].config.template = 'changed';

      const tab = manager.workflowTabs[0];
      expect(tab.workflow.agents['agent-1'].name).toBe('Agent 1');
      expect(tab.workflow.agents['agent-1'].config.nested.value).toBe(1);
      expect(tab.workflow.connections[0].config.template).toBe('test');
    });
  });

  describe('Different graph states', () => {
    test('should handle empty graph', () => {
      manager.createNewTab('Empty Tab');
      manager.graph = { agents: {}, connections: [], auto_forward: false };

      const result = manager.saveCurrentTabWorkflow();

      expect(result).toBe(true);
      expect(manager.workflowTabs[0].workflow).toEqual({
        agents: {},
        connections: [],
        auto_forward: false
      });
    });

    test('should handle graph with multiple agents', () => {
      manager.createNewTab('Multi-Agent Tab');

      manager.graph = {
        agents: {
          'agent-1': { id: 'agent-1', name: 'Agent 1', type: 'Normal' },
          'agent-2': { id: 'agent-2', name: 'Agent 2', type: 'Normal' },
          'agent-3': { id: 'agent-3', name: 'Aggregator', type: 'Report' }
        },
        connections: [
          { from: 'agent-1', to: 'agent-3' },
          { from: 'agent-2', to: 'agent-3' }
        ],
        auto_forward: true
      };

      manager.saveCurrentTabWorkflow();

      const tab = manager.workflowTabs[0];
      expect(Object.keys(tab.workflow.agents)).toHaveLength(3);
      expect(tab.workflow.connections).toHaveLength(2);
      expect(tab.workflow.auto_forward).toBe(true);
    });

    test('should handle graph with complex nested structures', () => {
      manager.createNewTab('Complex Tab');

      manager.graph = {
        agents: {
          'agent-1': {
            id: 'agent-1',
            name: 'Complex Agent',
            metadata: {
              tags: ['research', 'analysis'],
              config: {
                level1: {
                  level2: {
                    level3: {
                      value: 'deep'
                    }
                  }
                }
              }
            }
          }
        },
        connections: [
          {
            from: 'agent-1',
            to: 'agent-2',
            template: {
              header: 'Test',
              body: { nested: { data: [1, 2, 3] } }
            }
          }
        ],
        auto_forward: false
      };

      manager.saveCurrentTabWorkflow();

      const tab = manager.workflowTabs[0];
      expect(tab.workflow.agents['agent-1'].metadata.config.level1.level2.level3.value).toBe('deep');
      expect(tab.workflow.connections[0].template.body.nested.data).toEqual([1, 2, 3]);
    });

    test('should handle null graph', () => {
      manager.createNewTab('Null Graph Tab');
      manager.graph = null;

      const result = manager.saveCurrentTabWorkflow();

      expect(result).toBe(false);
      // Tab should keep its initial empty workflow
      expect(manager.workflowTabs[0].workflow).toEqual({
        agents: {},
        connections: [],
        auto_forward: false
      });
    });

    test('should handle undefined graph', () => {
      manager.createNewTab('Undefined Graph Tab');
      manager.graph = undefined;

      const result = manager.saveCurrentTabWorkflow();

      expect(result).toBe(false);
    });
  });

  describe('Edge cases', () => {
    test('should return false when no active tab', () => {
      manager.graph = { agents: {}, connections: [], auto_forward: false };

      const result = manager.saveCurrentTabWorkflow();

      expect(result).toBe(false);
    });

    test('should return false when active tab not found', () => {
      manager.createNewTab('Test Tab');
      manager.activeTabId = 'non-existent-tab';
      manager.graph = { agents: {}, connections: [], auto_forward: false };

      const result = manager.saveCurrentTabWorkflow();

      expect(result).toBe(false);
    });

    test('should persist to localStorage', () => {
      manager.createNewTab('Persist Tab');
      manager.graph = {
        agents: { 'a1': { name: 'Test' } },
        connections: [],
        auto_forward: false
      };

      manager.saveCurrentTabWorkflow();

      const stored = JSON.parse(localStorage.getItem('gluon_workflow_tabs'));
      expect(stored.tabs[0].workflow.agents['a1'].name).toBe('Test');
    });

    test('should handle circular reference gracefully (JSON.stringify limitation)', () => {
      manager.createNewTab('Circular Tab');

      // Create circular reference
      const agent = { id: 'agent-1', name: 'Circular' };
      agent.self = agent; // Circular reference

      manager.graph = {
        agents: { 'agent-1': agent },
        connections: [],
        auto_forward: false
      };

      // This should throw because JSON.stringify can't handle circular refs
      expect(() => manager.saveCurrentTabWorkflow()).toThrow();
    });
  });

  describe('Multiple tab scenarios', () => {
    test('should save different graphs to different tabs', () => {
      const tab1Id = manager.createNewTab('Tab 1');
      manager.graph = { agents: { 'a1': { name: 'Agent 1' } }, connections: [], auto_forward: false };
      manager.saveCurrentTabWorkflow();

      const tab2Id = manager.createNewTab('Tab 2');
      manager.graph = { agents: { 'a2': { name: 'Agent 2' } }, connections: [], auto_forward: false };
      manager.saveCurrentTabWorkflow();

      const tab1 = manager.workflowTabs.find(t => t.id === tab1Id);
      const tab2 = manager.workflowTabs.find(t => t.id === tab2Id);

      expect(tab1.workflow.agents).toHaveProperty('a1');
      expect(tab2.workflow.agents).toHaveProperty('a2');
      expect(tab1.workflow.agents).not.toHaveProperty('a2');
      expect(tab2.workflow.agents).not.toHaveProperty('a1');
    });

    test('should save current tab workflow when switching tabs', () => {
      const tab1Id = manager.createNewTab('Tab 1');
      manager.graph = { agents: { 'a1': { name: 'Agent 1' } }, connections: [], auto_forward: false };

      const tab2Id = manager.createNewTab('Tab 2');

      // Switch back to tab 1 should trigger save
      manager.graph = { agents: { 'a2': { name: 'Agent 2' } }, connections: [], auto_forward: false };
      manager.switchToTab(tab1Id);

      const tab2 = manager.workflowTabs.find(t => t.id === tab2Id);
      expect(tab2.workflow.agents).toHaveProperty('a2');
    });
  });
});

describe('WorkflowManager - Tab Lifecycle', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
  });

  describe('createNewTab()', () => {
    test('should create tab with default name', () => {
      const tabId = manager.createNewTab();

      expect(manager.workflowTabs).toHaveLength(1);
      expect(manager.workflowTabs[0].id).toBe(tabId);
      expect(manager.workflowTabs[0].name).toBe('Workflow 1');
    });

    test('should create tab with custom name', () => {
      const tabId = manager.createNewTab('Custom Workflow');

      expect(manager.workflowTabs[0].name).toBe('Custom Workflow');
    });

    test('should increment tab counter', () => {
      manager.createNewTab();
      manager.createNewTab();
      manager.createNewTab();

      expect(manager.nextTabId).toBe(4);
      expect(manager.workflowTabs[0].id).toBe('tab-1');
      expect(manager.workflowTabs[1].id).toBe('tab-2');
      expect(manager.workflowTabs[2].id).toBe('tab-3');
    });

    test('should make new tab active by default', () => {
      const tabId = manager.createNewTab();

      expect(manager.activeTabId).toBe(tabId);
    });

    test('should not make tab active when makeActive is false', () => {
      const tab1Id = manager.createNewTab('Tab 1');
      const tab2Id = manager.createNewTab('Tab 2', false);

      expect(manager.activeTabId).toBe(tab1Id);
      expect(manager.activeTabId).not.toBe(tab2Id);
    });

    test('should initialize tab with empty workflow', () => {
      manager.createNewTab('Empty Tab');

      const tab = manager.workflowTabs[0];
      expect(tab.workflow).toEqual({
        agents: {},
        connections: [],
        auto_forward: false
      });
    });

    test('should set timestamps', () => {
      const beforeTime = Date.now();
      manager.createNewTab('Test');
      const afterTime = Date.now();

      const tab = manager.workflowTabs[0];
      expect(tab.createdAt).toBeGreaterThanOrEqual(beforeTime);
      expect(tab.createdAt).toBeLessThanOrEqual(afterTime);
      expect(tab.modifiedAt).toBe(tab.createdAt);
    });
  });

  describe('switchToTab()', () => {
    test('should switch to existing tab', () => {
      const tab1 = manager.createNewTab('Tab 1');
      const tab2 = manager.createNewTab('Tab 2');

      const result = manager.switchToTab(tab1);

      expect(result).toBe(true);
      expect(manager.activeTabId).toBe(tab1);
    });

    test('should return false for non-existent tab', () => {
      manager.createNewTab('Tab 1');

      const result = manager.switchToTab('non-existent');

      expect(result).toBe(false);
      expect(manager.activeTabId).toBe('tab-1');
    });

    test('should load tab workflow into graph', () => {
      const tab1 = manager.createNewTab('Tab 1');
      manager.graph = { agents: { 'a1': { name: 'Agent 1' } }, connections: [], auto_forward: false };
      manager.saveCurrentTabWorkflow();

      const tab2 = manager.createNewTab('Tab 2');
      manager.graph = { agents: { 'a2': { name: 'Agent 2' } }, connections: [], auto_forward: false };
      manager.saveCurrentTabWorkflow();

      manager.switchToTab(tab1);

      expect(manager.graph.agents).toHaveProperty('a1');
      expect(manager.graph.agents).not.toHaveProperty('a2');
    });

    test('should save current tab before switching', () => {
      const tab1 = manager.createNewTab('Tab 1');
      manager.graph = { agents: { 'original': { name: 'Original' } }, connections: [], auto_forward: false };

      const tab2 = manager.createNewTab('Tab 2');
      manager.graph = { agents: { 'modified': { name: 'Modified' } }, connections: [], auto_forward: false };

      manager.switchToTab(tab1);

      const tab2Data = manager.workflowTabs.find(t => t.id === tab2);
      expect(tab2Data.workflow.agents).toHaveProperty('modified');
    });
  });

  describe('closeTab()', () => {
    test('should close tab successfully', () => {
      manager.createNewTab('Tab 1');
      const tab2 = manager.createNewTab('Tab 2');

      const result = manager.closeTab(tab2);

      expect(result).toBe(true);
      expect(manager.workflowTabs).toHaveLength(1);
      expect(manager.workflowTabs[0].name).toBe('Tab 1');
    });

    test('should prevent closing last tab', () => {
      const tabId = manager.createNewTab('Only Tab');

      const result = manager.closeTab(tabId);

      expect(result).toBe(false);
      expect(manager.workflowTabs).toHaveLength(1);
    });

    test('should return false for non-existent tab', () => {
      manager.createNewTab('Tab 1');

      const result = manager.closeTab('non-existent');

      expect(result).toBe(false);
      expect(manager.workflowTabs).toHaveLength(1);
    });

    test('should switch to next tab when closing active tab', () => {
      const tab1 = manager.createNewTab('Tab 1');
      const tab2 = manager.createNewTab('Tab 2');
      const tab3 = manager.createNewTab('Tab 3');

      manager.closeTab(tab2);

      expect(manager.activeTabId).toBe(tab3);
      expect(manager.workflowTabs).toHaveLength(2);
    });

    test('should switch to previous tab when closing last tab', () => {
      const tab1 = manager.createNewTab('Tab 1');
      const tab2 = manager.createNewTab('Tab 2');
      const tab3 = manager.createNewTab('Tab 3');

      manager.closeTab(tab3);

      expect(manager.activeTabId).toBe(tab2);
    });

    test('should not switch when closing non-active tab', () => {
      const tab1 = manager.createNewTab('Tab 1');
      const tab2 = manager.createNewTab('Tab 2');
      const tab3 = manager.createNewTab('Tab 3');

      manager.switchToTab(tab3);
      manager.closeTab(tab1);

      expect(manager.activeTabId).toBe(tab3);
      expect(manager.workflowTabs).toHaveLength(2);
    });
  });

  describe('duplicateTab()', () => {
    test('should duplicate tab with workflow', () => {
      const originalId = manager.createNewTab('Original');
      manager.graph = {
        agents: { 'a1': { name: 'Agent 1' } },
        connections: [{ from: 'a1', to: 'a2' }],
        auto_forward: true
      };
      manager.saveCurrentTabWorkflow();

      const duplicateId = manager.duplicateTab(originalId);

      expect(duplicateId).toBeTruthy();
      expect(manager.workflowTabs).toHaveLength(2);

      const duplicate = manager.workflowTabs.find(t => t.id === duplicateId);
      expect(duplicate.name).toBe('Original (kopia)');
      expect(duplicate.workflow.agents).toHaveProperty('a1');
      expect(duplicate.workflow.auto_forward).toBe(true);
    });

    test('should create independent copy', () => {
      const originalId = manager.createNewTab('Original');
      manager.graph = { agents: { 'a1': { name: 'Original Agent' } }, connections: [], auto_forward: false };
      manager.saveCurrentTabWorkflow();

      const duplicateId = manager.duplicateTab(originalId);

      // Modify duplicate
      manager.switchToTab(duplicateId);
      manager.graph.agents['a1'].name = 'Modified Agent';
      manager.saveCurrentTabWorkflow();

      // Original should be unchanged
      const original = manager.workflowTabs.find(t => t.id === originalId);
      expect(original.workflow.agents['a1'].name).toBe('Original Agent');
    });

    test('should return null for non-existent tab', () => {
      manager.createNewTab('Tab 1');

      const result = manager.duplicateTab('non-existent');

      expect(result).toBeNull();
      expect(manager.workflowTabs).toHaveLength(1);
    });
  });

  describe('renameTab()', () => {
    test('should rename tab successfully', () => {
      const tabId = manager.createNewTab('Old Name');

      const result = manager.renameTab(tabId, 'New Name');

      expect(result).toBe(true);
      expect(manager.workflowTabs[0].name).toBe('New Name');
    });

    test('should update modifiedAt timestamp', () => {
      const tabId = manager.createNewTab('Test');
      const originalTime = manager.workflowTabs[0].modifiedAt;

      jest.advanceTimersByTime(100);

      manager.renameTab(tabId, 'Renamed');

      expect(manager.workflowTabs[0].modifiedAt).toBeGreaterThanOrEqual(originalTime);
    });

    test('should return false for non-existent tab', () => {
      manager.createNewTab('Tab 1');

      const result = manager.renameTab('non-existent', 'New Name');

      expect(result).toBe(false);
      expect(manager.workflowTabs[0].name).toBe('Tab 1');
    });
  });

  describe('Persistence', () => {
    test('should persist tab state to localStorage', () => {
      manager.createNewTab('Tab 1');
      manager.createNewTab('Tab 2');

      const stored = JSON.parse(localStorage.getItem('gluon_workflow_tabs'));

      expect(stored.tabs).toHaveLength(2);
      expect(stored.activeTabId).toBe('tab-2');
      expect(stored.nextTabId).toBe(3);
    });

    test('should load tab state from localStorage', () => {
      const mockData = {
        tabs: [
          {
            id: 'tab-1',
            name: 'Persisted Tab',
            workflow: { agents: { 'a1': { name: 'Agent' } }, connections: [], auto_forward: false },
            createdAt: 123456,
            modifiedAt: 123456
          }
        ],
        activeTabId: 'tab-1',
        nextTabId: 2
      };

      localStorage.setItem('gluon_workflow_tabs', JSON.stringify(mockData));

      manager.loadTabsFromStorage();

      expect(manager.workflowTabs).toHaveLength(1);
      expect(manager.workflowTabs[0].name).toBe('Persisted Tab');
      expect(manager.activeTabId).toBe('tab-1');
      expect(manager.nextTabId).toBe(2);
    });

    test('should handle corrupted localStorage data', () => {
      localStorage.setItem('gluon_workflow_tabs', 'invalid json {');

      manager.loadTabsFromStorage();

      expect(manager.workflowTabs).toEqual([]);
    });

    test('should handle missing localStorage data', () => {
      localStorage.removeItem('gluon_workflow_tabs');

      manager.loadTabsFromStorage();

      expect(manager.workflowTabs).toEqual([]);
    });
  });
});
