use sandbox_utils::*;
use std::env;

#[test]
fn test_set_tool_unsupported_arch() {
    let name = "ArchLinux";
    let arch_env = "ALPACK_ARCH_FORCE";
    unsafe {
        env::set_var(arch_env, "armv7l");
    }

    sandbox_init(name, arch_env).expect("Init failed");

    let result = set_sandbox_tool("noexist");

    if let Err(e) = result {
        let msg = e.to_string();
        println!("\n\x1b[1;31m{}\x1b[0m\n", msg);
        assert!(msg.contains("not found and no binary available for armv7l"));
    }
}
