use std::{
    iter::once,
    num::{IntErrorKind, ParseIntError},
    ops::RangeInclusive,
    path::Path,
};

use clap::ValueEnum;
use itertools::Itertools;

use crate::{dir_size, size_in_bytes_pretty_string};

/// Represents the sorting type for the tree.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, ValueEnum)]
pub enum SortType {
    Name,
    Size,
    Date,
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
pub fn generate_tree_string(root: &Path, depth: usize, no_hidden: bool, show_size: bool) -> String {
    const INDENT: &str = "│   ";
    const BRANCH: &str = "├───";
    const BRANCH_LAST: &str = "└───";

    // these long and funky iterators are used to make a sliding window of the entries in
    // the walker that guarantees every entry will appear in the left side of the window
    // exactly once. this means that we can use the next entry to determine if the current
    // entry is the last entry in the directory and display the correct branch symbol.
    walkdir::WalkDir::new(root)
        .sort_by(|a, b| {
            // sorts by directories first, then by name
            b.path()
                .is_dir()
                .cmp(&a.path().is_dir())
                .then_with(|| a.file_name().cmp(b.file_name()))
        })
        .max_depth(depth)
        .into_iter()
        .filter_entry(|e| !no_hidden || e.file_name().to_str().is_some_and(|s| !s.starts_with('.')))
        .filter_map(std::result::Result::ok)
        .map(Some)
        .chain(once(None))
        .tuple_windows::<(_, _)>()
        .filter_map(|(entry, next_entry)| {
            let entry = entry?;
            let path = entry.path();
            let path_components_count = path.components().count();
            let depth_diff = path_components_count - root.components().count();
            // the root! show the root!
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
            // everything should be canonicalized at this point BUT just in case...
            let file_name = path.file_name()?.to_string_lossy();
            let entry_is_dir = entry.file_type().is_dir();
            let spacer = if entry_is_dir { " /" } else { " " };
            //? consider only showing the size on a folder if it's at the depth limit
            //? by adding "&& (depth_diff == depth || !entry_is_dir)"
            let size_str = if show_size {
                let size = if entry_is_dir {
                    let (size, _) = dir_size(path);
                    size
                } else {
                    // at this point, the metadata should be readable, but just in case...
                    entry.metadata().map_or(0, |m| m.len())
                };
                format!(" - {}", size_in_bytes_pretty_string(size))
            } else {
                String::new()
            };
            Some(format!("{indent}{branch}{spacer}{file_name}{size_str}"))
        })
        .join("\n")
}
