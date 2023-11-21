#![warn(clippy::all, clippy::pedantic)]

use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueHint};
use num_format::{Locale, ToFormattedString};

mod tree;

/// dsz, short for directory size, does as its name suggests: it calculates the size of a directory by
/// summing the sizes of all files in it. dsz can also generate a visual tree of the directory,
/// given you're on a terminal that supports unicode.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    /// The directory to calculate the size of.
    #[arg(default_value = ".", value_hint = ValueHint::DirPath)]
    dir: PathBuf,
    /// An optional subcommand.
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Tree(tree::TreeArgs),
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
fn dir_size(dir: &Path) -> anyhow::Result<(u64, u64)> {
    // rayon could parallelize this, but it needs par_bridge() and ends up being slower
    // than just doing it sequentially
    let file_sizes: Vec<u64> = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|entry| {
            entry
                .metadata()
                .map(|f| f.len())
                .with_context(|| format!("Error while reading file {}", entry.path().display()))
        })
        .collect::<Result<_, _>>()?;
    // parallelizing this part makes very little difference
    let size = file_sizes.iter().sum();
    Ok((size, file_sizes.len() as u64))
}

/// Makes a string from a size in bytes (up to TB), rounding to the nearest 2 decimal places.
/// If the rounded size has trailing zeros, they are removed (e.g. 1.00 MB -> 1 MB).
///
/// # Arguments
///
/// * `size` - The size in bytes.
///
/// # Returns
///
/// The size as a string, with the appropriate unit.
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
    let size_abbrv = SIZES[i];
    if i == 0 {
        format!("{size} {size_abbrv}")
    } else {
        format!("{size:.2} {size_abbrv}")
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    #[cfg(debug_assertions)]
    println!("{args:?}");
    if !args.dir.is_dir() {
        return Err(anyhow::anyhow!(
            "error: {} is not a directory",
            args.dir.display()
        ));
    }
    let canon_dir =
        dunce::canonicalize(args.dir).context("A fatal error occurred resolving directory path")?;
    // TODO: symbols
    let mut sp = spinners::Spinner::new(spinners::Spinners::Point, "Calculating size...".into());
    let (size, file_count) = dir_size(&canon_dir)?;
    sp.stop_with_message("Calculated size!".into());
    let size_str = size_in_bytes_pretty_string(size);
    let file_count_str = file_count.to_formatted_string(&Locale::en);
    if let Some(cmd) = args.command {
        match cmd {
            Commands::Tree(args) => {
                let mut sp =
                    spinners::Spinner::new(spinners::Spinners::Point, "Generating tree...".into());
                let tree_string = tree::generate_tree_string(&canon_dir, args);
                sp.stop_with_message("Generated tree!".into());
                println!("{tree_string}");
            }
        }
    } else {
        println!("{}", canon_dir.display());
    }
    println!("{file_count_str} files evaluated");
    println!("{size_str}");
    Ok(())
}
