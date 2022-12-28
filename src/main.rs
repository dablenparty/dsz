#![warn(clippy::all, clippy::pedantic)]

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The directory to calculate the size of. If not specified, the current directory is used.
    #[arg(default_value = ".")]
    dir: PathBuf,
}

fn recursive_dir_size(dir: &Path) -> io::Result<u64> {
    let mut size = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            size += recursive_dir_size(&entry.path()).unwrap_or_else(|e| {
                eprintln!(
                    "Error while reading directory {}: {e}",
                    entry.path().display()
                );
                0
            });
        } else {
            size += entry.metadata().map_or_else(
                |e| {
                    eprintln!("Error while reading file {}: {e}", entry.path().display());
                    0
                },
                |f| f.len(),
            );
        }
    }
    Ok(size)
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
    let size = recursive_dir_size(&canon_dir).unwrap();
    let size_str = size_in_bytes_pretty_string(size);
    println!("{}: {size_str}", canon_dir.display());
}
