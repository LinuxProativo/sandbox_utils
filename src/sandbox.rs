//! # Sandbox Execution Module
//!
//! This module manages the actual execution of the containerized environment.
//! It handles the translation of configuration into specific arguments for
//! PRoot or Bubblewrap, manages user identity (UID/EUID), and ensures
//! essential system paths are correctly mounted.
//!
//! ## OverlayFS Support
//!
//! When `use_overlay` is enabled, this module mounts a FUSE-based overlay
//! filesystem over the rootfs before launching the sandbox. The rootfs becomes
//! the immutable lower layer, and all writes go to a temporary upper layer.
//! After the sandbox exits, the configured `OverlayAction` determines what
//! happens to those changes (discard, preserve, commit, or atomic commit).

unsafe extern "C" {
    /// Retrieves the real user ID of the calling process.
    /// Used to map the host user to the sandbox environment.
    fn getuid() -> u32;

    /// Retrieves the effective user ID of the calling process.
    /// Used to determine the current privilege level before entering the sandbox.
    fn geteuid() -> u32;
}

use crate::{default_rootfs, safe_home, sandbox_tool, tool_target, USE_BWRAP, USE_PROOT};
use overlayfs_fuse::{CommitFilter, InodeMode, OverlayAction, OverlayFS};
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
    /// If true, enables the OverlayFS FUSE layering system.
    pub use_overlay: bool,
    /// Defines the inode management strategy for the overlay filesystem.
    /// - `Virtual`: Sequential, ephemeral inodes (default). Fastest option.
    /// - `Persistent`: FNV-1a hash-based inodes, stable across remounts.
    pub inode_mode: InodeMode,
    /// Determines what happens to the overlay upper layer after the sandbox exits.
    /// - `Discard`: Deletes all changes (ephemeral session).
    /// - `Preserve`: Keeps the upper layer on disk for inspection.
    /// - `Commit`: Merges upper → lower, processes whiteouts.
    /// - `CommitAtomic`: Crash-safe backup-and-swap merge.
    pub action: OverlayAction,
    /// Optional custom path for the overlay upper layer.
    pub overlay_upper: Option<PathBuf>,
    /// If true, relocates the overlay mount point to `~/.cache/` instead of `/tmp/`.
    pub overlay_as_home: bool,
    /// If true, restricts the sandbox to essential rootfs paths only.
    pub secure_rootfs: bool,
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
            use_overlay: false,
            inode_mode: InodeMode::Virtual,
            action: OverlayAction::Discard,
            overlay_upper: None,
            overlay_as_home: false,
            secure_rootfs: false,
        }
    }
}

impl SandBox {
    /// Executes the sandbox with the given configuration.
    ///
    /// # Overlay lifecycle (when `use_overlay = true`)
    ///
    /// 1. An [`OverlayFS`] is created with `config.rootfs` as the lower layer.
    /// 2. The filesystem is mounted; a temporary mount point is generated automatically.
    /// 3. The sandbox tool (proot/bwrap) is pointed at the mount point instead of the rootfs.
    /// 4. After the process exits, the overlay is unmounted.
    /// 5. [`OverlayAction`] is applied (Discard / Preserve / Commit / CommitAtomic).
    ///
    /// # Arguments
    /// * `config` - A `SandBoxConfig` containing all execution parameters.
    ///
    /// # Returns
    /// * `Ok(())` - If the process starts and exits successfully.
    /// * `Err` - If the rootfs is missing, the overlay fails to mount, or the process errors.
    pub fn run(config: SandBoxConfig) -> Result<(), Box<dyn Error>> {
        if !config.rootfs.exists() {
            return Err(Box::new(RootfsNotFoundError(config.rootfs)));
        }

        let overlay_handle: Option<OverlayFS>;
        let effective_rootfs: PathBuf;

        if config.use_overlay {
            let mut overlay = OverlayFS::new(config.rootfs.clone());
            overlay.set_inode_mode(config.inode_mode.clone());

            if let Some(upper) = config.overlay_upper.clone() {
                overlay.set_upper(upper);
            }

            if config.overlay_as_home {
                overlay.mountpoint_as_home();
            }

            let filter = CommitFilter::new().skip_zero_permissions(true);
            overlay.set_commit_filter(filter);

            overlay.mount()?;

            effective_rootfs = overlay.handle().mount_point().to_path_buf();
            overlay_handle = Some(overlay);
        } else {
            effective_rootfs = config.rootfs.clone();
            overlay_handle = None;
        }

        let run_result = Self::exec_sandbox(&config, &effective_rootfs);

        if let Some(mut overlay) = overlay_handle {
            overlay.umount();
            overlay.overlay_action(config.action);
        }

        run_result
    }

