/**
 * Unit Tests for WorkflowManager Memory Leaks
 *
 * CRITICAL SECURITY TESTS - Covers:
 * - pendingRequests Map memory leak
 * - Timeout cleanup
 * - Request cleanup on success/failure
 * - Abandoned requests
 * - Maximum request tracking
 */

import { describe, test, expect, jest, beforeEach, afterEach } from '@jest/globals';

// Mock chrome runtime
global.chrome = {
  runtime: {
    sendMessage: jest.fn(),
    lastError: null,
    onMessage: {
      addListener: jest.fn()
    }
  }
};

class TestableWorkflowManager {
  constructor() {
    this.pendingRequests = new Map();
    this.maxPendingRequests = 100; // Safety limit
  }

  sendWorkflowMessage(action, payload = {}) {
    return new Promise((resolve, reject) => {
      // Check if we're hitting memory limits
      if (this.pendingRequests.size >= this.maxPendingRequests) {
        reject(new Error('Too many pending requests'));
        return;
      }

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
        const timeoutId = setTimeout(() => {
          if (this.pendingRequests.has(requestId)) {
            this.pendingRequests.delete(requestId);
            reject(new Error('Request timeout'));
          }
        }, 15000);

        // Store timeout ID for cleanup
        this.pendingRequests.get(requestId).timeoutId = timeoutId;
      });
    });
  }

  handleWorkflowResponse(message) {
    const { action, success, data, error, request_id } = message;

    // Check if we have a pending request for this ID
    if (request_id && this.pendingRequests.has(request_id)) {
      const request = this.pendingRequests.get(request_id);

      // Clear timeout
      if (request.timeoutId) {
        clearTimeout(request.timeoutId);
      }

      // Remove from pending requests
      this.pendingRequests.delete(request_id);

      if (success) {
        request.resolve({ success: true, data });
      } else {
        request.reject(new Error(error || 'Unknown error'));
      }
    }
  }

  // Cleanup all pending requests (e.g., on component unmount)
  cleanup() {
    for (const [requestId, request] of this.pendingRequests.entries()) {
      if (request.timeoutId) {
        clearTimeout(request.timeoutId);
      }
      request.reject(new Error('Cleanup: Request cancelled'));
    }
    this.pendingRequests.clear();
  }

  // Get memory stats
  getMemoryStats() {
    return {
      pendingCount: this.pendingRequests.size,
      pendingIds: Array.from(this.pendingRequests.keys())
    };
  }
}

