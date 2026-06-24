use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use git2::Repository;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};

use crate::build;
use crate::cli::WatchArgs;
use regress_render::terminal::fmt_bytes;

#[derive(Debug, Serialize, Deserialize)]
struct WatchEntry {
    timestamp: u64,
    sha: String,
    branch: String,
    binary: String,
    size_bytes: u64,
}

pub fn run(args: &WatchArgs, repo: &Path) -> Result<()> {
    if args.show {
        let history_path = history_path(repo)?;
        let history = load_history(&history_path)?;
        if history.is_empty() {
            eprintln!("No history yet. Run `cargo regress watch` to start recording.");
        } else {
            display_history(&history);
        }
        return Ok(());
    }

    loop {
        record_once(args, repo)?;

        if args.interval == 0 {
            break;
        }

        eprintln!(
            "{}",
            format!("⏱  Next build in {}s — Ctrl-C to stop.", args.interval).dimmed()
        );
        std::thread::sleep(std::time::Duration::from_secs(args.interval));
    }

    Ok(())
}

fn record_once(args: &WatchArgs, repo: &Path) -> Result<()> {
    let sha = build::resolve_commit(repo, "HEAD")?;
    let branch = current_branch(repo).unwrap_or_else(|_| "HEAD".to_string());

    eprintln!("▶ Building HEAD ({})…", &sha[..8]);
    let wt = build::Worktree::create(repo, &sha)?;
    let bin_path = wt.build_release(args.bin.as_deref(), args.lib)?;

    let size_bytes = std::fs::metadata(&bin_path)
        .with_context(|| format!("Cannot stat {}", bin_path.display()))?
        .len();

    let binary_name = args
        .bin
        .clone()
        .or_else(|| {
            bin_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(String::from)
        })
        .unwrap_or_else(|| "binary".to_string());

    let entry = WatchEntry {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        sha: sha[..8].to_string(),
        branch: branch.clone(),
        binary: binary_name,
        size_bytes,
    };

    let history_path = history_path(repo)?;
    std::fs::create_dir_all(history_path.parent().unwrap())?;

    let line = serde_json::to_string(&entry)? + "\n";
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&history_path)?
        .write_all(line.as_bytes())?;

    eprintln!(
        "✔ Recorded {} at {} ({}).",
        fmt_bytes(size_bytes as i64),
        &sha[..8],
        branch
    );

    let history = load_history(&history_path)?;
    display_history(&history);

    Ok(())
}

fn current_branch(repo: &Path) -> Result<String> {
    let repo = Repository::open(repo)?;
    let head = repo.head()?;
    Ok(head.shorthand().unwrap_or("HEAD").to_string())
}

fn history_path(repo: &Path) -> Result<PathBuf> {
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
        .join("watch")
        .join(format!("{slug}.jsonl")))
}

fn load_history(path: &Path) -> Result<Vec<WatchEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    Ok(content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

fn display_history(history: &[WatchEntry]) {
    if history.is_empty() {
        return;
    }

    println!();
    println!("{}", "Size History".bold());
    println!("{}", "─".repeat(56).dimmed());
    println!(
        "{:<13}  {:<8}  {:<10}  {:<12}  {}",
        "When".dimmed(),
        "Commit".dimmed(),
        "Branch".dimmed(),
        "Size".dimmed(),
        "Delta".dimmed()
    );
    println!("{}", "─".repeat(56).dimmed());

    // Show newest-first, up to 10 entries
    let to_show: Vec<(usize, &WatchEntry)> = history.iter().enumerate().rev().take(10).collect();

    for (hist_idx, entry) in &to_show {
        let delta = if *hist_idx > 0 {
            let prev = history[hist_idx - 1].size_bytes as i64;
            let d = entry.size_bytes as i64 - prev;
            if d > 0 {
                format!("+{}", fmt_bytes(d)).red().to_string()
            } else if d < 0 {
                fmt_bytes(d).green().to_string()
            } else {
                "±0".to_string()
            }
        } else {
            "—".dimmed().to_string()
        };

        println!(
            "{:<13}  {:<8}  {:<10}  {:<12}  {}",
            format_timestamp(entry.timestamp),
            entry.sha,
            truncate(&entry.branch, 10),
            fmt_bytes(entry.size_bytes as i64),
            delta
        );
    }
    println!();
}

fn format_timestamp(ts: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let ago = now.saturating_sub(ts);
    if ago < 60 {
        format!("{ago}s ago")
    } else if ago < 3600 {
        format!("{}m ago", ago / 60)
    } else if ago < 86400 {
        format!("{}h ago", ago / 3600)
    } else {
        format!("{}d ago", ago / 86400)
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
