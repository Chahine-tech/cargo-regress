use anyhow::Result;
use regress_core::causal::{CausalCause, CausalEntry};
use regress_core::classify::{self, BloatCategory};
use regress_core::diff::{group_by_crate, BinaryDiff};
use regress_core::suggest;
use serde::Serialize;
use std::collections::HashMap;

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
    pub cause: Option<CauseJson>,
    pub import_path: Vec<String>,
    pub active_features: Vec<String>,
    pub symbols: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CauseJson {
    NewDependency { version: String },
    VersionBump { from: String, to: String },
    SymbolGrowth,
}

pub fn render(diff: &BinaryDiff, causal: &[CausalEntry], from: &str, to: &str) -> Result<String> {
    let growing: Vec<_> = diff.all_growing().cloned().collect();
    let groups = group_by_crate(&growing);

    let causal_map: HashMap<&str, &CausalEntry> =
        causal.iter().map(|e| (e.crate_name.as_str(), e)).collect();

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

            let (cause, import_path, active_features) =
                if let Some(entry) = causal_map.get(group.name.as_str()) {
                    let cause = match &entry.cause {
                        CausalCause::NewDependency { version } => {
                            Some(CauseJson::NewDependency { version: version.clone() })
                        }
                        CausalCause::VersionBump { from, to } => {
                            Some(CauseJson::VersionBump { from: from.clone(), to: to.clone() })
                        }
                        CausalCause::SymbolGrowth => Some(CauseJson::SymbolGrowth),
                        _ => None,
                    };
                    (cause, entry.import_path.clone(), entry.active_features.clone())
                } else {
                    (None, vec![], vec![])
                };

            RegressionEntry {
                crate_name: group.name.clone(),
                delta_bytes: group.delta,
                category,
                cause,
                import_path,
                active_features,
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
