use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use owo_colors::OwoColorize;
use regress_core::causal::CausalCause;
use regress_core::classify::{self, BloatCategory};
use regress_core::diff::group_by_crate;
use regress_core::suggest;
use regress_core::{CausalEntry, binary, diff};

use crate::build;
use crate::cli::DiffArgs;
use regress_render::terminal::fmt_bytes;

use crate::commands::diff::{build_causal, read_lock_diff};

pub fn run(symbol: &str, args: &DiffArgs, repo: &Path) -> Result<()> {
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

    let lock_diff = read_lock_diff(wt_from.root(), wt_to.root());
    let causal_entries = build_causal(&binary_diff, &lock_diff, wt_to.root());

    let query = symbol.to_lowercase();
    let growing: Vec<_> = binary_diff
        .all_growing()
        .filter(|s| s.demangled.to_lowercase().contains(&query))
        .cloned()
        .collect();

    if growing.is_empty() {
        eprintln!(
            "No growing symbol matching {:?} between {} and {}.",
            symbol, args.from, args.to
        );
        return Ok(());
    }

    let causal_map: HashMap<&str, &CausalEntry> = causal_entries
        .iter()
        .map(|e| (e.crate_name.as_str(), e))
        .collect();

    println!();
    println!("{} {}", "Explaining:".bold(), symbol.yellow());
    println!(
        "{} {} → {}",
        "Commits:".dimmed(),
        args.from.dimmed(),
        args.to.dimmed()
    );
    println!("{}", "─".repeat(60).dimmed());

    let groups = group_by_crate(&growing);
    for (i, group) in groups.iter().enumerate() {
        if i > 0 {
            println!("{}", "─".repeat(40).dimmed());
        }
        println!();

        let entry = causal_map.get(group.name.as_str()).copied();
        let result = classify::classify_group(group, entry);

        println!(
            "{}  {}   {} {}",
            "Crate:".bold(),
            group.name,
            format!("+{}", fmt_bytes(group.delta)).red().bold(),
            format!("[{}] ({})", result.category, result.confidence_label()).dimmed()
        );

        if let Some(e) = entry {
            match &e.cause {
                CausalCause::NewDependency { version } => {
                    println!("{} new dependency ({})", "Cause:".bold(), version);
                }
                CausalCause::VersionBump { from, to } => {
                    println!("{} version bump {} → {}", "Cause:".bold(), from, to);
                }
                _ => println!("{} existing dep, symbol growth", "Cause:".bold()),
            }
            if e.import_path.len() > 1 {
                println!("{} {}", "Import:".bold(), e.import_path.join(" → "));
            }
            if !e.active_features.is_empty() {
                println!("{} [{}]", "Features:".bold(), e.active_features.join(", "));
            }
        }

        if result.category == BloatCategory::Monomorphization {
            if let Some(ref mono) = result.mono_group {
                println!(
                    "{} {} instantiations of `{}`",
                    "Mono:".bold(),
                    mono.instantiation_count,
                    mono.base_name
                );
            }
        }

        println!();
        println!("{}", "Symbols:".bold());
        for sym in &group.symbols {
            println!(
                "  {} {} (+{})",
                "│".dimmed(),
                sym.demangled,
                fmt_bytes(sym.delta)
            );
        }
        println!();

        // Suggestions
        let mut printed_header = false;
        let print_header = |printed: &mut bool| {
            if !*printed {
                println!("{}", "Suggestions:".bold());
                *printed = true;
            }
        };

        if let Some(ref mono) = result.mono_group {
            for s in
                suggest::for_monomorph(&mono.base_name, mono.instantiation_count, mono.total_delta)
            {
                print_header(&mut printed_header);
                println!("  {} {}", "→".yellow(), s.text);
                if let Some(saving) = s.estimated_savings_bytes {
                    println!("    Estimated saving: {}", fmt_bytes(saving).green());
                }
            }
        }
        for s in suggest::for_crate(&group.name) {
            print_header(&mut printed_header);
            println!("  {} {}", "→".yellow(), s.text);
            if let Some(saving) = s.estimated_savings_bytes {
                println!("    Estimated saving: {}", fmt_bytes(saving).green());
            }
        }
    }

    println!();
    Ok(())
}
