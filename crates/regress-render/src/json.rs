use anyhow::Result;
use regress_core::classify::{self, BloatCategory};
use regress_core::diff::{group_by_crate, BinaryDiff};
use regress_core::suggest;
use serde::Serialize;

#[derive(Serialize)]
pub struct DiffReport<'a> {
    pub from: &'a str,
    pub to: &'a str,
    pub from_total_bytes: u64,
    pub to_total_bytes: u64,
    pub total_delta_bytes: i64,
    pub total_delta_pct: f64,
    pub regressions: Vec<RegressionEntry>,
}

#[derive(Serialize)]
pub struct RegressionEntry {
    pub crate_name: String,
    pub delta_bytes: i64,
    pub category: String,
    pub symbols: Vec<String>,
    pub suggestions: Vec<String>,
}

pub fn render(diff: &BinaryDiff, from: &str, to: &str) -> Result<String> {
    let growing: Vec<_> = diff.all_growing().cloned().collect();
    let groups = group_by_crate(&growing);

    let regressions = groups
        .iter()
        .map(|group| {
            let category = group
                .symbols
                .first()
                .map(classify::classify)
                .unwrap_or(BloatCategory::Unknown)
                .to_string();

            let symbols: Vec<String> =
                group.symbols.iter().take(20).map(|s| s.demangled.clone()).collect();

            let suggestions: Vec<String> =
                suggest::for_crate(&group.name).into_iter().map(|s| s.text).collect();

            RegressionEntry {
                crate_name: group.name.clone(),
                delta_bytes: group.delta,
                category,
                symbols,
                suggestions,
            }
        })
        .collect();

    let report = DiffReport {
        from,
        to,
        from_total_bytes: diff.from_total,
        to_total_bytes: diff.to_total,
        total_delta_bytes: diff.total_delta(),
        total_delta_pct: diff.total_delta_pct(),
        regressions,
    };

    Ok(serde_json::to_string_pretty(&report)?)
}
