use std::path::Path;

use anyhow::Result;
use regress_core::{binary, diff};
use regress_render::{github, json, terminal};

use crate::build;
use crate::cli::{DiffArgs, OutputFormat};

pub fn run(args: &DiffArgs, repo: &Path) -> Result<()> {
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

    let result = diff::compute_diff(&syms_from, &syms_to);

    match args.format {
        OutputFormat::Terminal => terminal::render_diff(&result, &args.from, &args.to),
        OutputFormat::Json => {
            let out = json::render(&result, &args.from, &args.to)?;
            println!("{out}");
        }
        OutputFormat::Github => {
            let out = github::render(&result, &args.from, &args.to);
            print!("{out}");
        }
    }

    if let Some(threshold) = &args.fail_on {
        let limit = parse_threshold(threshold)?;
        if result.total_delta() > limit {
            eprintln!(
                "Regression exceeds threshold ({} > {})",
                result.total_delta(),
                limit
            );
            std::process::exit(1);
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
    Ok(s.parse()?)
}
