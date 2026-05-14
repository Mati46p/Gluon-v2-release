/**
 * Unit Tests for Browser Watcher - Web Recorder
 * Tests RingBuffer logic and serialization functions
 */

describe('RingBuffer', () => {
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

      if (this.size < this.limit) {
        return this.buffer.slice(0, this.size);
      } else {
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

  describe('Basic Operations', () => {
    test('should initialize with correct properties', () => {
      const buffer = new RingBuffer(10);
      expect(buffer.limit).toBe(10);
      expect(buffer.size).toBe(0);
      expect(buffer.cursor).toBe(0);
      expect(buffer.getAll()).toEqual([]);
    });

    test('should add items correctly', () => {
      const buffer = new RingBuffer(5);
      buffer.add('item1');
      buffer.add('item2');
      buffer.add('item3');

      expect(buffer.size).toBe(3);
      expect(buffer.getAll()).toEqual(['item1', 'item2', 'item3']);
    });

    test('should return correct count', () => {
      const buffer = new RingBuffer(5);
      expect(buffer.getCount()).toBe(0);

      buffer.add('a');
      expect(buffer.getCount()).toBe(1);

      buffer.add('b');
      buffer.add('c');
      expect(buffer.getCount()).toBe(3);
    });
  });

  describe('Wrap-around Behavior', () => {
    test('should overwrite oldest items when full', () => {
      const buffer = new RingBuffer(3);
      buffer.add('1');
      buffer.add('2');
      buffer.add('3');
      buffer.add('4'); // Overwrites '1'
      buffer.add('5'); // Overwrites '2'

      const result = buffer.getAll();
      expect(result).toEqual(['3', '4', '5']);
      expect(buffer.size).toBe(3); // Size stays at limit
    });

    test('should maintain correct order after multiple wraps', () => {
      const buffer = new RingBuffer(3);

      // First wrap
      buffer.add('a');
      buffer.add('b');
      buffer.add('c');
      buffer.add('d'); // cursor = 1, overwrites 'a'

      expect(buffer.getAll()).toEqual(['b', 'c', 'd']);

      // Second wrap
      buffer.add('e'); // cursor = 2, overwrites 'b'
      buffer.add('f'); // cursor = 0, overwrites 'c'

      expect(buffer.getAll()).toEqual(['d', 'e', 'f']);
    });

    test('should handle exact limit boundary', () => {
      const buffer = new RingBuffer(5);

      for (let i = 1; i <= 5; i++) {
        buffer.add(`item${i}`);
      }

      expect(buffer.getAll()).toEqual(['item1', 'item2', 'item3', 'item4', 'item5']);
      expect(buffer.size).toBe(5);

      // Add one more to trigger wrap
      buffer.add('item6');
      expect(buffer.getAll()).toEqual(['item2', 'item3', 'item4', 'item5', 'item6']);
      expect(buffer.size).toBe(5);
    });
  });

  describe('Clear Operation', () => {
    test('should reset buffer to initial state', () => {
      const buffer = new RingBuffer(5);
      buffer.add('a');
      buffer.add('b');
      buffer.add('c');

      buffer.clear();

      expect(buffer.size).toBe(0);
      expect(buffer.cursor).toBe(0);
      expect(buffer.getAll()).toEqual([]);
    });

    test('should work correctly after clear', () => {
      const buffer = new RingBuffer(3);
      buffer.add('old1');
      buffer.add('old2');
      buffer.clear();

      buffer.add('new1');
      buffer.add('new2');

      expect(buffer.getAll()).toEqual(['new1', 'new2']);
    });
  });

  describe('Edge Cases', () => {
    test('should handle buffer of size 1', () => {
      const buffer = new RingBuffer(1);
      buffer.add('first');
      expect(buffer.getAll()).toEqual(['first']);

      buffer.add('second');
      expect(buffer.getAll()).toEqual(['second']);
    });

    test('should handle large number of additions', () => {
      const buffer = new RingBuffer(10);

      for (let i = 1; i <= 100; i++) {
        buffer.add(i);
      }

      // Should contain last 10 items
      expect(buffer.getAll()).toEqual([91, 92, 93, 94, 95, 96, 97, 98, 99, 100]);
      expect(buffer.size).toBe(10);
    });

    test('should handle complex objects', () => {
      const buffer = new RingBuffer(3);
      const obj1 = { type: 'console', message: 'log1' };
      const obj2 = { type: 'fetch', url: 'https://api.com' };
      const obj3 = { type: 'xhr', status: 200 };

      buffer.add(obj1);
      buffer.add(obj2);
      buffer.add(obj3);

      const result = buffer.getAll();
      expect(result).toHaveLength(3);
      expect(result[0]).toEqual(obj1);
      expect(result[1]).toEqual(obj2);
      expect(result[2]).toEqual(obj3);
    });
  });
});

describe('Safe Serialization', () => {
  const SENSITIVE_KEYS = /password|passwd|pwd|secret|token|auth|api[_-]?key|private[_-]?key|access[_-]?token|refresh[_-]?token|session|cookie|credential/i;

  function redactSensitiveData(key, value) {
    if (typeof key === 'string' && SENSITIVE_KEYS.test(key)) {
      return '[REDACTED]';
    }
    return value;
  }

  function safeStringify(obj, maxDepth = 3) {
    if (obj === null) return 'null';
    if (obj === undefined) return 'undefined';
    if (typeof obj !== 'object') return String(obj);

    try {
      if (obj instanceof Error) {
        return `${obj.name}: ${obj.message}`;
      }

      const seen = new WeakSet();

      return JSON.stringify(obj, (key, value) => {
        const redacted = redactSensitiveData(key, value);
        if (redacted === '[REDACTED]') {
          return '[REDACTED]';
        }

        if (typeof value === 'object' && value !== null) {
          if (seen.has(value)) {
            return '[Circular]';
          }
          seen.add(value);
        }

        return value;
      }, 2);
    } catch (e) {
      return `[Object: ${Object.prototype.toString.call(obj)}]`;
    }
  }

  describe('Circular References', () => {
    test('should handle circular references without crashing', () => {
      const obj = { name: 'test' };
      obj.self = obj; // Create circular reference

      const result = safeStringify(obj);
      expect(result).toContain('[Circular]');
      expect(result).not.toThrow();
    });

    test('should handle nested circular references', () => {
      const parent = { name: 'parent' };
      const child = { name: 'child', parent: parent };
      parent.child = child;

      const result = safeStringify(parent);
      expect(result).toContain('[Circular]');
    });
  });

  describe('Privacy Filtering', () => {
    test('should redact password fields', () => {
      const obj = { username: 'john', password: 'secret123' };
      const result = safeStringify(obj);

      expect(result).toContain('john');
      expect(result).toContain('[REDACTED]');
      expect(result).not.toContain('secret123');
    });

    test('should redact various sensitive key patterns', () => {
      const obj = {
        user: 'alice',
        api_key: 'abc123',
        apiKey: 'def456',
        access_token: 'xyz789',
        accessToken: 'token123',
        secret: 'shhh',
        session_id: 'session123',
        cookie: 'cookie_value'
      };

      const result = safeStringify(obj);

      expect(result).toContain('alice');
      expect(result).not.toContain('abc123');
      expect(result).not.toContain('def456');
      expect(result).not.toContain('xyz789');
      expect(result).not.toContain('shhh');

      // Should have multiple REDACTED markers
      const redactedCount = (result.match(/\[REDACTED\]/g) || []).length;
      expect(redactedCount).toBeGreaterThan(5);
    });

    test('should not redact safe fields', () => {
      const obj = { email: 'test@test.com', data: [1, 2, 3] };
      const result = safeStringify(obj);

      expect(result).toContain('test@test.com');
      expect(result).toContain('[1,2,3]');
      expect(result).not.toContain('[REDACTED]');
    });
  });

  describe('Error Objects', () => {
    test('should serialize Error objects with name and message', () => {
      const error = new Error('Something went wrong');
      const result = safeStringify(error);

      expect(result).toContain('Error');
      expect(result).toContain('Something went wrong');
    });

    test('should handle TypeError', () => {
      const error = new TypeError('Invalid type');
      const result = safeStringify(error);

      expect(result).toContain('TypeError');
      expect(result).toContain('Invalid type');
    });
  });

  describe('Primitive Values', () => {
    test('should handle null and undefined', () => {
      expect(safeStringify(null)).toBe('null');
      expect(safeStringify(undefined)).toBe('undefined');
    });

    test('should handle strings', () => {
      expect(safeStringify('hello')).toBe('hello');
    });

    test('should handle numbers', () => {
      expect(safeStringify(42)).toBe('42');
      expect(safeStringify(3.14)).toBe('3.14');
    });

    test('should handle booleans', () => {
      expect(safeStringify(true)).toBe('true');
      expect(safeStringify(false)).toBe('false');
    });
  });
});
