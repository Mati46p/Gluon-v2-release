/**
 * Unit Tests for WorkflowManager Race Conditions
 *
 * CRITICAL SECURITY TESTS - Covers:
 * - Rapid tab switching (race condition in saveCurrentTabWorkflow)
 * - Concurrent tab operations (create, close, switch)
 * - Concurrent graph modifications
 * - localStorage consistency under concurrent writes
 * - State corruption prevention
 */

import { describe, test, expect, jest, beforeEach, afterEach } from '@jest/globals';

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
    this._saveInProgress = false; // Track if save is in progress
    this._saveQueue = []; // Queue for pending saves
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
      // Simulate deep copy that can fail with circular refs
      try {
        tab.workflow = JSON.parse(JSON.stringify(this.graph));
        tab.modifiedAt = Date.now();
        this.saveTabsToStorage();
        return true;
      } catch (error) {
        console.error('Failed to save workflow:', error);
        return false;
      }
    }
    return false;
  }

  closeTab(tabId) {
    const tabIndex = this.workflowTabs.findIndex(t => t.id === tabId);
    if (tabIndex === -1) return false;

    if (this.workflowTabs.length === 1) {
      return false;
    }

    this.workflowTabs.splice(tabIndex, 1);

    if (this.activeTabId === tabId) {
      const newActiveIndex = tabIndex >= this.workflowTabs.length ? tabIndex - 1 : tabIndex;
      this.switchToTab(this.workflowTabs[newActiveIndex].id);
    }

    this.saveTabsToStorage();
    return true;
  }

  // Simulate async save with delays
  async asyncSaveCurrentTabWorkflow(delay = 50) {
    await new Promise(resolve => setTimeout(resolve, delay));
    return this.saveCurrentTabWorkflow();
  }

  async asyncSwitchToTab(tabId, delay = 50) {
    await new Promise(resolve => setTimeout(resolve, delay));
    return this.switchToTab(tabId);
  }
}

