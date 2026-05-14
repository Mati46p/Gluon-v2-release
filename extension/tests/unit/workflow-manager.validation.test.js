/**
 * Unit Tests for WorkflowManager Validation Logic
 *
 * Tests cover:
 * - handleAddAgent() input validation
 * - connectionExists() edge cases
 * - escapeHtml() XSS prevention
 * - Template validation
 */

import { describe, test, expect, jest, beforeEach } from '@jest/globals';

const mockPresetManager = {
  init: jest.fn(() => Promise.resolve()),
  presets: { agents: [], connections: [], workflows: [] }
};

class TestableWorkflowManager {
  constructor() {
    this.graph = {
      agents: {},
      connections: [],
      auto_forward: false
    };
    this.presetManager = mockPresetManager;
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
      });
    });
  }

  showErrorMessage(message) {
    // Mock implementation
    this.lastErrorMessage = message;
  }

  showSuccessMessage(message) {
    // Mock implementation
    this.lastSuccessMessage = message;
  }

  async handleAddAgent() {
    const nameInput = document.getElementById('agentNameInput');
    const wrapperCheckbox = document.getElementById('addWrapperCheckbox');
    const wrapperInput = document.getElementById('wrapperTemplateInput');
    const agentTypeRadio = document.querySelector('input[name="agentType"]:checked');

    const name = nameInput?.value.trim() || '';
    if (!name) {
      this.showErrorMessage('Agent name is required');
      return;
    }

    const wrapper = wrapperCheckbox?.checked ? wrapperInput?.value.trim() : null;
    const agentType = agentTypeRadio ? agentTypeRadio.value : 'Normal';

    // Validate wrapper template if provided
    if (wrapper && !wrapper.includes('{content}')) {
      this.showErrorMessage('Wrapper template must contain {content} placeholder');
      return;
    }

    try {
      const response = await this.sendWorkflowMessage('workflow_add_agent', {
        name: name,
        output_wrapper: wrapper || null,
        agent_type: agentType,
        position: null
      });

      if (response && response.success) {
        this.showSuccessMessage(`Agent "${name}" created successfully`);
        return response.data;
      } else {
        this.showErrorMessage('Failed to create agent: ' + (response?.error || 'Unknown error'));
      }
    } catch (error) {
      this.showErrorMessage('Error adding agent: ' + error.message);
    }
  }

  connectionExists(fromId, toId) {
    if (!this.graph || !this.graph.connections) return false;
    return this.graph.connections.some(
      c => c.from_agent_id === fromId && c.to_agent_id === toId
    );
  }

  escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }
}

