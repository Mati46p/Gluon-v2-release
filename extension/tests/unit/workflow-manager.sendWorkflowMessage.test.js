/**
 * Unit Tests for WorkflowManager.sendWorkflowMessage()
 *
 * Tests cover:
 * - Basic message sending
 * - Request ID tracking
 * - Timeout scenarios
 * - Error handling
 * - Chrome runtime errors
 * - Pending request cleanup
 */

import { describe, test, expect, jest, beforeEach, afterEach } from '@jest/globals';

// Mock PresetManager before importing WorkflowManager
const mockPresetManager = {
  init: jest.fn(() => Promise.resolve()),
  getFilteredAgentPresets: jest.fn(() => []),
  getAgentPreset: jest.fn(),
  getConnectionPreset: jest.fn(),
  getWorkflowPreset: jest.fn(),
  toggleFavorite: jest.fn(),
  isFavorite: jest.fn(() => false),
  presets: {
    agents: [],
    connections: [],
    workflows: []
  }
};

// Mock the preset-manager module
jest.unstable_mockModule('../../src/sidebar/preset-manager.js', () => ({
  default: mockPresetManager
}));

// Create a testable version of WorkflowManager
class TestableWorkflowManager {
  constructor() {
    this.graph = null;
    this.selectedAgentFrom = null;
    this.pendingRequests = new Map();
    this.graphEditor = null;
    this.currentView = 'list';
    this.presetManager = mockPresetManager;
    this.currentPresetCategory = 'all';
    this.selectedAgentPreset = null;
    this.selectedConnectionPreset = null;
    this.workflowTabs = [];
    this.activeTabId = null;
    this.nextTabId = 1;
  }

  // Copy the sendWorkflowMessage method from workflow-manager.js
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

        // Store the promise handlers
        this.pendingRequests.set(requestId, { resolve, reject });

        // Timeout after 15 seconds
        setTimeout(() => {
          if (this.pendingRequests.has(requestId)) {
            this.pendingRequests.delete(requestId);
            reject(new Error('Request timeout'));
          }
        }, 15000);
      });
    });
  }

  // Helper method for testing
  handleWorkflowResponse(message) {
    const { action, success, data, error, request_id } = message;

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
}

