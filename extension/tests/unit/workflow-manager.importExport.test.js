/**
 * Unit Tests for WorkflowManager Import/Export
 *
 * Tests cover:
 * - importTabFromFile() with invalid data
 * - exportCurrentTab() functionality
 * - File format validation
 * - Error handling for corrupted data
 */

import { describe, test, expect, jest, beforeEach } from '@jest/globals';

const mockPresetManager = {
  init: jest.fn(() => Promise.resolve()),
  presets: { agents: [], connections: [], workflows: [] }
};

class TestableWorkflowManager {
  constructor() {
    this.workflowTabs = [];
    this.activeTabId = null;
    this.nextTabId = 1;
    this.presetManager = mockPresetManager;
    this.graph = null;
    this.pendingRequests = new Map();
  }

  showErrorMessage(message) {
    this.lastErrorMessage = message;
  }

  showSuccessMessage(message) {
    this.lastSuccessMessage = message;
  }

  saveTabsToStorage() {
    const data = {
      tabs: this.workflowTabs,
      activeTabId: this.activeTabId,
      nextTabId: this.nextTabId
    };
    localStorage.setItem('gluon_workflow_tabs', JSON.stringify(data));
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

  switchToTab(tabId) {
    const tab = this.workflowTabs.find(t => t.id === tabId);
    if (!tab) return false;

    this.activeTabId = tabId;
    this.graph = tab.workflow;
    this.saveTabsToStorage();
    return true;
  }

  saveCurrentTabWorkflow() {
    if (!this.activeTabId) return false;

    const tab = this.workflowTabs.find(t => t.id === this.activeTabId);
    if (tab && this.graph) {
      tab.workflow = JSON.parse(JSON.stringify(this.graph));
      tab.modifiedAt = Date.now();
      this.saveTabsToStorage();
      return true;
    }
    return false;
  }

  normalizeAgentType(agent) {
    if (!agent.agent_type) return 'Normal';
    if (typeof agent.agent_type === 'string') return agent.agent_type;
    if (typeof agent.agent_type === 'object') {
      if ('Report' in agent.agent_type) return 'Report';
      if ('Normal' in agent.agent_type) return 'Normal';
    }
    return 'Normal';
  }

  exportCurrentTab() {
    if (!this.activeTabId) {
      this.showErrorMessage('Brak aktywnej zakładki do eksportu');
      return null;
    }

    const tab = this.workflowTabs.find(t => t.id === this.activeTabId);
    if (!tab) {
      this.showErrorMessage('Nie znaleziono aktywnej zakładki');
      return null;
    }

    this.saveCurrentTabWorkflow();

    try {
      const exportData = {
        version: "2.0",
        type: "gluon_workflow_tab",
        exported_at: new Date().toISOString(),
        tab: {
          name: tab.name,
          workflow: tab.workflow,
          metadata: {
            agent_count: Object.keys(tab.workflow.agents || {}).length,
            connection_count: (tab.workflow.connections || []).length,
            aggregator_count: Object.values(tab.workflow.agents || {}).filter(a =>
              this.normalizeAgentType(a) === 'Report'
            ).length
          }
        }
      };

      const jsonString = JSON.stringify(exportData, null, 2);
      this.showSuccessMessage(`Zakładka wyeksportowana`);
      return jsonString;
    } catch (error) {
      this.showErrorMessage('Błąd podczas eksportowania zakładki');
      return null;
    }
  }

  async importTabFromFile(file) {
    try {
      const text = await file.text();
      const importData = JSON.parse(text);

      // Validate import data
      if (!importData.type || importData.type !== 'gluon_workflow_tab') {
        this.showErrorMessage('Nieprawidłowy format pliku. Oczekiwano pliku zakładki workflow.');
        return false;
      }

      if (!importData.tab || !importData.tab.workflow) {
        this.showErrorMessage('Brak danych workflow w pliku');
        return false;
      }

      const tabName = importData.tab.name || 'Imported Workflow';
      const workflow = importData.tab.workflow;

      // Create new tab with imported data
      const tabId = `tab-${this.nextTabId++}`;
      const newTab = {
        id: tabId,
        name: tabName,
        workflow: workflow,
        createdAt: Date.now(),
        modifiedAt: Date.now()
      };

      this.workflowTabs.push(newTab);
      this.switchToTab(tabId);

      this.saveTabsToStorage();
      this.showSuccessMessage(`Zaimportowano zakładkę: ${tabName}`);
      return true;
    } catch (error) {
      this.showErrorMessage('Błąd podczas importowania zakładki: ' + error.message);
      return false;
    }
  }

  sendWorkflowMessage(action, payload = {}) {
    return new Promise((resolve, reject) => {
      chrome.runtime.sendMessage({ action, ...payload }, (requestId) => {
        if (chrome.runtime.lastError) {
          reject(new Error(chrome.runtime.lastError.message));
          return;
        }
        if (!requestId) {
          reject(new Error('No request ID returned'));
          return;
        }
        this.pendingRequests.set(requestId, { resolve, reject });
      });
    });
  }

  async recreateWorkflowFromImport(workflow) {
    if (!workflow || !workflow.agents) return;

    const agentIdMap = {};

    // Create agents
    for (const [oldId, agent] of Object.entries(workflow.agents)) {
      const response = await this.sendWorkflowMessage('workflow_add_agent', {
        name: agent.name,
        output_wrapper: agent.output_wrapper || null,
        agent_type: agent.agent_type || 'Normal',
        position: agent.position || null
      });

      if (response && response.success) {
        agentIdMap[oldId] = response.data.id;
      }
    }

    // Create connections
    for (const conn of workflow.connections || []) {
      const newFromId = agentIdMap[conn.from_agent_id];
      const newToId = agentIdMap[conn.to_agent_id];

      if (!newFromId || !newToId) continue;

      await this.sendWorkflowMessage('workflow_add_connection', {
        from_id: newFromId,
        to_id: newToId,
        template: conn.message_template || null
      });
    }

    if (workflow.auto_forward) {
      await this.sendWorkflowMessage('workflow_set_auto_forward', {
        enabled: true
      });
    }
  }
}

describe('WorkflowManager - Export Functionality', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
  });

  describe('exportCurrentTab()', () => {
    test('should export tab with valid structure', () => {
      manager.createNewTab('Test Workflow');
      manager.graph = {
        agents: {
          'agent-1': { id: 'agent-1', name: 'Agent 1', agent_type: 'Normal' }
        },
        connections: [{ from_agent_id: 'agent-1', to_agent_id: 'agent-2' }],
        auto_forward: false
      };

      const result = manager.exportCurrentTab();

      expect(result).toBeTruthy();
      const parsed = JSON.parse(result);
      expect(parsed.version).toBe('2.0');
      expect(parsed.type).toBe('gluon_workflow_tab');
      expect(parsed.tab.name).toBe('Test Workflow');
      expect(parsed.tab.workflow).toEqual(manager.graph);
    });

    test('should include metadata', () => {
      manager.createNewTab('Metadata Test');
      manager.graph = {
        agents: {
          'a1': { id: 'a1', name: 'Agent 1', agent_type: 'Normal' },
          'a2': { id: 'a2', name: 'Agent 2', agent_type: 'Normal' },
          'a3': { id: 'a3', name: 'Aggregator', agent_type: 'Report' }
        },
        connections: [
          { from_agent_id: 'a1', to_agent_id: 'a3' },
          { from_agent_id: 'a2', to_agent_id: 'a3' }
        ],
        auto_forward: false
      };

      const result = manager.exportCurrentTab();
      const parsed = JSON.parse(result);

      expect(parsed.tab.metadata.agent_count).toBe(3);
      expect(parsed.tab.metadata.connection_count).toBe(2);
      expect(parsed.tab.metadata.aggregator_count).toBe(1);
    });

    test('should include timestamp', () => {
      manager.createNewTab('Timestamp Test');
      const beforeTime = new Date().toISOString();

      const result = manager.exportCurrentTab();
      const parsed = JSON.parse(result);

      expect(parsed.exported_at).toBeTruthy();
      expect(new Date(parsed.exported_at)).toBeInstanceOf(Date);
      expect(parsed.exported_at).toBeGreaterThanOrEqual(beforeTime);
    });

    test('should return null when no active tab', () => {
      const result = manager.exportCurrentTab();

      expect(result).toBeNull();
      expect(manager.lastErrorMessage).toBe('Brak aktywnej zakładki do eksportu');
    });

    test('should return null when active tab not found', () => {
      manager.activeTabId = 'non-existent';

      const result = manager.exportCurrentTab();

      expect(result).toBeNull();
      expect(manager.lastErrorMessage).toBe('Nie znaleziono aktywnej zakładki');
    });

    test('should export empty workflow', () => {
      manager.createNewTab('Empty Workflow');

      const result = manager.exportCurrentTab();
      const parsed = JSON.parse(result);

      expect(parsed.tab.workflow.agents).toEqual({});
      expect(parsed.tab.workflow.connections).toEqual([]);
      expect(parsed.tab.metadata.agent_count).toBe(0);
      expect(parsed.tab.metadata.connection_count).toBe(0);
    });

    test('should handle complex nested structures', () => {
      manager.createNewTab('Complex Workflow');
      manager.graph = {
        agents: {
          'a1': {
            id: 'a1',
            name: 'Complex Agent',
            agent_type: 'Normal',
            metadata: {
              tags: ['test', 'complex'],
              config: { nested: { deep: { value: 123 } } }
            }
          }
        },
        connections: [],
        auto_forward: false
      };

      const result = manager.exportCurrentTab();
      const parsed = JSON.parse(result);

      expect(parsed.tab.workflow.agents['a1'].metadata.config.nested.deep.value).toBe(123);
    });
  });
});

