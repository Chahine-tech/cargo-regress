use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        Self { name, demangled, size, section, address }
    }

    pub fn crate_name(&self) -> &str {
        // Strip leading '<' (impl blocks, trait impls)
        let s = self.demangled.trim_start_matches('<');
        s.split("::").next().unwrap_or("unknown")
    }

    pub fn is_in_section(&self, needle: &str) -> bool {
        self.section.contains(needle)
    }
}
