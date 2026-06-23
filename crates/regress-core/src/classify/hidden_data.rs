use crate::diff::SymbolDiff;

const MIN_BYTES: i64 = 1024;

const KNOWN_PATTERNS: &[&str] = &[
    "panic",
    "__rust_",
    "vtable",
    "static_initializer",
];

pub fn detect(symbols: &[SymbolDiff]) -> Vec<&SymbolDiff> {
    symbols
        .iter()
        .filter(|s| {
            s.delta >= MIN_BYTES
                && (s.section.contains("rodata")
                    || s.section.contains(".data")
                    || KNOWN_PATTERNS.iter().any(|p| s.demangled.contains(p)))
        })
        .collect()
}
