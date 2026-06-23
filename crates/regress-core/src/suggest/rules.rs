use serde::{Deserialize, Serialize};

use crate::classify::monomorph::MonomorphGroup;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub text: String,
    pub estimated_savings_bytes: Option<i64>,
}

impl Suggestion {
    fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), estimated_savings_bytes: None }
    }

    fn with_savings(text: impl Into<String>, bytes: i64) -> Self {
        Self { text: text.into(), estimated_savings_bytes: Some(bytes) }
    }
}

pub fn for_crate(crate_name: &str) -> Vec<Suggestion> {
    match crate_name {
        "regex" => vec![
            Suggestion::with_savings(
                r#"Disable unicode feature: regex = { version = "...", default-features = false, features = ["std"] }"#,
                140 * 1024,
            ),
        ],
        "serde_json" | "serde" => vec![
            Suggestion::new(
                "Consider miniserde or nanoserde for simpler types to reduce monomorphization bloat",
            ),
        ],
        "tokio" => vec![
            Suggestion::new(
                r#"Enable only needed features: tokio = { version = "...", features = ["rt", "net"] }"#,
            ),
        ],
        _ => vec![],
    }
}

pub fn for_monomorph(group: &MonomorphGroup) -> Vec<Suggestion> {
    vec![
        Suggestion::with_savings(
            format!(
                "Use the `momo` crate or Box<dyn Fn> to de-duplicate {} instantiations of `{}`",
                group.instantiations.len(),
                group.base_name
            ),
            group.total_delta / 2,
        ),
    ]
}