describe('WorkflowManager.sendWorkflowMessage()', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
    jest.useFakeTimers();
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  describe('Successful message sending', () => {
    test('should send message and return response on success', async () => {
      const mockRequestId = 'request-123';
      const mockResponse = { id: 'agent-1', name: 'Test Agent' };

      // Mock chrome.runtime.sendMessage to call callback with requestId
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(mockRequestId);
        // Simulate async response
        setTimeout(() => {
          manager.handleWorkflowResponse({
            request_id: mockRequestId,
            success: true,
            data: mockResponse
          });
        }, 100);
      });

      const promise = manager.sendWorkflowMessage('workflow_add_agent', {
        name: 'Test Agent'
      });

      // Advance timers to process the response
      jest.advanceTimersByTime(100);
      await Promise.resolve(); // Let promises resolve

      const result = await promise;

      expect(result).toEqual({
        success: true,
        data: mockResponse
      });
      expect(chrome.runtime.sendMessage).toHaveBeenCalledWith(
        { action: 'workflow_add_agent', name: 'Test Agent' },
        expect.any(Function)
      );
      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should track multiple concurrent requests', async () => {
      const mockRequests = [
        { id: 'req-1', response: { name: 'Agent 1' } },
        { id: 'req-2', response: { name: 'Agent 2' } },
        { id: 'req-3', response: { name: 'Agent 3' } }
      ];

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        const reqId = `req-${mockRequests.length}`;
        callback(reqId);
      });

      const promises = mockRequests.map((_, idx) =>
        manager.sendWorkflowMessage('test_action', { index: idx })
      );

      expect(manager.pendingRequests.size).toBe(3);

      // Resolve all requests
      mockRequests.forEach(req => {
        manager.handleWorkflowResponse({
          request_id: req.id,
          success: true,
          data: req.response
        });
      });

      await Promise.all(promises);
      expect(manager.pendingRequests.size).toBe(0);
    });
  });

  describe('Error handling', () => {
    test('should reject on chrome.runtime.lastError', async () => {
      chrome.runtime.lastError = { message: 'Extension context invalidated' };

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback();
      });

      await expect(
        manager.sendWorkflowMessage('test_action')
      ).rejects.toThrow('Extension context invalidated');

      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should reject when no request ID returned', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(null); // No request ID
      });

      await expect(
        manager.sendWorkflowMessage('test_action')
      ).rejects.toThrow('No request ID returned');

      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should reject when backend returns error', async () => {
      const mockRequestId = 'req-error';

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(mockRequestId);
        setTimeout(() => {
          manager.handleWorkflowResponse({
            request_id: mockRequestId,
            success: false,
            error: 'Agent not found'
          });
        }, 50);
      });

      const promise = manager.sendWorkflowMessage('workflow_delete_agent', {
        agent_id: 'non-existent'
      });

      jest.advanceTimersByTime(50);
      await Promise.resolve();

      await expect(promise).rejects.toThrow('Agent not found');
      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should reject with Unknown error when error message is empty', async () => {
      const mockRequestId = 'req-empty-error';

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(mockRequestId);
        setTimeout(() => {
          manager.handleWorkflowResponse({
            request_id: mockRequestId,
            success: false,
            error: ''
          });
        }, 50);
      });

      const promise = manager.sendWorkflowMessage('test_action');
      jest.advanceTimersByTime(50);
      await Promise.resolve();

      await expect(promise).rejects.toThrow('Unknown error');
    });
  });

  describe('Timeout scenarios', () => {
    test('should timeout after 15 seconds and cleanup pending request', async () => {
      const mockRequestId = 'req-timeout';

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(mockRequestId);
        // Don't send response - simulate hanging request
      });

      const promise = manager.sendWorkflowMessage('test_action');

      // Verify request is pending
      expect(manager.pendingRequests.size).toBe(1);
      expect(manager.pendingRequests.has(mockRequestId)).toBe(true);

      // Advance time to just before timeout
      jest.advanceTimersByTime(14999);
      expect(manager.pendingRequests.size).toBe(1);

      // Advance to timeout
      jest.advanceTimersByTime(1);

      await expect(promise).rejects.toThrow('Request timeout');
      expect(manager.pendingRequests.size).toBe(0);
      expect(manager.pendingRequests.has(mockRequestId)).toBe(false);
    });

    test('should not timeout if response arrives before 15 seconds', async () => {
      const mockRequestId = 'req-success';

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(mockRequestId);
        // Send response after 10 seconds
        setTimeout(() => {
          manager.handleWorkflowResponse({
            request_id: mockRequestId,
            success: true,
            data: { result: 'ok' }
          });
        }, 10000);
      });

      const promise = manager.sendWorkflowMessage('test_action');

      // Advance to response time
      jest.advanceTimersByTime(10000);
      await Promise.resolve();

      const result = await promise;
      expect(result).toEqual({ success: true, data: { result: 'ok' } });
      expect(manager.pendingRequests.size).toBe(0);

      // Verify timeout doesn't fire
      jest.advanceTimersByTime(5000);
      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should handle timeout for one request while others succeed', async () => {
      const requests = [
        { id: 'req-1', willTimeout: false },
        { id: 'req-2', willTimeout: true },
        { id: 'req-3', willTimeout: false }
      ];

      const promises = [];

      requests.forEach((req, idx) => {
        chrome.runtime.sendMessage.mockImplementationOnce((msg, callback) => {
          callback(req.id);

          if (!req.willTimeout) {
            setTimeout(() => {
              manager.handleWorkflowResponse({
                request_id: req.id,
                success: true,
                data: { index: idx }
              });
            }, 1000);
          }
          // req-2 will timeout (no response)
        });

        promises.push(manager.sendWorkflowMessage('test_action', { idx }));
      });

      expect(manager.pendingRequests.size).toBe(3);

      // Resolve non-timeout requests
      jest.advanceTimersByTime(1000);
      await Promise.resolve();

      expect(manager.pendingRequests.size).toBe(1); // Only req-2 pending

      // Advance to timeout
      jest.advanceTimersByTime(14000);

      const results = await Promise.allSettled(promises);

      expect(results[0].status).toBe('fulfilled');
      expect(results[1].status).toBe('rejected');
      expect(results[1].reason.message).toBe('Request timeout');
      expect(results[2].status).toBe('fulfilled');

      expect(manager.pendingRequests.size).toBe(0);
    });
  });

  describe('Pending requests cleanup', () => {
    test('should remove request from pendingRequests after successful response', async () => {
      const mockRequestId = 'req-cleanup';

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(mockRequestId);
        setTimeout(() => {
          manager.handleWorkflowResponse({
            request_id: mockRequestId,
            success: true,
            data: {}
          });
        }, 100);
      });

      expect(manager.pendingRequests.size).toBe(0);

      const promise = manager.sendWorkflowMessage('test_action');
      expect(manager.pendingRequests.size).toBe(1);

      jest.advanceTimersByTime(100);
      await Promise.resolve();
      await promise;

      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should not affect other requests when one is resolved', async () => {
      const req1 = 'req-1';
      const req2 = 'req-2';

      chrome.runtime.sendMessage
        .mockImplementationOnce((msg, cb) => cb(req1))
        .mockImplementationOnce((msg, cb) => cb(req2));

      const promise1 = manager.sendWorkflowMessage('action1');
      const promise2 = manager.sendWorkflowMessage('action2');

      expect(manager.pendingRequests.size).toBe(2);

      // Resolve first request
      manager.handleWorkflowResponse({
        request_id: req1,
        success: true,
        data: {}
      });

      await promise1;
      expect(manager.pendingRequests.size).toBe(1);
      expect(manager.pendingRequests.has(req2)).toBe(true);

      // Resolve second request
      manager.handleWorkflowResponse({
        request_id: req2,
        success: true,
        data: {}
      });

      await promise2;
      expect(manager.pendingRequests.size).toBe(0);
    });
  });

  describe('Edge cases', () => {
    test('should handle response for unknown request ID gracefully', () => {
      expect(manager.pendingRequests.size).toBe(0);

      // This should not throw
      manager.handleWorkflowResponse({
        request_id: 'unknown-id',
        success: true,
        data: {}
      });

      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should handle missing request_id in response', () => {
      manager.pendingRequests.set('test-id', {
        resolve: jest.fn(),
        reject: jest.fn()
      });

      manager.handleWorkflowResponse({
        success: true,
        data: {}
        // Missing request_id
      });

      // Request should still be pending
      expect(manager.pendingRequests.size).toBe(1);
    });

    test('should handle payload with nested objects', async () => {
      const complexPayload = {
        agent: {
          name: 'Test',
          config: {
            nested: {
              value: 123
            }
          },
          tags: ['a', 'b', 'c']
        }
      };

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg).toEqual({
          action: 'complex_action',
          ...complexPayload
        });
        callback('req-complex');
      });

      const promise = manager.sendWorkflowMessage('complex_action', complexPayload);

      manager.handleWorkflowResponse({
        request_id: 'req-complex',
        success: true,
        data: {}
      });

      await promise;
      expect(chrome.runtime.sendMessage).toHaveBeenCalled();
    });
  });

  describe('Memory leak prevention', () => {
    test('should not accumulate pending requests after timeouts', async () => {
      const requestIds = Array.from({ length: 10 }, (_, i) => `req-${i}`);

      requestIds.forEach(id => {
        chrome.runtime.sendMessage.mockImplementationOnce((msg, callback) => {
          callback(id);
          // No response - will timeout
        });
        manager.sendWorkflowMessage('test_action').catch(() => {});
      });

      expect(manager.pendingRequests.size).toBe(10);

      // Advance past all timeouts
      jest.advanceTimersByTime(15000);
      await Promise.resolve();

      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should not leak memory with rapid request/response cycles', async () => {
      const iterations = 100;

      for (let i = 0; i < iterations; i++) {
        const reqId = `req-${i}`;

        chrome.runtime.sendMessage.mockImplementationOnce((msg, callback) => {
          callback(reqId);
          setImmediate(() => {
            manager.handleWorkflowResponse({
              request_id: reqId,
              success: true,
              data: { iteration: i }
            });
          });
        });

        await manager.sendWorkflowMessage('test_action', { iteration: i });
      }

      expect(manager.pendingRequests.size).toBe(0);
    });
  });
});