describe('WorkflowManager - Memory Leak Prevention', () => {
  let manager;

  beforeEach(() => {
    jest.useFakeTimers();
    manager = new TestableWorkflowManager();
    chrome.runtime.sendMessage.mockClear();
    chrome.runtime.lastError = null;
  });

  afterEach(() => {
    jest.useRealTimers();
    manager.cleanup();
  });

  describe('🔴 CRITICAL: pendingRequests Map Cleanup', () => {
    test('should clean up successful requests from pendingRequests', async () => {
      // Mock successful response
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-123');
      });

      const promise = manager.sendWorkflowMessage('test_action');

      expect(manager.pendingRequests.size).toBe(1);
      expect(manager.pendingRequests.has('req-123')).toBe(true);

      // Simulate response
      manager.handleWorkflowResponse({
        request_id: 'req-123',
        success: true,
        data: { result: 'ok' }
      });

      await expect(promise).resolves.toEqual({ success: true, data: { result: 'ok' } });

      // Should be cleaned up
      expect(manager.pendingRequests.size).toBe(0);
      expect(manager.pendingRequests.has('req-123')).toBe(false);
    });

    test('should clean up failed requests from pendingRequests', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-456');
      });

      const promise = manager.sendWorkflowMessage('test_action');

      expect(manager.pendingRequests.size).toBe(1);

      // Simulate error response
      manager.handleWorkflowResponse({
        request_id: 'req-456',
        success: false,
        error: 'Something went wrong'
      });

      await expect(promise).rejects.toThrow('Something went wrong');

      // Should be cleaned up
      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should clean up timed out requests from pendingRequests', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-timeout');
      });

      const promise = manager.sendWorkflowMessage('test_action');

      expect(manager.pendingRequests.size).toBe(1);

      // Fast-forward past timeout (15 seconds)
      jest.advanceTimersByTime(15001);

      await expect(promise).rejects.toThrow('Request timeout');

      // Should be cleaned up
      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should handle multiple concurrent requests', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        const id = `req-${Math.random()}`;
        callback(id);
      });

      // Create 10 concurrent requests
      const promises = [];
      for (let i = 0; i < 10; i++) {
        promises.push(manager.sendWorkflowMessage('test_action'));
      }

      expect(manager.pendingRequests.size).toBe(10);

      // Resolve all requests
      const requestIds = Array.from(manager.pendingRequests.keys());
      requestIds.forEach(id => {
        manager.handleWorkflowResponse({
          request_id: id,
          success: true,
          data: { result: 'ok' }
        });
      });

      await Promise.all(promises);

      // All should be cleaned up
      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should handle mixed success/failure/timeout scenarios', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(`req-${Date.now()}-${Math.random()}`);
      });

      const promises = [];

      // Create 5 requests
      for (let i = 0; i < 5; i++) {
        promises.push(manager.sendWorkflowMessage('test_action').catch(() => null));
      }

      const requestIds = Array.from(manager.pendingRequests.keys());

      // Resolve first request (success)
      manager.handleWorkflowResponse({
        request_id: requestIds[0],
        success: true,
        data: {}
      });

      // Fail second request
      manager.handleWorkflowResponse({
        request_id: requestIds[1],
        success: false,
        error: 'Error'
      });

      // Let third request timeout
      jest.advanceTimersByTime(15001);

      // Resolve fourth request
      manager.handleWorkflowResponse({
        request_id: requestIds[3],
        success: true,
        data: {}
      });

      await Promise.all(promises);

      // Should only have unresolved requests (if any)
      expect(manager.pendingRequests.size).toBeLessThanOrEqual(1);
    });
  });

  describe('🔴 CRITICAL: Timeout Cleanup', () => {
    test('should clear timeout on successful response', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-123');
      });

      const clearTimeoutSpy = jest.spyOn(global, 'clearTimeout');

      manager.sendWorkflowMessage('test_action');

      const request = manager.pendingRequests.get('req-123');
      const timeoutId = request.timeoutId;

      // Respond before timeout
      manager.handleWorkflowResponse({
        request_id: 'req-123',
        success: true,
        data: {}
      });

      expect(clearTimeoutSpy).toHaveBeenCalledWith(timeoutId);

      clearTimeoutSpy.mockRestore();
    });

    test('should clear timeout on error response', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-456');
      });

      const clearTimeoutSpy = jest.spyOn(global, 'clearTimeout');

      manager.sendWorkflowMessage('test_action').catch(() => null);

      const request = manager.pendingRequests.get('req-456');
      const timeoutId = request.timeoutId;

      manager.handleWorkflowResponse({
        request_id: 'req-456',
        success: false,
        error: 'Error'
      });

      expect(clearTimeoutSpy).toHaveBeenCalledWith(timeoutId);

      clearTimeoutSpy.mockRestore();
    });

    test('should not leak timeout handlers', () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(`req-${Date.now()}`);
      });

      // Create many requests
      for (let i = 0; i < 100; i++) {
        manager.sendWorkflowMessage('test_action').catch(() => null);
      }

      const requestIds = Array.from(manager.pendingRequests.keys());

      // Resolve all
      requestIds.forEach(id => {
        manager.handleWorkflowResponse({
          request_id: id,
          success: true,
          data: {}
        });
      });

      // No pending requests should remain
      expect(manager.pendingRequests.size).toBe(0);

      // Advance timers - no timeouts should fire
      const consoleErrorSpy = jest.spyOn(console, 'error').mockImplementation();
      jest.advanceTimersByTime(20000);

      // No errors should occur from orphaned timeouts
      expect(consoleErrorSpy).not.toHaveBeenCalled();

      consoleErrorSpy.mockRestore();
    });
  });

  describe('🔴 CRITICAL: Abandoned Requests', () => {
    test('should handle request with no response (orphaned)', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-orphan');
      });

      const promise = manager.sendWorkflowMessage('test_action');

      expect(manager.pendingRequests.size).toBe(1);

      // Never send response, wait for timeout
      jest.advanceTimersByTime(15001);

      await expect(promise).rejects.toThrow('Request timeout');

      // Should be cleaned up
      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should handle response for non-existent request ID', () => {
      // No pending requests
      expect(manager.pendingRequests.size).toBe(0);

      // Receive response for request that doesn't exist
      manager.handleWorkflowResponse({
        request_id: 'non-existent',
        success: true,
        data: {}
      });

      // Should not crash or create entries
      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should handle duplicate responses for same request', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-double');
      });

      const promise = manager.sendWorkflowMessage('test_action');

      // Send first response
      manager.handleWorkflowResponse({
        request_id: 'req-double',
        success: true,
        data: { first: true }
      });

      // Send duplicate response (should be ignored)
      manager.handleWorkflowResponse({
        request_id: 'req-double',
        success: true,
        data: { second: true }
      });

      const result = await promise;

      // Should resolve with first response
      expect(result.data.first).toBe(true);
      expect(result.data.second).toBeUndefined();

      // Should be cleaned up
      expect(manager.pendingRequests.size).toBe(0);
    });
  });

  describe('🔴 CRITICAL: Memory Limits', () => {
    test('should reject new requests when pending limit reached', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(`req-${Date.now()}-${Math.random()}`);
      });

      // Fill up to limit
      const promises = [];
      for (let i = 0; i < 100; i++) {
        promises.push(manager.sendWorkflowMessage('test_action').catch(e => e));
      }

      expect(manager.pendingRequests.size).toBe(100);

      // Try to create one more (should fail)
      const overLimitPromise = manager.sendWorkflowMessage('test_action');

      await expect(overLimitPromise).rejects.toThrow('Too many pending requests');

      // Should still have 100 pending
      expect(manager.pendingRequests.size).toBe(100);
    });

    test('should allow new requests after cleanup', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(`req-${Date.now()}-${Math.random()}`);
      });

      // Fill up to limit
      for (let i = 0; i < 100; i++) {
        manager.sendWorkflowMessage('test_action').catch(() => null);
      }

      expect(manager.pendingRequests.size).toBe(100);

      // Cleanup all
      const requestIds = Array.from(manager.pendingRequests.keys());
      requestIds.forEach(id => {
        manager.handleWorkflowResponse({
          request_id: id,
          success: true,
          data: {}
        });
      });

      expect(manager.pendingRequests.size).toBe(0);

      // Should be able to create new requests
      const newPromise = manager.sendWorkflowMessage('test_action');
      expect(manager.pendingRequests.size).toBe(1);
    });

    test('should track memory stats correctly', () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(`req-${Date.now()}`);
      });

      expect(manager.getMemoryStats().pendingCount).toBe(0);

      // Create requests
      for (let i = 0; i < 5; i++) {
        manager.sendWorkflowMessage('test_action').catch(() => null);
      }

      const stats = manager.getMemoryStats();
      expect(stats.pendingCount).toBe(5);
      expect(stats.pendingIds).toHaveLength(5);
    });
  });

  describe('🔴 CRITICAL: Cleanup on Component Unmount', () => {
    test('should cleanup all pending requests', () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(`req-${Date.now()}-${Math.random()}`);
      });

      // Create multiple pending requests
      const promises = [];
      for (let i = 0; i < 10; i++) {
        promises.push(manager.sendWorkflowMessage('test_action').catch(e => e.message));
      }

      expect(manager.pendingRequests.size).toBe(10);

      // Cleanup (simulate component unmount)
      manager.cleanup();

      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should clear all timeouts on cleanup', () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(`req-${Date.now()}`);
      });

      const clearTimeoutSpy = jest.spyOn(global, 'clearTimeout');

      // Create requests
      for (let i = 0; i < 5; i++) {
        manager.sendWorkflowMessage('test_action').catch(() => null);
      }

      manager.cleanup();

      // Should have called clearTimeout for each request
      expect(clearTimeoutSpy).toHaveBeenCalledTimes(5);

      clearTimeoutSpy.mockRestore();
    });

    test('should reject all pending promises on cleanup', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(`req-${Date.now()}`);
      });

      const promises = [];
      for (let i = 0; i < 5; i++) {
        promises.push(
          manager.sendWorkflowMessage('test_action').catch(e => e.message)
        );
      }

      manager.cleanup();

      const results = await Promise.all(promises);

      // All should be rejected with cleanup message
      results.forEach(result => {
        expect(result).toBe('Cleanup: Request cancelled');
      });
    });
  });

  describe('🔴 CRITICAL: Error Handling', () => {
    test('should handle chrome.runtime.lastError', async () => {
      chrome.runtime.lastError = { message: 'Extension context invalidated' };
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(null);
      });

      const promise = manager.sendWorkflowMessage('test_action');

      await expect(promise).rejects.toThrow('Extension context invalidated');

      // Should not create pending request
      expect(manager.pendingRequests.size).toBe(0);

      // Cleanup
      chrome.runtime.lastError = null;
    });

    test('should handle missing request ID', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(null); // No request ID
      });

      const promise = manager.sendWorkflowMessage('test_action');

      await expect(promise).rejects.toThrow('No request ID returned');

      // Should not create pending request
      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should not leak memory on repeated errors', async () => {
      chrome.runtime.lastError = { message: 'Error' };
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback(null);
      });

      // Try to create many failed requests
      const promises = [];
      for (let i = 0; i < 50; i++) {
        promises.push(manager.sendWorkflowMessage('test_action').catch(() => null));
      }

      await Promise.all(promises);

      // Should have no pending requests
      expect(manager.pendingRequests.size).toBe(0);

      chrome.runtime.lastError = null;
    });
  });

  describe('🔴 CRITICAL: Long-Running Requests', () => {
    test('should handle requests that take longer than timeout', async () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-slow');
      });

      const promise = manager.sendWorkflowMessage('test_action');

      // Advance time past timeout
      jest.advanceTimersByTime(20000);

      await expect(promise).rejects.toThrow('Request timeout');

      // Late response should be ignored
      manager.handleWorkflowResponse({
        request_id: 'req-slow',
        success: true,
        data: {}
      });

      // Should not create any issues
      expect(manager.pendingRequests.size).toBe(0);
    });

    test('should handle very slow responses gracefully', () => {
      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-very-slow');
      });

      manager.sendWorkflowMessage('test_action').catch(() => null);

      // Advance far into future
      jest.advanceTimersByTime(100000);

      // Try to respond
      manager.handleWorkflowResponse({
        request_id: 'req-very-slow',
        success: true,
        data: {}
      });

      // Should handle gracefully (no crash)
      expect(manager.pendingRequests.size).toBe(0);
    });
  });
});
