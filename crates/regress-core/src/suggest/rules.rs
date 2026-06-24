use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    pub text: String,
    pub estimated_savings_bytes: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RuleFile {
    rule: Vec<RuleEntry>,
}

#[derive(Debug, Deserialize)]
struct RuleEntry {
    #[serde(rename = "crate")]
    crate_name: String,
    suggestion: String,
    estimated_savings_bytes: Option<i64>,
}

fn load_rules() -> Vec<RuleEntry> {
    const BUILTIN: &str = include_str!("rules.toml");

    let mut rules: Vec<RuleEntry> = toml::from_str::<RuleFile>(BUILTIN)
        .expect("built-in rules.toml is malformed")
        .rule;

    if let Some(path) = user_rules_path() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            match toml::from_str::<RuleFile>(&content) {
                Ok(extra) => rules.extend(extra.rule),
                Err(e) => eprintln!("⚠ Could not parse {}: {e}", path.display()),
            }
        }
    }

    rules
}

fn user_rules_path() -> Option<PathBuf> {
    // $HOME on Unix, %USERPROFILE% on Windows.
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|h| {
            PathBuf::from(h)
                .join(".cargo")
                .join("regress")
                .join("rules.toml")
        })
}

pub fn for_crate(crate_name: &str) -> Vec<Suggestion> {
    load_rules()
        .into_iter()
        .filter(|r| r.crate_name == crate_name)
        .map(|r| Suggestion {
            text: r.suggestion,
            estimated_savings_bytes: r.estimated_savings_bytes,
        })
        .collect()
}

pub fn for_monomorph(
    base_name: &str,
    instantiation_count: usize,
    total_delta: i64,
) -> Vec<Suggestion> {
    vec![Suggestion {
        text: format!(
            "Use the `momo` crate or Box<dyn Fn> to de-duplicate {instantiation_count} instantiations of `{base_name}`"
        ),
        estimated_savings_bytes: Some(total_delta / 2),
    }]
}

/// Generic build-profile suggestions triggered by the overall regression pattern.
pub fn for_build_profile(
    total_delta: i64,
    has_hidden_data: bool,
    growing_crate_count: usize,
) -> Vec<Suggestion> {
    let mut out = Vec::new();

    if has_hidden_data {
        out.push(Suggestion {
            text: r#"[profile.release] strip = "debuginfo"  — removes debug sections from binary"#
                .to_string(),
            estimated_savings_bytes: None,
        });
    }

    if total_delta > 50_000 {
        out.push(Suggestion {
            text: r#"[profile.release] panic = "abort"  — removes unwinding tables (~20–50 KB)"#
                .to_string(),
            estimated_savings_bytes: Some(30_000),
        });
    }

    if growing_crate_count >= 3 || total_delta > 100_000 {
        out.push(Suggestion {
            text: r#"[profile.release] lto = "thin"  — enables cross-crate dead code elimination"#
                .to_string(),
            estimated_savings_bytes: None,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_has_suggestion_with_savings() {
        let s = for_crate("regex");
        assert!(!s.is_empty());
        assert!(s[0].estimated_savings_bytes.is_some());
        assert!(s[0].text.contains("default-features"));
    }

    #[test]
    fn serde_has_suggestion() {
        assert!(!for_crate("serde").is_empty());
        assert!(!for_crate("serde_json").is_empty());
    }

    #[test]
    fn unknown_crate_has_no_suggestion() {
        assert!(for_crate("nonexistent_crate_xyz").is_empty());
    }
}
