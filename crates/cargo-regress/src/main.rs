mod build;
mod cli;
mod commands;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

fn main() -> Result<()> {
    // When invoked as `cargo regress`, cargo passes "regress" as argv[1].
    // We skip it so clap sees the actual args.
    let args: Vec<String> = {
        let mut a: Vec<String> = std::env::args().collect();
        if a.get(1).map(String::as_str) == Some("regress") {
            a.remove(1);
        }
        a
    };

    let cli = Cli::parse_from(args);
    let repo = build::find_repo_root()?;

    match cli.command {
        Some(Command::Explain { symbol }) => commands::explain::run(&symbol)?,
        Some(Command::Watch) => commands::watch::run()?,
        Some(Command::Snapshot { .. }) => {
            eprintln!("cargo regress snapshot is not yet implemented (v0.3)");
        }
        None => commands::diff::run(&cli.diff, &repo)?,
    }

    Ok(())
}
