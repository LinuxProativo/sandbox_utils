//! # Progress and File Management Module
//!
//! This module handles external resource acquisition and decompression.
//! It provides visual feedback in the terminal using progress bars for both
//! downloading files and extracting bootstrap archives.

use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::{self, BufWriter};
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::result::Result;
use tar::Archive;

/// Template string for the `indicatif` progress bar styling.
const DOWNLOAD_TEMPLATE: &str = "{msg} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})";

/// Downloads a file from a URL to a local destination with a progress bar.
///
/// If the file already exists at the destination, the download is skipped.
///
/// # Arguments
/// * `url` - The source URL of the file.
/// * `dest` - The directory where the file should be saved.
/// * `filename` - The name to give to the downloaded file.
///
/// # Returns
/// * `Ok(())` - If the file was downloaded successfully or already exists.
/// * `Err` - If networked, I/O, or directory creation fails.
pub fn download_file(url: &str, dest: PathBuf, filename: &str) -> Result<(), Box<dyn Error>> {
    let save_path = dest.join(filename);

    if save_path.exists() {
        return Ok(());
    }

    fs::create_dir_all(&dest)?;
    let resp = ureq::get(url).call()?;

    let total_size = resp
        .headers()
        .get("Content-Length")
        .and_then(|v| v.to_str().unwrap().parse::<u64>().ok())
        .unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_message("Downloading...");
    pb.set_style(ProgressStyle::with_template(DOWNLOAD_TEMPLATE)?.progress_chars("##-"));

    let file = File::create(&save_path)?;
    let mut writer = BufWriter::new(file);
    let mut reader = pb.wrap_read(resp.into_body().into_reader());

    io::copy(&mut reader, &mut writer)?;
    pb.finish_with_message("Downloaded!");

    Ok(())
}

/// Extracts a compressed bootstrap archive (tar) to a destination directory.
///
/// Supports `.gz`, `.xz`, and `.zst` formats based on enabled crate features.
///
/// # Arguments
/// * `file_path` - Path to the compressed archive file.
/// * `destination` - Directory where the contents will be extracted.
///
/// # Returns
/// * `Ok(())` - If extraction completes successfully.
/// * `Err` - If the format is unsupported, the file is corrupted, or I/O fails.
pub fn extract_bootstrap(file_path: PathBuf, destination: PathBuf) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(&destination)?;

    let file = File::open(&file_path)?;
    let total_size = file.metadata()?.len();

    let pb = ProgressBar::new(total_size);
    pb.set_message("Extracting...");
    pb.set_style(ProgressStyle::with_template(DOWNLOAD_TEMPLATE)?.progress_chars("##-"));

    let reader = pb.wrap_read(BufReader::with_capacity(64 * 1024, file));
    let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");

    let decoder: Box<dyn Read> = match ext {
        #[cfg(feature = "gz")]
        "gz" => Box::new(flate2::read::GzDecoder::new(reader)),

        #[cfg(feature = "xz")]
        "xz" => Box::new(xz2::read::XzDecoder::new(reader)),

        #[cfg(feature = "zst")]
        "zst" | "zstd" => Box::new(zstd::stream::read::Decoder::new(reader)?),

        _ => {
            return Err(format!("Unsupported or disabled format: .{ext}",).into());
        }
    };

    let mut archive = Archive::new(decoder);
    archive.unpack(&destination)?;

    pb.finish_with_message("Extracted! ");
    Ok(())
}