describe('WorkflowManager - handleAddAgent() Validation', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
    document.body.innerHTML = `
      <input id="agentNameInput" type="text" />
      <input id="addWrapperCheckbox" type="checkbox" />
      <textarea id="wrapperTemplateInput"></textarea>
      <input type="radio" name="agentType" value="Normal" />
      <input type="radio" name="agentType" value="Report" />
    `;
  });

  describe('Name validation', () => {
    test('should reject empty agent name', async () => {
      document.getElementById('agentNameInput').value = '';

      await manager.handleAddAgent();

      expect(manager.lastErrorMessage).toBe('Agent name is required');
      expect(chrome.runtime.sendMessage).not.toHaveBeenCalled();
    });

    test('should reject whitespace-only name', async () => {
      document.getElementById('agentNameInput').value = '   ';

      await manager.handleAddAgent();

      expect(manager.lastErrorMessage).toBe('Agent name is required');
      expect(chrome.runtime.sendMessage).not.toHaveBeenCalled();
    });

    test('should accept valid name', async () => {
      document.getElementById('agentNameInput').value = 'Valid Agent Name';
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-1');
        setTimeout(() => {
          const handlers = manager.pendingRequests.get('req-1');
          handlers.resolve({
            success: true,
            data: { id: 'agent-1', name: 'Valid Agent Name' }
          });
          manager.pendingRequests.delete('req-1');
        }, 0);
      });

      await manager.handleAddAgent();

      expect(manager.lastErrorMessage).toBeUndefined();
      expect(chrome.runtime.sendMessage).toHaveBeenCalledWith(
        expect.objectContaining({
          action: 'workflow_add_agent',
          name: 'Valid Agent Name'
        }),
        expect.any(Function)
      );
    });

    test('should trim agent name', async () => {
      document.getElementById('agentNameInput').value = '  Trimmed Name  ';
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg.name).toBe('Trimmed Name');
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();
    });

    test('should handle special characters in name', async () => {
      const specialName = 'Agent-123_Test@#$%';
      document.getElementById('agentNameInput').value = specialName;
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg.name).toBe(specialName);
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();
    });

    test('should handle unicode characters in name', async () => {
      const unicodeName = 'Agent 测试 🤖';
      document.getElementById('agentNameInput').value = unicodeName;
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg.name).toBe(unicodeName);
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();
    });
  });

  describe('Wrapper template validation', () => {
    test('should reject wrapper without {content} placeholder', async () => {
      document.getElementById('agentNameInput').value = 'Test Agent';
      document.getElementById('addWrapperCheckbox').checked = true;
      document.getElementById('wrapperTemplateInput').value = 'Invalid wrapper without placeholder';

      await manager.handleAddAgent();

      expect(manager.lastErrorMessage).toBe('Wrapper template must contain {content} placeholder');
      expect(chrome.runtime.sendMessage).not.toHaveBeenCalled();
    });

    test('should accept wrapper with {content} placeholder', async () => {
      document.getElementById('agentNameInput').value = 'Test Agent';
      document.getElementById('addWrapperCheckbox').checked = true;
      document.getElementById('wrapperTemplateInput').value = 'Header\n{content}\nFooter';
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg.output_wrapper).toBe('Header\n{content}\nFooter');
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();

      expect(manager.lastErrorMessage).toBeUndefined();
    });

    test('should send null wrapper when checkbox unchecked', async () => {
      document.getElementById('agentNameInput').value = 'Test Agent';
      document.getElementById('addWrapperCheckbox').checked = false;
      document.getElementById('wrapperTemplateInput').value = 'Ignored wrapper {content}';
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg.output_wrapper).toBeNull();
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();
    });

    test('should accept multiple {content} placeholders', async () => {
      document.getElementById('agentNameInput').value = 'Test Agent';
      document.getElementById('addWrapperCheckbox').checked = true;
      document.getElementById('wrapperTemplateInput').value = 'Start {content} Middle {content} End';
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();

      expect(manager.lastErrorMessage).toBeUndefined();
    });

    test('should trim wrapper template', async () => {
      document.getElementById('agentNameInput').value = 'Test Agent';
      document.getElementById('addWrapperCheckbox').checked = true;
      document.getElementById('wrapperTemplateInput').value = '  {content}  ';
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg.output_wrapper).toBe('{content}');
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();
    });

    test('should handle empty wrapper when checkbox is checked', async () => {
      document.getElementById('agentNameInput').value = 'Test Agent';
      document.getElementById('addWrapperCheckbox').checked = true;
      document.getElementById('wrapperTemplateInput').value = '';
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg.output_wrapper).toBeNull();
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();
    });
  });

  describe('Agent type validation', () => {
    test('should default to Normal when no radio selected', async () => {
      document.getElementById('agentNameInput').value = 'Test Agent';

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg.agent_type).toBe('Normal');
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();
    });

    test('should use Report type when selected', async () => {
      document.getElementById('agentNameInput').value = 'Test Aggregator';
      document.querySelector('input[name="agentType"][value="Report"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg.agent_type).toBe('Report');
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();
    });
  });

  describe('Backend response handling', () => {
    test('should show error when backend returns error', async () => {
      document.getElementById('agentNameInput').value = 'Test Agent';
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({
          success: false,
          error: 'Database connection failed'
        });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();

      expect(manager.lastErrorMessage).toBe('Failed to create agent: Database connection failed');
    });

    test('should handle unknown backend error', async () => {
      document.getElementById('agentNameInput').value = 'Test Agent';
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({
          success: false
        });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();

      expect(manager.lastErrorMessage).toBe('Failed to create agent: Unknown error');
    });

    test('should handle network error', async () => {
      document.getElementById('agentNameInput').value = 'Test Agent';
      document.querySelector('input[name="agentType"][value="Normal"]').checked = true;

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.reject(new Error('Network timeout'));
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();

      expect(manager.lastErrorMessage).toBe('Error adding agent: Network timeout');
    });
  });

  describe('Missing DOM elements', () => {
    test('should handle missing name input', async () => {
      document.body.innerHTML = '';

      await manager.handleAddAgent();

      expect(manager.lastErrorMessage).toBe('Agent name is required');
    });

    test('should handle missing wrapper elements', async () => {
      document.body.innerHTML = '<input id="agentNameInput" value="Test" />';

      chrome.runtime.sendMessage.mockImplementation((msg, callback) => {
        expect(msg.output_wrapper).toBeNull();
        callback('req-1');
        const handlers = manager.pendingRequests.get('req-1');
        handlers.resolve({ success: true, data: {} });
        manager.pendingRequests.delete('req-1');
      });

      await manager.handleAddAgent();
    });
  });
});

