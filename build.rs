use anyhow::Context;
use clap::CommandFactory;
use clap_complete::shells;

fn main() -> anyhow::Result<()> {
    let out_dir = std::env::var("OUT_DIR").context("OUT_DIR not set")?;
    let mut cmd = cli::Args::command();

    let completion_file_path =
        clap_complete::generate_to(shells::Zsh, &mut cmd, env!("CARGO_PKG_NAME"), out_dir)?;

    if cfg!(debug_assertions) {
        println!("cargo:warning=Completion file written to {completion_file_path:?}");
    }
    println!("cargo:rerun-if-changed={completion_file_path:?}");
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
