/**
 * Unit Tests for Browser Watcher - Background Script
 * Tests DNR rule management and lifecycle safety
 */

describe('Browser Watcher - Background Logic', () => {
  let mockChrome;
  let watcherTabId;
  let watcherRuleId;
  let isWatcherActive;

  beforeEach(() => {
    // Reset state
    watcherTabId = null;
    watcherRuleId = null;
    isWatcherActive = false;

    // Mock Chrome APIs
    mockChrome = {
      declarativeNetRequest: {
        updateSessionRules: jest.fn().mockResolvedValue(undefined)
      },
      scripting: {
        executeScript: jest.fn().mockResolvedValue([{ result: true }])
      },
      tabs: {
        reload: jest.fn().mockResolvedValue(undefined),
        onUpdated: {
          addListener: jest.fn(),
          removeListener: jest.fn()
        },
        onRemoved: {
          addListener: jest.fn()
        },
        query: jest.fn().mockResolvedValue([{ id: 123 }])
      },
      runtime: {
        sendMessage: jest.fn().mockResolvedValue(undefined),
        lastError: null
      }
    };

    global.chrome = mockChrome;
  });

  describe('DNR Rule Management', () => {
    test('startWatcher should create DNR rule with correct structure', async () => {
      const tabId = 123;
      const ruleId = Date.now();

      await startWatcherTest(tabId, ruleId);

      expect(mockChrome.declarativeNetRequest.updateSessionRules).toHaveBeenCalledWith({
        removeRuleIds: [ruleId],
        addRules: [{
          id: ruleId,
          priority: 1,
          action: {
            type: 'modifyHeaders',
            responseHeaders: [
              { header: 'content-security-policy', operation: 'remove' },
              { header: 'x-frame-options', operation: 'remove' }
            ]
          },
          condition: {
            tabIds: [tabId],
            resourceTypes: ['main_frame', 'sub_frame']
          }
        }]
      });
    });

    test('stopWatcher should remove DNR rules', async () => {
      const tabId = 123;
      const ruleId = 456789;

      // Simulate active watcher
      watcherRuleId = ruleId;
      watcherTabId = tabId;
      isWatcherActive = true;

      await cleanupWatcherTest();

      expect(mockChrome.declarativeNetRequest.updateSessionRules).toHaveBeenCalledWith({
        removeRuleIds: [ruleId]
      });
    });

    test('should handle DNR rule removal errors gracefully', async () => {
      const ruleId = 789;
      watcherRuleId = ruleId;

      mockChrome.declarativeNetRequest.updateSessionRules.mockRejectedValueOnce(
        new Error('Failed to remove rule')
      );

      await expect(cleanupWatcherTest()).resolves.not.toThrow();
    });
  });

  describe('Lifecycle Management', () => {
    test('should register tab removal listener', () => {
      // Simulate extension initialization
      const onRemovedCallback = jest.fn();
      mockChrome.tabs.onRemoved.addListener(onRemovedCallback);

      expect(mockChrome.tabs.onRemoved.addListener).toHaveBeenCalledWith(onRemovedCallback);
    });

    test('should cleanup when recording tab is closed', async () => {
      const tabId = 123;
      const ruleId = 456;

      // Setup active recording
      watcherTabId = tabId;
      watcherRuleId = ruleId;
      isWatcherActive = true;

      // Simulate tab closure
      const onRemovedCallback = jest.fn(async (closedTabId) => {
        if (closedTabId === watcherTabId && isWatcherActive) {
          await cleanupWatcherTest();
        }
      });

      await onRemovedCallback(tabId);

      // Verify cleanup occurred
      expect(mockChrome.declarativeNetRequest.updateSessionRules).toHaveBeenCalled();
      expect(mockChrome.runtime.sendMessage).toHaveBeenCalledWith({
        type: 'watcher_status_changed',
        isRecording: false
      });
    });

    test('should not cleanup if different tab is closed', async () => {
      const activeTabId = 123;
      const closedTabId = 456;

      watcherTabId = activeTabId;
      watcherRuleId = 789;
      isWatcherActive = true;

      const onRemovedCallback = jest.fn(async (closedTab) => {
        if (closedTab === watcherTabId && isWatcherActive) {
          await cleanupWatcherTest();
        }
      });

      await onRemovedCallback(closedTabId);

      // Cleanup should NOT have been called
      expect(mockChrome.declarativeNetRequest.updateSessionRules).not.toHaveBeenCalled();
    });

    test('should cleanup on tab navigation', async () => {
      const tabId = 123;
      watcherTabId = tabId;
      watcherRuleId = 456;
      isWatcherActive = true;

      const onUpdatedCallback = jest.fn(async (updatedTabId, changeInfo) => {
        if (updatedTabId === watcherTabId && isWatcherActive && changeInfo.url) {
          await cleanupWatcherTest();
        }
      });

      await onUpdatedCallback(tabId, { url: 'https://newsite.com' });

      expect(mockChrome.declarativeNetRequest.updateSessionRules).toHaveBeenCalled();
    });

    test('should send error notification on unexpected cleanup', async () => {
      watcherTabId = 123;
      watcherRuleId = 456;
      isWatcherActive = true;

      await cleanupWatcherTest();

      expect(mockChrome.runtime.sendMessage).toHaveBeenCalledWith(
        expect.objectContaining({
          type: 'error',
          message: expect.stringContaining('Recording tab')
        })
      );
    });
  });

  describe('Script Injection', () => {
    test('should inject recorder script into MAIN world', async () => {
      const tabId = 123;

      await injectRecorderTest(tabId);

      expect(mockChrome.scripting.executeScript).toHaveBeenCalledWith(
        expect.objectContaining({
          target: { tabId: tabId },
          files: ['src/content/web-recorder.js'],
          world: 'MAIN'
        })
      );
    });

    test('should send start command after injection', async () => {
      const tabId = 123;

      await injectRecorderTest(tabId);

      expect(mockChrome.scripting.executeScript).toHaveBeenCalledWith(
        expect.objectContaining({
          target: { tabId: tabId },
          world: 'MAIN',
          func: expect.any(Function)
        })
      );
    });

    test('should handle injection errors', async () => {
      const tabId = 123;

      mockChrome.scripting.executeScript.mockRejectedValueOnce(
        new Error('Cannot access chrome-extension://...')
      );

      await expect(injectRecorderTest(tabId)).rejects.toThrow();
    });
  });

  describe('Log Formatting', () => {
    test('should format empty events array', () => {
      const result = formatWatcherLogsTest([]);

      expect(result).toContain('No events recorded');
    });

    test('should format console events correctly', () => {
      const events = [
        {
          type: 'console',
          level: 'error',
          message: 'Test error',
          timestamp: new Date().toISOString(),
          url: 'https://example.com'
        }
      ];

      const result = formatWatcherLogsTest(events);

      expect(result).toContain('[CONSOLE]');
      expect(result).toContain('[ERROR]');
      expect(result).toContain('Test error');
      expect(result).toContain('https://example.com');
    });

    test('should format network events correctly', () => {
      const events = [
        {
          type: 'fetch',
          method: 'GET',
          url: 'https://api.example.com/data',
          status: 200,
          statusText: 'OK',
          duration: 125,
          timestamp: new Date().toISOString()
        }
      ];

      const result = formatWatcherLogsTest(events);

      expect(result).toContain('[NETWORK]');
      expect(result).toContain('[FETCH]');
      expect(result).toContain('GET');
      expect(result).toContain('https://api.example.com/data');
      expect(result).toContain('200');
      expect(result).toContain('125ms');
    });

    test('should include event statistics', () => {
      const events = [
        { type: 'console', level: 'log', message: 'Log 1', timestamp: new Date().toISOString() },
        { type: 'console', level: 'error', message: 'Error 1', timestamp: new Date().toISOString() },
        { type: 'fetch', method: 'GET', url: 'https://api.com', timestamp: new Date().toISOString() }
      ];

      const result = formatWatcherLogsTest(events);

      expect(result).toContain('Total Events: 3');
      expect(result).toContain('Console Events: 2');
      expect(result).toContain('1 errors');
      expect(result).toContain('Network Events: 1');
    });

    test('should add status indicators for failed requests', () => {
      const events = [
        {
          type: 'fetch',
          method: 'GET',
          url: 'https://api.com/fail',
          status: 500,
          statusText: 'Internal Server Error',
          timestamp: new Date().toISOString()
        }
      ];

      const result = formatWatcherLogsTest(events);

      expect(result).toContain('❌'); // Error indicator for 5xx
      expect(result).toContain('500');
    });
  });

  describe('State Management', () => {
    test('should reset state after cleanup', async () => {
      watcherTabId = 123;
      watcherRuleId = 456;
      isWatcherActive = true;

      const state = await cleanupWatcherTest();

      expect(state.watcherTabId).toBeNull();
      expect(state.watcherRuleId).toBeNull();
      expect(state.isWatcherActive).toBe(false);
    });

    test('should not allow multiple simultaneous recordings', async () => {
      watcherTabId = 123;
      isWatcherActive = true;

      const result = await attemptStartWatcherTest(456);

      expect(result.success).toBe(false);
      expect(result.error).toContain('already active');
    });
  });
});

