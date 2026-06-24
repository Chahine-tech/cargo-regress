use std::collections::HashMap;

use regress_core::causal::CausalEntry;
use regress_core::classify;
use regress_core::diff::{BinaryDiff, group_by_crate};
use serde::Serialize;

use crate::terminal::fmt_bytes;

#[derive(Serialize)]
struct CodeQualityIssue {
    description: String,
    fingerprint: String,
    severity: &'static str,
    location: Location,
}

#[derive(Serialize)]
struct Location {
    path: &'static str,
    lines: Lines,
}

#[derive(Serialize)]
struct Lines {
    begin: u32,
}

pub fn render(diff: &BinaryDiff, causal: &[CausalEntry]) -> anyhow::Result<String> {
    let growing: Vec<_> = diff.all_growing().cloned().collect();
    let groups = group_by_crate(&growing);

    let causal_map: HashMap<&str, &CausalEntry> =
        causal.iter().map(|e| (e.crate_name.as_str(), e)).collect();

    let issues: Vec<CodeQualityIssue> = groups
        .iter()
        .map(|group| {
            let entry = causal_map.get(group.name.as_str()).copied();
            let result = classify::classify_group(group, entry);

            let description = format!(
                "+{} in crate `{}` [{}, {}]",
                fmt_bytes(group.delta),
                group.name,
                result.category,
                result.confidence_label(),
            );

            let severity = if group.delta > 100_000 {
                "critical"
            } else if group.delta > 50_000 {
                "major"
            } else {
                "minor"
            };

            CodeQualityIssue {
                fingerprint: format!("binary-size:{}", group.name),
                description,
                severity,
                location: Location {
                    path: "Cargo.toml",
                    lines: Lines { begin: 1 },
                },
            }
        })
        .collect();

    Ok(serde_json::to_string_pretty(&issues)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sarif::make_test_diff;

    #[test]
    fn gitlab_is_valid_json_array() {
        let diff = make_test_diff();
        let out = render(&diff, &[]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).expect("GitLab output must be valid JSON");
        assert!(v.is_array());
    }

    #[test]
    fn gitlab_has_fingerprint_and_severity() {
        let diff = make_test_diff();
        let out = render(&diff, &[]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let issues = v.as_array().unwrap();
        assert!(!issues.is_empty());
        assert!(issues[0]["fingerprint"].as_str().unwrap().starts_with("binary-size:"));
        assert!(["critical", "major", "minor"].contains(&issues[0]["severity"].as_str().unwrap()));
    }

    #[test]
    fn gitlab_large_delta_is_critical() {
        let diff = make_test_diff();
        let out = render(&diff, &[]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let issues = v.as_array().unwrap();
        assert!(issues.iter().any(|i| i["severity"] == "critical"));
    }
}
