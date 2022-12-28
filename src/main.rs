#![warn(clippy::all, clippy::pedantic)]

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(required = true)]
    dir: PathBuf,
}

fn recursive_dir_size(dir: &Path) -> io::Result<u64> {
    let mut size = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            size += recursive_dir_size(&entry.path())?;
        } else {
            size += entry.metadata()?.len();
        }
    }
    Ok(size)
}

fn size_in_bytes_pretty_string(size: u64) -> String {
    const SIZES: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut i = 0;
    let mut size = size as f64;
    while i < 4 && size >= 1024.0 {
        size /= 1024.0;
        i += 1;
    }
    format!("{:.2} {}", size, SIZES[i])
}

fn main() {
    let args = Args::parse();
    let canon_dir = dunce::canonicalize(&args.dir).unwrap();
    println!("working...");
    let size = recursive_dir_size(&canon_dir).unwrap();
    println!(
        "{}: {}",
        canon_dir.display(),
        size_in_bytes_pretty_string(size)
    );
}