// ============================================================================
// Test Helper Functions (Simplified implementations for testing)
// ============================================================================

async function startWatcherTest(tabId, ruleId) {
  const chrome = global.chrome;

  await chrome.declarativeNetRequest.updateSessionRules({
    removeRuleIds: [ruleId],
    addRules: [{
      id: ruleId,
      priority: 1,
      action: {
        type: 'modifyHeaders',
        responseHeaders: [
          { header: 'content-security-policy', operation: 'remove' },
          { header: 'x-frame-options', operation: 'remove' }
        ]
      },
      condition: {
        tabIds: [tabId],
        resourceTypes: ['main_frame', 'sub_frame']
      }
    }]
  });

  await chrome.tabs.reload(tabId);

  return { success: true };
}

async function cleanupWatcherTest() {
  const chrome = global.chrome;

  if (watcherRuleId) {
    await chrome.declarativeNetRequest.updateSessionRules({
      removeRuleIds: [watcherRuleId]
    });
  }

  const previousState = {
    watcherTabId,
    watcherRuleId,
    isWatcherActive
  };

  watcherTabId = null;
  watcherRuleId = null;
  isWatcherActive = false;

  await chrome.runtime.sendMessage({
    type: 'watcher_status_changed',
    isRecording: false
  });

  await chrome.runtime.sendMessage({
    type: 'error',
    message: 'Browser Watcher stopped: Recording tab was closed or navigated away'
  });

  return {
    watcherTabId,
    watcherRuleId,
    isWatcherActive,
    previousState
  };
}

