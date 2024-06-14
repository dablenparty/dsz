#![cfg(feature = "completion")]

use clap::CommandFactory;
use clap_complete::shells;

fn main() -> anyhow::Result<()> {
    const PKG_NAME: &str = env!("CARGO_PKG_NAME");
    const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

    // TODO: support other shells (the enum already has impl FromStr)

    println!("# {PKG_NAME} {PKG_VERSION} completion script for Zsh");

    let mut cmd = cli::Args::command();

    clap_complete::generate(shells::Zsh, &mut cmd, PKG_NAME, &mut std::io::stdout());

    Ok(())
}
