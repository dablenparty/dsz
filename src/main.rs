#![warn(clippy::all, clippy::pedantic)]

use std::path::{Path, PathBuf};

use clap::Parser;
use num_format::{Locale, ToFormattedString};
use rayon::prelude::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The directory to calculate the size of.
    #[arg(default_value = ".")]
    dir: PathBuf,
}

/// Computes the size of a directory, returning the size in bytes and the number of files.
/// This function is parallelized using rayon.
///
/// # Arguments
///
/// * `dir` - The directory to calculate the size of.
fn parallel_dir_size(dir: &Path) -> (u64, u64) {
    let walker: Vec<u64> = walkdir::WalkDir::new(dir)
        .into_iter()
        .par_bridge()
        .filter_map(std::result::Result::ok)
        .filter(|e| !e.path_is_symlink() && e.file_type().is_file())
        .map(|entry| {
            entry.metadata().map_or_else(
                |e| {
                    eprintln!("Error while reading file {}: {e}", entry.path().display());
                    0
                },
                |f| f.len(),
            )
        })
        .collect();
    let size = walker.iter().sum();
    (size, walker.len() as u64)
}

fn size_in_bytes_pretty_string(size: u64) -> String {
    const SIZES: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut i = 0;
    // if you have this much data... god help you
    #[allow(clippy::cast_precision_loss)]
    let mut size = size as f64;
    while i < 4 && size >= 1024.0 {
        size /= 1024.0;
        i += 1;
    }
    let size_str = SIZES[i];
    format!("{size:.2} {size_str}")
}

fn main() {
    let args = Args::parse();
    let canon_dir = dunce::canonicalize(args.dir).unwrap();
    println!("{}", canon_dir.display());
    let (size, file_count) = parallel_dir_size(&canon_dir);
    let size_str = size_in_bytes_pretty_string(size);
    let file_count_str = file_count.to_formatted_string(&Locale::en);
    println!("{file_count_str} files");
    println!("{size_str}");
}
