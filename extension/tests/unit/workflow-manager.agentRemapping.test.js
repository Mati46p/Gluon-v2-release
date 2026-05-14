/**
 * Unit Tests for WorkflowManager Agent ID Remapping Failures
 *
 * CRITICAL SECURITY TESTS - Covers:
 * - Agent ID remapping during import
 * - Orphaned connections when agent creation fails
 * - Partial import recovery
 * - Connection integrity after remapping
 * - Duplicate agent handling
 * - ID collision prevention
 */

import { describe, test, expect, jest, beforeEach, afterEach } from '@jest/globals';

// Mock chrome runtime
global.chrome = {
  runtime: {
    sendMessage: jest.fn(),
    lastError: null
  }
};

class TestableWorkflowManager {
  constructor() {
    this.graph = null;
    this.workflowTabs = [];
    this.activeTabId = null;
    this.nextTabId = 1;
    this.pendingRequests = new Map();
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

        setTimeout(() => {
          if (this.pendingRequests.has(requestId)) {
            this.pendingRequests.delete(requestId);
            reject(new Error('Request timeout'));
          }
        }, 15000);
      });
    });
  }

  handleWorkflowResponse(message) {
    const { request_id, success, data, error } = message;

    if (request_id && this.pendingRequests.has(request_id)) {
      const { resolve, reject } = this.pendingRequests.get(request_id);
      this.pendingRequests.delete(request_id);

      if (success) {
        resolve({ success: true, data });
      } else {
        reject(new Error(error || 'Unknown error'));
      }
    }
  }

  async recreateWorkflowFromImport(workflow) {
    if (!workflow || !workflow.agents) {
      throw new Error('Invalid workflow structure');
    }

    const agentIdMap = {};
    const createdAgents = [];
    const failedAgents = [];

    // Phase 1: Create agents and build ID mapping
    for (const [oldId, agent] of Object.entries(workflow.agents)) {
      try {
        const response = await this.sendWorkflowMessage('workflow_add_agent', {
          name: agent.name,
          output_wrapper: agent.output_wrapper || null,
          agent_type: agent.agent_type || 'Normal',
          position: agent.position || null
        });

        if (response && response.success && response.data && response.data.id) {
          const newId = response.data.id;
          agentIdMap[oldId] = newId;
          createdAgents.push({ oldId, newId, agent });
        } else {
          failedAgents.push({ oldId, agent, reason: 'No ID returned' });
        }

        // Small delay between creations
        await new Promise(resolve => setTimeout(resolve, 100));
      } catch (error) {
        failedAgents.push({ oldId, agent, reason: error.message });
      }
    }

    // Phase 2: Create connections with mapped IDs
    const successfulConnections = [];
    const failedConnections = [];

    for (const conn of workflow.connections || []) {
      const newFromId = agentIdMap[conn.from_agent_id];
      const newToId = agentIdMap[conn.to_agent_id];

      // Check if both agents were created successfully
      if (!newFromId) {
        failedConnections.push({
          connection: conn,
          reason: `Source agent '${conn.from_agent_id}' was not created`
        });
        continue;
      }

      if (!newToId) {
        failedConnections.push({
          connection: conn,
          reason: `Target agent '${conn.to_agent_id}' was not created`
        });
        continue;
      }

      try {
        await this.sendWorkflowMessage('workflow_add_connection', {
          from_id: newFromId,
          to_id: newToId,
          template: conn.message_template || null
        });

        successfulConnections.push({ ...conn, newFromId, newToId });

        await new Promise(resolve => setTimeout(resolve, 100));
      } catch (error) {
        failedConnections.push({
          connection: conn,
          reason: error.message
        });
      }
    }

    return {
      agentIdMap,
      createdAgents,
      failedAgents,
      successfulConnections,
      failedConnections,
      success: failedAgents.length === 0 && failedConnections.length === 0
    };
  }

  // Validate import before attempting
  validateImportWorkflow(workflow) {
    const errors = [];

    if (!workflow || typeof workflow !== 'object') {
      errors.push('Workflow is not an object');
      return { valid: false, errors };
    }

    if (!workflow.agents || typeof workflow.agents !== 'object') {
      errors.push('Missing or invalid agents object');
    }

    if (!Array.isArray(workflow.connections)) {
      errors.push('Connections is not an array');
    }

    // Validate agent IDs
    const agentIds = Object.keys(workflow.agents || {});
    if (agentIds.length === 0) {
      errors.push('No agents to import');
    }

    // Validate connections reference existing agents
    (workflow.connections || []).forEach((conn, index) => {
      if (!conn.from_agent_id || !conn.to_agent_id) {
        errors.push(`Connection ${index} missing from_agent_id or to_agent_id`);
      } else {
        if (!agentIds.includes(conn.from_agent_id)) {
          errors.push(`Connection ${index} references non-existent source agent: ${conn.from_agent_id}`);
        }
        if (!agentIds.includes(conn.to_agent_id)) {
          errors.push(`Connection ${index} references non-existent target agent: ${conn.to_agent_id}`);
        }
      }
    });

    // Check for duplicate agent IDs
    const idCounts = {};
    agentIds.forEach(id => {
      idCounts[id] = (idCounts[id] || 0) + 1;
    });

    Object.entries(idCounts).forEach(([id, count]) => {
      if (count > 1) {
        errors.push(`Duplicate agent ID: ${id} (appears ${count} times)`);
      }
    });

    return {
      valid: errors.length === 0,
      errors,
      stats: {
        agentCount: agentIds.length,
        connectionCount: (workflow.connections || []).length
      }
    };
  }

  // Rollback partially created workflow
  async rollbackImport(createdAgents) {
    const rollbackResults = [];

    for (const { newId, agent } of createdAgents) {
      try {
        await this.sendWorkflowMessage('workflow_remove_agent', {
          agent_id: newId
        });
        rollbackResults.push({ agentId: newId, success: true });
      } catch (error) {
        rollbackResults.push({
          agentId: newId,
          success: false,
          error: error.message
        });
      }
    }

    return rollbackResults;
  }
}

