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
        let entry = causal_map.get(group.name.as_str()).copied();
        let result = classify::classify_group(group, entry);

        println!(
            "  {}  {}  {}",
            format!("+{}", fmt_bytes(group.delta)).red().bold(),
            group.name.bold(),
            format!("[{}] ({})", result.category, result.confidence_label()).dimmed()
        );

        // Causal lines
        if let Some(e) = entry {
            match &e.cause {
                CausalCause::NewDependency { version } => {
                    println!("     {} new dependency ({})", "●".yellow(), version.dimmed());
                }
                CausalCause::VersionBump { from, to } => {
                    println!(
                        "     {} version bump {} → {}",
                        "●".cyan(),
                        from.dimmed(),
                        to.dimmed()
                    );
                }
                _ => {}
            }
            if e.import_path.len() > 1 {
                println!(
                    "     {} import path: {}",
                    "└─".dimmed(),
                    e.import_path.join(" → ").dimmed()
                );
            }
            if !e.active_features.is_empty() {
                println!(
                    "     {} features: [{}]",
                    "└─".dimmed(),
                    e.active_features.join(", ").dimmed()
                );
            }
        }

        // Monomorphization instantiation detail
        if result.category == BloatCategory::Monomorphization {
            if let Some(ref mono) = result.mono_group {
                println!(
                    "     {} {} instantiations of `{}`",
                    "└─".dimmed(),
                    mono.instantiation_count,
                    mono.base_name.dimmed()
                );
            }
        }

        // Top symbols
        for sym in group.symbols.iter().take(3) {
            println!("     {} {}", "└─".dimmed(), sym.demangled.dimmed());
        }
        if group.symbols.len() > 3 {
            println!("        … and {} more symbols", group.symbols.len() - 3);
        }

        // Suggestions
        if let Some(ref mono) = result.mono_group {
            for s in suggest::for_monomorph(&mono.base_name, mono.instantiation_count, mono.total_delta) {
                print_suggestion(&s);
            }
        }
        for s in suggest::for_crate(&group.name) {
            print_suggestion(&s);
        }

        println!();
    }

    let shrink_total: u64 = diff.all_shrinking().map(|s| s.delta.unsigned_abs()).sum();
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

    // Build-profile suggestions based on the overall regression pattern
    let has_hidden = groups.iter().any(|g| {
        let entry = causal_map.get(g.name.as_str()).copied();
        classify::classify_group(g, entry).category == BloatCategory::HiddenData
    });
    let profile_suggestions =
        suggest::for_build_profile(diff.total_delta(), has_hidden, groups.len());

    if !profile_suggestions.is_empty() {
        println!("{}", "BUILD PROFILE SUGGESTIONS".bold());
        println!("{}", "━".repeat(60).dimmed());
        for s in &profile_suggestions {
            print_suggestion(s);
        }
        println!();
    }

    println!("{}", "Run `cargo regress explain <symbol>` for deeper analysis.".dimmed());
}

fn print_suggestion(s: &suggest::Suggestion) {
    println!("     {} {}", "→".yellow(), s.text.yellow());
    if let Some(savings) = s.estimated_savings_bytes {
        println!("       Estimated saving: {}", fmt_bytes(savings).green());
    }
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
