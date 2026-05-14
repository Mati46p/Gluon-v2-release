/**
 * Unit Tests for WorkflowManager XSS Vulnerability Prevention
 *
 * CRITICAL SECURITY TESTS - Covers:
 * - XSS through agent names
 * - XSS through templates
 * - XSS through wrapper templates
 * - HTML escaping in rendering
 * - Script injection prevention
 * - Event handler injection
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
    this.presetManager = mockPresetManager;
  }

  escapeHtml(text) {
    if (!text) return '';
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  truncate(str, maxLen) {
    if (!str) return '';
    if (str.length <= maxLen) return str;
    return str.substring(0, maxLen - 3) + '...';
  }

  getAgentName(agentId) {
    return this.graph?.agents?.[agentId]?.name || 'Unknown';
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

  // Core rendering function - MUST escape user input
  renderAgentCard(agent) {
    const agentType = this.normalizeAgentType(agent);

    if (agentType === 'Report') {
      return this.renderAggregatorCard(agent);
    }

    const statusClass = agent.status === 'Connected' ? 'status-connected' :
                       agent.status === 'Waiting' ? 'status-waiting' : 'status-disconnected';
    const statusIcon = agent.status === 'Connected' ? '🟢' :
                      agent.status === 'Waiting' ? '🟡' : '🔴';

    const outgoing = this.graph.connections.filter(c => c.from_agent_id === agent.id);
    const incoming = this.graph.connections.filter(c => c.to_agent_id === agent.id);

    const outgoingHtml = outgoing.length > 0
        ? outgoing.map((c, idx) => `
            <span class="connection-item">
                ${this.escapeHtml(this.getAgentName(c.to_agent_id))}
                <button id="delete-conn-${agent.id}-${idx}" class="conn-delete-btn">×</button>
            </span>
        `).join('')
        : '<span style="color: var(--text-muted);">none</span>';

    const incomingHtml = incoming.length > 0
        ? incoming.map(c => this.escapeHtml(this.getAgentName(c.from_agent_id))).join(', ')
        : '<span style="color: var(--text-muted);">none</span>';

    return `
        <div class="agent-card ${statusClass}" data-agent-id="${agent.id}" data-type="${agentType}">
            <div class="agent-header">
                <div class="agent-name">🤖 ${this.escapeHtml(agent.name)}</div>
                <div class="agent-status">${statusIcon} ${agent.status}</div>
            </div>
            <div class="agent-pairing">
                <strong>Pairing Code:</strong> <code>${agent.pairing_code}</code>
            </div>
            ${agent.output_wrapper ? `
                <div class="agent-wrapper">
                    <strong>Wrapper:</strong> <code title="${this.escapeHtml(agent.output_wrapper)}">${this.escapeHtml(this.truncate(agent.output_wrapper, 30))}</code>
                </div>
            ` : ''}
            <div class="agent-connections">
                <div class="connections-row">
                    <span style="color: var(--text-secondary);">→ Sends to:</span>
                    ${outgoingHtml}
                </div>
                <div class="connections-row">
                    <span style="color: var(--text-secondary);">← Receives from:</span>
                    ${incomingHtml}
                </div>
            </div>
        </div>
    `;
  }

  renderAggregatorCard(agent) {
    const outgoing = this.graph.connections.filter(c => c.from_agent_id === agent.id);
    const incoming = this.graph.connections.filter(c => c.to_agent_id === agent.id);

    const outgoingHtml = outgoing.length > 0
        ? outgoing.map((c, idx) => `
            <span class="connection-item">
                ${this.escapeHtml(this.getAgentName(c.to_agent_id))}
                <button id="delete-conn-${agent.id}-${idx}">×</button>
            </span>
        `).join('')
        : '<span style="color: var(--text-muted);">brak</span>';

    const incomingHtml = incoming.length > 0
        ? incoming.map(c => this.escapeHtml(this.getAgentName(c.from_agent_id))).join(', ')
        : '<span style="color: var(--text-muted);">brak</span>';

    const sourceCount = incoming.length;

    return `
        <div class="agent-card aggregator-card" data-agent-id="${agent.id}" data-type="Report">
            <div class="agent-header">
                <div class="agent-name">🗂️ ${this.escapeHtml(agent.name)}</div>
                <div class="agent-status">📊 Kolektor</div>
            </div>
            <div class="agent-connections">
                <div class="connections-row">
                    <span>📥 Zbiera od:</span>
                    ${incomingHtml}
                </div>
                <div class="connections-row">
                    <span>📤 Wysyła do:</span>
                    ${outgoingHtml}
                </div>
            </div>
        </div>
    `;
  }

  // Validate input before saving
  validateAgentInput(name, wrapper, template) {
    const errors = [];

    // Check for script tags
    if (/<script[\s\S]*?<\/script>/i.test(name)) {
      errors.push('Agent name contains script tag');
    }

    if (wrapper && /<script[\s\S]*?<\/script>/i.test(wrapper)) {
      errors.push('Wrapper template contains script tag');
    }

    if (template && /<script[\s\S]*?<\/script>/i.test(template)) {
      errors.push('Message template contains script tag');
    }

    // Check for event handlers
    const dangerousPatterns = [
      /on\w+\s*=/i,  // onclick=, onerror=, etc.
      /javascript:/i,
      /<iframe/i,
      /<embed/i,
      /<object/i
    ];

    [name, wrapper, template].forEach((input, idx) => {
      if (input) {
        dangerousPatterns.forEach(pattern => {
          if (pattern.test(input)) {
            const fieldName = ['name', 'wrapper', 'template'][idx];
            errors.push(`${fieldName} contains potentially dangerous content`);
          }
        });
      }
    });

    return errors;
  }

  sanitizeInput(input) {
    if (!input) return '';

    // Remove script tags
    let cleaned = input.replace(/<script[\s\S]*?<\/script>/gi, '');

    // Remove event handlers
    cleaned = cleaned.replace(/on\w+\s*=\s*["'][^"']*["']/gi, '');
    cleaned = cleaned.replace(/on\w+\s*=\s*[^\s>]*/gi, '');

    // Remove javascript: protocol
    cleaned = cleaned.replace(/javascript:/gi, '');

    // Remove dangerous tags
    cleaned = cleaned.replace(/<iframe[\s\S]*?<\/iframe>/gi, '');
    cleaned = cleaned.replace(/<embed[\s\S]*?>/gi, '');
    cleaned = cleaned.replace(/<object[\s\S]*?<\/object>/gi, '');

    return cleaned;
  }
}