describe('WorkflowManager - Import Functionality', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
  });

  describe('importTabFromFile() - Valid Data', () => {
    test('should import valid workflow file', async () => {
      const validData = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        exported_at: new Date().toISOString(),
        tab: {
          name: 'Imported Workflow',
          workflow: {
            agents: {
              'a1': { id: 'a1', name: 'Agent 1', agent_type: 'Normal' }
            },
            connections: [],
            auto_forward: false
          }
        }
      };

      const file = new File([JSON.stringify(validData)], 'workflow.json', {
        type: 'application/json'
      });

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(true);
      expect(manager.workflowTabs).toHaveLength(1);
      expect(manager.workflowTabs[0].name).toBe('Imported Workflow');
      expect(manager.workflowTabs[0].workflow.agents['a1'].name).toBe('Agent 1');
    });

    test('should switch to imported tab', async () => {
      const existingTab = manager.createNewTab('Existing Tab');

      const validData = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        tab: {
          name: 'New Imported',
          workflow: { agents: {}, connections: [], auto_forward: false }
        }
      };

      const file = new File([JSON.stringify(validData)], 'workflow.json');

      await manager.importTabFromFile(file);

      expect(manager.activeTabId).not.toBe(existingTab);
      expect(manager.workflowTabs).toHaveLength(2);
    });

    test('should use default name when name is missing', async () => {
      const validData = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        tab: {
          workflow: { agents: {}, connections: [], auto_forward: false }
        }
      };

      const file = new File([JSON.stringify(validData)], 'workflow.json');

      await manager.importTabFromFile(file);

      expect(manager.workflowTabs[0].name).toBe('Imported Workflow');
    });
  });

  describe('importTabFromFile() - Invalid Data', () => {
    test('should reject invalid JSON', async () => {
      const file = new File(['{ invalid json }'], 'invalid.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(false);
      expect(manager.lastErrorMessage).toContain('Błąd podczas importowania');
    });

    test('should reject wrong file type', async () => {
      const invalidData = {
        type: 'wrong_type',
        data: {}
      };

      const file = new File([JSON.stringify(invalidData)], 'wrong.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(false);
      expect(manager.lastErrorMessage).toBe('Nieprawidłowy format pliku. Oczekiwano pliku zakładki workflow.');
    });

    test('should reject missing type field', async () => {
      const invalidData = {
        version: '2.0',
        tab: {
          name: 'Test',
          workflow: {}
        }
      };

      const file = new File([JSON.stringify(invalidData)], 'missing-type.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(false);
      expect(manager.lastErrorMessage).toContain('Nieprawidłowy format pliku');
    });

    test('should reject missing tab field', async () => {
      const invalidData = {
        version: '2.0',
        type: 'gluon_workflow_tab'
        // Missing tab field
      };

      const file = new File([JSON.stringify(invalidData)], 'missing-tab.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(false);
      expect(manager.lastErrorMessage).toBe('Brak danych workflow w pliku');
    });

    test('should reject missing workflow field', async () => {
      const invalidData = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Test'
          // Missing workflow field
        }
      };

      const file = new File([JSON.stringify(invalidData)], 'missing-workflow.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(false);
      expect(manager.lastErrorMessage).toBe('Brak danych workflow w pliku');
    });

    test('should reject empty file', async () => {
      const file = new File([''], 'empty.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(false);
      expect(manager.lastErrorMessage).toContain('Błąd podczas importowania');
    });

    test('should reject non-JSON file', async () => {
      const file = new File(['This is plain text'], 'text.txt');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(false);
      expect(manager.lastErrorMessage).toContain('Błąd podczas importowania');
    });

    test('should reject file with null workflow', async () => {
      const invalidData = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Test',
          workflow: null
        }
      };

      const file = new File([JSON.stringify(invalidData)], 'null-workflow.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(false);
      expect(manager.lastErrorMessage).toBe('Brak danych workflow w pliku');
    });

    test('should handle corrupted agents data', async () => {
      const corruptedData = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Corrupted',
          workflow: {
            agents: 'not-an-object',
            connections: [],
            auto_forward: false
          }
        }
      };

      const file = new File([JSON.stringify(corruptedData)], 'corrupted.json');

      // Should import but with corrupted agents structure
      const result = await manager.importTabFromFile(file);

      expect(result).toBe(true);
      // The workflow will have the corrupted structure as-is
      expect(manager.workflowTabs[0].workflow.agents).toBe('not-an-object');
    });

    test('should handle corrupted connections data', async () => {
      const corruptedData = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Corrupted Connections',
          workflow: {
            agents: {},
            connections: 'not-an-array',
            auto_forward: false
          }
        }
      };

      const file = new File([JSON.stringify(corruptedData)], 'corrupted-conn.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(true);
      expect(manager.workflowTabs[0].workflow.connections).toBe('not-an-array');
    });
  });

  describe('importTabFromFile() - Edge Cases', () => {
    test('should handle very large files', async () => {
      const largeWorkflow = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Large Workflow',
          workflow: {
            agents: {},
            connections: [],
            auto_forward: false
          }
        }
      };

      // Create 1000 agents
      for (let i = 0; i < 1000; i++) {
        largeWorkflow.tab.workflow.agents[`agent-${i}`] = {
          id: `agent-${i}`,
          name: `Agent ${i}`,
          agent_type: 'Normal'
        };
      }

      const file = new File([JSON.stringify(largeWorkflow)], 'large.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(true);
      expect(Object.keys(manager.workflowTabs[0].workflow.agents)).toHaveLength(1000);
    });

    test('should handle unicode characters in workflow name', async () => {
      const unicodeData = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Workflow 工作流程 🚀',
          workflow: { agents: {}, connections: [], auto_forward: false }
        }
      };

      const file = new File([JSON.stringify(unicodeData)], 'unicode.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(true);
      expect(manager.workflowTabs[0].name).toBe('Workflow 工作流程 🚀');
    });

    test('should handle special characters in agent names', async () => {
      const specialData = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Special Chars',
          workflow: {
            agents: {
              'a1': { id: 'a1', name: 'Agent<>"/&\'', agent_type: 'Normal' }
            },
            connections: [],
            auto_forward: false
          }
        }
      };

      const file = new File([JSON.stringify(specialData)], 'special.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(true);
      expect(manager.workflowTabs[0].workflow.agents['a1'].name).toBe('Agent<>"/&\'');
    });

    test('should handle file with BOM (Byte Order Mark)', async () => {
      const validData = {
        version: '2.0',
        type: 'gluon_workflow_tab',
        tab: {
          name: 'BOM Test',
          workflow: { agents: {}, connections: [], auto_forward: false }
        }
      };

      // Add BOM to beginning of JSON string
      const bomString = '\uFEFF' + JSON.stringify(validData);
      const file = new File([bomString], 'bom.json');

      const result = await manager.importTabFromFile(file);

      expect(result).toBe(true);
    });

    test('should handle old version format gracefully', async () => {
      const oldVersion = {
        version: '1.0',
        type: 'gluon_workflow_tab',
        tab: {
          name: 'Old Version',
          workflow: { agents: {}, connections: [], auto_forward: false }
        }
      };

      const file = new File([JSON.stringify(oldVersion)], 'old.json');

      // Should still import as long as type is correct
      const result = await manager.importTabFromFile(file);

      expect(result).toBe(true);
    });
  });

  describe('recreateWorkflowFromImport()', () => {
    test('should handle empty workflow', async () => {
      const workflow = {
        agents: {},
        connections: [],
        auto_forward: false
      };

      await manager.recreateWorkflowFromImport(workflow);

      expect(chrome.runtime.sendMessage).not.toHaveBeenCalled();
    });

    test('should handle null workflow', async () => {
      await manager.recreateWorkflowFromImport(null);

      expect(chrome.runtime.sendMessage).not.toHaveBeenCalled();
    });

    test('should handle workflow with missing agents field', async () => {
      const workflow = {
        connections: [],
        auto_forward: false
      };

      await manager.recreateWorkflowFromImport(workflow);

      expect(chrome.runtime.sendMessage).not.toHaveBeenCalled();
    });

    test('should skip connections with missing agent IDs', async () => {
      const workflow = {
        agents: {
          'a1': { id: 'a1', name: 'Agent 1', agent_type: 'Normal' }
        },
        connections: [
          { from_agent_id: 'a1', to_agent_id: 'non-existent' }
        ],
        auto_forward: false
      };

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        if (msg.action === 'workflow_add_agent') {
          callback('req-1');
          const handlers = manager.pendingRequests.get('req-1');
          handlers.resolve({ success: true, data: { id: 'new-a1' } });
          manager.pendingRequests.delete('req-1');
        }
      });

      await manager.recreateWorkflowFromImport(workflow);

      // Should create agent but skip connection
      const agentCalls = chrome.runtime.sendMessage.mock.calls.filter(
        call => call[0].action === 'workflow_add_agent'
      );
      const connectionCalls = chrome.runtime.sendMessage.mock.calls.filter(
        call => call[0].action === 'workflow_add_connection'
      );

      expect(agentCalls).toHaveLength(1);
      expect(connectionCalls).toHaveLength(0);
    });
  });
});

