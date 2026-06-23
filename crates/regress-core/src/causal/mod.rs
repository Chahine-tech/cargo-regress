pub mod dep_graph;
pub mod lock_diff;

pub use dep_graph::DepGraph;
pub use lock_diff::{diff as diff_lockfiles, LockDiff};

use serde::{Deserialize, Serialize};

use crate::diff::grouping::CrateGroup;

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CausalCause {
    /// A crate that wasn't in the lock file before.
    NewDependency { version: String },
    /// A crate that changed version between the two commits.
    VersionBump { from: String, to: String },
    /// The crate was already present; its symbols just grew.
    SymbolGrowth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalEntry {
    pub crate_name: String,
    pub delta: i64,
    pub cause: CausalCause,
    /// Import path from a workspace root to this crate, e.g. ["my-app", "some-lib", "regex"].
    pub import_path: Vec<String>,
    /// Features that were active for this crate at the "to" revision.
    pub active_features: Vec<String>,
}

/// Enrich each crate-level regression group with causal information.
pub fn attribute(
    groups: &[CrateGroup],
    lock_diff: &LockDiff,
    dep_graph: &DepGraph,
) -> Vec<CausalEntry> {
    // Build lookup maps from the lock diff.
    // lock_diff.added entries are "name version" strings.
    let added_names: std::collections::HashMap<&str, &str> = lock_diff
        .added
        .iter()
        .filter_map(|s| {
            let mut it = s.splitn(2, ' ');
            Some((it.next()?, it.next()?))
        })
        .collect();

    let bumped: std::collections::HashMap<&str, (&str, &str)> = lock_diff
        .updated
        .iter()
        .map(|(name, from, to)| (name.as_str(), (from.as_str(), to.as_str())))
        .collect();

    groups
        .iter()
        .map(|group| {
            let name = group.name.as_str();

            let cause = if let Some(version) = added_names.get(name) {
                CausalCause::NewDependency { version: version.to_string() }
            } else if let Some((from, to)) = bumped.get(name) {
                CausalCause::VersionBump {
                    from: from.to_string(),
                    to: to.to_string(),
                }
            } else {
                CausalCause::SymbolGrowth
            };

            let import_path = dep_graph.path_to(name).unwrap_or_default();
            let active_features = dep_graph.features_for(name).to_vec();

            CausalEntry {
                crate_name: name.to_string(),
                delta: group.delta,
                cause,
                import_path,
                active_features,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::grouping::CrateGroup;

    fn make_group(name: &str, delta: i64) -> CrateGroup {
        CrateGroup { name: name.to_string(), delta, symbols: vec![] }
    }

    fn graph_with_regex() -> DepGraph {
        DepGraph::from_raw(
            vec!["my-app".to_string()],
            &[("my-app", &["regex"]), ("regex", &[])],
            &[("regex", &["unicode"])],
        )
    }

    #[test]
    fn new_dep_cause_when_in_lock_added() {
        let groups = vec![make_group("regex", 140_000)];
        let lock_diff = LockDiff {
            added: vec!["regex 1.11.0".to_string()],
            removed: vec![],
            updated: vec![],
        };
        let entries = attribute(&groups, &lock_diff, &graph_with_regex());
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            &entries[0].cause,
            CausalCause::NewDependency { version } if version == "1.11.0"
        ));
    }

    #[test]
    fn version_bump_cause_when_in_lock_updated() {
        let groups = vec![make_group("regex", 5_000)];
        let lock_diff = LockDiff {
            added: vec![],
            removed: vec![],
            updated: vec![("regex".to_string(), "1.10.0".to_string(), "1.11.0".to_string())],
        };
        let entries = attribute(&groups, &lock_diff, &graph_with_regex());
        assert!(matches!(
            &entries[0].cause,
            CausalCause::VersionBump { from, to } if from == "1.10.0" && to == "1.11.0"
        ));
    }

    #[test]
    fn symbol_growth_cause_for_existing_dep() {
        let groups = vec![make_group("regex", 2_000)];
        let lock_diff = LockDiff { added: vec![], removed: vec![], updated: vec![] };
        let entries = attribute(&groups, &lock_diff, &graph_with_regex());
        assert!(matches!(&entries[0].cause, CausalCause::SymbolGrowth));
    }

    #[test]
    fn import_path_populated_from_dep_graph() {
        let groups = vec![make_group("regex", 1_000)];
        let lock_diff = LockDiff { added: vec![], removed: vec![], updated: vec![] };
        let entries = attribute(&groups, &lock_diff, &graph_with_regex());
        assert_eq!(entries[0].import_path, vec!["my-app", "regex"]);
    }

    #[test]
    fn active_features_populated_from_dep_graph() {
        let groups = vec![make_group("regex", 1_000)];
        let lock_diff = LockDiff { added: vec![], removed: vec![], updated: vec![] };
        let entries = attribute(&groups, &lock_diff, &graph_with_regex());
        assert_eq!(entries[0].active_features, vec!["unicode"]);
    }
}
