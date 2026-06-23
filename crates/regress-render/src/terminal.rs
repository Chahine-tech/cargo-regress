use owo_colors::OwoColorize;
use regress_core::classify::{self, BloatCategory};
use regress_core::diff::{group_by_crate, BinaryDiff};
use regress_core::suggest;

pub fn render_diff(diff: &BinaryDiff, from: &str, to: &str) {
    let delta = diff.total_delta();
    let pct = diff.total_delta_pct();

    // Header line
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

    let groups = group_by_crate(&growing);
    for group in groups.iter().take(10) {
        let delta_str = format!("+{}", fmt_bytes(group.delta));

        // Classify the dominant symbol in the group
        let category = group
            .symbols
            .first()
            .map(classify::classify)
            .unwrap_or(BloatCategory::Unknown);

        println!(
            "  {}  {}  {}",
            delta_str.red().bold(),
            group.name.bold(),
            format!("[{}]", category).dimmed()
        );

        for sym in group.symbols.iter().take(3) {
            println!("     {} {}", "└─".dimmed(), sym.demangled.dimmed());
        }
        if group.symbols.len() > 3 {
            println!(
                "     {}   … and {} more symbols",
                " ".dimmed(),
                group.symbols.len() - 3
            );
        }

        // Actionable suggestions
        for s in suggest::for_crate(&group.name) {
            println!("     {} {}", "→".yellow(), s.text.yellow());
            if let Some(savings) = s.estimated_savings_bytes {
                println!(
                    "       {} Estimated saving: {}",
                    " ".dimmed(),
                    fmt_bytes(savings).green()
                );
            }
        }

        println!();
    }

    let shrink_total: u64 = diff
        .all_shrinking()
        .map(|s| s.delta.unsigned_abs())
        .sum();

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
