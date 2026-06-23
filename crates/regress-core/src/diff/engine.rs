use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::binary::SymbolEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDiff {
    pub name: String,
    pub demangled: String,
    pub section: String,
    pub size_before: u64,
    pub size_after: u64,
    pub delta: i64,
}

impl SymbolDiff {
    pub fn crate_name(&self) -> &str {
        let s = self.demangled.trim_start_matches('<');
        s.split("::").next().unwrap_or("unknown")
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BinaryDiff {
    pub from_total: u64,
    pub to_total: u64,
    pub added: Vec<SymbolDiff>,
    pub removed: Vec<SymbolDiff>,
    pub grown: Vec<SymbolDiff>,
    pub shrunk: Vec<SymbolDiff>,
}

impl BinaryDiff {
    pub fn total_delta(&self) -> i64 {
        self.to_total as i64 - self.from_total as i64
    }

    pub fn total_delta_pct(&self) -> f64 {
        if self.from_total == 0 {
            return 0.0;
        }
        (self.total_delta() as f64 / self.from_total as f64) * 100.0
    }

    pub fn all_growing(&self) -> impl Iterator<Item = &SymbolDiff> {
        self.added.iter().chain(self.grown.iter())
    }

    pub fn all_shrinking(&self) -> impl Iterator<Item = &SymbolDiff> {
        self.removed.iter().chain(self.shrunk.iter())
    }
}

pub fn compute_diff(from: &[SymbolEntry], to: &[SymbolEntry]) -> BinaryDiff {
    let from_map: HashMap<&str, &SymbolEntry> =
        from.iter().map(|s| (s.name.as_str(), s)).collect();
    let to_map: HashMap<&str, &SymbolEntry> =
        to.iter().map(|s| (s.name.as_str(), s)).collect();

    let from_total: u64 = from.iter().map(|s| s.size).sum();
    let to_total: u64 = to.iter().map(|s| s.size).sum();

    let mut diff = BinaryDiff { from_total, to_total, ..Default::default() };

    for (name, sym) in &to_map {
        if !from_map.contains_key(name) {
            diff.added.push(SymbolDiff {
                name: sym.name.clone(),
                demangled: sym.demangled.clone(),
                section: sym.section.clone(),
                size_before: 0,
                size_after: sym.size,
                delta: sym.size as i64,
            });
        }
    }

    for (name, sym) in &from_map {
        if !to_map.contains_key(name) {
            diff.removed.push(SymbolDiff {
                name: sym.name.clone(),
                demangled: sym.demangled.clone(),
                section: sym.section.clone(),
                size_before: sym.size,
                size_after: 0,
                delta: -(sym.size as i64),
            });
        }
    }

    for (name, from_sym) in &from_map {
        if let Some(to_sym) = to_map.get(name) {
            if from_sym.size != to_sym.size {
                let d = SymbolDiff {
                    name: from_sym.name.clone(),
                    demangled: from_sym.demangled.clone(),
                    section: from_sym.section.clone(),
                    size_before: from_sym.size,
                    size_after: to_sym.size,
                    delta: to_sym.size as i64 - from_sym.size as i64,
                };
                if d.delta > 0 {
                    diff.grown.push(d);
                } else {
                    diff.shrunk.push(d);
                }
            }
        }
    }

    diff.added.sort_by(|a, b| b.delta.cmp(&a.delta));
    diff.removed.sort_by(|a, b| a.delta.cmp(&b.delta));
    diff.grown.sort_by(|a, b| b.delta.cmp(&a.delta));
    diff.shrunk.sort_by(|a, b| a.delta.cmp(&b.delta));

    diff
}
