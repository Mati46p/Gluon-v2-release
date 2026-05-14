// Blackboard - Shared memory accessible by all nodes

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Blackboard pattern: Shared memory for inter-node communication
///
/// Instead of passing data directly between nodes (which requires
/// copying large strings), nodes read/write to a shared blackboard.
///
/// Thread-safety: Wrapped in Arc<RwLock<Blackboard>> for concurrent access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blackboard {
    data: HashMap<String, Value>,
}

impl Blackboard {
    /// Create a new empty blackboard
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Get a value from the blackboard
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    /// Insert a value into the blackboard
    pub fn insert(&mut self, key: String, value: Value) {
        self.data.insert(key, value);
    }

    /// Remove a value from the blackboard
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }

    /// Check if a key exists
    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Get all keys
    pub fn keys(&self) -> Vec<String> {
        self.data.keys().cloned().collect()
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if blackboard is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Iterate over key-value pairs
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.data.iter()
    }

    // === Typed Getters (convenience methods) ===

    /// Get a string value
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|v| v.as_str().map(String::from))
    }

    /// Get a boolean value
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    /// Get an i64 value
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_i64())
    }

    /// Get a u64 value
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.get(key).and_then(|v| v.as_u64())
    }

    /// Get an f64 value
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.as_f64())
    }

    /// Get an array value
    pub fn get_array(&self, key: &str) -> Option<&Vec<Value>> {
        self.get(key).and_then(|v| v.as_array())
    }

    /// Get an object value
    pub fn get_object(&self, key: &str) -> Option<&serde_json::Map<String, Value>> {
        self.get(key).and_then(|v| v.as_object())
    }

    // === Typed Setters (convenience methods) ===

    /// Insert a string value
    pub fn insert_string(&mut self, key: String, value: String) {
        self.insert(key, Value::String(value));
    }

    /// Insert a boolean value
    pub fn insert_bool(&mut self, key: String, value: bool) {
        self.insert(key, Value::Bool(value));
    }

    /// Insert an i64 value
    pub fn insert_i64(&mut self, key: String, value: i64) {
        self.insert(key, Value::Number(value.into()));
    }

    /// Insert a u64 value
    pub fn insert_u64(&mut self, key: String, value: u64) {
        self.insert(key, Value::Number(value.into()));
    }

    /// Insert an f64 value
    pub fn insert_f64(&mut self, key: String, value: f64) {
        if let Some(num) = serde_json::Number::from_f64(value) {
            self.insert(key, Value::Number(num));
        }
    }

    // === Serialization for checkpointing ===

    /// Serialize to JSON for checkpoint storage
    pub fn to_json(&self) -> Value {
        serde_json::to_value(&self.data).unwrap()
    }

    /// Restore from JSON checkpoint
    pub fn from_json(json: Value) -> Self {
        let data = serde_json::from_value(json).unwrap_or_default();
        Self { data }
    }
}

impl Default for Blackboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut bb = Blackboard::new();
        assert!(bb.is_empty());

        bb.insert("key1".to_string(), Value::String("value1".to_string()));
        assert_eq!(bb.len(), 1);
        assert!(bb.contains_key("key1"));

        let val = bb.get("key1").unwrap();
        assert_eq!(val.as_str(), Some("value1"));

        bb.remove("key1");
        assert!(bb.is_empty());
    }

    #[test]
    fn test_typed_getters_setters() {
        let mut bb = Blackboard::new();

        bb.insert_string("name".to_string(), "Alice".to_string());
        bb.insert_bool("active".to_string(), true);
        bb.insert_i64("count".to_string(), 42);
        bb.insert_f64("score".to_string(), 98.5);

        assert_eq!(bb.get_string("name"), Some("Alice".to_string()));
        assert_eq!(bb.get_bool("active"), Some(true));
        assert_eq!(bb.get_i64("count"), Some(42));
        assert_eq!(bb.get_f64("score"), Some(98.5));
    }

    #[test]
    fn test_serialization() {
        let mut bb = Blackboard::new();
        bb.insert_string("test".to_string(), "data".to_string());
        bb.insert_bool("flag".to_string(), true);

        let json = bb.to_json();
        let restored = Blackboard::from_json(json);

        assert_eq!(restored.get_string("test"), Some("data".to_string()));
        assert_eq!(restored.get_bool("flag"), Some(true));
    }

    #[test]
    fn test_concurrent_access_pattern() {
        use std::sync::{Arc, RwLock};
        use std::thread;

        let bb = Arc::new(RwLock::new(Blackboard::new()));

        // Writer thread
        let bb_clone = bb.clone();
        let writer = thread::spawn(move || {
            let mut bb = bb_clone.write().unwrap();
            bb.insert_i64("counter".to_string(), 100);
        });

        writer.join().unwrap();

        // Reader thread
        let bb_clone = bb.clone();
        let reader = thread::spawn(move || {
            let bb = bb_clone.read().unwrap();
            bb.get_i64("counter")
        });

        let result = reader.join().unwrap();
        assert_eq!(result, Some(100));
    }
}
