use overlayfs_fuse::{InodeMode, OverlayAction};
use sandbox_utils::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn test_file(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("files");
    p.push(name);
    p
}

fn get_unique_test_path(prefix: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    PathBuf::from(format!("/tmp/sandbox_test_{}_{}", prefix, timestamp))
}

fn fresh_rootfs(dest: &Path) {
    if dest.exists() {
        let _ = fs::remove_dir_all(dest);
    }
    fs::create_dir_all(dest).expect("Failed to create rootfs directory");

    extract_bootstrap(test_file("rootfs.tar.gz"), dest.to_path_buf())
        .expect("Failed to extract rootfs");
}

fn setup_test(test_id: &str) -> PathBuf {
    sandbox_init("ArchLinux", "ARCH").expect("Failed to initialize sandbox");
    set_sandbox_tool(USE_PROOT).expect("Failed to set sandbox tool");
    let rootfs = get_unique_test_path(test_id);
    fresh_rootfs(&rootfs);
    rootfs
}

#[cfg(feature = "gz")]
mod overlay_tests {
    use super::*;

    #[test]
    fn test_overlay_discard() {
        let rootfs = setup_test("discard");

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "touch /discard_test.txt".to_string(),
            use_overlay: true,
            action: OverlayAction::Discard,
            ..Default::default()
        })
        .expect("Discard action: run failed");

        assert!(
            !rootfs.join("discard_test.txt").exists(),
            "Discard: file leaked to lower layer"
        );
        let _ = fs::remove_dir_all(&rootfs);
    }

    #[test]
    fn test_overlay_commit() {
        let rootfs = setup_test("commit");

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "touch /commit_test.txt".to_string(),
            use_overlay: true,
            action: OverlayAction::Commit,
            ..Default::default()
        })
        .expect("Commit action: run failed");

        assert!(
            rootfs.join("commit_test.txt").exists(),
            "Commit: file was not merged into lower layer"
        );
        let _ = fs::remove_dir_all(&rootfs);
    }

    #[test]
    fn test_overlay_commit_atomic() {
        let rootfs = setup_test("atomic");

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "touch /atomic_test.txt".to_string(),
            use_overlay: true,
            action: OverlayAction::CommitAtomic,
            ..Default::default()
        })
        .expect("CommitAtomic action: run failed");

        assert!(
            rootfs.join("atomic_test.txt").exists(),
            "CommitAtomic: file was not merged into lower layer"
        );
        let _ = fs::remove_dir_all(&rootfs);
    }

    #[test]
    fn test_overlay_preserve_with_custom_upper() {
        let rootfs = setup_test("preserve");
        let custom_upper = get_unique_test_path("custom_upper");
        let _ = fs::remove_dir_all(&custom_upper);

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "touch /preserve_test.txt".to_string(),
            use_overlay: true,
            action: OverlayAction::Preserve,
            overlay_upper: Some(custom_upper.clone()),
            ..Default::default()
        })
        .expect("Preserve action: run failed");

        assert!(
            !rootfs.join("preserve_test.txt").exists(),
            "Preserve: file leaked to lower layer"
        );
        assert!(
            custom_upper.exists(),
            "Preserve: custom upper was removed"
        );

        let _ = fs::remove_dir_all(&rootfs);
        let _ = fs::remove_dir_all(&custom_upper);
    }

    #[test]
    fn test_overlay_as_home_cleanup() {
        let rootfs = setup_test("as_home");

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "echo 'as_home ok'".to_string(),
            use_overlay: true,
            action: OverlayAction::Discard,
            overlay_as_home: true,
            ..Default::default()
        })
        .expect("overlay_as_home: run failed");

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
        let _ = fs::remove_dir_all(&rootfs);
    }

    #[test]
    fn test_overlay_persistent_inodes() {
        let rootfs = setup_test("persistent");

        let config = SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "stat /bin/sh".to_string(),
            use_overlay: true,
            action: OverlayAction::Discard,
            inode_mode: InodeMode::Persistent,
            ..Default::default()
        };

        SandBox::run(config.clone()).expect("Persistent Inodes: first session failed");
        SandBox::run(config).expect("Persistent Inodes: second session failed");
        let _ = fs::remove_dir_all(&rootfs);
    }

    #[test]
    fn test_overlay_with_root_privileges() {
        let rootfs = setup_test("root_overlay");

        SandBox::run(SandBoxConfig {
            rootfs: rootfs.clone(),
            run_cmd: "id -u | grep -q '0'".to_string(),
            use_overlay: true,
            action: OverlayAction::Discard,
            use_root: true,
            ..Default::default()
        })
        .expect("use_root + overlay: run failed");
        let _ = fs::remove_dir_all(&rootfs);
    }
}
