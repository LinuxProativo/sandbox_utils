use sandbox_utils::{download_file, extract_bootstrap};
use std::fs;
use std::path::PathBuf;

pub fn test_file(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("files");
    p.push(name);
    p
}

#[test]
#[cfg(feature = "gz")]
fn test1_extract_gz() {
    let archive = test_file("rootfs.tar.gz");
    let dest = PathBuf::from("/tmp/test_gz");
    extract_bootstrap(archive, dest.clone()).expect("Failed to extract GZ");
    fs::remove_dir_all(dest).expect("Failed");
    println!("\n\x1b[1;32m--> Extração GZ Passou!\x1b[0m");
}

#[test]
#[cfg(feature = "xz")]
fn test2_extract_xz() {
    let archive = test_file("rootfs.tar.xz");
    let dest = PathBuf::from("/tmp/test_xz");
    extract_bootstrap(archive, dest.clone()).expect("Failed to extract XZ");
    fs::remove_dir_all(dest).expect("Failed");
    println!("\x1b[1;32m--> Extração XZ Passou!\x1b[0m");
}

#[test]
#[cfg(feature = "zst")]
fn test3_extract_zst() {
    let archive = test_file("rootfs.tar.zst");
    let dest = PathBuf::from("/tmp/test_zst");
    extract_bootstrap(archive, dest.clone()).expect("Failed to extract ZST");
    fs::remove_dir_all(dest).expect("Failed");
    println!("\x1b[1;32m--> Extração ZST Passou!\x1b[0m");
}

#[test]
fn test4_download_test() {
    let link = "https://license.md/wp-content/uploads/2022/06/mit.txt";
    let dest = PathBuf::from("/tmp/test_download");
    download_file(link, dest.clone(), "mit.txt").expect("Failed to download");
    fs::remove_dir_all(dest).expect("Failed");
    println!("\x1b[1;32m--> Download Passou!\x1b[0m\n");
}
