use std::{cmp::Ordering, io, path::Path};

use chrono::{DateTime, Local};
use cli::{SortType, TreeArgs};
use walkdir::DirEntry;

use crate::{dir_size, size_in_bytes_pretty_string};

/// Sorts two [`walkdir::DirEntry`]s and returns the ordering.
///
/// # Arguments
///
/// * `sort_type` - The type of sorting to use. This determines how the entries are compared.
/// * `a` - The first entry to compare.
/// * `b` - The second entry to compare.
///
/// # Returns
///
/// The ordering of the two entries.
///
/// # Errors
///
/// If an error occurs while getting the metadata of the entries. Sorting with [`SortType::Name`]
/// should never error.
pub fn sort_entries(sort_type: SortType, a: &DirEntry, b: &DirEntry) -> anyhow::Result<Ordering> {
    let ord = match sort_type {
        SortType::Name => a.file_name().cmp(b.file_name()),
        SortType::Size => dir_entry_size(b)?.cmp(&dir_entry_size(a)?),
        SortType::ModifiedDate => b.metadata()?.modified()?.cmp(&a.metadata()?.modified()?),
        SortType::CreatedDate => b.metadata()?.created()?.cmp(&a.metadata()?.created()?),
    };
    Ok(ord)
}

/// Checks if a file is hidden. On Windows, this will read the file attributes. On other platforms,
/// it simply checks if the file name starts with a period.
///
/// # Arguments
///
/// * `path` - The path to check.
///
/// # Returns
///
/// `true` if the file is hidden, `false` otherwise.
///
/// # Errors
///
/// If an error occurs while reading the file attributes (on Windows) or while getting the file name
/// (on other platforms).
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

/// Calculates the size of a [`walkdir::DirEntry`], recursing into directories if necessary.
/// For files, this is just the file length. For directories, it is the recursive size of the directory.
///
/// # Arguments
///
/// * `entry` - The entry to calculate the size of.
///
/// # Returns
///
/// The size of the entry, in bytes.
fn dir_entry_size(entry: &DirEntry) -> walkdir::Result<u64> {
    if entry.file_type().is_dir() {
        dir_size(entry.path()).map(|(size, _)| size)
    } else {
        entry.metadata().map(|m| m.len())
    }
}

/// Generates a tree of the directory, up to the specified depth. This function is not parallelized.
///
/// # Arguments
///
/// * `root` - The directory to generate the tree of.
/// * `args` - The arguments to use when generating the tree.
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
        reverse_sort,
    } = args;

    let mut peekable_tree_iter = walkdir::WalkDir::new(root)
        .sort_by(move |a, b| {
            // sorts by directories first, then by specified sorting
            // if an error happens while sorting, it gets sent to the bottom
            let secondary_ordering =
                sort_entries(sort_type, a, b).unwrap_or(std::cmp::Ordering::Less);
            b.path().is_dir().cmp(&a.path().is_dir()).then_with(|| {
                if reverse_sort {
                    secondary_ordering.reverse()
                } else {
                    secondary_ordering
                }
            })
        })
        .max_depth(depth)
        .into_iter()
        .filter_entry(|e| !no_hidden || !file_is_hidden(e.path()).unwrap_or(false))
        .skip(1) // skip the root
        .filter_map(std::result::Result::ok)
        .peekable();
    let mut branches = Vec::with_capacity(peekable_tree_iter.size_hint().0 + 1);
    branches.push(root.display().to_string());
    while let Some(entry) = peekable_tree_iter.next() {
        let entry_path = entry.path();
        // everything should be canonicalized at this point BUT just in case...
        let file_name = entry_path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("???");
        let path_components_count = entry_path.components().count();
        let depth_diff = path_components_count - root.components().count();
        // +2 for spacer and directory indicator
        let mut string_builder =
            String::with_capacity(depth_diff * INDENT.len() + BRANCH.len() + file_name.len() + 2);
        let next_entry = peekable_tree_iter.peek();
        let (indent, branch) = match next_entry {
            Some(next_entry) => {
                let indent = INDENT.repeat(depth_diff - 1);
                // entry      = /path/to/something
                // next_entry = /path/to
                // this example would be a BRANCH_LAST
                let branch = if next_entry.path().components().count() < path_components_count {
                    BRANCH_LAST
                } else {
                    BRANCH
                };
                (indent, branch)
            }
            None => (BRANCH_LAST.repeat(depth_diff - 1), BRANCH_LAST),
        };
        string_builder.push_str(&indent);
        string_builder.push_str(branch);
        let entry_is_dir = entry.file_type().is_dir();
        let meta = entry.metadata().ok(); // I don't care about the error, only if the metadata exists
        let spacer = if meta.is_some() { ' ' } else { '!' };
        string_builder.push(spacer);
        if entry_is_dir {
            string_builder.push('/');
        };
        string_builder.push_str(file_name);
        // only shows size if it's a file or it's a directory that isn't being expanded
        if depth_diff == depth || !entry_is_dir {
            if show_size {
                let size_str = if entry_is_dir {
                    dir_size(entry_path).map(|(size, _)| size).ok()
                } else {
                    meta.as_ref().map(std::fs::Metadata::len)
                }
                .map_or_else(|| String::from("???"), size_in_bytes_pretty_string);
                string_builder.push_str(&format!(" - {size_str}"));
            }
            let maybe_file_time = match sort_type {
                SortType::ModifiedDate => meta.map(|m| m.modified().ok()),
                SortType::CreatedDate => meta.map(|m| m.created().ok()),
                _ => None,
            };
            if let Some(file_time) = maybe_file_time {
                let date_str = file_time.map(DateTime::<Local>::from).map_or_else(
                    || String::from(" (???)"),
                    |d| format!(" ({})", d.format("%Y-%m-%d %H:%M:%S")),
                );
                string_builder.push_str(&date_str);
            }
        };
        branches.push(string_builder);
    }
    branches.join("\n")
}
