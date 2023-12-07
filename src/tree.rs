use std::{
    io,
    iter::once,
    num::{IntErrorKind, ParseIntError},
    ops::RangeInclusive,
    path::Path,
};

use chrono::{DateTime, Local};
use clap::{Args, ValueEnum};
use itertools::Itertools;

use crate::{dir_size, size_in_bytes_pretty_string};

/// Displays a visual tree of the directory, up to the specified depth. WARNING: this may be slow
#[derive(Args, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[allow(clippy::module_name_repetitions)]
pub struct TreeArgs {
    #[arg(short, long, value_hint = clap::ValueHint::Other, value_parser = tree_depth_validator, num_args = 0..=1, require_equals = true, default_value = "1")]
    /// The depth of the tree to generate
    depth: usize,
    /// Exclude hidden files from the tree.
    #[arg(short, long)]
    no_hidden: bool,
    /// Show file size in the tree.
    #[arg(short = 'i', long)]
    show_size: bool,
    /// Sort the tree by name, file size, or file date.
    #[arg(short, long = "sort", value_hint = clap::ValueHint::Other, default_value = "name")]
    sort_type: SortType,
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

#[cfg(windows)]
fn file_is_hidden(path: &Path) -> io::Result<bool> {
    // adapted from: https://users.rust-lang.org/t/read-windows-hidden-file-attribute/51180/7
    use std::os::windows::fs::MetadataExt;
    let metadata = path.metadata()?;
    let attributes = metadata.file_attributes();

    Ok(attributes & 0x2 > 0)
}

#[cfg(any(unix, not(windows)))]
fn file_is_hidden(path: &Path) -> io::Result<bool> {
    path.file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No file name"))
        .map(|s| s.to_str().is_some_and(|s| s.starts_with('.')))
}

/// Generates a tree of the directory, up to the specified depth. This function is not parallelized.
///
/// # Arguments
///
/// * `root` - The directory to generate the tree of.
/// * `depth` - The depth of the tree to generate.
/// * `sort_type` - The sorting to use.
/// * `no_hidden` - Whether to include hidden files in the tree.
/// * `show_size` - Whether to show the size of files/folders in the tree.
///
/// # Returns
/// The tree as a string.
pub fn generate_tree_string(root: &Path, args: TreeArgs) -> String {
    const INDENT: &str = "│   ";
    const BRANCH: &str = "├───";
    const BRANCH_LAST: &str = "└───";

    // copy args out
    let TreeArgs {
        depth,
        sort_type,
        no_hidden,
        show_size,
    } = args;

    // these long and funky iterators are used to make a sliding window of the entries in
    // the walker that guarantees every entry will appear in the left side of the window
    // exactly once. this means that we can use the next entry to determine if the current
    // entry is the last entry in the directory and display the correct branch symbol.
    let tree_iter = walkdir::WalkDir::new(root)
        .sort_by(move |a, b| {
            // sorts by directories first, then by specified sorting
            // if an error happens while sorting, it gets sent to the bottom
            // I used closures to allow using "?"
            let secondary_ordering = match sort_type {
                SortType::Name => Ok::<_, io::Error>(a.file_name().cmp(b.file_name())),
                SortType::Size => (|| Ok(b.metadata()?.len().cmp(&a.metadata()?.len())))(),
                SortType::ModifiedDate => {
                    (|| Ok(b.metadata()?.modified()?.cmp(&a.metadata()?.modified()?)))()
                }
                SortType::CreatedDate => {
                    (|| Ok(b.metadata()?.created()?.cmp(&a.metadata()?.created()?)))()
                }
            }
            .unwrap_or(std::cmp::Ordering::Less);
            b.path()
                .is_dir()
                .cmp(&a.path().is_dir())
                .then(secondary_ordering)
        })
        .max_depth(depth)
        .into_iter()
        .filter_entry(|e| !no_hidden || !file_is_hidden(e.path()).unwrap_or(false))
        .skip(1)
        .filter_map(std::result::Result::ok)
        .map(Some)
        .chain(once(None))
        .tuple_windows::<(_, _)>()
        .filter_map(|(e, ne)| e.map(|e| (e, ne)))
        .map(|(entry, next_entry)| {
            // TODO: use a vec as a StringBuilder instead of a huge format string
            let entry_path = entry.path();
            let path_components_count = entry_path.components().count();
            let depth_diff = path_components_count - root.components().count();
            let (indent, branch) = match next_entry {
                Some(next_entry) => {
                    let indent = INDENT.repeat(depth_diff - 1);
                    let branch = if next_entry.path().components().count() < path_components_count {
                        BRANCH_LAST
                    } else {
                        BRANCH
                    };
                    (indent, branch)
                }
                None => (BRANCH_LAST.repeat(depth_diff - 1), BRANCH_LAST),
            };
            let entry_is_dir = entry.file_type().is_dir();
            let meta = entry.metadata().ok(); // I don't care about the error, only if the metadata exists
            let dir_slash = if entry_is_dir { "/" } else { "" };
            let spacer = if meta.is_some() { " " } else { "!!" };
            // everything should be canonicalized at this point BUT just in case...
            let file_name = entry_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("???");
            // only shows size if it's a file or it's a directory that isn't being expanded
            let size_str = if show_size && (depth_diff == depth || !entry_is_dir) {
                let size = if entry_is_dir {
                    dir_size(entry_path).map(|(size, _)| size).ok()
                } else {
                    meta.as_ref().map(std::fs::Metadata::len)
                }
                .map_or_else(|| String::from("???"), size_in_bytes_pretty_string);
                format!(" - {size}")
            } else {
                String::new()
            };
            // TODO: simplify this and the one above (check earlier TODO about StringBuilder)
            let date_str = if depth_diff == depth || !entry_is_dir {
                match sort_type {
                    SortType::ModifiedDate => meta.and_then(|m| m.modified().ok()),
                    SortType::CreatedDate => meta.and_then(|m| m.created().ok()),
                    _ => None,
                }
                .map(DateTime::<Local>::from)
                .map_or_else(
                    || String::from("???"),
                    |d| format!(" ({})", d.format("%Y-%m-%d %H:%M:%S")),
                )
            } else {
                String::default()
            };
            format!("{indent}{branch}{spacer}{dir_slash}{file_name}{size_str}{date_str}")
        });
    once(root.display().to_string()).chain(tree_iter).join("\n")
}
