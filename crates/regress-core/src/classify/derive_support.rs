use crate::diff::SymbolDiff;

pub const DERIVE_PATTERNS: &[&str] = &[
    "fmt::Debug",
    "fmt::Display",
    "Drop::drop",
    "Clone::clone",
    "PartialEq",
    "Hash::hash",
    "core::fmt::Debug",
];

pub fn detect(symbols: &[SymbolDiff]) -> Vec<&SymbolDiff> {
    symbols
        .iter()
        .filter(|s| {
            s.delta > 0 && DERIVE_PATTERNS.iter().any(|p| s.demangled.contains(p))
        })
        .collect()
}
