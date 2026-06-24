use std::path::Path;

use anyhow::{Result, bail};

use crate::cli::InitArgs;

pub fn run(args: &InitArgs, repo: &Path) -> Result<()> {
    let bin_name = resolve_bin_name(args, repo)?;

    let config_path = repo.join(".cargo-regress.toml");
    let workflow_dir = repo.join(".github").join("workflows");
    let workflow_path = workflow_dir.join("binary-size.yml");

    let mut created = Vec::new();
    let mut skipped = Vec::new();

    // Config file
    if config_path.exists() && !args.force {
        skipped.push(".cargo-regress.toml");
    } else {
        std::fs::write(&config_path, config_toml(args.fail_on))?;
        created.push(".cargo-regress.toml");
    }

    // GitHub workflow
    if !args.no_github {
        if workflow_path.exists() && !args.force {
            skipped.push(".github/workflows/binary-size.yml");
        } else {
            std::fs::create_dir_all(&workflow_dir)?;
            std::fs::write(&workflow_path, workflow_yaml(&bin_name, args.fail_on))?;
            created.push(".github/workflows/binary-size.yml");
        }
    }

    for f in &created {
        println!("  {} {f}", "created".green());
    }
    for f in &skipped {
        println!("  {} {f}  (use --force to overwrite)", "skipped".dimmed());
    }

    if !created.is_empty() {
        println!();
        println!("{}", "Next steps:".bold());
        println!("  1. Commit the generated files and push to GitHub.");
        println!("  2. Open a pull request — the binary-size check runs automatically.");
        println!("  3. Adjust thresholds in .cargo-regress.toml as needed.");
    }

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn resolve_bin_name(args: &InitArgs, repo: &Path) -> Result<String> {
    if let Some(ref b) = args.bin {
        return Ok(b.clone());
    }

    // Simple line-by-line scan — avoids pulling in a TOML parser just for this.
    // Strategy: after a `[[bin]]` header, grab the first `name = "..."` line.
    // Fall back to `[package]` name if no explicit [[bin]] section exists.
    let cargo_toml_path = repo.join("Cargo.toml");
    if let Ok(content) = std::fs::read_to_string(&cargo_toml_path) {
        let mut in_bin = false;
        let mut in_package = false;
        let mut package_name: Option<String> = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[[bin]]" {
                in_bin = true;
                in_package = false;
                continue;
            }
            if trimmed.starts_with('[') {
                in_package = trimmed == "[package]";
                if trimmed != "[[bin]]" {
                    in_bin = false;
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("name") {
                let rest = rest.trim_start_matches([' ', '=']).trim();
                let name = rest.trim_matches('"').to_string();
                if in_bin {
                    return Ok(name);
                }
                if in_package {
                    package_name = Some(name);
                }
            }
        }
        if let Some(name) = package_name {
            return Ok(name);
        }
    }

    bail!("Could not detect binary name. Pass --bin <name> to specify it explicitly.");
}

fn config_toml(fail_on: u64) -> String {
    format!(
        r#"[defaults]
# Output format: terminal | github | json
format = "terminal"

# Fail CI if total regression exceeds this threshold in bytes (0 = disabled)
fail_on_bytes = {fail_on}

# Binary to analyse — override auto-detection if needed
# bin = "my-binary"
"#
    )
}

fn workflow_yaml(bin_name: &str, fail_on: u64) -> String {
    format!(
        r#"name: Binary Size

on:
  pull_request:
    branches: [main]

jobs:
  binary-size:
    runs-on: ubuntu-latest
    permissions:
      pull-requests: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@v2

      - uses: Chahine-tech/cargo-regress@v0.5.1
        with:
          from: ${{{{ github.event.pull_request.base.sha }}}}
          to: ${{{{ github.event.pull_request.head.sha }}}}
          bin: {bin_name}
          fail-on: "{fail_on}"
          format: github
"#
    )
}

// Pull in owo-colors for terminal output
use owo_colors::OwoColorize;
