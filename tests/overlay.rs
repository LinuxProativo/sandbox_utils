use overlayfs_fuse::{InodeMode, OverlayAction};
use sandbox_utils::*;
use std::fs;
use std::path::PathBuf;

/// Helper to get the path for test files.
pub fn test_file(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("files");
    p.push(name);
    p
}

/// Prepares a fresh rootfs for testing by extracting the bootstrap tarball.
fn fresh_rootfs(dest: &PathBuf) {
    let _ = fs::remove_dir_all(dest);
    extract_bootstrap(test_file("rootfs.tar.gz"), dest.clone())
        .expect("Failed to extract rootfs");
}

/// Common setup for all overlay tests.
fn setup_test() -> PathBuf {
    sandbox_init("ArchLinux", "ARCH").expect("Failed to init sandbox");
    set_sandbox_tool(USE_PROOT).expect("Failed to set sandbox tool");

    let rootfs = PathBuf::from("/tmp/test_overlay");
    fresh_rootfs(&rootfs);
    rootfs
}

#[cfg(feature = "gz")]
mod overlay_tests {
    use super::*;

    #[test]
    fn test_overlay_discard() {
        let rootfs = setup_test();

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "touch /discard_test.txt".to_string(),
            use_overlay: true,
            ignore_extra_bind: true,
            action: OverlayAction::Discard,
            ..Default::default()
        }).expect("Discard: run failed");

        assert!(
            !rootfs.join("discard_test.txt").exists(),
            "Discard: file leaked to lower layer"
        );
    }

    #[test]
    fn test_overlay_commit() {
        let rootfs = setup_test();

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "touch /commit_test.txt".to_string(),
            use_overlay: true,
            action: OverlayAction::Commit,
            ..Default::default()
        }).expect("Commit: run failed");

        assert!(
            rootfs.join("commit_test.txt").exists(),
            "Commit: file was not merged into lower layer"
        );
    }

    #[test]
    fn test_overlay_commit_atomic() {
        let rootfs = setup_test();

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "touch /atomic_test.txt".to_string(),
            use_overlay: true,
            action: OverlayAction::CommitAtomic,
            ..Default::default()
        }).expect("CommitAtomic: run failed");

        assert!(
            rootfs.join("atomic_test.txt").exists(),
            "CommitAtomic: file was not merged into lower layer"
        );
    }

    #[test]
    fn test_overlay_preserve_custom_upper() {
        let rootfs = setup_test();
        let custom_upper = PathBuf::from("/tmp/test_overlay_upper");
        let _ = fs::remove_dir_all(&custom_upper);

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "touch /preserve_test.txt".to_string(),
            use_overlay: true,
            action: OverlayAction::Preserve,
            overlay_upper: Some(custom_upper.clone()),
            ..Default::default()
        }).expect("Preserve: run failed");

        assert!(
            !rootfs.join("preserve_test.txt").exists(),
            "Preserve: file leaked to lower layer"
        );
        assert!(
            custom_upper.exists(),
            "Preserve: custom upper was removed"
        );

        let _ = fs::remove_dir_all(&custom_upper);
    }

    #[test]
    fn test_overlay_as_home() {
        let rootfs = setup_test();

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "echo 'as_home ok'".to_string(),
            use_overlay: true,
            action: OverlayAction::Discard,
            overlay_as_home: true,
            ..Default::default()
        }).expect("overlay_as_home: run failed");

        let leftover = fs::read_dir(safe_home().join(".cache"))
            .map(|e| {
                e.flatten()
                    .any(|f| f.file_name().to_string_lossy().starts_with("mount_"))
            })
            .unwrap_or(false);

        assert!(
            !leftover,
            "overlay_as_home: mount point was not removed from ~/.cache"
        );
    }

    #[test]
    fn test_overlay_inode_persistent() {
        let rootfs = setup_test();

        let config = SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "stat /bin/sh".to_string(),
            use_overlay: true,
            action: OverlayAction::Discard,
            inode_mode: InodeMode::Persistent,
            ..Default::default()
        };

        SandBox::run(config.clone()).expect("Persistent: first session failed");
        SandBox::run(config).expect("Persistent: second session failed");
    }

    #[test]
    fn test_overlay_with_root() {
        let rootfs = setup_test();

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "id -u | grep -q '0'".to_string(),
            use_overlay: true,
            action: OverlayAction::Discard,
            use_root: true,
            ..Default::default()
        }).expect("use_root + overlay: run failed");
    }
}
