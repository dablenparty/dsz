use std::{
    num::{IntErrorKind, ParseIntError},
    ops::RangeInclusive,
    path::PathBuf,
};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum, ValueHint};

/// dsz, short for directory size, does as its name suggests: it calculates the size of a directory by
/// summing the sizes of all files in it. dsz can also generate a visual tree of the directory,
/// given you're on a terminal that supports unicode.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    /// The path to calculate the size of.
    #[arg(default_value = ".", value_hint = ValueHint::AnyPath, value_parser = path_arg_validator)]
    pub path: PathBuf,
    /// Show the size of the directory in bytes.
    #[arg(short = 'b', long)]
    pub show_bytes: bool,
    /// An optional subcommand.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Tree(TreeArgs),
}

fn path_arg_validator(s: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(s);
    if path.exists() {
        dunce::canonicalize(path)
            .context("Failed to canonicalize path")
            .map_err(|e| e.to_string())
    } else {
        Err(format!("Path '{s}' does not exist"))
    }
}

/// Displays a visual tree of the directory, up to the specified depth. WARNING: this may be slow
#[derive(clap::Args, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[allow(clippy::module_name_repetitions)]
pub struct TreeArgs {
    /// The depth of the tree to generate
    #[arg(short, long, value_hint = ValueHint::Other, value_parser = tree_depth_validator, num_args = 1, require_equals = true, default_value = "1")]
    pub depth: usize,
    /// Exclude hidden files from the tree.
    #[arg(short, long)]
    pub no_hidden: bool,
    /// Show file size in the tree.
    #[arg(short = 'i', long)]
    pub show_size: bool,
    /// Sort the tree by name (A-Z), file size (big->small), or file date (newest->oldest)
    #[arg(short, long = "sort", value_hint = ValueHint::Other, default_value = "name")]
    pub sort_type: SortType,
    /// Reverse the sorting order.
    #[arg(short, long = "reverse")]
    pub reverse_sort: bool,
}

/// Represents the sorting type for the tree.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, ValueEnum)]
pub enum SortType {
    Name,
    Size,
    #[value(name = "modified")]
    ModifiedDate,
    #[value(name = "created")]
    CreatedDate,
}

impl Default for SortType {
    fn default() -> Self {
        Self::Name
    }
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
#[allow(clippy::module_name_repetitions)]
pub fn tree_depth_validator(s: &str) -> Result<usize, String> {
    // insane? yes. does it work? also yes, as long as your computer doesn't explode.
    const TREE_RANGE: RangeInclusive<usize> = 1..=usize::MAX;
    s.parse()
        .map_err(|err: ParseIntError| match err.kind() {
            IntErrorKind::Empty => {
                "No value provided. Either provide a value with = or remove the flag.".into()
            }
            IntErrorKind::PosOverflow => {
                format!("Depth must be less than {}", TREE_RANGE.end())
            }
            IntErrorKind::InvalidDigit => {
                // I could just check the first character, but this way gives a more helpful error message
                if let Ok(digit) = s.parse::<i64>() {
                    if digit < 0 {
                        return "Negative depth values are not allowed".into();
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
                    "Depth must be greater than or equal to {}",
                    TREE_RANGE.start()
                ))
            }
        })
}