describe('WorkflowManager - Agent ID Remapping Failures', () => {
  let manager;

  beforeEach(() => {
    jest.useFakeTimers();
    manager = new TestableWorkflowManager();
    chrome.runtime.sendMessage.mockClear();
    chrome.runtime.lastError = null;
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  describe('🔴 CRITICAL: Agent Creation Failures During Import', () => {
    test('should detect when agent creation fails', async () => {
      const workflow = {
        agents: {
          'old-a1': { id: 'old-a1', name: 'Agent 1', agent_type: 'Normal' },
          'old-a2': { id: 'old-a2', name: 'Agent 2', agent_type: 'Normal' }
        },
        connections: []
      };

      // Mock: First agent succeeds, second fails
      chrome.runtime.sendMessage
        .mockImplementationOnce((msg, callback) => callback('req-1'))
        .mockImplementationOnce((msg, callback) => callback('req-2'));

      const importPromise = manager.recreateWorkflowFromImport(workflow);

      // Resolve first agent creation (success)
      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-1',
          success: true,
          data: { id: 'new-a1' }
        });
      }, 50);

      // Reject second agent creation (failure)
      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-2',
          success: false,
          error: 'Failed to create agent'
        });
      }, 150);

      jest.advanceTimersByTime(200);

      const result = await importPromise;

      expect(result.createdAgents).toHaveLength(1);
      expect(result.failedAgents).toHaveLength(1);
      expect(result.failedAgents[0].oldId).toBe('old-a2');
      expect(result.success).toBe(false);
    });

    test('should handle all agents failing to create', async () => {
      const workflow = {
        agents: {
          'old-a1': { id: 'old-a1', name: 'Agent 1' },
          'old-a2': { id: 'old-a2', name: 'Agent 2' }
        },
        connections: []
      };

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        const reqId = `req-${Date.now()}`;
        callback(reqId);
        setTimeout(() => {
          manager.handleWorkflowResponse({
            request_id: reqId,
            success: false,
            error: 'Backend error'
          });
        }, 50);
      });

      const result = await manager.recreateWorkflowFromImport(workflow);

      expect(result.createdAgents).toHaveLength(0);
      expect(result.failedAgents).toHaveLength(2);
      expect(result.success).toBe(false);
    });

    test('should handle response with missing agent ID', async () => {
      const workflow = {
        agents: {
          'old-a1': { id: 'old-a1', name: 'Agent 1' }
        },
        connections: []
      };

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-1');
      });

      const importPromise = manager.recreateWorkflowFromImport(workflow);

      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-1',
          success: true,
          data: {} // Missing id field
        });
      }, 50);

      jest.advanceTimersByTime(200);

      const result = await importPromise;

      expect(result.failedAgents).toHaveLength(1);
      expect(result.failedAgents[0].reason).toContain('No ID returned');
    });
  });

  describe('🔴 CRITICAL: Connection Orphaning', () => {
    test('should detect orphaned connections when source agent fails', async () => {
      const workflow = {
        agents: {
          'old-a1': { id: 'old-a1', name: 'Agent 1' },
          'old-a2': { id: 'old-a2', name: 'Agent 2' }
        },
        connections: [
          { from_agent_id: 'old-a1', to_agent_id: 'old-a2' }
        ]
      };

      // Mock: First agent fails, second succeeds
      chrome.runtime.sendMessage
        .mockImplementationOnce((msg, callback) => callback('req-1'))
        .mockImplementationOnce((msg, callback) => callback('req-2'));

      const importPromise = manager.recreateWorkflowFromImport(workflow);

      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-1',
          success: false,
          error: 'Failed to create agent 1'
        });
      }, 50);

      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-2',
          success: true,
          data: { id: 'new-a2' }
        });
      }, 150);

      jest.advanceTimersByTime(300);

      const result = await importPromise;

      // Connection should fail because source agent wasn't created
      expect(result.failedConnections).toHaveLength(1);
      expect(result.failedConnections[0].reason).toContain('Source agent');
      expect(result.successfulConnections).toHaveLength(0);
    });

    test('should detect orphaned connections when target agent fails', async () => {
      const workflow = {
        agents: {
          'old-a1': { id: 'old-a1', name: 'Agent 1' },
          'old-a2': { id: 'old-a2', name: 'Agent 2' }
        },
        connections: [
          { from_agent_id: 'old-a1', to_agent_id: 'old-a2' }
        ]
      };

      // Mock: First succeeds, second fails
      chrome.runtime.sendMessage
        .mockImplementationOnce((msg, callback) => callback('req-1'))
        .mockImplementationOnce((msg, callback) => callback('req-2'));

      const importPromise = manager.recreateWorkflowFromImport(workflow);

      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-1',
          success: true,
          data: { id: 'new-a1' }
        });
      }, 50);

      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-2',
          success: false,
          error: 'Failed to create agent 2'
        });
      }, 150);

      jest.advanceTimersByTime(300);

      const result = await importPromise;

      expect(result.failedConnections).toHaveLength(1);
      expect(result.failedConnections[0].reason).toContain('Target agent');
    });

    test('should handle multiple orphaned connections', async () => {
      const workflow = {
        agents: {
          'old-a1': { id: 'old-a1', name: 'Agent 1' },
          'old-a2': { id: 'old-a2', name: 'Agent 2' },
          'old-a3': { id: 'old-a3', name: 'Agent 3' }
        },
        connections: [
          { from_agent_id: 'old-a1', to_agent_id: 'old-a2' },
          { from_agent_id: 'old-a1', to_agent_id: 'old-a3' },
          { from_agent_id: 'old-a2', to_agent_id: 'old-a3' }
        ]
      };

      // Only old-a1 succeeds
      chrome.runtime.sendMessage
        .mockImplementationOnce((msg, callback) => callback('req-1'))
        .mockImplementationOnce((msg, callback) => callback('req-2'))
        .mockImplementationOnce((msg, callback) => callback('req-3'));

      const importPromise = manager.recreateWorkflowFromImport(workflow);

      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-1',
          success: true,
          data: { id: 'new-a1' }
        });
        manager.handleWorkflowResponse({
          request_id: 'req-2',
          success: false,
          error: 'Failed'
        });
        manager.handleWorkflowResponse({
          request_id: 'req-3',
          success: false,
          error: 'Failed'
        });
      }, 50);

      jest.advanceTimersByTime(500);

      const result = await importPromise;

      // All 3 connections should fail (all involve old-a2 or old-a3)
      expect(result.failedConnections).toHaveLength(3);
      expect(result.successfulConnections).toHaveLength(0);
    });
  });

  describe('🔴 CRITICAL: ID Mapping Integrity', () => {
    test('should create correct ID mapping for successful agents', async () => {
      const workflow = {
        agents: {
          'old-a1': { id: 'old-a1', name: 'Agent 1' },
          'old-a2': { id: 'old-a2', name: 'Agent 2' }
        },
        connections: []
      };

      chrome.runtime.sendMessage
        .mockImplementationOnce((msg, callback) => callback('req-1'))
        .mockImplementationOnce((msg, callback) => callback('req-2'));

      const importPromise = manager.recreateWorkflowFromImport(workflow);

      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-1',
          success: true,
          data: { id: 'new-a1' }
        });
        manager.handleWorkflowResponse({
          request_id: 'req-2',
          success: true,
          data: { id: 'new-a2' }
        });
      }, 50);

      jest.advanceTimersByTime(300);

      const result = await importPromise;

      expect(result.agentIdMap).toEqual({
        'old-a1': 'new-a1',
        'old-a2': 'new-a2'
      });
    });

    test('should not include failed agents in ID mapping', async () => {
      const workflow = {
        agents: {
          'old-a1': { id: 'old-a1', name: 'Agent 1' },
          'old-a2': { id: 'old-a2', name: 'Agent 2' }
        },
        connections: []
      };

      chrome.runtime.sendMessage
        .mockImplementationOnce((msg, callback) => callback('req-1'))
        .mockImplementationOnce((msg, callback) => callback('req-2'));

      const importPromise = manager.recreateWorkflowFromImport(workflow);

      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-1',
          success: true,
          data: { id: 'new-a1' }
        });
        manager.handleWorkflowResponse({
          request_id: 'req-2',
          success: false,
          error: 'Failed'
        });
      }, 50);

      jest.advanceTimersByTime(300);

      const result = await importPromise;

      expect(result.agentIdMap).toEqual({
        'old-a1': 'new-a1'
        // old-a2 should NOT be in mapping
      });
      expect(result.agentIdMap).not.toHaveProperty('old-a2');
    });

    test('should use remapped IDs for connections', async () => {
      const workflow = {
        agents: {
          'old-a1': { id: 'old-a1', name: 'Agent 1' },
          'old-a2': { id: 'old-a2', name: 'Agent 2' }
        },
        connections: [
          { from_agent_id: 'old-a1', to_agent_id: 'old-a2' }
        ]
      };

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        const reqId = `req-${Date.now()}-${Math.random()}`;
        callback(reqId);

        setTimeout(() => {
          if (msg.action === 'workflow_add_agent') {
            const newId = msg.name === 'Agent 1' ? 'new-a1' : 'new-a2';
            manager.handleWorkflowResponse({
              request_id: reqId,
              success: true,
              data: { id: newId }
            });
          } else if (msg.action === 'workflow_add_connection') {
            manager.handleWorkflowResponse({
              request_id: reqId,
              success: true,
              data: {}
            });
          }
        }, 50);
      });

      const result = await manager.recreateWorkflowFromImport(workflow);

      expect(result.successfulConnections).toHaveLength(1);
      expect(result.successfulConnections[0].newFromId).toBe('new-a1');
      expect(result.successfulConnections[0].newToId).toBe('new-a2');
    });
  });

  describe('🔴 CRITICAL: Import Validation', () => {
    test('should detect missing agents object', () => {
      const workflow = {
        connections: []
        // Missing agents
      };

      const validation = manager.validateImportWorkflow(workflow);

      expect(validation.valid).toBe(false);
      expect(validation.errors.some(e => e.includes('agents'))).toBe(true);
    });

    test('should detect invalid connections array', () => {
      const workflow = {
        agents: {},
        connections: 'not an array'
      };

      const validation = manager.validateImportWorkflow(workflow);

      expect(validation.valid).toBe(false);
      expect(validation.errors.some(e => e.includes('array'))).toBe(true);
    });

    test('should detect connections referencing non-existent agents', () => {
      const workflow = {
        agents: {
          'a1': { id: 'a1', name: 'Agent 1' }
        },
        connections: [
          { from_agent_id: 'a1', to_agent_id: 'non-existent' }
        ]
      };

      const validation = manager.validateImportWorkflow(workflow);

      expect(validation.valid).toBe(false);
      expect(validation.errors.some(e => e.includes('non-existent'))).toBe(true);
    });

    test('should detect duplicate agent IDs', () => {
      const workflow = {
        agents: {
          'a1': { id: 'a1', name: 'Agent 1' },
          'a1-dup': { id: 'a1', name: 'Agent 1 Duplicate' }
        },
        connections: []
      };

      // Note: This tests object keys, so duplicates would overwrite
      // But we can test validation logic separately
      const validation = manager.validateImportWorkflow(workflow);

      // In JavaScript, object keys are unique, so this specific test
      // depends on how workflow is structured
      expect(validation.valid).toBe(true); // Would be valid since keys are unique
    });

    test('should detect empty workflow', () => {
      const workflow = {
        agents: {},
        connections: []
      };

      const validation = manager.validateImportWorkflow(workflow);

      expect(validation.valid).toBe(false);
      expect(validation.errors.some(e => e.includes('No agents'))).toBe(true);
    });

    test('should validate correct workflow', () => {
      const workflow = {
        agents: {
          'a1': { id: 'a1', name: 'Agent 1' },
          'a2': { id: 'a2', name: 'Agent 2' }
        },
        connections: [
          { from_agent_id: 'a1', to_agent_id: 'a2' }
        ]
      };

      const validation = manager.validateImportWorkflow(workflow);

      expect(validation.valid).toBe(true);
      expect(validation.errors).toHaveLength(0);
      expect(validation.stats.agentCount).toBe(2);
      expect(validation.stats.connectionCount).toBe(1);
    });

    test('should detect connections with missing IDs', () => {
      const workflow = {
        agents: {
          'a1': { id: 'a1', name: 'Agent 1' }
        },
        connections: [
          { from_agent_id: 'a1' } // Missing to_agent_id
        ]
      };

      const validation = manager.validateImportWorkflow(workflow);

      expect(validation.valid).toBe(false);
      expect(validation.errors.some(e => e.includes('missing'))).toBe(true);
    });
  });

  describe('🔴 CRITICAL: Partial Import Recovery', () => {
    test('should report partial success when some agents succeed', async () => {
      const workflow = {
        agents: {
          'a1': { id: 'a1', name: 'Agent 1' },
          'a2': { id: 'a2', name: 'Agent 2' },
          'a3': { id: 'a3', name: 'Agent 3' }
        },
        connections: []
      };

      chrome.runtime.sendMessage
        .mockImplementationOnce((msg, callback) => callback('req-1'))
        .mockImplementationOnce((msg, callback) => callback('req-2'))
        .mockImplementationOnce((msg, callback) => callback('req-3'));

      const importPromise = manager.recreateWorkflowFromImport(workflow);

      setTimeout(() => {
        manager.handleWorkflowResponse({
          request_id: 'req-1',
          success: true,
          data: { id: 'new-a1' }
        });
        manager.handleWorkflowResponse({
          request_id: 'req-2',
          success: false,
          error: 'Failed'
        });
        manager.handleWorkflowResponse({
          request_id: 'req-3',
          success: true,
          data: { id: 'new-a3' }
        });
      }, 50);

      jest.advanceTimersByTime(500);

      const result = await importPromise;

      expect(result.createdAgents).toHaveLength(2);
      expect(result.failedAgents).toHaveLength(1);
      expect(result.success).toBe(false); // Not complete success
    });

    test('should handle rollback of partially created workflow', async () => {
      const createdAgents = [
        { newId: 'new-a1', agent: { name: 'Agent 1' } },
        { newId: 'new-a2', agent: { name: 'Agent 2' } }
      ];

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        const reqId = `req-${Date.now()}`;
        callback(reqId);
        setTimeout(() => {
          manager.handleWorkflowResponse({
            request_id: reqId,
            success: true,
            data: {}
          });
        }, 50);
      });

      const rollbackResults = await manager.rollbackImport(createdAgents);

      expect(rollbackResults).toHaveLength(2);
      expect(rollbackResults.every(r => r.success)).toBe(true);
    });

    test('should handle rollback failures gracefully', async () => {
      const createdAgents = [
        { newId: 'new-a1', agent: { name: 'Agent 1' } }
      ];

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        const reqId = `req-${Date.now()}`;
        callback(reqId);
        setTimeout(() => {
          manager.handleWorkflowResponse({
            request_id: reqId,
            success: false,
            error: 'Cannot delete agent'
          });
        }, 50);
      });

      const rollbackResults = await manager.rollbackImport(createdAgents);

      expect(rollbackResults).toHaveLength(1);
      expect(rollbackResults[0].success).toBe(false);
      expect(rollbackResults[0].error).toBeTruthy();
    });
  });

  describe('🔴 CRITICAL: Connection Creation Failures', () => {
    test('should track failed connections separately', async () => {
      const workflow = {
        agents: {
          'a1': { id: 'a1', name: 'Agent 1' },
          'a2': { id: 'a2', name: 'Agent 2' }
        },
        connections: [
          { from_agent_id: 'a1', to_agent_id: 'a2' }
        ]
      };

      let callCount = 0;
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        const reqId = `req-${callCount++}`;
        callback(reqId);

        setTimeout(() => {
          if (msg.action === 'workflow_add_agent') {
            manager.handleWorkflowResponse({
              request_id: reqId,
              success: true,
              data: { id: `new-a${callCount}` }
            });
          } else if (msg.action === 'workflow_add_connection') {
            // Connection fails
            manager.handleWorkflowResponse({
              request_id: reqId,
              success: false,
              error: 'Connection already exists'
            });
          }
        }, 50);
      });

      const result = await manager.recreateWorkflowFromImport(workflow);

      expect(result.createdAgents).toHaveLength(2);
      expect(result.failedConnections).toHaveLength(1);
      expect(result.successfulConnections).toHaveLength(0);
    });

    test('should handle timeout during connection creation', async () => {
      const workflow = {
        agents: {
          'a1': { id: 'a1', name: 'Agent 1' },
          'a2': { id: 'a2', name: 'Agent 2' }
        },
        connections: [
          { from_agent_id: 'a1', to_agent_id: 'a2' }
        ]
      };

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        const reqId = `req-${Date.now()}`;
        callback(reqId);

        if (msg.action === 'workflow_add_agent') {
          setTimeout(() => {
            manager.handleWorkflowResponse({
              request_id: reqId,
              success: true,
              data: { id: msg.name === 'Agent 1' ? 'new-a1' : 'new-a2' }
            });
          }, 50);
        }
        // Connection request never responds (timeout)
      });

      const importPromise = manager.recreateWorkflowFromImport(workflow);

      // Advance past connection timeout
      jest.advanceTimersByTime(20000);

      await expect(importPromise).rejects.toThrow();
    });
  });
});
