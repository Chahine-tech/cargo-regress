use std::collections::HashMap;

use regress_core::causal::{CausalCause, CausalEntry};
use regress_core::classify::{self, BloatCategory};
use regress_core::diff::{BinaryDiff, group_by_crate};
use regress_core::suggest;

use crate::terminal::fmt_bytes;

pub fn render(diff: &BinaryDiff, causal: &[CausalEntry], from: &str, to: &str) -> String {
    let mut out = String::new();
    let delta = diff.total_delta();
    let pct = diff.total_delta_pct();

    let emoji = if delta > 0 {
        "🔴"
    } else if delta < 0 {
        "🟢"
    } else {
        "⚪"
    };
    let sign = if delta >= 0 { "+" } else { "" };

    out.push_str(&format!(
        "## {emoji} Binary Size: {sign}{} ({sign}{:.1}%)\n\n",
        fmt_bytes(delta),
        pct
    ));
    out.push_str(&format!("`{from}` → `{to}`\n\n"));

    let growing: Vec<_> = diff.all_growing().cloned().collect();
    if growing.is_empty() {
        out.push_str("No regressions detected.\n");
        return out;
    }

    let causal_map: HashMap<&str, &CausalEntry> =
        causal.iter().map(|e| (e.crate_name.as_str(), e)).collect();

    let groups = group_by_crate(&growing);

    out.push_str("### Top Regressions\n\n");
    out.push_str("| Crate | Delta | Category | Confidence |\n");
    out.push_str("|-------|-------|----------|------------|\n");

    for group in groups.iter().take(10) {
        let entry = causal_map.get(group.name.as_str()).copied();
        let result = classify::classify_group(group, entry);

        out.push_str(&format!(
            "| `{}` | +{} | {} | {} |\n",
            group.name,
            fmt_bytes(group.delta),
            result.category,
            result.confidence_label()
        ));
    }

    // Detail section: import paths, features, suggestions
    let has_details = groups.iter().take(10).any(|g| {
        let entry = causal_map.get(g.name.as_str()).copied();
        entry
            .map(|e| e.import_path.len() > 1 || !e.active_features.is_empty())
            .unwrap_or(false)
            || !suggest::for_crate(&g.name).is_empty()
    });

    if has_details {
        out.push_str("\n<details>\n<summary>Details &amp; suggestions</summary>\n\n");

        for group in groups.iter().take(10) {
            let entry = causal_map.get(group.name.as_str()).copied();
            let result = classify::classify_group(group, entry);

            out.push_str(&format!("#### `{}`\n\n", group.name));

            if let Some(e) = entry {
                match &e.cause {
                    CausalCause::NewDependency { version } => {
                        out.push_str(&format!("- **Cause:** new dependency ({})\n", version));
                    }
                    CausalCause::VersionBump { from, to } => {
                        out.push_str(&format!("- **Cause:** version bump {} → {}\n", from, to));
                    }
                    _ => {}
                }
                if e.import_path.len() > 1 {
                    out.push_str(&format!(
                        "- **Import path:** {}\n",
                        e.import_path.join(" → ")
                    ));
                }
                if !e.active_features.is_empty() {
                    out.push_str(&format!(
                        "- **Active features:** `{}`\n",
                        e.active_features.join("`, `")
                    ));
                }
            }

            if result.category == BloatCategory::Monomorphization {
                if let Some(ref mono) = result.mono_group {
                    out.push_str(&format!(
                        "- **Monomorphization:** {} instantiations of `{}`\n",
                        mono.instantiation_count, mono.base_name
                    ));
                }
            }

            // Suggestions
            let mono_suggestions = result
                .mono_group
                .as_ref()
                .map(|m| suggest::for_monomorph(&m.base_name, m.instantiation_count, m.total_delta))
                .unwrap_or_default();
            let crate_suggestions = suggest::for_crate(&group.name);
            let all_suggestions: Vec<_> = mono_suggestions
                .iter()
                .chain(crate_suggestions.iter())
                .collect();

            if !all_suggestions.is_empty() {
                out.push_str("\n**Suggestions:**\n\n");
                for s in &all_suggestions {
                    out.push_str(&format!("- {}", s.text));
                    if let Some(saving) = s.estimated_savings_bytes {
                        out.push_str(&format!(" *(~{} saving)*", fmt_bytes(saving)));
                    }
                    out.push('\n');
                }
            }

            out.push('\n');
        }

        out.push_str("</details>\n");
    }

    out
}
