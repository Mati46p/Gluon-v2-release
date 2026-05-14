// Gluon v3 Command Center - Phase 5
// The UI/UX layer that provides a "Mission Control" cockpit for monitoring
// and controlling the execution engine, agents, and sensors.
//
// Architecture Overview:
// ┌─────────────────────────────────────────────────┐
// │           COMMAND CENTER (Tauri App)            │
// │  ┌──────────────┐  ┌──────────────────────┐   │
// │  │ Graph Viz    │  │ Virtual Terminal     │   │
// │  │ (Live Topo)  │  │ (Shared PTY)         │   │
// │  └──────────────┘  └──────────────────────┘   │
// │  ┌──────────────┐  ┌──────────────────────┐   │
// │  │ State Panel  │  │ Bifocal Logs         │   │
// │  │ (Inspector)  │  │ (Thoughts vs System) │   │
// │  └──────────────┘  └──────────────────────┘   │
// └─────────────────────────────────────────────────┘
//                         ↕ EventBus (WebSocket/IPC)
// ┌─────────────────────────────────────────────────┐
// │         ENGINE CORE (Graph Executor)            │
// └─────────────────────────────────────────────────┘

pub mod commands;      // Tauri commands for UI control (pause, resume, inject)
pub mod terminal;      // Virtual Terminal with PTY integration
pub mod state_sync;    // Real-time state broadcasting to frontend
pub mod events;        // Event definitions and EventBus

// Re-exports for convenience
pub use commands::*;
pub use terminal::{VirtualTerminal, TerminalSession};
pub use state_sync::{UIStateBroadcaster, ExecutionState};
pub use events::{UIEvent, EventBus};

use std::fmt;

/// Errors that can occur in the UI subsystem
#[derive(Debug)]
pub enum UIError {
    TerminalError(String),
    BroadcastError(String),
    CommandError(String),
    InvalidState(String),
}

impl fmt::Display for UIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UIError::TerminalError(msg) => write!(f, "Terminal error: {}", msg),
            UIError::BroadcastError(msg) => write!(f, "Broadcast error: {}", msg),
            UIError::CommandError(msg) => write!(f, "Command error: {}", msg),
            UIError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
        }
    }
}

impl std::error::Error for UIError {}

pub type UIResult<T> = Result<T, UIError>;