describe('WorkflowManager - connectionExists()', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
  });

  describe('Basic functionality', () => {
    test('should return true for existing connection', () => {
      manager.graph.connections = [
        { from_agent_id: 'a1', to_agent_id: 'a2' },
        { from_agent_id: 'a2', to_agent_id: 'a3' }
      ];

      expect(manager.connectionExists('a1', 'a2')).toBe(true);
      expect(manager.connectionExists('a2', 'a3')).toBe(true);
    });

    test('should return false for non-existing connection', () => {
      manager.graph.connections = [
        { from_agent_id: 'a1', to_agent_id: 'a2' }
      ];

      expect(manager.connectionExists('a1', 'a3')).toBe(false);
      expect(manager.connectionExists('a3', 'a1')).toBe(false);
    });

    test('should return false for reverse connection', () => {
      manager.graph.connections = [
        { from_agent_id: 'a1', to_agent_id: 'a2' }
      ];

      expect(manager.connectionExists('a2', 'a1')).toBe(false);
    });
  });

  describe('Edge cases', () => {
    test('should return false when graph is null', () => {
      manager.graph = null;

      expect(manager.connectionExists('a1', 'a2')).toBe(false);
    });

    test('should return false when connections is null', () => {
      manager.graph = { agents: {}, connections: null };

      expect(manager.connectionExists('a1', 'a2')).toBe(false);
    });

    test('should return false when connections is undefined', () => {
      manager.graph = { agents: {} };

      expect(manager.connectionExists('a1', 'a2')).toBe(false);
    });

    test('should return false for empty connections array', () => {
      manager.graph.connections = [];

      expect(manager.connectionExists('a1', 'a2')).toBe(false);
    });

    test('should handle same agent IDs', () => {
      manager.graph.connections = [
        { from_agent_id: 'a1', to_agent_id: 'a1' }
      ];

      expect(manager.connectionExists('a1', 'a1')).toBe(true);
    });

    test('should handle null agent IDs', () => {
      manager.graph.connections = [
        { from_agent_id: 'a1', to_agent_id: 'a2' }
      ];

      expect(manager.connectionExists(null, 'a2')).toBe(false);
      expect(manager.connectionExists('a1', null)).toBe(false);
      expect(manager.connectionExists(null, null)).toBe(false);
    });

    test('should handle undefined agent IDs', () => {
      manager.graph.connections = [
        { from_agent_id: 'a1', to_agent_id: 'a2' }
      ];

      expect(manager.connectionExists(undefined, 'a2')).toBe(false);
      expect(manager.connectionExists('a1', undefined)).toBe(false);
    });

    test('should handle connections with additional properties', () => {
      manager.graph.connections = [
        {
          from_agent_id: 'a1',
          to_agent_id: 'a2',
          message_template: 'test',
          metadata: { custom: 'data' }
        }
      ];

      expect(manager.connectionExists('a1', 'a2')).toBe(true);
    });

    test('should handle empty string agent IDs', () => {
      manager.graph.connections = [
        { from_agent_id: '', to_agent_id: '' }
      ];

      expect(manager.connectionExists('', '')).toBe(true);
      expect(manager.connectionExists('a1', '')).toBe(false);
    });

    test('should be case-sensitive', () => {
      manager.graph.connections = [
        { from_agent_id: 'Agent1', to_agent_id: 'Agent2' }
      ];

      expect(manager.connectionExists('Agent1', 'Agent2')).toBe(true);
      expect(manager.connectionExists('agent1', 'agent2')).toBe(false);
      expect(manager.connectionExists('AGENT1', 'AGENT2')).toBe(false);
    });
  });

  describe('Performance with large datasets', () => {
    test('should handle many connections efficiently', () => {
      // Create 1000 connections
      manager.graph.connections = Array.from({ length: 1000 }, (_, i) => ({
        from_agent_id: `agent-${i}`,
        to_agent_id: `agent-${i + 1}`
      }));

      const start = performance.now();
      const result = manager.connectionExists('agent-500', 'agent-501');
      const end = performance.now();

      expect(result).toBe(true);
      expect(end - start).toBeLessThan(10); // Should be very fast
    });
  });
});

