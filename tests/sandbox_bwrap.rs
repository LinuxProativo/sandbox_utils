use sandbox_utils::*;
use std::fs;
use std::path::PathBuf;

pub fn test_file(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("files");
    p.push(name);
    p
}

#[test]
fn test1_target_not_found() {
    sandbox_init("ArchLinux", "ARCH").expect("Failed");
    set_sandbox_tool(USE_BWRAP).expect("Failed");

    let config = SandBoxConfig {
        rootfs: PathBuf::from("/tmp/pasta_inexistente"),
        ..Default::default()
    };

    let _ = SandBox::run(config).map_err(|e| {
        if let Some(err) = e.downcast_ref::<RootfsNotFoundError>() {
            match failed_exist_rootfs(&format!("{} setup", app_name()), &err.0.to_string_lossy()) {
                Ok(_) => {}
                Err(err) => {
                    eprintln!("\n\x1b[1;31m{}\x1b[0m\n", err)
                }
            }
        }
        e
    });
}

#[test]
fn test2_run_command_bwrap() {
    sandbox_init("ArchLinux", "ARCH").expect("Failed");
    set_sandbox_tool(USE_BWRAP).expect("Failed");

    let archive = test_file("rootfs.tar.gz");
    let dest = PathBuf::from("/tmp/test_gz3");
    extract_bootstrap(archive, dest.clone()).expect("Failed to extract GZ");

    let mut config = SandBoxConfig {
        rootfs: PathBuf::from("/tmp/test_gz3"),
        ..Default::default()
    };

    config.run_cmd = "ls -l /usr/share/icons/*".to_string();
    SandBox::run(config.clone()).expect("Failed");

    config.run_cmd =
        "echo -e \"\\nUSER: $USER - LOGNAME: $LOGNAME - UID: $UID - EUID: $EUID\"".to_string();
    SandBox::run(config.clone()).expect("Failed");

    config.run_cmd =
        "echo -e \"USER: $USER - LOGNAME: $LOGNAME - UID: $UID - EUID: $EUID\\n\"".to_string();
    config.use_root = true;
    SandBox::run(config.clone()).expect("Failed");
    fs::remove_dir_all(dest).expect("Failed");
}
