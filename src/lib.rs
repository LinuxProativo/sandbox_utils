//! # Sandbox Library
//!
//! This library provides a complete environment for managing Linux containers
//! using tools like `PRoot` and `Bubblewrap`. It handles everything from
//! initialization and configuration to file downloading and sandboxed execution.

mod dialogs;
mod init;
mod progress;
mod sandbox;

/// Re-exporting UI and formatting utilities for tables and dialogs.
pub use dialogs::{
    failed_exist_rootfs, get_cmd_box, get_config_diff, render_table, success_finish_setup,
    SEPARATOR,
};

/// Re-exporting core sandbox execution logic and configuration structures.
pub use sandbox::{RootfsNotFoundError, SandBox, SandBoxConfig};

/// Re-exporting utilities for file transfer and bootstrap extraction.
pub use progress::{download_file, extract_bootstrap};

/// Re-exporting initialization functions and environment getters.
///
/// These functions manage the global state of the application paths and
/// detect the host architecture.
pub use init::{
    app_arch, app_name, config_dir, config_file, default_cache, default_rootfs, safe_home,
    sandbox_init, sandbox_tool, set_sandbox_tool, temp_cache, tool_target, USE_BWRAP,
    USE_PROOT,
};

/// Unit tests for ensuring the integrity of the sandbox logic.
#[cfg(test)]
mod tests;