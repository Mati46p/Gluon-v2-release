// Gluon v3 Graph Execution Engine
// Phase 1: The Engine Core
//
// This module implements a Directed Cyclic Graph (DCG) execution engine with:
// - Trampoline executor (iterative, no recursion)
// - Blackboard shared memory pattern
// - Checkpointing and resume capability
// - Budget tracking for LLM costs
// - Support for retry loops and conditional branching

pub mod graph;
pub mod execution;
pub mod memory;
pub mod persistence;
pub mod nodes;
pub mod concurrency;

// Re-export commonly used types for convenience
pub use graph::{ExecutionGraph, Node, NodeType, Edge, EdgeType};
pub use execution::{GraphExecutor, ControlFlow, ExecutionContext};
pub use memory::{Blackboard, BudgetTracker};

// Error types
use std::fmt;

#[derive(Debug)]
pub enum EngineError {
    GraphError(String),
    ExecutionError(String),
    PersistenceError(String),
    BudgetExceeded { used: u64, limit: u64 },
    NodeNotFound(String),
    InvalidControlFlow(String),
    InfiniteLoopDetected,
    TimeoutExceeded,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::GraphError(msg) => write!(f, "Graph error: {}", msg),
            EngineError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
            EngineError::PersistenceError(msg) => write!(f, "Persistence error: {}", msg),
            EngineError::BudgetExceeded { used, limit } => {
                write!(f, "Budget exceeded: used {} cents, limit {} cents", used, limit)
            }
            EngineError::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            EngineError::InvalidControlFlow(msg) => write!(f, "Invalid control flow: {}", msg),
            EngineError::InfiniteLoopDetected => write!(f, "Infinite loop detected"),
            EngineError::TimeoutExceeded => write!(f, "Execution timeout exceeded"),
        }
    }
}

impl std::error::Error for EngineError {}

// Convenience type alias
pub type EngineResult<T> = Result<T, EngineError>;
