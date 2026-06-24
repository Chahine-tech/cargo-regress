use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "cargo-regress",
    bin_name = "cargo regress",
    about = "Binary size regression analysis between git commits",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub diff: DiffArgs,
}

#[derive(Subcommand)]
pub enum Command {
    /// Explain a specific symbol in detail
    Explain {
        /// Demangled symbol name to explain
        symbol: String,
    },
    /// Interactive TUI for exploring regressions
    Tui,
    /// Record current binary size to local history and show trend
    Watch {
        #[command(flatten)]
        watch: WatchArgs,
    },
    /// Show a size snapshot of the current binary (like cargo-bloat)
    Snapshot {
        #[command(flatten)]
        snapshot: SnapshotArgs,
    },
}

#[derive(Args, Clone)]
pub struct WatchArgs {
    /// Specific binary to record (workspace)
    #[arg(long)]
    pub bin: Option<String>,

    /// Record the library instead of a binary
    #[arg(long)]
    pub lib: bool,

    /// Display history without building
    #[arg(long)]
    pub show: bool,
}

#[derive(Args, Clone)]
pub struct SnapshotArgs {
    /// Specific binary to analyse (workspace)
    #[arg(long)]
    pub bin: Option<String>,

    /// Analyse the library instead of a binary
    #[arg(long)]
    pub lib: bool,

    /// Number of top crates to display
    #[arg(long, default_value_t = 20)]
    pub top: usize,
}

#[derive(Args, Clone)]
pub struct DiffArgs {
    /// Starting commit/tag/branch (default: HEAD~1)
    #[arg(long, default_value = "HEAD~1")]
    pub from: String,

    /// Ending commit/tag/branch (default: HEAD)
    #[arg(long, default_value = "HEAD")]
    pub to: String,

    /// Specific binary to analyse (workspace)
    #[arg(long)]
    pub bin: Option<String>,

    /// Analyse the library instead of a binary
    #[arg(long)]
    pub lib: bool,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
    pub format: OutputFormat,

    /// Fail with exit code 1 if regression exceeds threshold (e.g. +100kb, +1mb)
    #[arg(long)]
    pub fail_on: Option<String>,
}

#[derive(Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    Terminal,
    Json,
    Github,
}
