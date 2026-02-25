//! # Sandbox Initialization Module
//!
//! This module handles the global state setup for the sandbox environment.
//! It is divided into two phases: path/architecture initialization and
//! sandbox tool (PRoot/Bwrap) configuration.

use crate::download_file;

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use which::which;

/// Constant identifier for the PRoot tool.
pub const USE_PROOT: &str = "proot";
/// Constant identifier for the Bubblewrap tool.
pub const USE_BWRAP: &str = "bwrap";

/// Holds the core path and environment configurations for the application.
#[derive(Clone)]
pub struct SandboxConfig {
    /// Name of the application.
    pub app_name: String,
    /// Detected or defined CPU architecture.
    pub app_arch: String,
    /// Path to the user's safe home directory.
    pub safe_home: PathBuf,
    /// Directory for configuration files.
    pub config_dir: PathBuf,
    /// Path to the specific configuration file.
    pub config_file: PathBuf,
    /// Default directory for permanent cache.
    pub default_cache: PathBuf,
    /// Default directory for the root filesystem.
    pub default_rootfs: PathBuf,
    /// Directory for temporary files (usually in /tmp).
    pub temp_cache: PathBuf,
}

/// Holds information about the chosen sandbox execution tool.
#[derive(Clone)]
pub struct SandboxTool {
    /// The name of the tool (proot or bwrap).
    pub name: String,
    /// The absolute path to the tool's binary.
    pub target: PathBuf,
}

/// Internal structure to map tool IDs to their download URLs.
struct Link {
    id: &'static str,
    link: &'static str,
}

/// List of available download links for supported tools on x86_64.
const LINK_OPTIONS: &[Link] = &[
    Link {
        id: USE_PROOT,
        link: "https://github.com/LinuxProativo/StaticHub/releases/download/proot/proot",
    },
    Link {
        id: USE_BWRAP,
        link: "https://github.com/LinuxProativo/StaticHub/releases/download/bwrap/bwrap",
    },
];

/// Global storage for application paths and environment config.
static CONFIG: OnceLock<SandboxConfig> = OnceLock::new();

/// Global storage for the selected sandbox tool.
static TOOL: OnceLock<SandboxTool> = OnceLock::new();

/// Target architecture for binary downloads.
static AMD64: &str = "x86_64";

/// Initializes the base directories and detects the system architecture.
///
/// # Arguments
/// * `name` - The internal name of the application for path generation.
/// * `arch_env` - Environment variable name to override architecture detection.
///
/// # Returns
/// * `Ok(())` if initialization succeeds.
/// * `Err` if directory creation fails.
pub fn sandbox_init(
    name: &str,
    arch_env: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let config_dir = home.join(".config").join(name);
    fs::create_dir_all(&config_dir)?;

    let default_cache = home.join(".cache").join(name);
    fs::create_dir_all(&default_cache)?;

    let app_name = env::args_os()
        .next()
        .and_then(|s| {
            Path::new(&s)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| name.to_string());

    let arch = env::var(arch_env)
        .or_else(|_| env::var("ARCH"))
        .unwrap_or_else(|_| env::consts::ARCH.to_string());

    let config = SandboxConfig {
        app_name,
        app_arch: arch,
        safe_home: home.clone(),
        config_file: config_dir.join("config.toml"),
        default_cache,
        default_rootfs: home.join(format!(".{}", name)),
        temp_cache: Path::new("/tmp").join(name),
        config_dir,
    };

    let _ = CONFIG.set(config);
    Ok(())
}

/// Configures the sandbox tool, downloading it if not found in the system.
///
/// # Arguments
/// * `sandbox_tool` - The name of the tool to use (`proot` or `bwrap`).
///
/// # Returns
/// * `Ok(())` if the tool is ready for use.
/// * `Err` if the tool is missing and cannot be downloaded for the current arch.
pub fn set_sandbox_tool(sandbox_tool: &str) -> Result<(), Box<dyn std::error::Error>> {
    let arch = app_arch();
    let path = env::var_os("PATH").unwrap_or_default();
    let local_dir = safe_home().join(".local").join("bin");
    let new_path = format!("{}:{}", path.display(), local_dir.display());
    unsafe {
        env::set_var("PATH", new_path);
    }

    let tool_target = match which(sandbox_tool) {
        Ok(target) => target,
        Err(_) => {
            if arch == AMD64 {
                let local_tool = local_dir.join(sandbox_tool);
                let link_info = LINK_OPTIONS
                    .iter()
                    .find(|l| l.id == sandbox_tool)
                    .ok_or_else(|| format!("No download link found for tool: {sandbox_tool}"))?;

                fs::create_dir_all(&local_dir)?;
                download_file(link_info.link, local_dir, sandbox_tool)?;

                let mut perms = fs::metadata(&local_tool)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&local_tool, perms)?;

                local_tool
            } else {
                return Err(
                    format!("{sandbox_tool} not found and no binary available for {arch}").into(),
                );
            }
        }
    };

    let _ = TOOL.set(SandboxTool {
        name: sandbox_tool.to_string(),
        target: tool_target,
    });

    Ok(())
}

/// Returns the application name from global config.
pub fn app_name() -> String {
    CONFIG.wait().app_name.clone()
}

/// Returns the detected architecture from global config.
pub fn app_arch() -> String {
    CONFIG.wait().app_arch.clone()
}

/// Returns the safe home path from global config.
pub fn safe_home() -> PathBuf {
    CONFIG.wait().safe_home.clone()
}

/// Returns the configuration directory path.
pub fn config_dir() -> PathBuf {
    CONFIG.wait().config_dir.clone()
}

/// Returns the path to the configuration file.
pub fn config_file() -> PathBuf {
    CONFIG.wait().config_file.clone()
}

/// Returns the default cache directory.
pub fn default_cache() -> PathBuf {
    CONFIG.wait().default_cache.clone()
}

/// Returns the default rootfs directory.
pub fn default_rootfs() -> PathBuf {
    CONFIG.wait().default_rootfs.clone()
}

/// Returns the temporary cache directory.
pub fn temp_cache() -> PathBuf {
    CONFIG.wait().temp_cache.clone()
}

/// Returns the name of the selected sandbox tool.
pub fn sandbox_tool() -> String {
    TOOL.wait().name.clone()
}

/// Returns the absolute path to the sandbox tool binary.
pub fn tool_target() -> PathBuf {
    TOOL.wait().target.clone()
}
