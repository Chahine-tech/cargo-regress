use std::collections::HashMap;

use crate::diff::SymbolDiff;

#[derive(Debug)]
pub struct MonomorphGroup {
    pub base_name: String,
    pub instantiations: Vec<MonomorphInstance>,
    pub total_delta: i64,
}

#[derive(Debug)]
pub struct MonomorphInstance {
    pub type_args: String,
    pub delta: i64,
}

const MIN_TOTAL_BYTES: i64 = 4096;

pub fn detect(symbols: &[SymbolDiff]) -> Vec<MonomorphGroup> {
    let mut by_base: HashMap<String, Vec<&SymbolDiff>> = HashMap::new();

    for sym in symbols {
        let base = strip_type_params(&sym.demangled);
        by_base.entry(base).or_default().push(sym);
    }

    let mut groups: Vec<MonomorphGroup> = by_base
        .into_iter()
        .filter(|(_, syms)| syms.len() >= 2)
        .filter_map(|(base_name, syms)| {
            let total_delta: i64 = syms.iter().map(|s| s.delta).sum();
            if total_delta < MIN_TOTAL_BYTES {
                return None;
            }
            let instantiations = syms
                .iter()
                .map(|s| MonomorphInstance {
                    type_args: extract_type_args(&s.demangled),
                    delta: s.delta,
                })
                .collect();
            Some(MonomorphGroup { base_name, instantiations, total_delta })
        })
        .collect();

    groups.sort_by(|a, b| b.total_delta.cmp(&a.total_delta));
    groups
}

fn strip_type_params(demangled: &str) -> String {
    let mut result = String::with_capacity(demangled.len());
    let mut depth: i32 = 0;
    for ch in demangled.chars() {
        match ch {
            '<' => depth += 1,
            '>' if depth > 0 => {
                depth -= 1;
                continue;
            }
            _ if depth > 0 => continue,
            _ => result.push(ch),
        }
    }
    result
}

fn extract_type_args(demangled: &str) -> String {
    demangled.find('<').map(|i| demangled[i..].to_string()).unwrap_or_default()
}
