use std::collections::HashMap;

use super::engine::SymbolDiff;

#[derive(Debug, Clone)]
pub struct CrateGroup {
    pub name: String,
    pub delta: i64,
    pub symbols: Vec<SymbolDiff>,
}

pub fn group_by_crate(symbols: &[SymbolDiff]) -> Vec<CrateGroup> {
    let mut groups: HashMap<String, CrateGroup> = HashMap::new();

    for sym in symbols {
        let crate_name = sym.crate_name().to_string();
        let group = groups.entry(crate_name.clone()).or_insert_with(|| CrateGroup {
            name: crate_name,
            delta: 0,
            symbols: Vec::new(),
        });
        group.delta += sym.delta;
        group.symbols.push(sym.clone());
    }

    let mut result: Vec<CrateGroup> = groups.into_values().collect();
    // Sort by absolute delta descending so biggest regressions come first
    result.sort_by(|a, b| b.delta.abs().cmp(&a.delta.abs()));
    result
}