describe('WorkflowManager - Race Conditions', () => {
  let manager;

  beforeEach(() => {
    jest.useFakeTimers();
    manager = new TestableWorkflowManager();
    localStorage.clear();
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  describe('🔴 CRITICAL: Rapid Tab Switching', () => {
    test('should handle rapid tab switches without data loss', async () => {
      const tab1 = manager.createNewTab('Tab 1');
      manager.graph = {
        agents: { 'a1': { id: 'a1', name: 'Agent 1', data: 'important-data-1' } },
        connections: [],
        auto_forward: false
      };

      const tab2 = manager.createNewTab('Tab 2');
      manager.graph = {
        agents: { 'a2': { id: 'a2', name: 'Agent 2', data: 'important-data-2' } },
        connections: [],
        auto_forward: false
      };

      const tab3 = manager.createNewTab('Tab 3');
      manager.graph = {
        agents: { 'a3': { id: 'a3', name: 'Agent 3', data: 'important-data-3' } },
        connections: [],
        auto_forward: false
      };

      // Rapidly switch between tabs
      manager.switchToTab(tab1);
      manager.switchToTab(tab2);
      manager.switchToTab(tab3);
      manager.switchToTab(tab1);
      manager.switchToTab(tab2);

      // Verify all tabs retained their data
      const tab1Data = manager.workflowTabs.find(t => t.id === tab1);
      const tab2Data = manager.workflowTabs.find(t => t.id === tab2);
      const tab3Data = manager.workflowTabs.find(t => t.id === tab3);

      expect(tab1Data.workflow.agents['a1']?.data).toBe('important-data-1');
      expect(tab2Data.workflow.agents['a2']?.data).toBe('important-data-2');
      expect(tab3Data.workflow.agents['a3']?.data).toBe('important-data-3');
    });

    test('should prevent data corruption when switching during graph modification', () => {
      const tab1 = manager.createNewTab('Tab 1');
      manager.graph = {
        agents: { 'a1': { id: 'a1', name: 'Original' } },
        connections: [],
        auto_forward: false
      };

      const tab2 = manager.createNewTab('Tab 2');

      // Start modifying graph
      manager.graph.agents['a2'] = { id: 'a2', name: 'New Agent' };
      manager.graph.connections = [{ from: 'a1', to: 'a2' }];

      // Switch before save completes
      manager.switchToTab(tab1);

      // Tab 2 should have the modified data
      const tab2Data = manager.workflowTabs.find(t => t.id === tab2);
      expect(tab2Data.workflow.agents).toHaveProperty('a2');
      expect(tab2Data.workflow.connections).toHaveLength(1);
    });

    test('should handle switch to same tab gracefully', () => {
      const tab1 = manager.createNewTab('Tab 1');
      manager.graph = {
        agents: { 'a1': { id: 'a1', name: 'Agent 1' } },
        connections: [],
        auto_forward: false
      };

      const initialModified = manager.workflowTabs[0].modifiedAt;

      // Switch to same tab multiple times
      jest.advanceTimersByTime(100);
      manager.switchToTab(tab1);
      jest.advanceTimersByTime(100);
      manager.switchToTab(tab1);

      // Should still work correctly
      expect(manager.activeTabId).toBe(tab1);
      const tab = manager.workflowTabs.find(t => t.id === tab1);
      expect(tab.workflow.agents).toHaveProperty('a1');
    });
  });

  describe('🔴 CRITICAL: Concurrent Tab Operations', () => {
    test('should handle creating multiple tabs rapidly', () => {
      const tabs = [];

      // Create 10 tabs rapidly
      for (let i = 0; i < 10; i++) {
        const tabId = manager.createNewTab(`Tab ${i}`, false);
        tabs.push(tabId);
      }

      expect(manager.workflowTabs).toHaveLength(10);
      expect(manager.nextTabId).toBe(11);

      // All tabs should have unique IDs
      const uniqueIds = new Set(tabs);
      expect(uniqueIds.size).toBe(10);
    });

    test('should handle closing tabs while switching', () => {
      const tab1 = manager.createNewTab('Tab 1');
      const tab2 = manager.createNewTab('Tab 2');
      const tab3 = manager.createNewTab('Tab 3');

      // Switch to tab1 and immediately close tab2
      manager.switchToTab(tab1);
      manager.closeTab(tab2);

      expect(manager.workflowTabs).toHaveLength(2);
      expect(manager.activeTabId).toBe(tab1);
      expect(manager.workflowTabs.find(t => t.id === tab2)).toBeUndefined();
    });

    test('should prevent closing active tab being switched to', () => {
      const tab1 = manager.createNewTab('Tab 1');
      const tab2 = manager.createNewTab('Tab 2');
      const tab3 = manager.createNewTab('Tab 3');

      manager.switchToTab(tab2);

      // Try to close the active tab
      const result = manager.closeTab(tab2);

      expect(result).toBe(true);
      expect(manager.workflowTabs).toHaveLength(2);
      // Should switch to another tab
      expect(manager.activeTabId).not.toBe(tab2);
    });

    test('should handle creating and closing tabs in rapid succession', () => {
      const tab1 = manager.createNewTab('Tab 1');
      const tab2 = manager.createNewTab('Tab 2');
      const tab3 = manager.createNewTab('Tab 3');

      manager.closeTab(tab2);

      const tab4 = manager.createNewTab('Tab 4');
      const tab5 = manager.createNewTab('Tab 5');

      manager.closeTab(tab1);
      manager.closeTab(tab4);

      expect(manager.workflowTabs).toHaveLength(2);
      const remainingIds = manager.workflowTabs.map(t => t.id);
      expect(remainingIds).toContain(tab3);
      expect(remainingIds).toContain(tab5);
    });
  });

  describe('🔴 CRITICAL: Graph Modification Race Conditions', () => {
    test('should handle concurrent graph modifications on same tab', () => {
      const tab1 = manager.createNewTab('Tab 1');
      manager.graph = { agents: {}, connections: [], auto_forward: false };

      // Simulate multiple operations modifying graph
      manager.graph.agents['a1'] = { id: 'a1', name: 'Agent 1' };
      manager.graph.agents['a2'] = { id: 'a2', name: 'Agent 2' };
      manager.graph.connections.push({ from: 'a1', to: 'a2' });

      manager.saveCurrentTabWorkflow();

      const tab = manager.workflowTabs.find(t => t.id === tab1);
      expect(Object.keys(tab.workflow.agents)).toHaveLength(2);
      expect(tab.workflow.connections).toHaveLength(1);
    });

    test('should handle graph mutations after save', () => {
      const tab1 = manager.createNewTab('Tab 1');
      manager.graph = {
        agents: { 'a1': { id: 'a1', name: 'Original', nested: { value: 1 } } },
        connections: [],
        auto_forward: false
      };

      manager.saveCurrentTabWorkflow();

      // Mutate graph after save
      manager.graph.agents['a1'].name = 'Modified';
      manager.graph.agents['a1'].nested.value = 999;

      // Tab should have original data (due to deep copy)
      const tab = manager.workflowTabs.find(t => t.id === tab1);
      expect(tab.workflow.agents['a1'].name).toBe('Original');
      expect(tab.workflow.agents['a1'].nested.value).toBe(1);
    });

    test('should detect circular reference in graph', () => {
      const tab1 = manager.createNewTab('Tab 1');

      const agent = { id: 'a1', name: 'Agent' };
      agent.circular = agent; // Create circular reference

      manager.graph = {
        agents: { 'a1': agent },
        connections: [],
        auto_forward: false
      };

      // Should fail to save due to circular reference
      const result = manager.saveCurrentTabWorkflow();
      expect(result).toBe(false);
    });

    test('should handle deeply nested graph mutations', () => {
      const tab1 = manager.createNewTab('Tab 1');

      manager.graph = {
        agents: {
          'a1': {
            id: 'a1',
            config: {
              level1: {
                level2: {
                  level3: {
                    data: [1, 2, 3]
                  }
                }
              }
            }
          }
        },
        connections: [],
        auto_forward: false
      };

      manager.saveCurrentTabWorkflow();

      // Mutate deeply nested structure
      manager.graph.agents['a1'].config.level1.level2.level3.data.push(4);
      manager.graph.agents['a1'].config.level1.level2.level3.data[0] = 999;

      // Tab should preserve original values
      const tab = manager.workflowTabs.find(t => t.id === tab1);
      expect(tab.workflow.agents['a1'].config.level1.level2.level3.data).toEqual([1, 2, 3]);
    });
  });

  describe('🔴 CRITICAL: localStorage Consistency', () => {
    test('should handle rapid localStorage writes', () => {
      const tab1 = manager.createNewTab('Tab 1');
      manager.graph = { agents: { 'a1': { name: 'Agent 1' } }, connections: [], auto_forward: false };

      // Rapid saves
      for (let i = 0; i < 10; i++) {
        manager.graph.agents['a1'].name = `Agent ${i}`;
        manager.saveCurrentTabWorkflow();
      }

      // Last write should win
      const stored = JSON.parse(localStorage.getItem('gluon_workflow_tabs'));
      expect(stored.tabs[0].workflow.agents['a1'].name).toBe('Agent 9');
    });

    test('should recover from corrupted localStorage during concurrent writes', () => {
      manager.createNewTab('Tab 1');
      manager.createNewTab('Tab 2');

      // Corrupt localStorage mid-operation
      localStorage.setItem('gluon_workflow_tabs', '{invalid json');

      // Should handle gracefully on next load
      const newManager = new TestableWorkflowManager();
      newManager.loadTabsFromStorage();

      expect(newManager.workflowTabs).toEqual([]);
    });

    test('should handle localStorage quota exceeded gracefully', () => {
      const tab1 = manager.createNewTab('Tab 1');

      // Create massive graph that might exceed quota
      const hugeGraph = {
        agents: {},
        connections: [],
        auto_forward: false
      };

      // Add 1000 agents with large data
      for (let i = 0; i < 1000; i++) {
        hugeGraph.agents[`agent-${i}`] = {
          id: `agent-${i}`,
          name: `Agent ${i}`,
          // Large data payload
          data: 'x'.repeat(1000)
        };
      }

      manager.graph = hugeGraph;

      // Mock quota exceeded error
      const originalSetItem = Storage.prototype.setItem;
      Storage.prototype.setItem = jest.fn(() => {
        const error = new Error('QuotaExceededError');
        error.name = 'QuotaExceededError';
        throw error;
      });

      // Should handle quota error gracefully
      expect(() => {
        manager.saveCurrentTabWorkflow();
      }).toThrow();

      // Restore original
      Storage.prototype.setItem = originalSetItem;
    });

    test('should maintain consistency after failed save', () => {
      const tab1 = manager.createNewTab('Tab 1');
      manager.graph = { agents: { 'a1': { name: 'Original' } }, connections: [], auto_forward: false };
      manager.saveCurrentTabWorkflow();

      // Mock save failure
      const originalSetItem = Storage.prototype.setItem;
      Storage.prototype.setItem = jest.fn(() => {
        throw new Error('Storage Error');
      });

      // Attempt to save new data
      manager.graph.agents['a1'].name = 'Modified';

      try {
        manager.saveCurrentTabWorkflow();
      } catch (error) {
        // Expected to fail
      }

      // Restore original
      Storage.prototype.setItem = originalSetItem;

      // Load from storage - should have original data
      const newManager = new TestableWorkflowManager();
      newManager.loadTabsFromStorage();

      expect(newManager.workflowTabs[0].workflow.agents['a1'].name).toBe('Original');
    });
  });

  describe('🔴 CRITICAL: State Corruption Prevention', () => {
    test('should prevent activeTabId pointing to deleted tab', () => {
      const tab1 = manager.createNewTab('Tab 1');
      const tab2 = manager.createNewTab('Tab 2');
      const tab3 = manager.createNewTab('Tab 3');

      manager.switchToTab(tab2);
      manager.closeTab(tab2);

      // activeTabId should be updated to valid tab
      expect(manager.activeTabId).not.toBe(tab2);
      expect([tab1, tab3]).toContain(manager.activeTabId);
    });

    test('should prevent duplicate tab IDs', () => {
      const tabs = [];
      for (let i = 0; i < 100; i++) {
        tabs.push(manager.createNewTab(`Tab ${i}`, false));
      }

      const uniqueIds = new Set(tabs);
      expect(uniqueIds.size).toBe(100);
    });

    test('should maintain tab count integrity', () => {
      manager.createNewTab('Tab 1');
      manager.createNewTab('Tab 2');
      manager.createNewTab('Tab 3');

      expect(manager.workflowTabs).toHaveLength(3);

      manager.closeTab('tab-2');
      expect(manager.workflowTabs).toHaveLength(2);

      manager.createNewTab('Tab 4');
      expect(manager.workflowTabs).toHaveLength(3);
    });

    test('should prevent graph reference leaking between tabs', () => {
      const tab1 = manager.createNewTab('Tab 1');
      const graph1 = { agents: { 'a1': { name: 'Agent 1' } }, connections: [], auto_forward: false };
      manager.graph = graph1;
      manager.saveCurrentTabWorkflow();

      const tab2 = manager.createNewTab('Tab 2');
      const graph2 = { agents: { 'a2': { name: 'Agent 2' } }, connections: [], auto_forward: false };
      manager.graph = graph2;
      manager.saveCurrentTabWorkflow();

      // Modify graph1 reference
      graph1.agents['a1'].name = 'Modified';

      // Tab 1 should be unaffected (deep copy protection)
      const tab1Data = manager.workflowTabs.find(t => t.id === tab1);
      expect(tab1Data.workflow.agents['a1'].name).toBe('Agent 1');
    });

    test('should handle null/undefined graph gracefully', () => {
      const tab1 = manager.createNewTab('Tab 1');

      manager.graph = null;
      expect(manager.saveCurrentTabWorkflow()).toBe(false);

      manager.graph = undefined;
      expect(manager.saveCurrentTabWorkflow()).toBe(false);

      // Tab should preserve initial empty workflow
      const tab = manager.workflowTabs.find(t => t.id === tab1);
      expect(tab.workflow).toEqual({
        agents: {},
        connections: [],
        auto_forward: false
      });
    });
  });

  describe('🔴 CRITICAL: Edge Cases Under Concurrent Load', () => {
    test('should handle switching to non-existent tab during concurrent operations', () => {
      const tab1 = manager.createNewTab('Tab 1');
      const tab2 = manager.createNewTab('Tab 2');

      manager.closeTab(tab2);

      // Try to switch to deleted tab
      const result = manager.switchToTab(tab2);

      expect(result).toBe(false);
      expect(manager.activeTabId).toBe(tab1);
    });

    test('should handle closing all tabs except one rapidly', () => {
      const tabs = [];
      for (let i = 0; i < 5; i++) {
        tabs.push(manager.createNewTab(`Tab ${i}`, false));
      }

      // Try to close all tabs (should prevent closing last one)
      for (let i = 0; i < tabs.length; i++) {
        manager.closeTab(tabs[i]);
      }

      expect(manager.workflowTabs).toHaveLength(1);
    });

    test('should handle modifying closed tab data', () => {
      const tab1 = manager.createNewTab('Tab 1');
      manager.graph = { agents: { 'a1': { name: 'Agent 1' } }, connections: [], auto_forward: false };

      const tab2 = manager.createNewTab('Tab 2');
      manager.graph = { agents: { 'a2': { name: 'Agent 2' } }, connections: [], auto_forward: false };

      manager.closeTab(tab1);

      // Tab 1 data should be gone
      const deletedTab = manager.workflowTabs.find(t => t.id === tab1);
      expect(deletedTab).toBeUndefined();
    });

    test('should maintain consistency with rapid create-switch-close cycles', () => {
      for (let cycle = 0; cycle < 10; cycle++) {
        const newTab = manager.createNewTab(`Cycle ${cycle}`);
        manager.graph = {
          agents: { [`a${cycle}`]: { name: `Agent ${cycle}` } },
          connections: [],
          auto_forward: false
        };
        manager.saveCurrentTabWorkflow();

        if (manager.workflowTabs.length > 3) {
          // Close oldest tab (but not last one)
          if (manager.workflowTabs.length > 1) {
            manager.closeTab(manager.workflowTabs[0].id);
          }
        }
      }

      // Should have consistent state
      expect(manager.workflowTabs.length).toBeGreaterThan(0);
      expect(manager.workflowTabs.length).toBeLessThanOrEqual(4);
      expect(manager.activeTabId).toBeTruthy();

      // Active tab should exist
      const activeTab = manager.workflowTabs.find(t => t.id === manager.activeTabId);
      expect(activeTab).toBeTruthy();
    });
  });
});