describe('WorkflowManager - Import/Export Round-trip', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
  });

  test('should preserve data through export and import', async () => {
    // Create original workflow
    manager.createNewTab('Original Workflow');
    manager.graph = {
      agents: {
        'a1': { id: 'a1', name: 'Agent 1', agent_type: 'Normal', position: [100, 200] },
        'a2': { id: 'a2', name: 'Agent 2', agent_type: 'Report' }
      },
      connections: [
        { from_agent_id: 'a1', to_agent_id: 'a2', message_template: 'Test template {content}' }
      ],
      auto_forward: true
    };

    // Export
    const exported = manager.exportCurrentTab();
    expect(exported).toBeTruthy();

    // Import
    const file = new File([exported], 'roundtrip.json');
    await manager.importTabFromFile(file);

    // Verify
    const importedTab = manager.workflowTabs[1]; // Second tab (0 is original)
    expect(importedTab.workflow.agents['a1'].name).toBe('Agent 1');
    expect(importedTab.workflow.agents['a2'].agent_type).toBe('Report');
    expect(importedTab.workflow.connections[0].message_template).toBe('Test template {content}');
    expect(importedTab.workflow.auto_forward).toBe(true);
  });

  test('should maintain independence after import', async () => {
    manager.createNewTab('Original');
    manager.graph = {
      agents: { 'a1': { id: 'a1', name: 'Original Agent' } },
      connections: [],
      auto_forward: false
    };

    const exported = manager.exportCurrentTab();
    const file = new File([exported], 'test.json');
    await manager.importTabFromFile(file);

    // Modify imported workflow
    const importedTab = manager.workflowTabs[1];
    importedTab.workflow.agents['a1'].name = 'Modified Agent';

    // Original should be unchanged
    const originalTab = manager.workflowTabs[0];
    expect(originalTab.workflow.agents['a1'].name).toBe('Original Agent');
  });
});
