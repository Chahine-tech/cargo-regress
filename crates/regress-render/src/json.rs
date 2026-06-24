use std::collections::HashMap;

use anyhow::Result;
use regress_core::causal::{CausalCause, CausalEntry};
use regress_core::classify::{self, MonomorphSummary};
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
    pub profile_suggestions: Vec<String>,
}

#[derive(Serialize)]
pub struct RegressionEntry {
    pub crate_name: String,
    pub delta_bytes: i64,
    pub category: String,
    pub confidence: f64,
    pub mono_group: Option<MonomorphSummary>,
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
            let entry = causal_map.get(group.name.as_str()).copied();
            let result = classify::classify_group(group, entry);

            let symbols: Vec<String> =
                group.symbols.iter().take(20).map(|s| s.demangled.clone()).collect();

            let mut suggestions: Vec<String> = Vec::new();
            if let Some(ref mono) = result.mono_group {
                suggestions.extend(
                    suggest::for_monomorph(&mono.base_name, mono.instantiation_count, mono.total_delta)
                        .into_iter()
                        .map(|s| s.text),
                );
            }
            suggestions.extend(suggest::for_crate(&group.name).into_iter().map(|s| s.text));

            let (cause, import_path, active_features) = match entry {
                Some(e) => {
                    let cause = match &e.cause {
                        CausalCause::NewDependency { version } => {
                            Some(CauseJson::NewDependency { version: version.clone() })
                        }
                        CausalCause::VersionBump { from, to } => {
                            Some(CauseJson::VersionBump { from: from.clone(), to: to.clone() })
                        }
                        _ => Some(CauseJson::SymbolGrowth),
                    };
                    (cause, e.import_path.clone(), e.active_features.clone())
                }
                None => (None, vec![], vec![]),
            };

            RegressionEntry {
                crate_name: group.name.clone(),
                delta_bytes: group.delta,
                category: result.category.to_string(),
                confidence: result.confidence,
                mono_group: result.mono_group,
                cause,
                import_path,
                active_features,
                symbols,
                suggestions,
            }
        })
        .collect();

    let has_hidden = groups.iter().any(|g| {
        let entry = causal_map.get(g.name.as_str()).copied();
        classify::classify_group(g, entry).category
            == regress_core::classify::BloatCategory::HiddenData
    });
    let profile_suggestions = suggest::for_build_profile(diff.total_delta(), has_hidden, groups.len())
        .into_iter()
        .map(|s| s.text)
        .collect();

    let report = DiffReport {
        from,
        to,
        from_total_bytes: diff.from_total,
        to_total_bytes: diff.to_total,
        total_delta_bytes: diff.total_delta(),
        total_delta_pct: diff.total_delta_pct(),
        regressions,
        profile_suggestions,
    };

    Ok(serde_json::to_string_pretty(&report)?)
}
