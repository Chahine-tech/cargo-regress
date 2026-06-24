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
        let group = groups
            .entry(crate_name.clone())
            .or_insert_with(|| CrateGroup {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::engine::SymbolDiff;

    fn sym(demangled: &str, delta: i64) -> SymbolDiff {
        SymbolDiff {
            name: demangled.to_string(),
            demangled: demangled.to_string(),
            section: ".text".to_string(),
            size_before: 0,
            size_after: delta.unsigned_abs(),
            delta,
        }
    }

    #[test]
    fn groups_symbols_under_same_crate() {
        let syms = vec![
            sym("regex::find", 100),
            sym("regex::replace", 200),
            sym("serde::de::Deserialize", 50),
        ];
        let groups = group_by_crate(&syms);
        let regex = groups.iter().find(|g| g.name == "regex").unwrap();
        assert_eq!(regex.delta, 300);
        assert_eq!(regex.symbols.len(), 2);
    }

    #[test]
    fn sorts_by_absolute_delta_descending() {
        let syms = vec![
            sym("small::foo", 10),
            sym("big::bar", 1000),
            sym("medium::baz", 100),
        ];
        let groups = group_by_crate(&syms);
        assert_eq!(groups[0].name, "big");
        assert_eq!(groups[1].name, "medium");
        assert_eq!(groups[2].name, "small");
    }

    #[test]
    fn empty_input_produces_empty_output() {
        assert!(group_by_crate(&[]).is_empty());
    }
}
