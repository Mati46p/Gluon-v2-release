// Event Bus for real-time UI updates
// This is the nervous system of the Command Center - every significant
// change in the execution engine is broadcast through this bus to the frontend.

use crossbeam_channel::{bounded, Sender, Receiver};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use parking_lot::RwLock;

/// Events that can be broadcast to the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UIEvent {
    /// A node in the execution graph changed state
    NodeStateChanged {
        node_id: String,
        node_name: String,
        status: NodeStatus,
        timestamp: u64,
    },

    /// Data flowed across an edge in the graph
    DataFlow {
        from_node: String,
        to_node: String,
        data_type: String,
        size_bytes: usize,
        timestamp: u64,
    },

    /// Agent produced a "thought" (reasoning step)
    AgentThought {
        agent_id: String,
        thought_type: ThoughtType,
        content: String,
        timestamp: u64,
    },

    /// System-level log (HTTP request, file write, etc.)
    SystemLog {
        level: LogLevel,
        source: String,
        message: String,
        metadata: serde_json::Value,
        timestamp: u64,
    },

    /// Terminal output (stdout/stderr from Virtual Terminal)
    TerminalOutput {
        session_id: String,
        stream: OutputStream,
        data: Vec<u8>,
        timestamp: u64,
    },

    /// Execution paused (user or system initiated)
    ExecutionPaused {
        reason: String,
        timestamp: u64,
    },

    /// Execution resumed
    ExecutionResumed {
        timestamp: u64,
    },

    /// Critical error that requires user intervention
    CriticalError {
        error_type: String,
        message: String,
        stack_trace: Option<String>,
        timestamp: u64,
    },

    /// Budget update (LLM cost tracking)
    BudgetUpdate {
        used_cents: u64,
        limit_cents: u64,
        percentage: f32,
        timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Pending,
    Running,
    Success,
    Failed,
    Paused,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThoughtType {
    Planning,      // "I need to check the API docs..."
    Decision,      // "I'll use approach A because..."
    Observation,   // "The response shows..."
    Critique,      // "This might fail if..."
    Summary,       // "Completed X, next is Y"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// The EventBus - a broadcast channel for UI events
/// Multiple subscribers can listen to the same stream of events
/// Uses a multi-producer, multi-consumer pattern
pub struct EventBus {
    subscribers: Arc<RwLock<Vec<Sender<UIEvent>>>>,
}

impl EventBus {
    /// Create a new EventBus
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Publish an event to all subscribers
    /// Events are cloned for each subscriber
    pub fn publish(&self, event: UIEvent) {
        let subscribers = self.subscribers.read();
        for sender in subscribers.iter() {
            // Fire-and-forget - if subscriber buffer is full, skip
            let _ = sender.try_send(event.clone());
        }
    }

    /// Subscribe to events (creates a new channel)
    /// Returns a receiver that will get all future events
    pub fn subscribe(&self) -> Receiver<UIEvent> {
        let (sender, receiver) = bounded(100);
        self.subscribers.write().push(sender);
        receiver
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().len()
    }

    /// Clear all subscribers (useful for cleanup)
    pub fn clear_subscribers(&self) {
        self.subscribers.write().clear();
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions for creating common events
impl UIEvent {
    pub fn node_started(node_id: String, node_name: String) -> Self {
        UIEvent::NodeStateChanged {
            node_id,
            node_name,
            status: NodeStatus::Running,
            timestamp: current_timestamp(),
        }
    }

    pub fn node_completed(node_id: String, node_name: String) -> Self {
        UIEvent::NodeStateChanged {
            node_id,
            node_name,
            status: NodeStatus::Success,
            timestamp: current_timestamp(),
        }
    }

    pub fn node_failed(node_id: String, node_name: String) -> Self {
        UIEvent::NodeStateChanged {
            node_id,
            node_name,
            status: NodeStatus::Failed,
            timestamp: current_timestamp(),
        }
    }

    pub fn agent_planning(agent_id: String, thought: String) -> Self {
        UIEvent::AgentThought {
            agent_id,
            thought_type: ThoughtType::Planning,
            content: thought,
            timestamp: current_timestamp(),
        }
    }

    pub fn system_info(source: String, message: String) -> Self {
        UIEvent::SystemLog {
            level: LogLevel::Info,
            source,
            message,
            metadata: serde_json::Value::Null,
            timestamp: current_timestamp(),
        }
    }

    pub fn system_error(source: String, message: String) -> Self {
        UIEvent::SystemLog {
            level: LogLevel::Error,
            source,
            message,
            metadata: serde_json::Value::Null,
            timestamp: current_timestamp(),
        }
    }
}

/// Get current Unix timestamp in milliseconds
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus_publish_subscribe() {
        let bus = EventBus::new();
        let receiver = bus.subscribe();

        let event = UIEvent::node_started("node-1".to_string(), "TestNode".to_string());
        bus.publish(event.clone());

        let received = receiver.try_recv().unwrap();
        match received {
            UIEvent::NodeStateChanged { node_id, status, .. } => {
                assert_eq!(node_id, "node-1");
                assert!(matches!(status, NodeStatus::Running));
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let receiver1 = bus.subscribe();
        let receiver2 = bus.subscribe();

        let event = UIEvent::system_info("test".to_string(), "hello".to_string());
        bus.publish(event);

        assert!(receiver1.try_recv().is_ok());
        assert!(receiver2.try_recv().is_ok());
    }
}
