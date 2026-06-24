use std::path::Path;

use anyhow::Result;
use owo_colors::OwoColorize;
use regress_core::binary;
use regress_core::classify::{self, BloatCategory};
use regress_core::diff::{group_by_crate, SymbolDiff};

use crate::build;
use crate::cli::SnapshotArgs;
use regress_render::terminal::fmt_bytes;

pub fn run(args: &SnapshotArgs, repo: &Path) -> Result<()> {
    let sha = build::resolve_commit(repo, "HEAD")?;

    eprintln!("▶ Building HEAD ({})…", &sha[..8]);
    let wt = build::Worktree::create(repo, &sha)?;
    let bin_path = wt.build_release(args.bin.as_deref(), args.lib)?;

    eprintln!("▶ Analysing symbols…");
    let symbols = binary::parse_symbols(&bin_path)?;

    if symbols.is_empty() {
        eprintln!("No symbols found in binary.");
        return Ok(());
    }

    let total: u64 = symbols.iter().map(|s| s.size).sum();

    // Convert SymbolEntry → SymbolDiff (delta = size) to reuse group_by_crate.
    let as_diffs: Vec<SymbolDiff> = symbols
        .iter()
        .map(|s| SymbolDiff {
            name: s.name.clone(),
            demangled: s.demangled.clone(),
            section: s.section.clone(),
            size_before: 0,
            size_after: s.size,
            delta: s.size as i64,
        })
        .collect();

    let groups = group_by_crate(&as_diffs);

    println!();
    println!(
        "{}  {}  {}",
        "Snapshot:".bold(),
        &sha[..8],
        format!("({} symbols)", symbols.len()).dimmed()
    );
    println!("{} {}", "Total:".bold(), fmt_bytes(total as i64).bold());
    println!("{}", "─".repeat(60).dimmed());
    println!();
    println!("{}", "TOP CRATES BY SIZE".bold());
    println!("{}", "━".repeat(60));

    let top = args.top;
    for group in groups.iter().take(top) {
        let result = classify::classify_group(group, None);
        let pct = group.delta as f64 / total as f64 * 100.0;

        let category = if result.category != BloatCategory::Unknown {
            format!("  [{}]", result.category).dimmed().to_string()
        } else {
            String::new()
        };

        println!(
            "  {}  {}{}  {:.1}%",
            fmt_bytes(group.delta).bold(),
            group.name,
            category,
            pct
        );
    }

    if groups.len() > top {
        let rest: i64 = groups[top..].iter().map(|g| g.delta).sum();
        let rest_pct = rest as f64 / total as f64 * 100.0;
        println!(
            "  {}  … and {} more crates  {:.1}%",
            fmt_bytes(rest).dimmed(),
            groups.len() - top,
            rest_pct
        );
    }

    println!();
    Ok(())
}
