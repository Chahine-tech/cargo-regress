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
    /// Scaffold .cargo-regress.toml and .github/workflows/binary-size.yml
    Init {
        #[command(flatten)]
        init: InitArgs,
    },
    /// Save or compare a binary size baseline (no second commit needed)
    Baseline {
        #[command(subcommand)]
        cmd: BaselineCmd,
    },
}

#[derive(Subcommand, Clone)]
pub enum BaselineCmd {
    /// Save current binary size as baseline
    Save {
        /// Specific binary to save (workspace)
        #[arg(long)]
        bin: Option<String>,
        /// Record the library instead of a binary
        #[arg(long)]
        lib: bool,
    },
    /// Compare current binary against saved baseline
    Compare {
        /// Specific binary to compare (workspace)
        #[arg(long)]
        bin: Option<String>,
        /// Compare the library instead of a binary
        #[arg(long)]
        lib: bool,
        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
        /// Fail if regression exceeds threshold (e.g. +100kb)
        #[arg(long)]
        fail_on: Option<String>,
    },
}

#[derive(Args, Clone)]
pub struct InitArgs {
    /// Binary name to use in generated config (auto-detected from Cargo.toml if omitted)
    #[arg(long)]
    pub bin: Option<String>,

    /// Regression threshold in bytes written to config and workflow
    #[arg(long, default_value_t = 10_000)]
    pub fail_on: u64,

    /// Skip generating the GitHub Actions workflow
    #[arg(long)]
    pub no_github: bool,

    /// Overwrite files that already exist
    #[arg(long)]
    pub force: bool,
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

    /// Rebuild automatically every N seconds (0 = run once)
    #[arg(long, default_value_t = 0)]
    pub interval: u64,
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

    /// Path to a pre-built binary — skips git checkout and cargo build.
    /// Must be paired with --file-to.
    #[arg(long, value_name = "PATH")]
    pub file_from: Option<std::path::PathBuf>,

    /// Path to a pre-built binary — skips git checkout and cargo build.
    /// Must be paired with --file-from.
    #[arg(long, value_name = "PATH")]
    pub file_to: Option<std::path::PathBuf>,

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
    /// SARIF 2.1.0 — upload to GitHub Code Scanning
    Sarif,
    /// GitLab Code Quality JSON — for MR integration
    Gitlab,
    /// Interactive HTML treemap
    Html,
}
