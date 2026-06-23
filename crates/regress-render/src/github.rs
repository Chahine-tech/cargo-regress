use regress_core::diff::{group_by_crate, BinaryDiff};

use crate::terminal::fmt_bytes;

pub fn render(diff: &BinaryDiff, from: &str, to: &str) -> String {
    let mut out = String::new();
    let delta = diff.total_delta();
    let pct = diff.total_delta_pct();

    let emoji = if delta > 0 { "🔴" } else if delta < 0 { "🟢" } else { "⚪" };
    let sign = if delta >= 0 { "+" } else { "" };

    out.push_str(&format!(
        "## {emoji} Binary Size: {sign}{} ({sign}{:.1}%)\n\n",
        fmt_bytes(delta),
        pct
    ));
    out.push_str(&format!("`{}` → `{}`\n\n", from, to));

    let growing: Vec<_> = diff.all_growing().cloned().collect();
    if growing.is_empty() {
        out.push_str("No regressions detected.\n");
        return out;
    }

    out.push_str("### Top Regressions\n\n");
    out.push_str("| Crate | Delta | Symbols |\n");
    out.push_str("|-------|-------|---------|\n");

    let groups = group_by_crate(&growing);
    for group in groups.iter().take(10) {
        let symbols_preview: String = group
            .symbols
            .iter()
            .take(2)
            .map(|s| format!("`{}`", &s.demangled[..s.demangled.len().min(60)]))
            .collect::<Vec<_>>()
            .join(", ");

        out.push_str(&format!(
            "| {} | +{} | {} |\n",
            group.name,
            fmt_bytes(group.delta),
            symbols_preview
        ));
    }

    out
}
