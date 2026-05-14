/**
 * Unit Tests for WorkflowManager JSON Parsing Errors
 *
 * CRITICAL SECURITY TESTS - Covers:
 * - JSON.parse errors in localStorage operations
 * - Corrupted data recovery
 * - Malformed JSON handling
 * - Data integrity validation
 * - Circular reference handling
 * - Large payload handling
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
  }

  saveTabsToStorage() {
    try {
      const data = {
        tabs: this.workflowTabs,
        activeTabId: this.activeTabId,
        nextTabId: this.nextTabId
      };
      localStorage.setItem('gluon_workflow_tabs', JSON.stringify(data));
      return true;
    } catch (error) {
      console.error('Failed to save tabs to storage:', error);
      return false;
    }
  }

  loadTabsFromStorage() {
    const stored = localStorage.getItem('gluon_workflow_tabs');
    if (!stored) {
      this.workflowTabs = [];
      this.activeTabId = null;
      this.nextTabId = 1;
      return { success: true, source: 'empty' };
    }

    try {
      const data = JSON.parse(stored);

      // Validate structure
      if (!data || typeof data !== 'object') {
        throw new Error('Invalid data structure');
      }

      // Validate tabs array
      if (!Array.isArray(data.tabs)) {
        throw new Error('Tabs is not an array');
      }

      // Validate each tab
      data.tabs.forEach((tab, index) => {
        if (!tab.id || !tab.name || !tab.workflow) {
          throw new Error(`Invalid tab structure at index ${index}`);
        }
      });

      this.workflowTabs = data.tabs || [];
      this.activeTabId = data.activeTabId || null;
      this.nextTabId = data.nextTabId || 1;

      return { success: true, source: 'storage', count: this.workflowTabs.length };
    } catch (error) {
      console.error('Failed to parse tabs from storage:', error);

      // Reset to safe state
      this.workflowTabs = [];
      this.activeTabId = null;
      this.nextTabId = 1;

      // Try to backup corrupted data
      try {
        localStorage.setItem('gluon_workflow_tabs_corrupted_backup', stored);
      } catch (backupError) {
        console.error('Failed to backup corrupted data:', backupError);
      }

      return { success: false, error: error.message };
    }
  }

  saveCurrentTabWorkflow() {
    if (!this.activeTabId) return { success: false, error: 'No active tab' };

    const tab = this.workflowTabs.find(t => t.id === this.activeTabId);
    if (!tab) {
      return { success: false, error: 'Tab not found' };
    }

    if (!this.graph) {
      return { success: false, error: 'No graph to save' };
    }

    try {
      // Test if graph can be serialized (detects circular refs)
      JSON.stringify(this.graph);

      // Deep copy
      tab.workflow = JSON.parse(JSON.stringify(this.graph));
      tab.modifiedAt = Date.now();

      const saveResult = this.saveTabsToStorage();
      return {
        success: saveResult,
        error: saveResult ? null : 'Failed to save to storage'
      };
    } catch (error) {
      console.error('Failed to save workflow:', error);
      return { success: false, error: error.message };
    }
  }

  importTabFromFile(jsonString) {
    try {
      const importData = JSON.parse(jsonString);

      // Validate import data structure
      if (!importData.type || importData.type !== 'gluon_workflow_tab') {
        throw new Error('Invalid file type');
      }

      if (!importData.tab || !importData.tab.workflow) {
        throw new Error('Missing workflow data');
      }

      // Validate workflow structure
      const workflow = importData.tab.workflow;
      if (typeof workflow !== 'object') {
        throw new Error('Workflow is not an object');
      }

      if (!workflow.agents || typeof workflow.agents !== 'object') {
        throw new Error('Invalid agents structure');
      }

      if (!Array.isArray(workflow.connections)) {
        throw new Error('Connections is not an array');
      }

      // Validate agents
      Object.values(workflow.agents).forEach((agent, index) => {
        if (!agent.id || !agent.name) {
          throw new Error(`Invalid agent structure at index ${index}`);
        }
      });

      // Validate connections
      workflow.connections.forEach((conn, index) => {
        if (!conn.from_agent_id || !conn.to_agent_id) {
          throw new Error(`Invalid connection structure at index ${index}`);
        }
      });

      return { success: true, data: importData };
    } catch (error) {
      console.error('Failed to import tab:', error);
      return { success: false, error: error.message };
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
      this.activeTabId = tabId;
      this.graph = newTab.workflow;
    }

    this.saveTabsToStorage();
    return tabId;
  }
}

describe('WorkflowManager - JSON Parsing Error Handling', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
    localStorage.clear();
  });

  afterEach(() => {
    localStorage.clear();
  });

  describe('🔴 CRITICAL: localStorage JSON Parsing', () => {
    test('should handle corrupted JSON in localStorage', () => {
      localStorage.setItem('gluon_workflow_tabs', '{invalid json');

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
      expect(result.error).toBeTruthy();
      // Should reset to safe state
      expect(manager.workflowTabs).toEqual([]);
      expect(manager.activeTabId).toBeNull();
    });

    test('should handle incomplete JSON object', () => {
      localStorage.setItem('gluon_workflow_tabs', '{"tabs": [{"id": "tab-1"');

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
      expect(manager.workflowTabs).toEqual([]);
    });

    test('should handle empty localStorage gracefully', () => {
      localStorage.removeItem('gluon_workflow_tabs');

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(true);
      expect(result.source).toBe('empty');
      expect(manager.workflowTabs).toEqual([]);
    });

    test('should handle null value in localStorage', () => {
      localStorage.setItem('gluon_workflow_tabs', 'null');

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
      expect(manager.workflowTabs).toEqual([]);
    });

    test('should handle undefined string in localStorage', () => {
      localStorage.setItem('gluon_workflow_tabs', 'undefined');

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
    });

    test('should handle non-object JSON', () => {
      localStorage.setItem('gluon_workflow_tabs', '"just a string"');

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
      expect(manager.workflowTabs).toEqual([]);
    });

    test('should handle array instead of object', () => {
      localStorage.setItem('gluon_workflow_tabs', '[]');

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
    });
  });

  describe('🔴 CRITICAL: Data Structure Validation', () => {
    test('should reject data with missing tabs array', () => {
      localStorage.setItem('gluon_workflow_tabs', JSON.stringify({
        activeTabId: 'tab-1',
        nextTabId: 2
        // Missing tabs array
      }));

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
      expect(manager.workflowTabs).toEqual([]);
    });

    test('should reject data with tabs as non-array', () => {
      localStorage.setItem('gluon_workflow_tabs', JSON.stringify({
        tabs: 'not an array',
        activeTabId: 'tab-1',
        nextTabId: 2
      }));

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
    });

    test('should reject tabs with invalid structure', () => {
      localStorage.setItem('gluon_workflow_tabs', JSON.stringify({
        tabs: [
          {
            id: 'tab-1',
            // Missing name and workflow
          }
        ],
        activeTabId: 'tab-1',
        nextTabId: 2
      }));

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
    });

    test('should validate complete tab structure', () => {
      localStorage.setItem('gluon_workflow_tabs', JSON.stringify({
        tabs: [
          {
            id: 'tab-1',
            name: 'Test Tab',
            workflow: {
              agents: {},
              connections: [],
              auto_forward: false
            },
            createdAt: Date.now(),
            modifiedAt: Date.now()
          }
        ],
        activeTabId: 'tab-1',
        nextTabId: 2
      }));

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(true);
      expect(manager.workflowTabs).toHaveLength(1);
      expect(manager.workflowTabs[0].name).toBe('Test Tab');
    });

    test('should reject tabs with missing workflow', () => {
      localStorage.setItem('gluon_workflow_tabs', JSON.stringify({
        tabs: [
          {
            id: 'tab-1',
            name: 'Test Tab'
            // Missing workflow
          }
        ],
        activeTabId: 'tab-1',
        nextTabId: 2
      }));

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
    });
  });

  describe('🔴 CRITICAL: Circular Reference Handling', () => {
    test('should detect circular reference in graph', () => {
      manager.createNewTab('Test Tab');

      const agent = { id: 'agent-1', name: 'Agent' };
      agent.circular = agent; // Circular reference

      manager.graph = {
        agents: { 'agent-1': agent },
        connections: [],
        auto_forward: false
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(false);
      expect(result.error).toContain('circular');
    });

    test('should detect deep circular reference', () => {
      manager.createNewTab('Test Tab');

      const obj1 = { id: 'obj1' };
      const obj2 = { id: 'obj2', ref: obj1 };
      obj1.ref = obj2; // Circular reference

      manager.graph = {
        agents: { 'agent-1': { id: 'agent-1', data: obj1 } },
        connections: [],
        auto_forward: false
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(false);
    });

    test('should handle circular reference in connections', () => {
      manager.createNewTab('Test Tab');

      const connection = { from: 'a1', to: 'a2' };
      connection.self = connection;

      manager.graph = {
        agents: {},
        connections: [connection],
        auto_forward: false
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(false);
    });

    test('should allow valid complex nested structures', () => {
      manager.createNewTab('Test Tab');

      manager.graph = {
        agents: {
          'a1': {
            id: 'a1',
            config: {
              level1: {
                level2: {
                  level3: {
                    data: [1, 2, 3],
                    nested: {
                      value: 'deep'
                    }
                  }
                }
              }
            }
          }
        },
        connections: [],
        auto_forward: false
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(true);
    });
  });

  describe('🔴 CRITICAL: Import/Export JSON Parsing', () => {
    test('should reject invalid JSON in import', () => {
      const result = manager.importTabFromFile('{invalid json}');

      expect(result.success).toBe(false);
      expect(result.error).toBeTruthy();
    });

    test('should reject import with wrong type', () => {
      const invalidData = JSON.stringify({
        type: 'wrong_type',
        tab: { workflow: {} }
      });

      const result = manager.importTabFromFile(invalidData);

      expect(result.success).toBe(false);
      expect(result.error).toContain('Invalid file type');
    });

    test('should reject import with missing workflow', () => {
      const invalidData = JSON.stringify({
        type: 'gluon_workflow_tab',
        tab: { name: 'Test' } // Missing workflow
      });

      const result = manager.importTabFromFile(invalidData);

      expect(result.success).toBe(false);
      expect(result.error).toContain('Missing workflow');
    });

    test('should reject import with invalid agents structure', () => {
      const invalidData = JSON.stringify({
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Test',
          workflow: {
            agents: 'not an object', // Invalid
            connections: []
          }
        }
      });

      const result = manager.importTabFromFile(invalidData);

      expect(result.success).toBe(false);
      expect(result.error).toContain('Invalid agents');
    });

    test('should reject import with invalid connections', () => {
      const invalidData = JSON.stringify({
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Test',
          workflow: {
            agents: {},
            connections: 'not an array' // Invalid
          }
        }
      });

      const result = manager.importTabFromFile(invalidData);

      expect(result.success).toBe(false);
      expect(result.error).toContain('not an array');
    });

    test('should validate agent structure in import', () => {
      const invalidData = JSON.stringify({
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Test',
          workflow: {
            agents: {
              'a1': { /* Missing id and name */ }
            },
            connections: []
          }
        }
      });

      const result = manager.importTabFromFile(invalidData);

      expect(result.success).toBe(false);
      expect(result.error).toContain('Invalid agent structure');
    });

    test('should validate connection structure in import', () => {
      const invalidData = JSON.stringify({
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Test',
          workflow: {
            agents: {
              'a1': { id: 'a1', name: 'Agent 1' }
            },
            connections: [
              { from_agent_id: 'a1' } // Missing to_agent_id
            ]
          }
        }
      });

      const result = manager.importTabFromFile(invalidData);

      expect(result.success).toBe(false);
      expect(result.error).toContain('Invalid connection structure');
    });

    test('should accept valid import data', () => {
      const validData = JSON.stringify({
        type: 'gluon_workflow_tab',
        version: '2.0',
        exported_at: new Date().toISOString(),
        tab: {
          name: 'Valid Tab',
          workflow: {
            agents: {
              'a1': { id: 'a1', name: 'Agent 1' },
              'a2': { id: 'a2', name: 'Agent 2' }
            },
            connections: [
              { from_agent_id: 'a1', to_agent_id: 'a2' }
            ],
            auto_forward: false
          }
        }
      });

      const result = manager.importTabFromFile(validData);

      expect(result.success).toBe(true);
      expect(result.data.tab.workflow.agents).toHaveProperty('a1');
      expect(result.data.tab.workflow.connections).toHaveLength(1);
    });
  });

  describe('🔴 CRITICAL: Corrupted Data Recovery', () => {
    test('should backup corrupted data before reset', () => {
      localStorage.setItem('gluon_workflow_tabs', '{corrupted: data}');

      manager.loadTabsFromStorage();

      const backup = localStorage.getItem('gluon_workflow_tabs_corrupted_backup');
      expect(backup).toBe('{corrupted: data}');
    });

    test('should recover from partial JSON', () => {
      localStorage.setItem('gluon_workflow_tabs', '{"tabs": [{"id": "t1"');

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
      // Should have safe defaults
      expect(manager.workflowTabs).toEqual([]);
      expect(manager.activeTabId).toBeNull();
      expect(manager.nextTabId).toBe(1);
    });

    test('should handle JSON with trailing comma', () => {
      localStorage.setItem('gluon_workflow_tabs', '{"tabs": [],}'); // Trailing comma

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false);
    });

    test('should handle JSON with comments', () => {
      const jsonWithComments = `{
        // This is a comment
        "tabs": [],
        "activeTabId": null
      }`;

      localStorage.setItem('gluon_workflow_tabs', jsonWithComments);

      const result = manager.loadTabsFromStorage();

      expect(result.success).toBe(false); // Comments not allowed in JSON
    });
  });

  describe('🔴 CRITICAL: Large Payload Handling', () => {
    test('should handle very large workflow data', () => {
      manager.createNewTab('Large Tab');

      // Create large workflow
      const largeGraph = {
        agents: {},
        connections: [],
        auto_forward: false
      };

      // Add 1000 agents
      for (let i = 0; i < 1000; i++) {
        largeGraph.agents[`agent-${i}`] = {
          id: `agent-${i}`,
          name: `Agent ${i}`,
          config: {
            data: 'x'.repeat(100) // Some data per agent
          }
        };
      }

      manager.graph = largeGraph;

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(true);

      // Should be able to load back
      const loadResult = manager.loadTabsFromStorage();
      expect(loadResult.success).toBe(true);
    });

    test('should handle deeply nested structures', () => {
      manager.createNewTab('Deep Tab');

      // Create very deep nesting
      let deep = { value: 'bottom' };
      for (let i = 0; i < 100; i++) {
        deep = { nested: deep };
      }

      manager.graph = {
        agents: { 'a1': { id: 'a1', data: deep } },
        connections: [],
        auto_forward: false
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(true);
    });

    test('should handle workflow with many connections', () => {
      manager.createNewTab('Connected Tab');

      const graph = {
        agents: {},
        connections: [],
        auto_forward: false
      };

      // Create 100 agents
      for (let i = 0; i < 100; i++) {
        graph.agents[`a${i}`] = { id: `a${i}`, name: `Agent ${i}` };
      }

      // Create 500 connections (dense graph)
      for (let i = 0; i < 100; i++) {
        for (let j = i + 1; j < Math.min(i + 6, 100); j++) {
          graph.connections.push({
            from_agent_id: `a${i}`,
            to_agent_id: `a${j}`
          });
        }
      }

      manager.graph = graph;

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(true);
      expect(graph.connections.length).toBeGreaterThan(400);
    });
  });

  describe('🔴 CRITICAL: Edge Cases', () => {
    test('should handle special characters in JSON', () => {
      manager.createNewTab('Special Tab');

      manager.graph = {
        agents: {
          'a1': {
            id: 'a1',
            name: 'Agent with "quotes" and \'apostrophes\'',
            data: 'Line1\nLine2\tTabbed'
          }
        },
        connections: [],
        auto_forward: false
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(true);

      // Should load back correctly
      const loadResult = manager.loadTabsFromStorage();
      expect(loadResult.success).toBe(true);
      expect(manager.workflowTabs[0].workflow.agents['a1'].name).toContain('quotes');
    });

    test('should handle unicode characters', () => {
      manager.createNewTab('Unicode Tab');

      manager.graph = {
        agents: {
          'a1': {
            id: 'a1',
            name: '🤖 Agent with emoji 中文 العربية',
            data: '∑ ∫ ∂ ∞ π'
          }
        },
        connections: [],
        auto_forward: false
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(true);

      const loadResult = manager.loadTabsFromStorage();
      expect(loadResult.success).toBe(true);
      expect(manager.workflowTabs[0].workflow.agents['a1'].name).toContain('🤖');
    });

    test('should handle empty strings vs null', () => {
      manager.createNewTab('Empty Tab');

      manager.graph = {
        agents: {
          'a1': {
            id: 'a1',
            name: '',
            output_wrapper: null,
            system_prompt: ''
          }
        },
        connections: [],
        auto_forward: false
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(true);
    });

    test('should handle numbers as string keys', () => {
      manager.createNewTab('Numeric Keys Tab');

      manager.graph = {
        agents: {
          '123': { id: '123', name: 'Numeric ID' },
          '0': { id: '0', name: 'Zero ID' }
        },
        connections: [],
        auto_forward: false
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(true);

      const loadResult = manager.loadTabsFromStorage();
      expect(loadResult.success).toBe(true);
      expect(manager.workflowTabs[0].workflow.agents['123'].name).toBe('Numeric ID');
    });

    test('should handle boolean and numeric values', () => {
      manager.createNewTab('Mixed Types Tab');

      manager.graph = {
        agents: {
          'a1': {
            id: 'a1',
            name: 'Agent',
            enabled: true,
            count: 42,
            ratio: 3.14159,
            nullable: null
          }
        },
        connections: [],
        auto_forward: true
      };

      const result = manager.saveCurrentTabWorkflow();

      expect(result.success).toBe(true);

      const loadResult = manager.loadTabsFromStorage();
      expect(manager.workflowTabs[0].workflow.agents['a1'].enabled).toBe(true);
      expect(manager.workflowTabs[0].workflow.agents['a1'].count).toBe(42);
      expect(manager.workflowTabs[0].workflow.auto_forward).toBe(true);
    });
  });

  describe('🔴 CRITICAL: Error State Recovery', () => {
    test('should maintain consistent state after failed save', () => {
      manager.createNewTab('Test Tab');

      const validGraph = {
        agents: { 'a1': { id: 'a1', name: 'Agent 1' } },
        connections: [],
        auto_forward: false
      };

      manager.graph = validGraph;
      const firstSave = manager.saveCurrentTabWorkflow();
      expect(firstSave.success).toBe(true);

      // Try to save invalid graph (circular ref)
      const agent = { id: 'a2', name: 'Agent 2' };
      agent.self = agent;

      manager.graph = {
        agents: { 'a2': agent },
        connections: [],
        auto_forward: false
      };

      const secondSave = manager.saveCurrentTabWorkflow();
      expect(secondSave.success).toBe(false);

      // Previous valid data should still be in storage
      const loadResult = manager.loadTabsFromStorage();
      expect(loadResult.success).toBe(true);
      expect(manager.workflowTabs[0].workflow.agents).toHaveProperty('a1');
    });

    test('should handle rapid save failures gracefully', () => {
      manager.createNewTab('Test Tab');

      const circular = {};
      circular.self = circular;

      // Try multiple failed saves
      for (let i = 0; i < 10; i++) {
        manager.graph = {
          agents: { [`a${i}`]: circular },
          connections: [],
          auto_forward: false
        };

        const result = manager.saveCurrentTabWorkflow();
        expect(result.success).toBe(false);
      }

      // Manager should still be in valid state
      expect(manager.workflowTabs).toHaveLength(1);
      expect(manager.activeTabId).toBeTruthy();
    });
  });
});
