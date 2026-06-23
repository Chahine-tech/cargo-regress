pub mod derive_support;
pub mod hidden_data;
pub mod monomorph;

use crate::diff::SymbolDiff;
use serde::{Deserialize, Serialize};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BloatCategory {
    Monomorphization,
    HiddenData,
    DeriveSupport,
    NewDependency,
    Unknown,
}

impl std::fmt::Display for BloatCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Monomorphization => write!(f, "monomorphization"),
            Self::HiddenData => write!(f, "hidden data"),
            Self::DeriveSupport => write!(f, "derive support code"),
            Self::NewDependency => write!(f, "new dependency"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

pub fn classify(sym: &SymbolDiff) -> BloatCategory {
    if derive_support::DERIVE_PATTERNS
        .iter()
        .any(|p| sym.demangled.contains(p))
    {
        return BloatCategory::DeriveSupport;
    }
    if sym.section.contains("rodata") || sym.section.contains(".data") {
        return BloatCategory::HiddenData;
    }
    if sym.demangled.contains('<') && sym.demangled.contains("::") {
        return BloatCategory::Monomorphization;
    }
    BloatCategory::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym_diff(demangled: &str, section: &str) -> SymbolDiff {
        SymbolDiff {
            name: demangled.to_string(),
            demangled: demangled.to_string(),
            section: section.to_string(),
            size_before: 0,
            size_after: 100,
            delta: 100,
        }
    }

    #[test]
    fn fmt_debug_is_derive_support() {
        let s = sym_diff("serde_json::_::_::fmt::Debug", ".text");
        assert_eq!(classify(&s), BloatCategory::DeriveSupport);
    }

    #[test]
    fn rodata_symbol_is_hidden_data() {
        let s = sym_diff("some_static_string", "__rodata");
        assert_eq!(classify(&s), BloatCategory::HiddenData);
    }

    #[test]
    fn generic_symbol_is_monomorphization() {
        let s = sym_diff("alloc::vec::Vec<MyStruct>::retain", ".text");
        assert_eq!(classify(&s), BloatCategory::Monomorphization);
    }
}
