use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
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
