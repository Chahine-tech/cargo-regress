use std::collections::HashMap;

use regress_core::causal::CausalEntry;
use regress_core::classify;
use regress_core::diff::{BinaryDiff, group_by_crate};
use serde::Serialize;

use crate::terminal::fmt_bytes;

static TEMPLATE: &str = include_str!("html_template.html");

#[derive(Serialize)]
struct TreeNode {
    name: String,
    delta: i64,
    category: String,
    confidence: String,
    children: Vec<SymbolNode>,
}

#[derive(Serialize)]
struct SymbolNode {
    name: String,
    delta: i64,
}

pub fn render(
    diff: &BinaryDiff,
    causal: &[CausalEntry],
    from: &str,
    to: &str,
) -> anyhow::Result<String> {
    let growing: Vec<_> = diff.all_growing().cloned().collect();
    let groups = group_by_crate(&growing);

    let causal_map: HashMap<&str, &CausalEntry> =
        causal.iter().map(|e| (e.crate_name.as_str(), e)).collect();

    let nodes: Vec<TreeNode> = groups
        .iter()
        .map(|group| {
            let entry = causal_map.get(group.name.as_str()).copied();
            let result = classify::classify_group(group, entry);

            let children: Vec<SymbolNode> = group
                .symbols
                .iter()
                .take(50)
                .map(|s| SymbolNode {
                    name: s.demangled.clone(),
                    delta: s.delta,
                })
                .collect();

            TreeNode {
                name: group.name.clone(),
                delta: group.delta,
                category: result.category.to_string(),
                confidence: result.confidence_label().to_string(),
                children,
            }
        })
        .collect();

    let data_json = serde_json::to_string(&nodes)?;
    let total_delta = fmt_bytes(diff.total_delta());
    let pct = format!("{:.1}", diff.total_delta_pct());
    let sign = if diff.total_delta() >= 0 { "+" } else { "" };
    let cls = if diff.total_delta() >= 0 {
        "grow"
    } else {
        "shrink"
    };

    let html = TEMPLATE
        .replace("__FROM__", from)
        .replace("__TO__", to)
        .replace("__CLS__", cls)
        .replace("__SIGN__", sign)
        .replace("__DELTA__", &total_delta)
        .replace("__PCT__", &pct)
        .replace("__DATA_JSON__", &data_json);

    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sarif::make_test_diff;

    #[test]
    fn html_is_valid_html_document() {
        let diff = make_test_diff();
        let out = render(&diff, &[], "HEAD~1", "HEAD").unwrap();
        assert!(out.contains("<!DOCTYPE html>"));
        assert!(out.contains("</html>"));
    }

    #[test]
    fn html_contains_from_to_labels() {
        let diff = make_test_diff();
        let out = render(&diff, &[], "abc1234", "def5678").unwrap();
        assert!(out.contains("abc1234"));
        assert!(out.contains("def5678"));
    }

    #[test]
    fn html_embeds_json_data() {
        let diff = make_test_diff();
        let out = render(&diff, &[], "HEAD~1", "HEAD").unwrap();
        // The treemap data should contain crate names
        assert!(out.contains("hyper") || out.contains("serde"));
    }

    #[test]
    fn html_no_unfilled_placeholders() {
        let diff = make_test_diff();
        let out = render(&diff, &[], "HEAD~1", "HEAD").unwrap();
        assert!(!out.contains("__FROM__"));
        assert!(!out.contains("__TO__"));
        assert!(!out.contains("__DATA_JSON__"));
    }
}
