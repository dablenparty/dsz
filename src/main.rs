#![warn(clippy::all, clippy::pedantic)]

use std::{
    iter,
    path::{Path, PathBuf},
};

use clap::Parser;
use itertools::Itertools;
use num_format::{Locale, ToFormattedString};
use rayon::prelude::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The directory to calculate the size of.
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Whether to print a tree of the directory. Optionally, specify the depth of the tree.
    #[arg(short, long, num_args = 0..=1, require_equals = true, default_missing_value = "1")]
    tree: Option<usize>,
}

fn generate_tree_string(root: &Path, depth: usize) -> String {
    const INDENT: &str = "│   ";
    const BRANCH: &str = "├───";
    const BRANCH_LAST: &str = "└───";

    let walker = walkdir::WalkDir::new(root)
        .sort_by_key(|e| e.path().is_dir())
        .sort_by_file_name()
        .max_depth(depth)
        .into_iter()
        .filter_map(std::result::Result::ok);
    iter::once(None)
        .chain(walker.map(Some))
        .chain(iter::once(None))
        .tuple_windows::<(_, _)>()
        .filter_map(|(entry, next_entry)| {
            let entry = entry?;
            let path = entry.path();
            let path_components_count = path.components().count();
            let depth_diff = path_components_count - root.components().count();
            if depth_diff == 0 {
                return None;
            }
            let (indent, branch) = match next_entry {
                Some(next_entry) => {
                    let indent = INDENT.repeat(depth_diff - 1);
                    if next_entry.path().components().count() < path_components_count {
                        (indent, BRANCH_LAST)
                    } else {
                        (indent, BRANCH)
                    }
                }
                None => (BRANCH_LAST.repeat(depth_diff - 1), BRANCH_LAST),
            };
            let path_str = path.file_name()?.to_string_lossy();
            let spacer = if entry.file_type().is_dir() {
                " /"
            } else {
                " "
            };
            Some(format!("{indent}{branch}{spacer}{path_str}"))
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    const SIZES_SIZE: usize = 5;
    const SIZES: [&str; SIZES_SIZE] = ["B", "KB", "MB", "GB", "TB"];
    let mut i = 0;
    // if you have this much data... god help you
    #[allow(clippy::cast_precision_loss)]
    let mut size = size as f64;
    while i < SIZES_SIZE && size >= 1024.0 {
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
    if let Some(tree_depth) = args.tree {
        let tree_string = generate_tree_string(&canon_dir, tree_depth);
        println!("{tree_string}");
    }
    println!("{file_count_str} files");
    println!("{size_str}");
}