describe('WorkflowManager - XSS Vulnerability Prevention', () => {
  let manager;

  beforeEach(() => {
    manager = new TestableWorkflowManager();
    document.body.innerHTML = '<div id="test-container"></div>';
  });

  describe('🔴 CRITICAL: escapeHtml() Function', () => {
    test('should escape basic HTML characters', () => {
      const input = '<div>Hello & "World"</div>';
      const escaped = manager.escapeHtml(input);

      expect(escaped).not.toContain('<div>');
      expect(escaped).not.toContain('</div>');
      expect(escaped).toContain('&lt;div&gt;');
      expect(escaped).toContain('&quot;');
      expect(escaped).toContain('&amp;');
    });

    test('should escape script tags', () => {
      const malicious = '<script>alert("XSS")</script>';
      const escaped = manager.escapeHtml(malicious);

      expect(escaped).not.toContain('<script>');
      expect(escaped).toContain('&lt;script&gt;');
      expect(escaped).not.toContain('</script>');
    });

    test('should escape event handlers', () => {
      const malicious = '<img src="x" onerror="alert(\'XSS\')">';
      const escaped = manager.escapeHtml(malicious);

      expect(escaped).not.toContain('onerror=');
      expect(escaped).toContain('&lt;img');
      expect(escaped).not.toContain('<img');
    });

    test('should handle empty and null values', () => {
      expect(manager.escapeHtml('')).toBe('');
      expect(manager.escapeHtml(null)).toBe('');
      expect(manager.escapeHtml(undefined)).toBe('');
    });

    test('should handle special characters', () => {
      const input = `<>"'&`;
      const escaped = manager.escapeHtml(input);

      expect(escaped).toContain('&lt;');
      expect(escaped).toContain('&gt;');
      expect(escaped).toContain('&quot;');
      expect(escaped).toContain('&#39;'); // or &apos;
      expect(escaped).toContain('&amp;');
    });

    test('should not double-escape', () => {
      const input = '&lt;script&gt;';
      const escaped = manager.escapeHtml(input);

      // Should escape the & character
      expect(escaped).toContain('&amp;lt;');
    });
  });

  describe('🔴 CRITICAL: Agent Name XSS Prevention', () => {
    test('should escape malicious agent name in rendering', () => {
      manager.graph = {
        agents: {
          'agent-1': {
            id: 'agent-1',
            name: '<script>alert("XSS")</script>',
            status: 'Connected',
            pairing_code: 'ABC123',
            agent_type: 'Normal'
          }
        },
        connections: []
      };

      const html = manager.renderAgentCard(manager.graph.agents['agent-1']);

      // Should not contain executable script
      expect(html).not.toContain('<script>alert("XSS")</script>');
      // Should contain escaped version
      expect(html).toContain('&lt;script&gt;');
    });

    test('should escape agent name with event handlers', () => {
      manager.graph = {
        agents: {
          'agent-1': {
            id: 'agent-1',
            name: '<img src=x onerror="alert(1)">',
            status: 'Connected',
            pairing_code: 'ABC123',
            agent_type: 'Normal'
          }
        },
        connections: []
      };

      const html = manager.renderAgentCard(manager.graph.agents['agent-1']);

      expect(html).not.toContain('onerror=');
      expect(html).toContain('&lt;img');
    });

    test('should escape agent name with javascript protocol', () => {
      manager.graph = {
        agents: {
          'agent-1': {
            id: 'agent-1',
            name: '<a href="javascript:alert(1)">Click</a>',
            status: 'Connected',
            pairing_code: 'ABC123',
            agent_type: 'Normal'
          }
        },
        connections: []
      };

      const html = manager.renderAgentCard(manager.graph.agents['agent-1']);

      expect(html).not.toContain('javascript:alert');
      expect(html).toContain('&lt;a href=');
    });

    test('should escape complex XSS payload', () => {
      manager.graph = {
        agents: {
          'agent-1': {
            id: 'agent-1',
            name: '"><script>alert(String.fromCharCode(88,83,83))</script>',
            status: 'Connected',
            pairing_code: 'ABC123',
            agent_type: 'Normal'
          }
        },
        connections: []
      };

      const html = manager.renderAgentCard(manager.graph.agents['agent-1']);

      expect(html).not.toContain('"><script>');
      expect(html).toContain('&quot;&gt;&lt;script&gt;');
    });
  });

  describe('🔴 CRITICAL: Wrapper Template XSS Prevention', () => {
    test('should escape malicious wrapper template', () => {
      manager.graph = {
        agents: {
          'agent-1': {
            id: 'agent-1',
            name: 'Test Agent',
            status: 'Connected',
            pairing_code: 'ABC123',
            output_wrapper: '<script>alert("XSS")</script>{content}',
            agent_type: 'Normal'
          }
        },
        connections: []
      };

      const html = manager.renderAgentCard(manager.graph.agents['agent-1']);

      expect(html).not.toContain('<script>alert("XSS")</script>');
      expect(html).toContain('&lt;script&gt;');
    });

    test('should escape wrapper with event handlers', () => {
      manager.graph = {
        agents: {
          'agent-1': {
            id: 'agent-1',
            name: 'Test Agent',
            status: 'Connected',
            pairing_code: 'ABC123',
            output_wrapper: '<div onclick="alert(1)">{content}</div>',
            agent_type: 'Normal'
          }
        },
        connections: []
      };

      const html = manager.renderAgentCard(manager.graph.agents['agent-1']);

      expect(html).not.toContain('onclick="alert(1)"');
    });

    test('should handle long wrapper templates with truncation', () => {
      const longMalicious = '<script>alert("XSS")</script>'.repeat(10);
      manager.graph = {
        agents: {
          'agent-1': {
            id: 'agent-1',
            name: 'Test Agent',
            status: 'Connected',
            pairing_code: 'ABC123',
            output_wrapper: longMalicious,
            agent_type: 'Normal'
          }
        },
        connections: []
      };

      const html = manager.renderAgentCard(manager.graph.agents['agent-1']);

      // Should be truncated and escaped
      expect(html).not.toContain('<script>');
      expect(html.indexOf('&lt;script&gt;')).toBeGreaterThan(-1);
    });
  });

  describe('🔴 CRITICAL: Connection Name XSS Prevention', () => {
    test('should escape malicious agent names in connections', () => {
      manager.graph = {
        agents: {
          'agent-1': {
            id: 'agent-1',
            name: 'Source',
            status: 'Connected',
            pairing_code: 'ABC123',
            agent_type: 'Normal'
          },
          'agent-2': {
            id: 'agent-2',
            name: '<script>alert("XSS")</script>',
            status: 'Connected',
            pairing_code: 'DEF456',
            agent_type: 'Normal'
          }
        },
        connections: [
          { from_agent_id: 'agent-1', to_agent_id: 'agent-2' }
        ]
      };

      const html = manager.renderAgentCard(manager.graph.agents['agent-1']);

      // Connection target name should be escaped
      expect(html).not.toContain('<script>alert("XSS")</script>');
      expect(html).toContain('&lt;script&gt;');
    });

    test('should escape multiple malicious connection names', () => {
      manager.graph = {
        agents: {
          'agent-1': {
            id: 'agent-1',
            name: 'Source',
            status: 'Connected',
            pairing_code: 'ABC123',
            agent_type: 'Normal'
          },
          'agent-2': {
            id: 'agent-2',
            name: '<img src=x onerror=alert(1)>',
            status: 'Connected',
            pairing_code: 'DEF456',
            agent_type: 'Normal'
          },
          'agent-3': {
            id: 'agent-3',
            name: '"><script>alert(2)</script>',
            status: 'Connected',
            pairing_code: 'GHI789',
            agent_type: 'Normal'
          }
        },
        connections: [
          { from_agent_id: 'agent-1', to_agent_id: 'agent-2' },
          { from_agent_id: 'agent-1', to_agent_id: 'agent-3' }
        ]
      };

      const html = manager.renderAgentCard(manager.graph.agents['agent-1']);

      expect(html).not.toContain('onerror=alert');
      expect(html).not.toContain('"><script>');
    });
  });

  describe('🔴 CRITICAL: Input Validation', () => {
    test('should detect script tags in agent name', () => {
      const errors = manager.validateAgentInput(
        '<script>alert("XSS")</script>',
        null,
        null
      );

      expect(errors.length).toBeGreaterThan(0);
      expect(errors.some(e => e.includes('script tag'))).toBe(true);
    });

    test('should detect event handlers in inputs', () => {
      const errors = manager.validateAgentInput(
        'Agent <img src=x onerror=alert(1)>',
        null,
        null
      );

      expect(errors.length).toBeGreaterThan(0);
      expect(errors.some(e => e.includes('dangerous'))).toBe(true);
    });

    test('should detect javascript protocol', () => {
      const errors = manager.validateAgentInput(
        'Agent',
        null,
        'javascript:alert(1)'
      );

      expect(errors.length).toBeGreaterThan(0);
      expect(errors.some(e => e.includes('dangerous'))).toBe(true);
    });

    test('should detect iframe injection', () => {
      const errors = manager.validateAgentInput(
        'Agent',
        '<iframe src="http://evil.com"></iframe>{content}',
        null
      );

      expect(errors.length).toBeGreaterThan(0);
      expect(errors.some(e => e.includes('dangerous'))).toBe(true);
    });

    test('should allow safe input', () => {
      const errors = manager.validateAgentInput(
        'My Safe Agent',
        'Wrapper: {content}',
        'Template: {content}'
      );

      expect(errors).toEqual([]);
    });

    test('should detect multiple vulnerabilities', () => {
      const errors = manager.validateAgentInput(
        '<script>alert(1)</script>',
        '<img onerror=alert(2)>',
        'javascript:alert(3)'
      );

      expect(errors.length).toBeGreaterThan(2);
    });
  });

  describe('🔴 CRITICAL: Input Sanitization', () => {
    test('should remove script tags', () => {
      const malicious = 'Hello <script>alert("XSS")</script> World';
      const sanitized = manager.sanitizeInput(malicious);

      expect(sanitized).not.toContain('<script>');
      expect(sanitized).not.toContain('</script>');
      expect(sanitized).toContain('Hello');
      expect(sanitized).toContain('World');
    });

    test('should remove event handlers', () => {
      const malicious = '<div onclick="alert(1)" onerror="alert(2)">Test</div>';
      const sanitized = manager.sanitizeInput(malicious);

      expect(sanitized).not.toContain('onclick=');
      expect(sanitized).not.toContain('onerror=');
    });

    test('should remove javascript protocol', () => {
      const malicious = '<a href="javascript:alert(1)">Click</a>';
      const sanitized = manager.sanitizeInput(malicious);

      expect(sanitized).not.toContain('javascript:');
    });

    test('should remove iframe tags', () => {
      const malicious = '<iframe src="http://evil.com"></iframe>';
      const sanitized = manager.sanitizeInput(malicious);

      expect(sanitized).not.toContain('<iframe');
      expect(sanitized).not.toContain('</iframe>');
    });

    test('should remove embed and object tags', () => {
      const malicious = '<embed src="evil.swf"><object data="evil.swf"></object>';
      const sanitized = manager.sanitizeInput(malicious);

      expect(sanitized).not.toContain('<embed');
      expect(sanitized).not.toContain('<object');
    });

    test('should handle empty input', () => {
      expect(manager.sanitizeInput('')).toBe('');
      expect(manager.sanitizeInput(null)).toBe('');
      expect(manager.sanitizeInput(undefined)).toBe('');
    });

    test('should preserve safe content', () => {
      const safe = 'This is a {content} template with normal text';
      const sanitized = manager.sanitizeInput(safe);

      expect(sanitized).toBe(safe);
    });
  });

  describe('🔴 CRITICAL: Aggregator Card XSS Prevention', () => {
    test('should escape malicious aggregator name', () => {
      manager.graph = {
        agents: {
          'agg-1': {
            id: 'agg-1',
            name: '<script>alert("XSS")</script>',
            agent_type: 'Report'
          }
        },
        connections: []
      };

      const html = manager.renderAggregatorCard(manager.graph.agents['agg-1']);

      expect(html).not.toContain('<script>alert("XSS")</script>');
      expect(html).toContain('&lt;script&gt;');
    });

    test('should escape connection names in aggregator', () => {
      manager.graph = {
        agents: {
          'agg-1': {
            id: 'agg-1',
            name: 'Aggregator',
            agent_type: 'Report'
          },
          'agent-1': {
            id: 'agent-1',
            name: '<img src=x onerror=alert(1)>',
            agent_type: 'Normal'
          }
        },
        connections: [
          { from_agent_id: 'agent-1', to_agent_id: 'agg-1' }
        ]
      };

      const html = manager.renderAggregatorCard(manager.graph.agents['agg-1']);

      expect(html).not.toContain('onerror=');
      expect(html).toContain('&lt;img');
    });
  });

  describe('🔴 CRITICAL: Real-World XSS Payloads', () => {
    const xssPayloads = [
      '<script>alert(document.cookie)</script>',
      '"><script>alert(1)</script>',
      '<img src=x onerror=alert(1)>',
      '<svg/onload=alert(1)>',
      '<iframe src="javascript:alert(1)">',
      '<body onload=alert(1)>',
      '<input onfocus=alert(1) autofocus>',
      '<select onfocus=alert(1) autofocus>',
      '<textarea onfocus=alert(1) autofocus>',
      '<marquee onstart=alert(1)>',
      '<div style="background:url(javascript:alert(1))">',
      '\'><script>alert(String.fromCharCode(88,83,83))</script>'
    ];

    xssPayloads.forEach((payload, index) => {
      test(`should prevent XSS payload ${index + 1}: ${payload.substring(0, 30)}...`, () => {
        manager.graph = {
          agents: {
            'agent-1': {
              id: 'agent-1',
              name: payload,
              status: 'Connected',
              pairing_code: 'ABC123',
              agent_type: 'Normal'
            }
          },
          connections: []
        };

        const html = manager.renderAgentCard(manager.graph.agents['agent-1']);

        // Should not contain any of the dangerous parts unescaped
        expect(html).not.toContain('<script>');
        expect(html).not.toContain('javascript:');
        expect(html).not.toMatch(/on\w+=/);

        // Render to DOM and check no script execution
        const container = document.getElementById('test-container');
        container.innerHTML = html;

        // If XSS was successful, scripts would be in DOM
        const scripts = container.querySelectorAll('script');
        expect(scripts.length).toBe(0);
      });
    });
  });
});
