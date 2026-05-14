/**
 * Browser Watcher - Web Recorder
 *
 * This script is injected into the MAIN world of any webpage to intercept:
 * - Console logs (log, warn, error)
 * - Fetch requests
 * - XMLHttpRequest
 *
 * Uses a RingBuffer to store events efficiently without memory leaks.
 */

(function() {
  'use strict';

  // ============================================================================
  // RingBuffer Implementation
  // ============================================================================

  class RingBuffer {
    constructor(limit) {
      this.buffer = new Array(limit);
      this.limit = limit;
      this.cursor = 0;
      this.size = 0;
    }

    add(item) {
      this.buffer[this.cursor] = item;
      this.cursor = (this.cursor + 1) % this.limit;
      if (this.size < this.limit) {
        this.size++;
      }
    }

    getAll() {
      if (this.size === 0) return [];

      // Reconstruct ordered array based on cursor position
      if (this.size < this.limit) {
        // Buffer not full yet, return items from 0 to size
        return this.buffer.slice(0, this.size);
      } else {
        // Buffer is full, need to reconstruct from cursor position
        const older = this.buffer.slice(this.cursor);
        const newer = this.buffer.slice(0, this.cursor);
        return older.concat(newer);
      }
    }

    clear() {
      this.buffer = new Array(this.limit);
      this.cursor = 0;
      this.size = 0;
    }

    getCount() {
      return this.size;
    }
  }

  // ============================================================================
  // Global State
  // ============================================================================

  const eventBuffer = new RingBuffer(1000);
  let isRecording = false;

  // Store original functions
  const originalConsoleLog = console.log;
  const originalConsoleWarn = console.warn;
  const originalConsoleError = console.error;
  const originalFetch = window.fetch;
  const originalXHROpen = XMLHttpRequest.prototype.open;
  const originalXHRSend = XMLHttpRequest.prototype.send;

  // ============================================================================
  // Utility Functions
  // ============================================================================

  function getTimestamp() {
    const now = new Date();
    return now.toISOString();
  }

  /**
   * Privacy-aware sensitive key patterns
   */
  const SENSITIVE_KEYS = /password|passwd|pwd|secret|token|auth|api[_-]?key|private[_-]?key|access[_-]?token|refresh[_-]?token|session|cookie|credential/i;

  /**
   * Redacts sensitive values in objects
   */
  function redactSensitiveData(key, value) {
    if (typeof key === 'string' && SENSITIVE_KEYS.test(key)) {
      return '[REDACTED]';
    }
    return value;
  }

  /**
   * Safe stringify with circular reference handling, DOM node detection, and privacy filtering
   */
  function safeStringify(obj, maxDepth = 3) {
    if (obj === null) return 'null';
    if (obj === undefined) return 'undefined';
    if (typeof obj !== 'object') return String(obj);

    try {
      // Handle Error objects specially
      if (obj instanceof Error) {
        return `${obj.name}: ${obj.message}\n${obj.stack || ''}`;
      }

      // Handle DOM nodes - show tag structure instead of full object
      if (obj instanceof Element) {
        const tag = obj.tagName.toLowerCase();
        const id = obj.id ? `#${obj.id}` : '';
        const classes = obj.className ? `.${obj.className.split(' ').join('.')}` : '';
        return `<${tag}${id}${classes}>`;
      }

      if (obj instanceof Node) {
        return `[${obj.constructor.name}]`;
      }

      // Handle circular references and depth limiting
      const seen = new WeakSet();
      let depth = 0;

      return JSON.stringify(obj, (key, value) => {
        // Privacy filter - redact sensitive keys
        const redacted = redactSensitiveData(key, value);
        if (redacted === '[REDACTED]') {
          return '[REDACTED]';
        }

        // Handle circular references
        if (typeof value === 'object' && value !== null) {
          if (seen.has(value)) {
            return '[Circular]';
          }
          seen.add(value);

          // Handle DOM nodes in nested objects
          if (value instanceof Element) {
            const tag = value.tagName.toLowerCase();
            const id = value.id ? `#${value.id}` : '';
            const classes = value.className ? `.${value.className.split(' ').join('.')}` : '';
            return `<${tag}${id}${classes}>`;
          }

          if (value instanceof Node) {
            return `[${value.constructor.name}]`;
          }
        }

        // Limit depth
        depth++;
        if (depth > maxDepth) {
          return '[Max Depth Reached]';
        }

        return value;
      }, 2);
    } catch (e) {
      // Fallback for any serialization errors
      return `[Object: ${Object.prototype.toString.call(obj)}]`;
    }
  }

  function captureConsoleArgs(args) {
    return Array.from(args).map(arg => {
      if (typeof arg === 'string') return arg;
      return safeStringify(arg);
    }).join(' ');
  }

  // ============================================================================
  // Console Instrumentation
  // ============================================================================

  function wrapConsole(level, originalFn) {
    return function(...args) {
      // Call original function first
      originalFn.apply(console, args);

      if (isRecording) {
        try {
          const message = captureConsoleArgs(args);
          eventBuffer.add({
            type: 'console',
            level: level,
            message: message,
            timestamp: getTimestamp(),
            url: window.location.href
          });
        } catch (e) {
          // Silently fail to avoid breaking the page
        }
      }
    };
  }

  // ============================================================================
  // Fetch Instrumentation
  // ============================================================================

  function wrapFetch() {
    return async function(resource, init = {}) {
      const startTime = Date.now();
      const url = typeof resource === 'string' ? resource : resource.url;
      const method = init.method || 'GET';

      let response;
      let error;

      try {
        response = await originalFetch.call(window, resource, init);

        if (isRecording) {
          const duration = Date.now() - startTime;
          eventBuffer.add({
            type: 'fetch',
            method: method,
            url: url,
            status: response.status,
            statusText: response.statusText,
            duration: duration,
            timestamp: getTimestamp()
          });
        }

        return response;
      } catch (e) {
        error = e;

        if (isRecording) {
          const duration = Date.now() - startTime;
          eventBuffer.add({
            type: 'fetch',
            method: method,
            url: url,
            error: safeStringify(e),
            duration: duration,
            timestamp: getTimestamp()
          });
        }

        throw e;
      }
    };
  }

  // ============================================================================
  // XHR Instrumentation
  // ============================================================================

  function wrapXHROpen() {
    return function(method, url, async = true, username, password) {
      this._gluon_xhr_method = method;
      this._gluon_xhr_url = url;
      this._gluon_xhr_startTime = Date.now();

      return originalXHROpen.call(this, method, url, async, username, password);
    };
  }

  function wrapXHRSend() {
    return function(body) {
      if (isRecording) {
        this.addEventListener('loadend', function() {
          try {
            const duration = Date.now() - (this._gluon_xhr_startTime || 0);

            eventBuffer.add({
              type: 'xhr',
              method: this._gluon_xhr_method || 'GET',
              url: this._gluon_xhr_url || '',
              status: this.status,
              statusText: this.statusText,
              duration: duration,
              timestamp: getTimestamp()
            });
          } catch (e) {
            // Silently fail
          }
        });

        this.addEventListener('error', function() {
          try {
            const duration = Date.now() - (this._gluon_xhr_startTime || 0);

            eventBuffer.add({
              type: 'xhr',
              method: this._gluon_xhr_method || 'GET',
              url: this._gluon_xhr_url || '',
              error: 'Network Error',
              duration: duration,
              timestamp: getTimestamp()
            });
          } catch (e) {
            // Silently fail
          }
        });
      }

      return originalXHRSend.call(this, body);
    };
  }

  // ============================================================================
  // Apply Instrumentation
  // ============================================================================

  function applyInstrumentation() {
    console.log = wrapConsole('log', originalConsoleLog);
    console.warn = wrapConsole('warn', originalConsoleWarn);
    console.error = wrapConsole('error', originalConsoleError);
    window.fetch = wrapFetch();
    XMLHttpRequest.prototype.open = wrapXHROpen();
    XMLHttpRequest.prototype.send = wrapXHRSend();
  }

  function removeInstrumentation() {
    console.log = originalConsoleLog;
    console.warn = originalConsoleWarn;
    console.error = originalConsoleError;
    window.fetch = originalFetch;
    XMLHttpRequest.prototype.open = originalXHROpen;
    XMLHttpRequest.prototype.send = originalXHRSend;
  }

  // ============================================================================
  // Communication with Extension
  // ============================================================================

  // Listen for commands from the extension
  window.addEventListener('GLUON_WATCHER_START', () => {
    isRecording = true;
    eventBuffer.clear();
    applyInstrumentation();
    console.log('[Gluon Watcher] Recording started');
  });

  window.addEventListener('GLUON_WATCHER_STOP', () => {
    isRecording = false;
    console.log('[Gluon Watcher] Recording stopped');
  });

  window.addEventListener('GLUON_WATCHER_FLUSH', () => {
    const events = eventBuffer.getAll();
    const count = eventBuffer.getCount();

    // Send events back to extension via postMessage
    window.postMessage({
      type: 'GLUON_WATCHER_DATA',
      events: events,
      count: count,
      timestamp: getTimestamp()
    }, '*');

    console.log(`[Gluon Watcher] Flushed ${count} events`);
  });

  // ============================================================================
  // Initialization
  // ============================================================================

  console.log('[Gluon Watcher] Web recorder initialized');

  // Signal to the extension that we're ready
  window.postMessage({ type: 'GLUON_WATCHER_READY' }, '*');

})();