describe('WorkflowManager - escapeHtml()', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
  });

  describe('XSS Prevention', () => {
    test('should escape basic HTML tags', () => {
      const input = '<script>alert("XSS")</script>';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<script>');
      expect(result).not.toContain('</script>');
      expect(result).toContain('&lt;');
      expect(result).toContain('&gt;');
    });

    test('should escape malicious img tag', () => {
      const input = '<img src=x onerror="alert(1)">';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<img');
      expect(result).not.toContain('onerror');
      expect(result).toContain('&lt;');
      expect(result).toContain('&gt;');
    });

    test('should escape iframe injection', () => {
      const input = '<iframe src="javascript:alert(1)"></iframe>';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<iframe');
      expect(result).not.toContain('javascript:');
      expect(result).toContain('&lt;');
    });

    test('should escape SVG XSS vector', () => {
      const input = '<svg onload="alert(1)">';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<svg');
      expect(result).not.toContain('onload');
      expect(result).toContain('&lt;');
    });

    test('should escape anchor tag with javascript:', () => {
      const input = '<a href="javascript:alert(1)">Click</a>';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<a');
      expect(result).not.toContain('javascript:');
      expect(result).toContain('&lt;');
    });

    test('should escape data URI XSS', () => {
      const input = '<a href="data:text/html,<script>alert(1)</script>">Click</a>';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<a');
      expect(result).not.toContain('<script>');
      expect(result).toContain('&lt;');
    });

    test('should escape event handler attributes', () => {
      const handlers = [
        'onclick="alert(1)"',
        'onmouseover="alert(1)"',
        'onerror="alert(1)"',
        'onload="alert(1)"'
      ];

      handlers.forEach(handler => {
        const input = `<div ${handler}>Test</div>`;
        const result = manager.escapeHtml(input);

        expect(result).not.toContain(handler);
        expect(result).toContain('&lt;');
      });
    });
  });

  describe('Special characters', () => {
    test('should escape ampersand', () => {
      const result = manager.escapeHtml('Tom & Jerry');
      expect(result).toContain('&amp;');
    });

    test('should escape quotes', () => {
      const result = manager.escapeHtml('She said "Hello"');
      expect(result).toContain('&quot;');
    });

    test('should escape apostrophe (in some implementations)', () => {
      const result = manager.escapeHtml("It's working");
      // Note: textContent doesn't escape apostrophes by default
      // but the test documents the behavior
      expect(result).toContain("'");
    });

    test('should escape less than and greater than', () => {
      const result = manager.escapeHtml('5 < 10 > 3');
      expect(result).toContain('&lt;');
      expect(result).toContain('&gt;');
    });
  });

  describe('Normal text', () => {
    test('should not modify plain text', () => {
      const input = 'This is plain text without special characters';
      const result = manager.escapeHtml(input);

      expect(result).toBe(input);
    });

    test('should preserve whitespace', () => {
      const input = 'Text   with    multiple     spaces';
      const result = manager.escapeHtml(input);

      expect(result).toBe(input);
    });

    test('should preserve newlines', () => {
      const input = 'Line 1\nLine 2\nLine 3';
      const result = manager.escapeHtml(input);

      expect(result).toContain('\n');
    });

    test('should handle unicode characters', () => {
      const input = 'Unicode: 你好 🚀 ❤️';
      const result = manager.escapeHtml(input);

      expect(result).toBe(input);
    });
  });

  describe('Edge cases', () => {
    test('should handle empty string', () => {
      const result = manager.escapeHtml('');
      expect(result).toBe('');
    });

    test('should handle very long strings', () => {
      const input = 'A'.repeat(10000) + '<script>alert(1)</script>';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<script>');
      expect(result.length).toBeGreaterThan(10000);
    });

    test('should handle nested HTML tags', () => {
      const input = '<div><span><a href="#">Link</a></span></div>';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<div>');
      expect(result).not.toContain('<span>');
      expect(result).not.toContain('<a');
      expect(result).toContain('&lt;');
    });

    test('should handle malformed HTML', () => {
      const input = '<div<script>alert(1)</script>';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<script>');
      expect(result).toContain('&lt;');
    });

    test('should handle numeric entities', () => {
      const input = '&#60;script&#62;alert(1)&#60;/script&#62;';
      const result = manager.escapeHtml(input);

      // textContent treats these as plain text
      expect(result).toBe(input);
    });
  });

  describe('Real-world XSS attempts', () => {
    test('should escape polyglot XSS', () => {
      const input = 'javascript:/*--></title></style></textarea></script></xmp><svg/onload=\'+/"/+/onmouseover=1/+/[*/[]/+alert(1)//\'>';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<svg');
      expect(result).not.toContain('onload');
      expect(result).not.toContain('onmouseover');
    });

    test('should escape null byte injection', () => {
      const input = '<img src=x\x00 onerror=alert(1)>';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<img');
    });

    test('should escape HTML comment XSS', () => {
      const input = '<!--<script>alert(1)</script>-->';
      const result = manager.escapeHtml(input);

      expect(result).not.toContain('<!--');
      expect(result).not.toContain('<script>');
      expect(result).toContain('&lt;');
    });
  });
});
