use sandbox_utils::*;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

pub fn test_file(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("files");
    p.push(name);
    p
}

fn newfn() {
    println!("\x1b[1;32mCurrent Tool:  \"{}\"", sandbox_tool());
    println!("Tool Target:   {:?}\x1b[0m", tool_target());
}

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
fn test2_run_command_proot() {
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

    config.use_root = true;

    config.run_cmd =
        "echo -e \"USER: $USER - LOGNAME: $LOGNAME - UID: $UID - EUID: $EUID\\n\"".to_string();
    SandBox::run(config.clone()).expect("Failed");
    fs::remove_dir_all(dest).expect("Failed");
}

#[test]
fn test3_messages_dialog() {
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
    struct MyTest {
        os: String,
        arch: String,
        status: String,
    }

    let old = MyTest {
        os: "Debian".into(),
        arch: "x86_64".into(),
        status: "Online".into(),
    };
    let new = MyTest {
        os: "Debian".into(),
        arch: "x86_64".into(),
        status: "Active".into(),
    };

    let diff = get_config_diff(&old, &new);
    render_table(diff);
}
