//! Built-in Tool Implementations
//!
//! This module contains all built-in Gluon tools:
//! - File system operations (read, write)
//! - Code editing (apply_patch)
//! - Analysis (search_code)
//! - System operations (run_command)
//! - Browser automation (take_screenshot)
//! - Meta-tools (get_manifest)

pub mod read_file;
pub mod write_file;
pub mod get_manifest;
pub mod remote_tool;
pub mod apply_patch;
pub mod run_command;
pub mod search_code;

// Re-export tool implementations
pub use read_file::ReadFileTool;
pub use write_file::WriteFileTool;
pub use get_manifest::GetManifestTool;
pub use remote_tool::RemoteToolProxy;
pub use apply_patch::ApplyPatchTool;
pub use run_command::RunCommandTool;
pub use search_code::SearchCodeTool;
