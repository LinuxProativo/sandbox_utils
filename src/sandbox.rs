//! # Sandbox Execution Module
//!
//! This module manages the actual execution of the containerized environment.
//! It handles the translation of configuration into specific arguments for
//! PRoot or Bubblewrap, manages user identity (UID/EUID), and ensures
//! essential system paths are correctly mounted.

unsafe extern "C" {
    /// Retrieves the real user ID of the calling process.
    /// Used to map the host user to the sandbox environment.
    fn getuid() -> u32;

    /// Retrieves the effective user ID of the calling process.
    /// Used to determine the current privilege level before entering the sandbox.
    fn geteuid() -> u32;
}

use crate::{USE_BWRAP, USE_PROOT, default_rootfs, safe_home, sandbox_tool, tool_target};

use std::error::Error;
use std::os::unix;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{fmt, fs};

/// Custom error type for cases where the RootFS directory is missing.
#[derive(Debug)]
pub struct RootfsNotFoundError(pub PathBuf);

/// Configuration structure for defining how the sandbox should run.
#[derive(Clone)]
pub struct SandBoxConfig {
    /// Path to the RootFS directory.
    pub rootfs: PathBuf,
    /// Command to be executed inside the sandbox.
    pub run_cmd: String,
    /// Which tool to use (proot or bwrap).
    pub rootfs_tool: String,
    /// Path to the sandbox tool binary.
    pub tool_target: PathBuf,
    /// Custom bind mounts provided by the user.
    pub args_bind: String,
    /// If true, simulates a root user environment.
    pub use_root: bool,
    /// If true, skips mounting non-essential host paths (e.g., fonts, themes).
    pub ignore_extra_bind: bool,
    /// If true, skips mapping host's passwd and group files.
    pub no_group: bool,
}

/// Core structure for sandbox operations.
pub struct SandBox;

impl fmt::Display for RootfsNotFoundError {
    /// Formats the error message for the missing RootFS directory.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rootfs directory not found at: {:?}", self.0)
    }
}

/// Implements the standard Error trait for RootfsNotFoundError.
///
/// This allows the struct to be used with the `?` operator and integrated
/// into generic error handling containers like `Box<dyn Error>`.
impl Error for RootfsNotFoundError {}

impl Default for SandBoxConfig {
    /// Provides the default configuration for the sandbox.
    ///
    /// # Returns
    /// A `SandBoxConfig` instance populated with global defaults from the `init` module.
    fn default() -> Self {
        Self {
            rootfs: default_rootfs(),
            run_cmd: String::default(),
            rootfs_tool: sandbox_tool(),
            tool_target: tool_target(),
            args_bind: String::default(),
            use_root: false,
            ignore_extra_bind: false,
            no_group: false,
        }
    }
}

impl SandBox {
    /// Executes the sandbox with the given configuration.
    ///
    /// This function handles the logic for user namespaces, environment variables,
    /// and the final assembly of the execution command for the chosen backend.
    ///
    /// # Arguments
    /// * `config` - A `SandBoxConfig` containing all execution parameters.
    ///
    /// # Returns
    /// * `Ok(())` - If the process starts and exits successfully.
    /// * `Err` - If the rootfs is missing or the process fails to start.
    pub fn run(config: SandBoxConfig) -> Result<(), Box<dyn Error>> {
        if !config.rootfs.exists() {
            return Err(Box::new(RootfsNotFoundError(config.rootfs)));
        }

        let (uid, euid) = unsafe { (getuid(), geteuid()) };

        let tool_cmd = config.rootfs_tool;
        let rootfs: &str = &config.rootfs.to_string_lossy();

        let args = match tool_cmd.as_ref() {
            USE_PROOT => Self::build_proot_options(
                rootfs,
                &config.args_bind,
                config.ignore_extra_bind,
                config.no_group,
            ),
            USE_BWRAP => Self::build_bwrap_options(
                rootfs,
                &config.args_bind,
                config.ignore_extra_bind,
                config.no_group,
            ),
            other => return Err(format!("Unsupported rootfs command: {}", other).into()),
        };

        let new_cmd = config.run_cmd;
        let mut full_args: Vec<&str> = args.split_whitespace().collect();

        let user = match config.use_root {
            true => "PS1=# |USER=root|LOGNAME=root|UID=0|EUID=0".to_string(),
            false => format!("PS1=$ |UID={uid}|EUID={euid}"),
        };

        if tool_cmd == USE_PROOT && config.use_root {
            full_args.push("-0");
        }

        if tool_cmd == USE_BWRAP && config.use_root {
            full_args.extend([
                "--uid", "0", "--gid", "0", "--setenv", "USER", "root", "--setenv", "LOGNAME",
                "root",
            ]);
        }

        full_args.push("env");
        full_args.extend_from_slice(&user.split('|').collect::<Vec<_>>());
        full_args.extend([
            "SHELL=/bin/sh",
            "PATH=/bin:/sbin:/usr/bin:/usr/sbin:/usr/libexec",
            "/bin/sh",
        ]);

        if !new_cmd.is_empty() {
            full_args.push("-c");
            full_args.push(&new_cmd);
        }

        Command::new(config.tool_target)
            .args(&full_args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;

        Ok(())
    }

    /// Generates the argument string specifically for PRoot.
    ///
    /// # Arguments
    /// * `rootfs` - String slice of the guest root directory path.
    /// * `rootfs_args` - Extra user-defined bind arguments.
    /// * `no_extra_binds` - Boolean to toggle mounting of host fonts/themes.
    /// * `no_group` - Boolean to toggle mapping of host passwd/group files.
    ///
    /// # Returns
    /// A `String` containing the formatted CLI arguments for PRoot.
    fn build_proot_options(
        rootfs: &str,
        rootfs_args: &str,
        no_extra_binds: bool,
        no_group: bool,
    ) -> String {
        let mut proot_options = format!("-R {rootfs} --bind=/media --bind=/mnt {rootfs_args}");

        if no_group {
            let bind = format!(
                " --bind={rootfs}/etc/group:/etc/group --bind={rootfs}/etc/passwd:/etc/passwd"
            );

            proot_options.push_str(bind.as_str());
        }

        if !no_extra_binds {
            let extra_paths = [
                "/etc/asound.conf",
                "/etc/fonts",
                "/usr/share/font-config",
                "/usr/share/fontconfig",
                "/usr/share/fonts",
                "/usr/share/themes",
            ];

            for path in extra_paths {
                if Path::new(path).exists() {
                    proot_options.push_str(" --bind=");
                    proot_options.push_str(path);
                }
            }

            if let Ok(entries) = fs::read_dir("/usr/share/icons") {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let cursor_path = path.join("cursors");

                    if cursor_path.is_dir() {
                        if let Some(p_str) = cursor_path.to_str() {
                            proot_options.push_str(" --bind=");
                            proot_options.push_str(p_str);
                        }
                    }
                }
            }
        }

        proot_options
    }

