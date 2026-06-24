use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};

use crate::build;
use crate::cli::{BaselineCmd, OutputFormat};
use regress_core::{binary, diff};
use regress_render::terminal::fmt_bytes;

#[derive(Serialize, Deserialize)]
struct Baseline {
    timestamp: u64,
    sha: String,
    binary: String,
    total_bytes: u64,
    symbols: Vec<SymbolRecord>,
}

#[derive(Serialize, Deserialize)]
struct SymbolRecord {
    name: String,
    demangled: String,
    size: u64,
    section: String,
}

pub fn run(cmd: &BaselineCmd, repo: &Path) -> Result<()> {
    match cmd {
        BaselineCmd::Save { bin, lib } => save(bin.as_deref(), *lib, repo),
        BaselineCmd::Compare {
            bin,
            lib,
            format,
            fail_on,
        } => compare(bin.as_deref(), *lib, *format, fail_on.as_deref(), repo),
    }
}

fn save(bin: Option<&str>, lib: bool, repo: &Path) -> Result<()> {
    let sha = build::resolve_commit(repo, "HEAD")?;

    eprintln!("▶ Building HEAD ({})…", &sha[..8]);
    let wt = build::Worktree::create(repo, &sha)?;
    let bin_path = wt.build_release(bin, lib)?;

    let symbols = binary::parse_symbols(&bin_path)?;
    let total_bytes: u64 = symbols.iter().map(|s| s.size).sum();

    let binary_name = bin
        .map(String::from)
        .or_else(|| {
            bin_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(String::from)
        })
        .unwrap_or_else(|| "binary".to_string());

    let baseline = Baseline {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        sha: sha[..8].to_string(),
        binary: binary_name.clone(),
        total_bytes,
        symbols: symbols
            .iter()
            .map(|s| SymbolRecord {
                name: s.name.clone(),
                demangled: s.demangled.clone(),
                size: s.size,
                section: s.section.clone(),
            })
            .collect(),
    };

    let path = baseline_path(repo, &binary_name)?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, serde_json::to_string_pretty(&baseline)?)?;

    println!(
        "  {} baseline saved ({}, {} symbols, sha {})",
        "✔".green(),
        fmt_bytes(total_bytes as i64),
        baseline.symbols.len(),
        &baseline.sha,
    );

    Ok(())
}

fn compare(
    bin: Option<&str>,
    lib: bool,
    format: OutputFormat,
    fail_on: Option<&str>,
    repo: &Path,
) -> Result<()> {
    let sha = build::resolve_commit(repo, "HEAD")?;

    eprintln!("▶ Building HEAD ({})…", &sha[..8]);
    let wt = build::Worktree::create(repo, &sha)?;
    let bin_path = wt.build_release(bin, lib)?;

    let binary_name = bin
        .map(String::from)
        .or_else(|| {
            bin_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(String::from)
        })
        .unwrap_or_else(|| "binary".to_string());

    let path = baseline_path(repo, &binary_name)?;
    if !path.exists() {
        anyhow::bail!(
            "No baseline found for `{binary_name}`. Run `cargo regress baseline save` first."
        );
    }

    let baseline: Baseline =
        serde_json::from_str(&std::fs::read_to_string(&path)?).context("Corrupt baseline file")?;

    let syms_from: Vec<binary::SymbolEntry> = baseline
        .symbols
        .iter()
        .map(|s| binary::SymbolEntry {
            name: s.name.clone(),
            demangled: s.demangled.clone(),
            size: s.size,
            section: s.section.clone(),
            address: 0,
        })
        .collect();

    eprintln!("▶ Analysing symbols…");
    let syms_to = binary::parse_symbols(&bin_path)?;
    let binary_diff = diff::compute_diff(&syms_from, &syms_to);

    let from_label = format!("baseline@{}", baseline.sha);
    let to_label = &sha[..8];

    use regress_render::{github, gitlab, html, json, sarif, terminal};
    match format {
        OutputFormat::Terminal => terminal::render_diff(&binary_diff, &[], &from_label, to_label),
        OutputFormat::Json => println!(
            "{}",
            json::render(&binary_diff, &[], &from_label, to_label)?
        ),
        OutputFormat::Github => println!(
            "{}",
            github::render(&binary_diff, &[], &from_label, to_label)
        ),
        OutputFormat::Sarif => println!("{}", sarif::render(&binary_diff, &[])?),
        OutputFormat::Gitlab => println!("{}", gitlab::render(&binary_diff, &[])?),
        OutputFormat::Html => println!(
            "{}",
            html::render(&binary_diff, &[], &from_label, to_label)?
        ),
    }

    // Fail threshold check
    if let Some(threshold) = fail_on {
        let limit = parse_bytes(threshold)?;
        if binary_diff.total_delta() > limit {
            anyhow::bail!(
                "Regression {} exceeds threshold {}",
                fmt_bytes(binary_diff.total_delta()),
                fmt_bytes(limit),
            );
        }
    }

    Ok(())
}

fn baseline_path(repo: &Path, binary_name: &str) -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .context("Cannot determine home directory")?;

    let slug = repo
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    Ok(home
        .join(".cargo")
        .join("regress")
        .join("baseline")
        .join(format!("{slug}-{binary_name}.json")))
}

fn parse_bytes(s: &str) -> Result<i64> {
    let s = s.trim().to_lowercase();
    let s = s.trim_start_matches('+');
    if let Some(n) = s.strip_suffix("mb") {
        Ok((n.trim().parse::<f64>()? * 1024.0 * 1024.0) as i64)
    } else if let Some(n) = s.strip_suffix("kb") {
        Ok((n.trim().parse::<f64>()? * 1024.0) as i64)
    } else {
        Ok(s.parse::<i64>()?)
    }
}
