use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;

pub struct DepGraph {
    roots: Vec<String>,
    id_to_name: HashMap<String, String>,
    adjacency: HashMap<String, Vec<String>>,
    name_to_features: HashMap<String, Vec<String>>,
}

impl DepGraph {
    pub fn from_manifest(manifest_path: &Path) -> Result<Self> {
        let metadata = MetadataCommand::new()
            .manifest_path(manifest_path)
            .exec()
            .with_context(|| format!("cargo metadata failed for {}", manifest_path.display()))?;

        let id_to_name: HashMap<String, String> = metadata
            .packages
            .iter()
            .map(|p| (p.id.repr.clone(), p.name.clone()))
            .collect();

        let resolve = metadata
            .resolve
            .context("cargo metadata returned no resolve graph")?;

        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut name_to_features: HashMap<String, Vec<String>> = HashMap::new();

        for node in &resolve.nodes {
            let deps: Vec<String> = node.deps.iter().map(|d| d.pkg.repr.clone()).collect();
            adjacency.insert(node.id.repr.clone(), deps);

            if let Some(name) = id_to_name.get(&node.id.repr) {
                name_to_features
                    .entry(name.clone())
                    .or_insert_with(|| node.features.clone());
            }
        }

        let roots: Vec<String> = metadata
            .workspace_members
            .iter()
            .filter_map(|id| id_to_name.get(&id.repr))
            .cloned()
            .collect();

        Ok(Self { roots, id_to_name, adjacency, name_to_features })
    }

    /// BFS from workspace roots to `target_name`. Returns the first path found,
    /// as a list of crate names from root to target (inclusive).
    pub fn path_to(&self, target_name: &str) -> Option<Vec<String>> {
        let target_ids: HashSet<&str> = self
            .id_to_name
            .iter()
            .filter(|(_, name)| name.as_str() == target_name)
            .map(|(id, _)| id.as_str())
            .collect();

        if target_ids.is_empty() {
            return None;
        }

        let root_ids: Vec<(&str, &str)> = self
            .id_to_name
            .iter()
            .filter(|(_, name)| self.roots.contains(name))
            .map(|(id, name)| (id.as_str(), name.as_str()))
            .collect();

        let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();

        for (root_id, root_name) in root_ids {
            queue.push_back((root_id.to_string(), vec![root_name.to_string()]));
            visited.insert(root_id.to_string());
        }

        while let Some((id, path)) = queue.pop_front() {
            if target_ids.contains(id.as_str()) {
                return Some(path);
            }

            let Some(deps) = self.adjacency.get(&id) else { continue };

            for dep_id in deps {
                if visited.insert(dep_id.clone()) {
                    let mut new_path = path.clone();
                    if let Some(name) = self.id_to_name.get(dep_id) {
                        new_path.push(name.clone());
                    }
                    queue.push_back((dep_id.clone(), new_path));
                }
            }
        }

        None
    }

    pub fn features_for(&self, crate_name: &str) -> &[String] {
        self.name_to_features
            .get(crate_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Build a `DepGraph` from raw data for testing.
    /// Package IDs are the crate names themselves (simplified).
    #[cfg(test)]
    pub fn from_raw(
        roots: Vec<String>,
        deps: &[(&str, &[&str])],
        features: &[(&str, &[&str])],
    ) -> Self {
        let mut id_to_name: HashMap<String, String> = HashMap::new();
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut name_to_features: HashMap<String, Vec<String>> = HashMap::new();

        for (name, ds) in deps {
            id_to_name.insert(name.to_string(), name.to_string());
            adjacency.insert(name.to_string(), ds.iter().map(|s| s.to_string()).collect());
        }
        for root in &roots {
            id_to_name.entry(root.clone()).or_insert_with(|| root.clone());
        }
        for (name, fs) in features {
            name_to_features
                .insert(name.to_string(), fs.iter().map(|s| s.to_string()).collect());
        }

        Self { roots, id_to_name, adjacency, name_to_features }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_graph() -> DepGraph {
        // my-app → lib-a → regex
        //        → lib-b
        DepGraph::from_raw(
            vec!["my-app".to_string()],
            &[
                ("my-app", &["lib-a", "lib-b"]),
                ("lib-a", &["regex"]),
                ("lib-b", &[]),
                ("regex", &[]),
            ],
            &[("regex", &["unicode", "perf"])],
        )
    }

    #[test]
    fn path_to_direct_dep() {
        let g = simple_graph();
        assert_eq!(g.path_to("lib-a"), Some(vec!["my-app".into(), "lib-a".into()]));
    }

    #[test]
    fn path_to_transitive_dep() {
        let g = simple_graph();
        assert_eq!(
            g.path_to("regex"),
            Some(vec!["my-app".into(), "lib-a".into(), "regex".into()])
        );
    }

    #[test]
    fn path_to_unknown_returns_none() {
        let g = simple_graph();
        assert_eq!(g.path_to("nonexistent"), None);
    }

    #[test]
    fn features_for_known_crate() {
        let g = simple_graph();
        assert_eq!(g.features_for("regex"), &["unicode", "perf"]);
    }

    #[test]
    fn features_for_unknown_crate() {
        let g = simple_graph();
        assert_eq!(g.features_for("nonexistent"), &[] as &[String]);
    }
}
