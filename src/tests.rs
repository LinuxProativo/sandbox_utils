use super::*;
use std::path::PathBuf;
use std::{env, fs};
use serde::Serialize;

fn test_file(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push(name);
    p
}

fn newfn() {
    println!("\x1b[1;32mCurrent Tool:  \"{}\"", sandbox_tool());
    println!("Tool Target:   {:?}\x1b[0m", tool_target());
}

// #[test]
// fn test0_arquitecture_not_found() {
//     let fake_arch = "super_cpu_9000";
//
//     unsafe {
//         env::set_var("ARCH", fake_arch);
//     }
//
//     sandbox_init("ArchLinux", "ARCH").expect("Failed");
//     let a = set_sandbox_tool("seila");
//     eprintln!("\n\x1b[1;31m{:?}\x1b[0m", a.err().unwrap());
// }

#[test]
fn test1_sandbox_output() {
    sandbox_init("ArchLinux", "ARCH").expect("Failed");
    set_sandbox_tool(USE_PROOT).expect("Failed");

    println!("\n\x1b[1;34m------------ Sandbox Utils Debug Info ------------\x1b[1;33m");
    println!("App Name:      {}", app_name());
    println!(
        "Architecture:  {}\n\x1b[1;34m{}\x1b[1;33m",
        app_arch(),
        "-".repeat(50)
    );
    println!("Safe Home:     {:?}", safe_home());
    println!("Config Dir:    {:?}", config_dir());
    println!("Config File:   {:?}", config_file());
    println!("Cache Dir:     {:?}", default_cache());
    println!("Rootfs Dir:    {:?}", default_rootfs());
    println!("Temp Cache:    {:?}", temp_cache());
    println!("\x1b[1;34m{}\x1b[0m", "-".repeat(50));
    newfn();

    assert!(!app_name().is_empty());
    assert!(config_dir().to_string_lossy().contains("ArchLinux"));
}

#[test]
fn test2_target_not_found() {
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
fn test3_download_test() {
    let link = "https://license.md/wp-content/uploads/2022/06/mit.txt";
    let dest = PathBuf::from("/tmp/test_download");
    download_file(link, dest.clone(), "mit.txt").expect("Failed to download");
    fs::remove_dir_all(dest).expect("Failed");
    println!("\x1b[1;32m--> Download Passou!\x1b[0m");
}

#[test]
#[cfg(feature = "gz")]
fn test4_extract_gz() {
    let archive = test_file("rootfs.tar.gz");
    let dest = PathBuf::from("/tmp/test_gz");
    extract_bootstrap(archive, dest.clone()).expect("Failed to extract GZ");
    fs::remove_dir_all(dest).expect("Failed");
    println!("\x1b[1;32m--> Extração GZ Passou!\x1b[0m");
}

#[test]
#[cfg(feature = "xz")]
fn test5_extract_xz() {
    let archive = test_file("rootfs.tar.xz");
    let dest = PathBuf::from("/tmp/test_xz");
    extract_bootstrap(archive, dest.clone()).expect("Failed to extract XZ");
    fs::remove_dir_all(dest).expect("Failed");
    println!("\x1b[1;32m--> Extração XZ Passou!\x1b[0m");
}

#[test]
#[cfg(feature = "zst")]
fn test6_extract_zst() {
    let archive = test_file("rootfs.tar.zst");
    let dest = PathBuf::from("/tmp/test_zst");
    extract_bootstrap(archive, dest.clone()).expect("Failed to extract ZST");
    fs::remove_dir_all(dest).expect("Failed");
    println!("\x1b[1;32m--> Extração ZST Passou!\x1b[0m");
}

#[test]
fn test7_messages_dialog() {
    sandbox_init("ArchLinux", "ARCH").expect("Failed");
    set_sandbox_tool(USE_PROOT).expect("Failed");
    success_finish_setup(format!("{} run", app_name()).as_str()).expect("Failed");

    let res = "resultado de teste\nteste dois";

    println!(
        "\n{u}\n{}\n{res}\n{u}",
        get_cmd_box("SEARCH RESULT:", None, Some(18)).expect("Failed"),
        u = SEPARATOR
    );

    #[derive(Serialize)]
    struct MyTest { os: String, arch: String, status: String }

    let old = MyTest { os: "Debian".into(), arch: "x86_64".into(), status: "Online".into() };
    let new = MyTest { os: "Debian".into(), arch: "x86_64".into(), status: "Active".into() };

    let diff = get_config_diff(&old, &new);
    render_table(diff);
}

#[test]
fn test8_run_command_proot() {
    sandbox_init("ArchLinux", "ARCH").expect("Failed");
    set_sandbox_tool(USE_PROOT).expect("Failed");

    let archive = test_file("rootfs.tar.gz");
    let dest = PathBuf::from("/tmp/test_gz2");
    extract_bootstrap(archive, dest.clone()).expect("Failed to extract GZ");

    let mut config = SandBoxConfig {
        rootfs: PathBuf::from("/tmp/test_gz2"),
        ..Default::default()
    };

    config.run_cmd =
        "echo -e \"\\nUSER: $USER - LOGNAME: $LOGNAME - UID: $UID - EUID: $EUID\"".to_string();
    SandBox::run(config.clone()).expect("Failed");

    config.run_cmd =
        "echo -e \"USER: $USER - LOGNAME: $LOGNAME - UID: $UID - EUID: $EUID\\n\"".to_string();
    config.use_root = true;
    SandBox::run(config.clone()).expect("Failed");
    fs::remove_dir_all(dest).expect("Failed");
}

#[test]
fn test9_run_command_bwrap() {
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
