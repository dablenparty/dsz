#![warn(clippy::all, clippy::pedantic)]

use std::{ops::Mul, path::Path};

use cached::proc_macro::cached;
use clap::Parser;
use num_format::{Locale, SystemLocale, ToFormattedStr, ToFormattedString};
use once_cell::sync::Lazy;

mod tree;

/// Computes the size of a directory, returning the size in bytes and the number of files.
/// The results of this function are cached using [`cached`] for performance.
///
/// There is a mild overhead when calculating the initial size, but subsequent calls will
/// be much faster (especially when generating a large tree sorted by size).
///
/// # Arguments
///
/// * `dir` - The directory to calculate the size of.
///
/// # Returns
///
/// A tuple containing the size (in bytes) and the number of files.
#[cached(
    result = true,
    key = "String",
    convert = r##"{ dir.display().to_string() }"##
)]
fn dir_size(dir: &Path) -> walkdir::Result<(u64, u64)> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .map(|entry| entry?.metadata().map(|f| f.len()))
        .try_fold((0u64, 0u64), |(size, count), s| {
            s.map(|s| (size + s, count + 1))
        })
}

/// A locale-aware number formatter made with [`num_format`]. This only supports integer-like types.
/// For floating point types, use [`format_f64`].
///
/// # Arguments
///
/// * `num` - The number to format.
///
/// # Returns
///
/// The formatted number as an owned String.
fn format_number<N>(num: &N) -> String
where
    N: ToFormattedStr,
{
    static SYSTEM_LOCALE: Lazy<SystemLocale> = Lazy::new(|| {
        SystemLocale::default()
            .or_else(|_| SystemLocale::from_name("en_US"))
            .expect("Could not get default system locale or en_US locale")
    });

    let mut buf = num_format::Buffer::default();
    buf.write_formatted(num, &*SYSTEM_LOCALE);
    buf.to_string()
}

/// Like [`format_number`], but for floats.
///
/// # Arguments
///
/// * `num` - The number to format.
/// * `decimal_places` - The number of decimal places to round to.
#[allow(clippy::cast_possible_truncation)]
fn format_f64<F>(num: F) -> String
where
    F: Mul<f64, Output = F> + Into<f64> + Copy,
{
    const DECIMAL_PLACES: i32 = 2;
    let num: f64 = num.into();

    // split into whole and fractional parts
    let whole = num as i64;
    let fract = num.fract();
    // format whole part
    let whole_str = format_number(&whole);
    // extract fractional part
    let fract = fract.mul(10.0_f64.powi(DECIMAL_PLACES)) as i64;
    // format fractional part
    let fract_str = if fract == 0 {
        String::new()
    } else {
        format!(".{fract:0>2}")
    };
    format!("{whole_str}{fract_str}")
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
    let size_str = format_f64(size);
    format!("{size_str} {size_abbrv}")
}

fn main() -> anyhow::Result<()> {
    const SPINNER: spinners::Spinners = spinners::Spinners::Shark;

    let args = cli::Args::parse();
    #[cfg(debug_assertions)]
    println!("{args:?}");
    let canon_dir = args.path;
    // TODO: symbols
    let mut sp = spinners::Spinner::new(SPINNER, "Calculating size...".into());
    let (size, file_count) = dir_size(&canon_dir)?;
    sp.stop_with_message("Calculated size!".into());
    let size_str = size_in_bytes_pretty_string(size);
    let file_count_str = file_count.to_formatted_string(&Locale::en);
    if let Some(cmd) = args.command {
        match cmd {
            cli::Commands::Tree(args) => {
                if canon_dir.is_dir() {
                    let mut sp = spinners::Spinner::new(SPINNER, "Generating tree...".into());
                    let tree_string = tree::generate_tree_string(&canon_dir, args);
                    sp.stop_with_message("Generated tree!".into());
                    println!("{tree_string}");
                } else {
                    println!("No tree generated (not a directory)");
                    println!("{}", canon_dir.display());
                }
            }
        }
    } else {
        println!("{}", canon_dir.display());
    }
    let plural_files = if file_count == 1 { "file" } else { "files" };
    println!("{file_count_str} {plural_files} evaluated");
    println!("{size_str}");
    if args.show_bytes {
        println!("{} bytes", format_number(&size));
    }
    Ok(())
}
