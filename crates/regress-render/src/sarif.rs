use std::collections::HashMap;

use regress_core::causal::CausalEntry;
use regress_core::classify;
use regress_core::diff::{BinaryDiff, group_by_crate};
use serde::Serialize;

use crate::terminal::fmt_bytes;

#[derive(Serialize)]
struct SarifRoot {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun>,
}

#[derive(Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifDriver {
    name: &'static str,
    information_uri: &'static str,
    version: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRule {
    id: &'static str,
    name: &'static str,
    short_description: SarifMessage,
    help_uri: &'static str,
}

#[derive(Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: &'static str,
    level: &'static str,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifact,
    region: SarifRegion,
}

#[derive(Serialize)]
struct SarifArtifact {
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRegion {
    start_line: u32,
}

#[cfg(test)]
pub(crate) fn make_test_diff() -> BinaryDiff {
    use regress_core::diff::engine::SymbolDiff;
    BinaryDiff {
        from_total: 1_000_000,
        to_total: 1_250_000,
        added: vec![SymbolDiff {
            name: "hyper::client::connect".to_string(),
            demangled: "hyper::client::connect".to_string(),
            section: ".text".to_string(),
            size_before: 0,
            size_after: 150_000,
            delta: 150_000,
        }],
        removed: vec![],
        grown: vec![SymbolDiff {
            name: "serde::de::Deserialize".to_string(),
            demangled: "serde::de::Deserialize".to_string(),
            section: ".text".to_string(),
            size_before: 10_000,
            size_after: 110_000,
            delta: 100_000,
        }],
        shrunk: vec![],
    }
}

pub fn render(diff: &BinaryDiff, causal: &[CausalEntry]) -> anyhow::Result<String> {
    let growing: Vec<_> = diff.all_growing().cloned().collect();
    let groups = group_by_crate(&growing);

    let causal_map: HashMap<&str, &CausalEntry> =
        causal.iter().map(|e| (e.crate_name.as_str(), e)).collect();

    let results = groups
        .iter()
        .map(|group| {
            let entry = causal_map.get(group.name.as_str()).copied();
            let result = classify::classify_group(group, entry);

            let msg = format!(
                "+{} in crate `{}` [{}, {}]",
                fmt_bytes(group.delta),
                group.name,
                result.category,
                result.confidence_label(),
            );

            let level = if group.delta > 100_000 {
                "error"
            } else {
                "warning"
            };

            let cargo_toml = "Cargo.toml".to_string();

            SarifResult {
                rule_id: "binary-size-regression",
                level,
                message: SarifMessage { text: msg },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifact { uri: cargo_toml },
                        region: SarifRegion { start_line: 1 },
                    },
                }],
            }
        })
        .collect();

    let root = SarifRoot {
        schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        version: "2.1.0",
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "cargo-regress",
                    information_uri: "https://github.com/Chahine-tech/cargo-regress",
                    version: env!("CARGO_PKG_VERSION"),
                    rules: vec![SarifRule {
                        id: "binary-size-regression",
                        name: "BinarySizeRegression",
                        short_description: SarifMessage {
                            text: "Binary size regression detected in a crate".to_string(),
                        },
                        help_uri: "https://github.com/Chahine-tech/cargo-regress",
                    }],
                },
            },
            results,
        }],
    };

    Ok(serde_json::to_string_pretty(&root)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sarif_is_valid_json() {
        let diff = make_test_diff();
        let out = render(&diff, &[]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).expect("SARIF must be valid JSON");
        assert_eq!(v["version"], "2.1.0");
    }

    #[test]
    fn sarif_has_results() {
        let diff = make_test_diff();
        let out = render(&diff, &[]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let results = &v["runs"][0]["results"];
        assert!(results.as_array().unwrap().len() >= 1);
    }

    #[test]
    fn sarif_result_references_cargo_toml() {
        let diff = make_test_diff();
        let out = render(&diff, &[]).unwrap();
        assert!(out.contains("Cargo.toml"));
    }

    #[test]
    fn sarif_large_regression_is_error_level() {
        let diff = make_test_diff();
        let out = render(&diff, &[]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let results = v["runs"][0]["results"].as_array().unwrap();
        assert!(results.iter().any(|r| r["level"] == "error"));
    }
}
