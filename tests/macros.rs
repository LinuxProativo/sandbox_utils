use sandbox_utils::*;

#[test]
fn test_macro_invalid_arg() {
    sandbox_init("ALPack", "x86_64").expect("Init failed");

    let result: Result<(), Box<dyn std::error::Error>> = invalid_arg!("aports", "foo");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();

    println!("\n\x1b[1;31m{}\x1b[0m", err_msg);

    assert!(err_msg.contains(": aports"));
    assert!(err_msg.contains("invalid argument 'foo'"));
    assert!(err_msg.contains("--help"));
}

#[test]
fn test_macro_missing_arg() {
    sandbox_init("ALPack", "x86_64").expect("Init failed");

    let res_normal: Result<(), Box<dyn std::error::Error>> = missing_arg!("aports");
    let res_essential: Result<(), Box<dyn std::error::Error>> = missing_arg!("aports", essential);

    println!("\x1b[1;31m{:?}\x1b[0m", res_normal);
    println!("\x1b[1;31m{:?}\x1b[0m", res_essential);

    assert!(res_normal.unwrap_err().to_string().contains("no parameter specified"));
    assert!(res_essential.unwrap_err().to_string().contains("no essential parameter specified"));
}

#[test]
fn test_macro_parse_value() {
    sandbox_init("ALPack", "x86_64").expect("Init failed");

    let val1 = parse_value!("aports", "pkg", "--get=wget").expect("Failed to parse =");

    println!("\x1b[1;32m{:?}\x1b[0m", val1);
    assert_eq!(val1, "wget");

    let next_arg = Some("curl");
    let val2 = parse_value!("aports", "pkg", "--get", next_arg).expect("Failed to parse space");

    println!("\x1b[1;32m{:?}\x1b[0m", val2);
    assert_eq!(val2, "curl");

    let res_err = parse_value!("aports", "pkg", "--get=");
    println!("\x1b[1;31m{:?}\x1b[0m\n", res_err);

    assert!(res_err.is_err());
    assert!(res_err.unwrap_err().contains("requires a <pkg>"));
}
