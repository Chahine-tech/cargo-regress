use std::path::Path;

use anyhow::{Context, Result, bail};
use regress_core::{binary, causal, diff};
use regress_render::{github, gitlab, html, json, sarif, terminal};

use crate::build;
use crate::cli::{DiffArgs, OutputFormat};

pub fn run(args: &DiffArgs, repo: &Path) -> Result<()> {
    // File mode: --file-from / --file-to provided — skip git and cargo build.
    match (&args.file_from, &args.file_to) {
        (Some(from), Some(to)) => return run_files(args, from, to),
        (Some(_), None) | (None, Some(_)) => {
            bail!("--file-from and --file-to must be used together");
        }
        (None, None) => {}
    }

    let from_sha = build::resolve_commit(repo, &args.from)?;
    let to_sha = build::resolve_commit(repo, &args.to)?;

    eprintln!("▶ Building {} ({})…", args.from, &from_sha[..8]);
    let wt_from = build::Worktree::create(repo, &from_sha)?;
    let bin_from = wt_from.build_release(args.bin.as_deref(), args.lib)?;

    eprintln!("▶ Building {} ({})…", args.to, &to_sha[..8]);
    let wt_to = build::Worktree::create(repo, &to_sha)?;
    let bin_to = wt_to.build_release(args.bin.as_deref(), args.lib)?;

    eprintln!("▶ Analysing symbols…");
    let syms_from = binary::parse_symbols(&bin_from)?;
    let syms_to = binary::parse_symbols(&bin_to)?;

    let binary_diff = diff::compute_diff(&syms_from, &syms_to);

    // --- Causal attribution ---
    let lock_diff = read_lock_diff(wt_from.root(), wt_to.root());
    let causal_entries = build_causal(&binary_diff, &lock_diff, wt_to.root());

    render_output(args, &binary_diff, &causal_entries, &args.from, &args.to)?;

    if let Some(threshold) = &args.fail_on {
        let limit = parse_threshold(threshold)?;
        if binary_diff.total_delta() > limit {
            eprintln!(
                "Regression exceeds threshold ({} > {})",
                binary_diff.total_delta(),
                limit
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

pub fn read_lock_diff(from_root: &Path, to_root: &Path) -> regress_core::LockDiff {
    let read = |root: &Path| std::fs::read_to_string(root.join("Cargo.lock")).unwrap_or_default();
    causal::diff_lockfiles(&read(from_root), &read(to_root))
}

pub fn build_causal(
    binary_diff: &diff::BinaryDiff,
    lock_diff: &regress_core::LockDiff,
    to_root: &Path,
) -> Vec<regress_core::CausalEntry> {
    let growing: Vec<_> = binary_diff.all_growing().cloned().collect();
    let groups = diff::group_by_crate(&growing);

    let manifest = to_root.join("Cargo.toml");
    let dep_graph = match regress_core::DepGraph::from_manifest(&manifest) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("⚠ Could not load dependency graph: {e}");
            return groups
                .iter()
                .map(|g| regress_core::CausalEntry {
                    crate_name: g.name.clone(),
                    delta: g.delta,
                    cause: regress_core::CausalCause::SymbolGrowth,
                    import_path: vec![],
                    active_features: vec![],
                })
                .collect();
        }
    };

    causal::attribute(&groups, lock_diff, &dep_graph)
}

fn run_files(args: &DiffArgs, from: &std::path::Path, to: &std::path::Path) -> Result<()> {
    let from_label = from.file_name().and_then(|n| n.to_str()).unwrap_or("from");
    let to_label = to.file_name().and_then(|n| n.to_str()).unwrap_or("to");

    eprintln!("▶ Analysing {}…", from.display());
    let syms_from = binary::parse_symbols(from)?;
    eprintln!("▶ Analysing {}…", to.display());
    let syms_to = binary::parse_symbols(to)?;

    let binary_diff = diff::compute_diff(&syms_from, &syms_to);

    render_output(args, &binary_diff, &[], from_label, to_label)?;

    if let Some(threshold) = &args.fail_on {
        let limit = parse_threshold(threshold)?;
        if binary_diff.total_delta() > limit {
            eprintln!(
                "Regression exceeds threshold ({} > {})",
                binary_diff.total_delta(),
                limit
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

fn render_output(
    args: &DiffArgs,
    binary_diff: &diff::BinaryDiff,
    causal_entries: &[regress_core::CausalEntry],
    from_label: &str,
    to_label: &str,
) -> Result<()> {
    match args.format {
        OutputFormat::Terminal => {
            terminal::render_diff(binary_diff, causal_entries, from_label, to_label)
        }
        OutputFormat::Json => {
            let out = json::render(binary_diff, causal_entries, from_label, to_label)?;
            println!("{out}");
        }
        OutputFormat::Github => {
            let out = github::render(binary_diff, causal_entries, from_label, to_label);
            print!("{out}");
        }
        OutputFormat::Sarif => {
            let out = sarif::render(binary_diff, causal_entries)?;
            println!("{out}");
        }
        OutputFormat::Gitlab => {
            let out = gitlab::render(binary_diff, causal_entries)?;
            println!("{out}");
        }
        OutputFormat::Html => {
            let out = html::render(binary_diff, causal_entries, from_label, to_label)?;
            println!("{out}");
        }
    }
    Ok(())
}

fn parse_threshold(s: &str) -> Result<i64> {
    let s = s.trim_start_matches('+');
    if let Some(n) = s.strip_suffix("mb").or_else(|| s.strip_suffix("MB")) {
        return Ok(n.trim().parse::<i64>()? * 1024 * 1024);
    }
    if let Some(n) = s.strip_suffix("kb").or_else(|| s.strip_suffix("KB")) {
        return Ok(n.trim().parse::<i64>()? * 1024);
    }
    s.parse().context("Invalid threshold value")
}
