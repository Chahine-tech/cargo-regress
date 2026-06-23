use std::path::Path;

use anyhow::Result;
use cargo_metadata::MetadataCommand;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateDep {
    pub name: String,
    pub version: String,
    pub features: Vec<String>,
}

pub fn direct_deps(manifest_path: &Path) -> Result<Vec<CrateDep>> {
    let metadata = MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()?;

    let deps = metadata
        .packages
        .iter()
        .flat_map(|pkg| pkg.dependencies.iter())
        .map(|dep| CrateDep {
            name: dep.name.clone(),
            version: dep.req.to_string(),
            features: dep.features.clone(),
        })
        .collect();

    Ok(deps)
}
