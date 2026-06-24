use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolEntry {
    pub name: String,
    pub demangled: String,
    pub size: u64,
    pub section: String,
    pub address: u64,
}

impl SymbolEntry {
    pub fn new(name: String, size: u64, section: String, address: u64) -> Self {
        let demangled = rustc_demangle::demangle(&name).to_string();
        Self {
            name,
            demangled,
            size,
            section,
            address,
        }
    }

    pub fn crate_name(&self) -> &str {
        crate_from_demangled(&self.demangled)
    }
}

/// Extract the top-level crate name from a demangled symbol.
/// e.g. `serde_json::de::Visitor` → `"serde_json"`
/// e.g. `<Vec<u8> as core::fmt::Debug>::fmt` → `"Vec"`
pub(crate) fn crate_from_demangled(demangled: &str) -> &str {
    let s = demangled.trim_start_matches('<');
    s.split("::").next().unwrap_or("unknown")
}
