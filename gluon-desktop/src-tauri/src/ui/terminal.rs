// Virtual Terminal - Shared Console between User and Agents
// Uses portable-pty to create a real PTY session that both the user
// and AI agents can write to. Think "multiplayer mode" for the terminal.

use portable_pty::{
    native_pty_system, CommandBuilder, PtySize, PtySystem,
};
use std::io::{Read, Write};
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::{mpsc, oneshot};
use crate::ui::events::{EventBus, UIEvent, OutputStream};
use crate::ui::UIResult;

enum TerminalCommand {
    Write(Vec<u8>),
    Resize { rows: u16, cols: u16 },
    Kill,
    CheckAlive(oneshot::Sender<bool>),
}

/// A Virtual Terminal session
pub struct TerminalSession {
    session_id: String,
    event_bus: Arc<EventBus>,
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    command_tx: mpsc::UnboundedSender<TerminalCommand>,
}

impl TerminalSession {
    /// Create a new terminal session
    pub fn new(
        session_id: String,
        working_dir: Option<String>,
        event_bus: Arc<EventBus>,
    ) -> UIResult<Self> {
        let pty_system = native_pty_system();

        // Create PTY with reasonable size (80x24 is standard)
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| crate::ui::UIError::TerminalError(format!("Failed to open PTY: {}", e)))?;

        let mut master_pty = pair.master;
        let slave_pty = pair.slave;

        // Determine shell based on platform
        let shell = if cfg!(windows) {
            "powershell.exe".to_string()
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
        };

        let mut cmd = CommandBuilder::new(&shell);

        if let Some(dir) = working_dir {
            cmd.cwd(dir);
        }

        let mut child = slave_pty
            .spawn_command(cmd)
            .map_err(|e| crate::ui::UIError::TerminalError(format!("Failed to spawn shell: {}", e)))?;

        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<TerminalCommand>();

        // Spawn output reader thread
        let mut reader = master_pty
            .try_clone_reader()
            .map_err(|e| crate::ui::UIError::TerminalError(format!("Failed to clone reader: {}", e)))?;

        let session_id_clone = session_id.clone();
        let event_bus_clone = Arc::clone(&event_bus);

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 8192];

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let data = buffer[..n].to_vec();

                        // Broadcast terminal output to UI
                        event_bus_clone.publish(UIEvent::TerminalOutput {
                            session_id: session_id_clone.clone(),
                            stream: OutputStream::Stdout,
                            data,
                            timestamp: current_timestamp(),
                        });
                    }
                    Err(e) => {
                        eprintln!("Terminal read error: {}", e);
                        break;
                    }
                }
            }
        });

        // Spawn input writer thread
        let mut writer = master_pty
            .take_writer()
            .map_err(|e| crate::ui::UIError::TerminalError(format!("Failed to take writer: {}", e)))?;

        tokio::spawn(async move {
            while let Some(data) = input_rx.recv().await {
                if let Err(e) = writer.write_all(&data) {
                    eprintln!("Terminal write error: {}", e);
                    break;
                }
            }
        });

        // Spawn command handler thread for PTY operations
        tokio::spawn(async move {
            while let Some(cmd) = command_rx.recv().await {
                match cmd {
                    TerminalCommand::Write(data) => {
                        // Already handled by input_tx channel
                    }
                    TerminalCommand::Resize { rows, cols } => {
                        let _ = master_pty.resize(PtySize {
                            rows,
                            cols,
                            pixel_width: 0,
                            pixel_height: 0,
                        });
                    }
                    TerminalCommand::Kill => {
                        let _ = child.kill();
                    }
                    TerminalCommand::CheckAlive(tx) => {
                        let is_alive = child.try_wait().ok().flatten().is_none();
                        let _ = tx.send(is_alive);
                    }
                }
            }
        });

        Ok(Self {
            session_id,
            event_bus,
            input_tx,
            command_tx,
        })
    }

    /// Write data to the terminal (user input or agent injection)
    pub fn write(&self, data: &[u8]) -> UIResult<()> {
        self.input_tx
            .send(data.to_vec())
            .map_err(|e| crate::ui::UIError::TerminalError(format!("Failed to send input: {}", e)))
    }

    /// Write a command and press enter (convenience for agents)
    pub fn execute_command(&self, command: &str) -> UIResult<()> {
        let mut data = command.as_bytes().to_vec();
        data.push(b'\n');
        self.write(&data)
    }

    /// Send Ctrl+C signal
    pub fn send_interrupt(&self) -> UIResult<()> {
        self.write(&[0x03])  // ETX (Ctrl+C)
    }

    /// Resize the terminal
    pub fn resize(&self, rows: u16, cols: u16) -> UIResult<()> {
        self.command_tx
            .send(TerminalCommand::Resize { rows, cols })
            .map_err(|e| crate::ui::UIError::TerminalError(format!("Failed to send resize command: {}", e)))
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Check if child process is still alive
    pub async fn is_alive(&self) -> bool {
        let (tx, rx) = oneshot::channel();
        if self.command_tx.send(TerminalCommand::CheckAlive(tx)).is_err() {
            return false;
        }
        rx.await.unwrap_or(false)
    }

    /// Kill the child process
    pub fn kill(&self) -> UIResult<()> {
        self.command_tx
            .send(TerminalCommand::Kill)
            .map_err(|e| crate::ui::UIError::TerminalError(format!("Failed to send kill command: {}", e)))
    }
}

