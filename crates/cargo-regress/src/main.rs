mod build;
mod cli;
mod commands;
mod config;

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

    let mut cli = Cli::parse_from(args);
    let repo = build::find_repo_root()?;

    // Apply .cargo-regress.toml defaults (CLI flags take precedence)
    let cfg = config::Config::load(&repo);
    cfg.apply_to_diff(&mut cli.diff);

    match cli.command {
        Some(Command::Explain { symbol }) => commands::explain::run(&symbol, &cli.diff, &repo)?,
        Some(Command::Tui) => commands::tui::run(&cli.diff, &repo)?,
        Some(Command::Watch { watch }) => commands::watch::run(&watch, &repo)?,
        Some(Command::Snapshot { snapshot }) => commands::snapshot::run(&snapshot, &repo)?,
        Some(Command::Init { init }) => commands::init::run(&init, &repo)?,
        Some(Command::Baseline { cmd }) => commands::baseline::run(&cmd, &repo)?,
        None => commands::diff::run(&cli.diff, &repo)?,
    }

    Ok(())
}
