#![cfg(feature = "completion")]

use clap::{CommandFactory, Parser};
use clap_complete::shells;

#[derive(Debug, Parser)]
struct CompletionCli {
    #[arg(required = true)]
    shell: shells::Shell,
}

fn main() -> anyhow::Result<()> {
    const PKG_NAME: &str = env!("CARGO_PKG_NAME");

    let CompletionCli { shell } = CompletionCli::parse();

    let mut cmd = cli::Args::command();

    clap_complete::generate(shell, &mut cmd, PKG_NAME, &mut std::io::stdout());

    Ok(())
}
