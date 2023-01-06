#![warn(clippy::all, clippy::pedantic)]

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use clap::Parser;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The directory to calculate the size of. If not specified, the current directory is used.
    #[arg(default_value = ".")]
    dir: PathBuf,
    #[arg(short, long)]
    list: bool,
    #[arg(short, long, default_value = "1")]
    depth: usize,
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

fn tree_string(dir: &Path, max_depth: usize) -> String {
    const VERTICAL: &str = "│   ";
    const BRANCH: &str = "├── ";
    const LAST_BRANCH: &str = "└── ";

    let initial_depth = dir.components().count();

    let strings: Vec<String> = WalkDir::new(dir)
        .max_depth(max_depth)
        .sort_by_file_name()
        .into_iter()
        .map(|f| {
            let f = f.unwrap();
            let path = f.path();
            let depth = path.components().count() - initial_depth;
            let is_last = (path.is_file() || depth == max_depth)
                && WalkDir::new(path.parent().unwrap())
                    .sort_by_file_name()
                    .max_depth(1)
                    .into_iter()
                    .last()
                    .map(|f| f.unwrap())
                    .map(|f| f.path().to_path_buf())
                    .map_or(false, |p| p == path);
            let prefix = if depth == 0 {
                String::new()
            } else {
                let mut prefix = String::new();
                for _ in 0..depth - 1 {
                    prefix.push_str(VERTICAL);
                }
                prefix.push_str(if is_last { LAST_BRANCH } else { BRANCH });
                prefix
            };
            let suffix = if path.is_file() || depth == max_depth {
                let mut s = String::new();
                let size = if path.is_dir() {
                    s.push('/');
                    recursive_dir_size(path).unwrap_or(0)
                } else {
                    path.metadata().map_or(0, |m| m.len())
                };
                let size_str = size_in_bytes_pretty_string(size);
                format!("{s}: {size_str}")
            } else if path.is_dir() {
                String::from("/")
            } else {
                String::new()
            };
            format!("{prefix}{}{suffix}", f.file_name().to_string_lossy())
        })
        .collect();
    strings.join("\n")
}

fn main() {
    let args = Args::parse();
    let canon_dir = dunce::canonicalize(args.dir).unwrap();
    if args.list {
        let tree = tree_string(&canon_dir, args.depth);
        println!("{}\n", tree);
    }
    let size = recursive_dir_size(&canon_dir).unwrap();
    let size_str = size_in_bytes_pretty_string(size);
    println!("{}: {size_str}", canon_dir.display());
}
