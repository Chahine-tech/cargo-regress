use std::collections::HashMap;

use owo_colors::OwoColorize;
use regress_core::causal::{CausalCause, CausalEntry};
use regress_core::classify::{self, BloatCategory};
use regress_core::diff::{group_by_crate, BinaryDiff};
use regress_core::suggest;

pub fn render_diff(diff: &BinaryDiff, causal: &[CausalEntry], from: &str, to: &str) {
    let delta = diff.total_delta();
    let pct = diff.total_delta_pct();

    if delta > 0 {
        println!(
            "{}",
            format!("Binary size regression: +{} (+{:.1}%)", fmt_bytes(delta), pct)
                .red()
                .bold()
        );
    } else if delta < 0 {
        println!(
            "{}",
            format!("Binary size improvement: {} ({:.1}%)", fmt_bytes(delta), pct)
                .green()
                .bold()
        );
    } else {
        println!("{}", "Binary size unchanged".dimmed());
    }
    println!("{} → {}", from.dimmed(), to.dimmed());
    println!();

    let growing: Vec<_> = diff.all_growing().cloned().collect();

    if growing.is_empty() {
        println!("{}", "No regressions found.".green());
        return;
    }

    println!("{}", "TOP REGRESSIONS".bold());
    println!("{}", "━".repeat(60).dimmed());

    let causal_map: HashMap<&str, &CausalEntry> =
        causal.iter().map(|e| (e.crate_name.as_str(), e)).collect();

    let groups = group_by_crate(&growing);
    for group in groups.iter().take(10) {
        let category = group
            .symbols
            .first()
            .map(classify::classify)
            .unwrap_or(BloatCategory::Unknown);

        let delta_str = format!("+{}", fmt_bytes(group.delta));
        println!(
            "  {}  {}  {}",
            delta_str.red().bold(),
            group.name.bold(),
            format!("[{}]", category).dimmed()
        );

        // Causal block
        if let Some(entry) = causal_map.get(group.name.as_str()) {
            match &entry.cause {
                CausalCause::NewDependency { version } => {
                    println!(
                        "     {} new dependency ({})",
                        "●".yellow(),
                        version.dimmed()
                    );
                }
                CausalCause::VersionBump { from, to } => {
                    println!(
                        "     {} version bump {} → {}",
                        "●".cyan(),
                        from.dimmed(),
                        to.dimmed()
                    );
                }
                CausalCause::SymbolGrowth => {}
                _ => {}
            }

            if entry.import_path.len() > 1 {
                println!(
                    "     {} import path: {}",
                    "└─".dimmed(),
                    entry.import_path.join(" → ").dimmed()
                );
            }

            if !entry.active_features.is_empty() {
                println!(
                    "     {} features: [{}]",
                    "└─".dimmed(),
                    entry.active_features.join(", ").dimmed()
                );
            }
        }

        // Top symbols
        for sym in group.symbols.iter().take(3) {
            println!("     {} {}", "└─".dimmed(), sym.demangled.dimmed());
        }
        if group.symbols.len() > 3 {
            println!(
                "        … and {} more symbols",
                group.symbols.len() - 3
            );
        }

        // Suggestions
        for s in suggest::for_crate(&group.name) {
            println!("     {} {}", "→".yellow(), s.text.yellow());
            if let Some(savings) = s.estimated_savings_bytes {
                println!(
                    "       Estimated saving: {}",
                    fmt_bytes(savings).green()
                );
            }
        }

        println!();
    }

    let shrink_total: u64 =
        diff.all_shrinking().map(|s| s.delta.unsigned_abs()).sum();

    if shrink_total > 0 {
        println!(
            "{}",
            format!(
                "UNCHANGED / REMOVED: -{} saved across {} symbols",
                fmt_bytes(shrink_total as i64),
                diff.removed.len() + diff.shrunk.len()
            )
            .dimmed()
        );
    }

    println!();
    println!("{}", "Run `cargo regress explain <symbol>` for deeper analysis.".dimmed());
}

pub fn fmt_bytes(bytes: i64) -> String {
    let abs = bytes.unsigned_abs();
    if abs >= 1024 * 1024 {
        format!("{:.1} MB", abs as f64 / (1024.0 * 1024.0))
    } else if abs >= 1024 {
        format!("{:.1} KB", abs as f64 / 1024.0)
    } else {
        format!("{} B", abs)
    }
}