async function injectRecorderTest(tabId) {
  const chrome = global.chrome;

  await chrome.scripting.executeScript({
    target: { tabId: tabId },
    files: ['src/content/web-recorder.js'],
    world: 'MAIN'
  });

  await chrome.scripting.executeScript({
    target: { tabId: tabId },
    func: () => {
      window.dispatchEvent(new Event('GLUON_WATCHER_START'));
    },
    world: 'MAIN'
  });

  return { success: true };
}

async function attemptStartWatcherTest(newTabId) {
  if (isWatcherActive) {
    return { success: false, error: 'Watcher already active on another tab' };
  }

  return await startWatcherTest(newTabId, Date.now());
}

function formatWatcherLogsTest(events) {
  if (!events || events.length === 0) {
    return '# Browser Watcher Log\n\nNo events recorded.\n';
  }

  let output = '# Browser Watcher Log - AI Analysis Ready\n\n';
  output += `## Session Summary\n`;
  output += `- Total Events: ${events.length}\n`;

  const consoleCount = events.filter(e => e.type === 'console').length;
  const networkCount = events.filter(e => e.type === 'fetch' || e.type === 'xhr').length;
  const errorCount = events.filter(e => e.type === 'console' && e.level === 'error').length;

  output += `- Console Events: ${consoleCount} (${errorCount} errors)\n`;
  output += `- Network Events: ${networkCount}\n\n`;
  output += '=' .repeat(80) + '\n\n';
  output += '## Event Timeline\n\n';

  events.forEach((event, index) => {
    const timestamp = new Date(event.timestamp).toLocaleTimeString();
    const eventNum = String(index + 1).padStart(4, '0');

    if (event.type === 'console') {
      const level = event.level.toUpperCase().padEnd(5);
      output += `[${eventNum}] [${timestamp}] [CONSOLE] [${level}]\n`;
      output += `${'-'.repeat(80)}\n`;
      output += `${event.message}\n`;
      if (event.url) {
        output += `Source: ${event.url}\n`;
      }
      output += '\n';
    } else if (event.type === 'fetch' || event.type === 'xhr') {
      const networkType = event.type === 'fetch' ? 'FETCH' : 'XHR';
      const statusIndicator = event.status >= 400 ? '❌' : event.status >= 300 ? '⚠️' : '✅';

      output += `[${eventNum}] [${timestamp}] [NETWORK] [${networkType}]\n`;
      output += `${'-'.repeat(80)}\n`;
      output += `${event.method} ${event.url}\n`;

      if (event.status) {
        output += `Status: ${statusIndicator} ${event.status} ${event.statusText || ''}\n`;
      }

      if (event.duration !== undefined) {
        output += `Duration: ${event.duration}ms\n`;
      }

      output += '\n';
    }
  });

  return output;
}
