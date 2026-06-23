use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<(String, String, String)>, // (name, from_version, to_version)
}

/// Parse Cargo.lock and extract "name version" pairs.
fn parse_packages(lock_content: &str) -> HashSet<String> {
    let mut packages = HashSet::new();
    let mut in_package = false;
    let mut name = String::new();
    let mut version = String::new();

    for line in lock_content.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            if in_package && !name.is_empty() && !version.is_empty() {
                packages.insert(format!("{name} {version}"));
            }
            in_package = true;
            name.clear();
            version.clear();
        } else if in_package {
            if let Some(v) = line.strip_prefix("name = \"") {
                name = v.trim_end_matches('"').to_string();
            } else if let Some(v) = line.strip_prefix("version = \"") {
                version = v.trim_end_matches('"').to_string();
            }
        }
    }
    if in_package && !name.is_empty() && !version.is_empty() {
        packages.insert(format!("{name} {version}"));
    }
    packages
}

pub fn diff(from_lock: &str, to_lock: &str) -> LockDiff {
    let from = parse_packages(from_lock);
    let to = parse_packages(to_lock);

    let added: Vec<String> = to.difference(&from).cloned().collect();
    let removed: Vec<String> = from.difference(&to).cloned().collect();

    // Detect version changes (same crate name, different version)
    let from_names: std::collections::HashMap<&str, &str> = from
        .iter()
        .filter_map(|s| {
            let mut it = s.splitn(2, ' ');
            Some((it.next()?, it.next()?))
        })
        .collect();

    let mut updated = Vec::new();
    for entry in &to {
        let mut it = entry.splitn(2, ' ');
        if let (Some(name), Some(ver)) = (it.next(), it.next()) {
            if let Some(&old_ver) = from_names.get(name) {
                if old_ver != ver {
                    updated.push((name.to_string(), old_ver.to_string(), ver.to_string()));
                }
            }
        }
    }

    LockDiff { added, removed, updated }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK_A: &str = r#"
[[package]]
name = "foo"
version = "1.0.0"

[[package]]
name = "bar"
version = "0.2.0"
"#;

    const LOCK_B: &str = r#"
[[package]]
name = "foo"
version = "1.0.0"

[[package]]
name = "bar"
version = "0.3.0"

[[package]]
name = "baz"
version = "1.0.0"
"#;

    #[test]
    fn detects_added_package() {
        let d = diff(LOCK_A, LOCK_B);
        assert!(d.added.iter().any(|s| s.contains("baz")));
    }

    #[test]
    fn detects_version_bump() {
        let d = diff(LOCK_A, LOCK_B);
        assert!(d.updated.iter().any(|(name, from, to)| {
            name == "bar" && from == "0.2.0" && to == "0.3.0"
        }));
    }

    #[test]
    fn no_diff_on_identical_locks() {
        let d = diff(LOCK_A, LOCK_A);
        assert!(d.added.is_empty());
        assert!(d.removed.is_empty());
        assert!(d.updated.is_empty());
    }
}