/// Manager for multiple terminal sessions
pub struct VirtualTerminal {
    sessions: Arc<RwLock<Vec<TerminalSession>>>,
    event_bus: Arc<EventBus>,
}

impl VirtualTerminal {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(Vec::new())),
            event_bus,
        }
    }

    /// Create a new terminal session
    pub fn create_session(&self, working_dir: Option<String>) -> UIResult<String> {
        let session_id = format!("term-{}", uuid::Uuid::new_v4());

        let session = TerminalSession::new(
            session_id.clone(),
            working_dir,
            Arc::clone(&self.event_bus),
        )?;

        self.sessions.write().push(session);

        Ok(session_id)
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: &str) -> Option<()> {
        let sessions = self.sessions.read();
        sessions.iter().find(|s| s.session_id() == session_id).map(|_| ())
    }

    /// Write to a session
    pub fn write_to_session(&self, session_id: &str, data: &[u8]) -> UIResult<()> {
        let sessions = self.sessions.read();
        let session = sessions
            .iter()
            .find(|s| s.session_id() == session_id)
            .ok_or_else(|| crate::ui::UIError::TerminalError("Session not found".to_string()))?;

        session.write(data)
    }

    /// Execute command in a session (for agents)
    pub fn execute_in_session(&self, session_id: &str, command: &str) -> UIResult<()> {
        let sessions = self.sessions.read();
        let session = sessions
            .iter()
            .find(|s| s.session_id() == session_id)
            .ok_or_else(|| crate::ui::UIError::TerminalError("Session not found".to_string()))?;

        session.execute_command(command)
    }

    /// Send Ctrl+C to a session
    pub fn interrupt_session(&self, session_id: &str) -> UIResult<()> {
        let sessions = self.sessions.read();
        let session = sessions
            .iter()
            .find(|s| s.session_id() == session_id)
            .ok_or_else(|| crate::ui::UIError::TerminalError("Session not found".to_string()))?;

        session.send_interrupt()
    }

    /// Resize a session
    pub fn resize_session(&self, session_id: &str, rows: u16, cols: u16) -> UIResult<()> {
        let sessions = self.sessions.read();
        let session = sessions
            .iter()
            .find(|s| s.session_id() == session_id)
            .ok_or_else(|| crate::ui::UIError::TerminalError("Session not found".to_string()))?;

        session.resize(rows, cols)
    }

    /// Close a session
    pub fn close_session(&self, session_id: &str) -> UIResult<()> {
        let mut sessions = self.sessions.write();
        let index = sessions
            .iter()
            .position(|s| s.session_id() == session_id)
            .ok_or_else(|| crate::ui::UIError::TerminalError("Session not found".to_string()))?;

        let session = sessions.remove(index);
        session.kill()?;

        Ok(())
    }

    /// Get list of active session IDs
    pub fn list_sessions(&self) -> Vec<String> {
        self.sessions
            .read()
            .iter()
            .map(|s| s.session_id().to_string())
            .collect()
    }

    /// Cleanup dead sessions
    pub async fn cleanup_dead_sessions(&self) {
        let mut sessions = self.sessions.write();
        let mut alive_sessions = Vec::new();

        for session in sessions.drain(..) {
            if session.is_alive().await {
                alive_sessions.push(session);
            }
        }

        *sessions = alive_sessions;
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_terminal_creation() {
        let event_bus = Arc::new(EventBus::new());
        let vt = VirtualTerminal::new(event_bus);

        let session_id = vt.create_session(None).unwrap();
        assert!(vt.get_session(&session_id).is_some());
    }

    #[tokio::test]
    async fn test_execute_command() {
        let event_bus = Arc::new(EventBus::new());
        let vt = VirtualTerminal::new(event_bus);

        let session_id = vt.create_session(None).unwrap();
        let result = vt.execute_in_session(&session_id, "echo test");

        assert!(result.is_ok());
    }
}