    /// Generates the argument string specifically for Bubblewrap.
    ///
    /// # Arguments
    /// * `rootfs` - String slice of the guest root directory path.
    /// * `rootfs_args` - Extra user-defined bind arguments.
    /// * `ignore_extra_binds` - Boolean to toggle mounting of host fonts/themes.
    /// * `no_group` - Boolean to toggle mapping of host passwd/group files.
    ///
    /// # Returns
    /// A `String` containing the formatted CLI arguments for Bubblewrap.
    fn build_bwrap_options(
        rootfs: &str,
        rootfs_args: &str,
        ignore_extra_binds: bool,
        no_group: bool,
    ) -> String {
        let mut bwrap_options = format!(
            "--unshare-user \
             --share-net \
             --bind {rootfs} / \
             --die-with-parent \
             --ro-bind-try /etc/host.conf /etc/host.conf \
             --ro-bind-try /etc/hosts /etc/hosts \
             --ro-bind-try /etc/hosts.equiv /etc/hosts.equiv \
             --ro-bind-try /etc/netgroup /etc/netgroup \
             --ro-bind-try /etc/networks /etc/networks \
             --ro-bind-try /etc/nsswitch.conf /etc/nsswitch.conf \
             --ro-bind-try /etc/resolv.conf /etc/resolv.conf \
             --ro-bind-try /etc/localtime /etc/localtime \
             --dev-bind /dev /dev \
             --ro-bind /sys /sys \
             --bind-try /proc /proc \
             --bind-try /tmp /tmp \
             --bind-try /run /run \
             --ro-bind /var/run/dbus/system_bus_socket /var/run/dbus/system_bus_socket \
             --bind {home} {home} \
             --bind /media /media \
             --bind /mnt /mnt \
             {rootfs_args} \
             --setenv PATH \"/bin:/sbin:/usr/bin:/usr/sbin:/usr/libexec\"",
            home = safe_home().to_string_lossy(),
        );

        if !no_group {
            bwrap_options.push_str(
                " --ro-bind-try /etc/passwd /etc/passwd --ro-bind-try /etc/group /etc/group",
            );
        }

        Self::fix_mtab_symlink(rootfs);

        if !ignore_extra_binds {
            let extra_paths = [
                "/etc/asound.conf",
                "/etc/fonts",
                "/usr/share/font-config",
                "/usr/share/fontconfig",
                "/usr/share/fonts",
                "/usr/share/themes",
            ];

            for path in extra_paths {
                if Path::new(path).exists() {
                    bwrap_options.push_str(" --ro-bind ");
                    bwrap_options.push_str(path);
                    bwrap_options.push_str(" ");
                    bwrap_options.push_str(path);
                }
            }

            if let Ok(entries) = fs::read_dir("/usr/share/icons") {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let cursor_path = path.join("cursors");
                    if cursor_path.is_dir() {
                        if let Some(p_str) = cursor_path.to_str() {
                            bwrap_options.push_str(" --ro-bind ");
                            bwrap_options.push_str(p_str);
                            bwrap_options.push_str(" ");
                            bwrap_options.push_str(p_str);
                        }
                    }
                }
            }
        }

        bwrap_options
    }

    /// Fixes or creates the `/etc/mtab` symlink inside the RootFS.
    ///
    /// # Arguments
    /// * `rootfs` - String slice of the guest root directory path.
    fn fix_mtab_symlink(rootfs: &str) {
        let mtab_path = Path::new(rootfs).join("etc").join("mtab");
        let target = "/proc/self/mounts";

        if let Ok(md) = fs::symlink_metadata(&mtab_path) {
            if md.is_symlink() {
                if let Ok(existing_target) = fs::read_link(&mtab_path) {
                    if existing_target.to_string_lossy() == target {
                        return;
                    }
                }
            }
        }

        let _ = fs::remove_file(&mtab_path);
        if mtab_path.is_dir() {
            let _ = fs::remove_dir_all(&mtab_path);
        }

        if let Err(e) = unix::fs::symlink(target, &mtab_path) {
            eprintln!("\x1b[1;33mWarning\x1b[0m: Failed to fix mtab symlink: {e}");
        }
    }
}
