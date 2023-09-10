#![warn(clippy::all, clippy::pedantic)]

use std::{
    iter,
    num::{IntErrorKind, ParseIntError},
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use clap::Parser;
use itertools::Itertools;
use num_format::{Locale, ToFormattedString};
use rayon::prelude::{ParallelBridge, ParallelIterator};

/// dsz, short for directory size, does as its name suggests: it calculates the size of a directory by
/// summing the sizes of all files in it. dsz can also generate a visual tree of the directory,
/// given you're on a terminal that supports unicode.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The directory to calculate the size of.
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Display the directory tree, up to <TREE> depth. [default: 1]
    #[arg(short, long, value_parser = check_tree_depth, num_args = 0..=1, require_equals = true, default_missing_value = "1")]
    tree: Option<usize>,
    /// Exclude hidden files from the tree. (ignored if --tree is not specified)
    #[arg(short, long)]
    no_hidden: bool,
}

/// Checks if a string can be parsed into a `usize` and is greater than 1, returning an error if it can't.
///
/// This function is used as a value parser for clap and is not meant to be used directly, but the point
/// of it is to provide a more helpful error message than the default messages.
///
/// # Arguments
///
/// * `s` - The string to parse.
///
/// # Returns
///
/// The parsed value, or an error if the string couldn't be parsed or the value was less than 1.
fn check_tree_depth(s: &str) -> Result<usize, String> {
    // insane? yes. does it work? also yes, as long as your computer doesn't explode.
    const TREE_RANGE: RangeInclusive<usize> = 1..=usize::MAX;
    s.parse()
        .map_err(|err: ParseIntError| match err.kind() {
            IntErrorKind::Empty => {
                "No value provided. Either provide a value or remove the '=' from the flag.".into()
            }
            IntErrorKind::PosOverflow => {
                format!("Tree depth must be less than {}", TREE_RANGE.end())
            }
            IntErrorKind::InvalidDigit => {
                // I could just check the first character, but this way gives a more helpful error message
                if let Ok(digit) = s.parse::<i64>() {
                    if digit < 0 {
                        return "Negative values are not allowed".into();
                    }
                };
                err.to_string()
            }
            _ => err.to_string(),
        })
        .and_then(|depth| {
            if TREE_RANGE.contains(&depth) {
                Ok(depth)
            } else {
                Err(format!(
                    "Tree depth must be greater than {}",
                    TREE_RANGE.start()
                ))
            }
        })
}

/// Generates a tree of the directory, up to the specified depth. This function is not parallelized.
///
/// # Arguments
///
/// * `root` - The directory to generate the tree of.
/// * `depth` - The depth of the tree to generate.
/// * `no_hidden` - Whether to include hidden files in the tree.
///
/// # Returns
/// The tree as a string.
fn generate_tree_string(root: &Path, depth: usize, no_hidden: bool) -> String {
    const INDENT: &str = "│   ";
    const BRANCH: &str = "├───";
    const BRANCH_LAST: &str = "└───";

    // sorts by directories first, then by name
    let walker = walkdir::WalkDir::new(root)
        .sort_by(|a, b| {
            b.path()
                .is_dir()
                .cmp(&a.path().is_dir())
                .then_with(|| a.file_name().cmp(b.file_name()))
        })
        .max_depth(depth)
        .into_iter()
        .filter_entry(|e| !no_hidden || e.file_name().to_str().is_some_and(|s| !s.starts_with('.')))
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
                return Some(path.display().to_string());
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
///
/// # Returns
///
/// A tuple containing the size (in bytes) and the number of files.
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
    // TODO: better error handling, this is just a quick and dirty solution
    if !args.dir.exists() {
        eprintln!("error: {} does not exist", args.dir.display());
        std::process::exit(1);
    } else if !args.dir.is_dir() {
        eprintln!("error: {} is not a directory", args.dir.display());
        std::process::exit(1);
    }
    let canon_dir =
        dunce::canonicalize(args.dir).expect("A fatal error occurred while reading the directory");
    // TODO: symbols
    let mut sp = spinners::Spinner::new(spinners::Spinners::Point, "Calculating size...".into());
    let (size, file_count) = parallel_dir_size(&canon_dir);
    sp.stop_with_message("Calculated size!".into());
    let size_str = size_in_bytes_pretty_string(size);
    let file_count_str = file_count.to_formatted_string(&Locale::en);
    if let Some(tree_depth) = args.tree {
        let mut sp = spinners::Spinner::new(spinners::Spinners::Point, "Generating tree...".into());
        let tree_string = generate_tree_string(&canon_dir, tree_depth, args.no_hidden);
        sp.stop_with_message("Generated tree!".into());
        println!("{tree_string}");
    } else {
        println!("{}", canon_dir.display());
    }
    println!("{file_count_str} files evaluated");
    println!("{size_str}");
}