    /// Internal: builds and spawns the actual sandbox process.
    ///
    /// Separated from [`Self::run`] so the overlay teardown always executes
    /// regardless of whether the process itself succeeds or fails.
    ///
    /// # Arguments
    /// * `config` - The full sandbox configuration, used to resolve the tool, bind
    ///   mounts, user identity flags, and the command to run inside the container.
    /// * `rootfs` - The effective root directory to pass to the sandbox tool. When
    ///   overlay is active, this is the overlay mount point, not the original rootfs.
    ///
    /// # Returns
    /// * `Ok(())` - If the sandbox process spawns and exits without error.
    /// * `Err` - If the tool command is unrecognized or the process fails to start.
    fn exec_sandbox(config: &SandBoxConfig, rootfs: &Path) -> Result<(), Box<dyn Error>> {
        let (uid, euid) = unsafe { (getuid(), geteuid()) };

        let tool_cmd = &config.rootfs_tool;
        let rootfs_str: &str = &rootfs.to_string_lossy();

        let args = match tool_cmd.as_ref() {
            USE_PROOT => Self::build_proot_options(
                rootfs_str,
                &config.args_bind,
                config.ignore_extra_bind,
                config.secure_rootfs,
            ),
            USE_BWRAP => Self::build_bwrap_options(
                rootfs_str,
                &config.args_bind,
                config.ignore_extra_bind,
                config.secure_rootfs,
            ),
            other => return Err(format!("Unsupported rootfs command: {}", other).into()),
        };

        let new_cmd = &config.run_cmd;
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
            full_args.push(new_cmd);
        }

        Command::new(&config.tool_target)
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
    /// * `secure_rootfs` - Enables strict isolation, skipping host system path mapping.
    ///
    /// # Returns
    /// A `String` containing the formatted CLI arguments for PRoot.
    fn build_proot_options(
        rootfs: &str,
        rootfs_args: &str,
        no_extra_binds: bool,
        secure_rootfs: bool,
    ) -> String {
        let mut proot_options = match secure_rootfs {
            true => format!("-S {rootfs} {rootfs_args}"),
            false => format!("-R {rootfs} --bind=/media --bind=/mnt {rootfs_args}"),
        };

        if !secure_rootfs && !no_extra_binds {
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
    /// * `secure_rootfs` - Skips host file sharing for maximum sandbox isolation.
    ///
    /// # Returns
    /// A `String` containing the formatted CLI arguments for Bubblewrap.
    fn build_bwrap_options(
        rootfs: &str,
        rootfs_args: &str,
        ignore_extra_binds: bool,
        secure_rootfs: bool,
    ) -> String {
        let mut bwrap_options = format!(
            "--unshare-user \
             --share-net \
             --bind {rootfs} / \
             --die-with-parent \
             --ro-bind-try /etc/host.conf /etc/host.conf \
             --ro-bind-try /etc/hosts /etc/hosts \
             --ro-bind-try /etc/nsswitch.conf /etc/nsswitch.conf \
             --ro-bind-try /etc/resolv.conf /etc/resolv.conf \
             --dev-bind /dev /dev \
             --ro-bind /sys /sys \
             --bind-try /proc /proc \
             --bind-try /tmp /tmp \
             --bind-try /run /run \
             --bind {home} {home} \
             {rootfs_args} \
             --setenv PATH \"/bin:/sbin:/usr/bin:/usr/sbin:/usr/libexec\"",
            home = safe_home().to_string_lossy(),
        );

        if !secure_rootfs {
            bwrap_options.push_str(
                " --ro-bind-try /etc/hosts.equiv /etc/hosts.equiv \
                --ro-bind-try /etc/netgroup /etc/netgroup \
                --ro-bind-try /etc/networks /etc/networks \
                --ro-bind-try /etc/localtime /etc/localtime \
                --ro-bind-try /etc/passwd /etc/passwd \
                --ro-bind-try /etc/group /etc/group \
                --ro-bind /var/run/dbus/system_bus_socket /var/run/dbus/system_bus_socket \
                --bind /media /media \
                --bind /mnt /mnt",
            );

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
