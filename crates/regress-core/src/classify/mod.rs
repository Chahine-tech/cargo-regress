pub mod derive_support;
pub mod hidden_data;
pub mod monomorph;

use serde::{Deserialize, Serialize};

use crate::causal::{CausalCause, CausalEntry};
use crate::diff::engine::SymbolDiff;
use crate::diff::grouping::CrateGroup;

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

/// Summary of a monomorphization group within a classification result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonomorphSummary {
    pub base_name: String,
    pub instantiation_count: usize,
    pub total_delta: i64,
}

/// Result of classifying a crate-level regression group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub category: BloatCategory,
    /// Confidence in [0.0, 1.0]. Use `confidence_label()` for display.
    pub confidence: f64,
    /// Present when `category == Monomorphization`.
    pub mono_group: Option<MonomorphSummary>,
}

impl ClassificationResult {
    pub fn confidence_label(&self) -> &'static str {
        match self.confidence {
            c if c >= 0.85 => "high",
            c if c >= 0.60 => "medium",
            _ => "low",
        }
    }
}

/// Classify a single symbol (fast path used in simple contexts).
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

/// Classify a crate-level regression group, using causal attribution when available.
///
/// Priority order:
/// 1. `NewDependency` from causal info (high confidence)
/// 2. Monomorphization via N-instantiation detection
/// 3. Derive support code
/// 4. Hidden data (.rodata / .data)
/// 5. Symbol-level fallback
pub fn classify_group(group: &CrateGroup, causal: Option<&CausalEntry>) -> ClassificationResult {
    // 1. Causal attribution takes priority.
    if let Some(entry) = causal {
        if matches!(entry.cause, CausalCause::NewDependency { .. }) {
            return ClassificationResult {
                category: BloatCategory::NewDependency,
                confidence: 0.95,
                mono_group: None,
            };
        }
    }

    // 2. Monomorphization: check for N identical base functions with different type params.
    let mono_groups = monomorph::detect(&group.symbols);
    if let Some(best) = mono_groups.first() {
        let confidence = match best.instantiations.len() {
            n if n >= 4 => 0.90,
            3 => 0.80,
            _ => 0.65,
        };
        return ClassificationResult {
            category: BloatCategory::Monomorphization,
            confidence,
            mono_group: Some(MonomorphSummary {
                base_name: best.base_name.clone(),
                instantiation_count: best.instantiations.len(),
                total_delta: best.total_delta,
            }),
        };
    }

    // 3. Derive support code.
    let derive_hits = derive_support::detect(&group.symbols);
    if !derive_hits.is_empty() {
        let fraction = derive_hits.len() as f64 / group.symbols.len().max(1) as f64;
        return ClassificationResult {
            category: BloatCategory::DeriveSupport,
            confidence: (0.55 + fraction * 0.35).min(0.90),
            mono_group: None,
        };
    }

    // 4. Hidden data (.rodata, .data sections).
    let hidden_hits = hidden_data::detect(&group.symbols);
    if !hidden_hits.is_empty() {
        return ClassificationResult {
            category: BloatCategory::HiddenData,
            confidence: 0.75,
            mono_group: None,
        };
    }

    // 5. Symbol-level fallback on the largest symbol.
    if let Some(sym) = group.symbols.iter().max_by_key(|s| s.delta) {
        let cat = classify(sym);
        if cat != BloatCategory::Unknown {
            return ClassificationResult {
                category: cat,
                confidence: 0.45,
                mono_group: None,
            };
        }
    }

    ClassificationResult {
        category: BloatCategory::Unknown,
        confidence: 0.0,
        mono_group: None,
    }
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

    #[test]
    fn confidence_label_high() {
        let r = ClassificationResult {
            category: BloatCategory::NewDependency,
            confidence: 0.95,
            mono_group: None,
        };
        assert_eq!(r.confidence_label(), "high");
    }

    #[test]
    fn confidence_label_medium() {
        let r = ClassificationResult {
            category: BloatCategory::Monomorphization,
            confidence: 0.65,
            mono_group: None,
        };
        assert_eq!(r.confidence_label(), "medium");
    }

    #[test]
    fn new_dependency_cause_overrides_classification() {
        use crate::causal::{CausalCause, CausalEntry};
        let group = CrateGroup {
            name: "regex".into(),
            delta: 143_000,
            symbols: vec![],
        };
        let entry = CausalEntry {
            crate_name: "regex".into(),
            delta: 143_000,
            cause: CausalCause::NewDependency {
                version: "1.11.0".into(),
            },
            import_path: vec![],
            active_features: vec![],
        };
        let result = classify_group(&group, Some(&entry));
        assert_eq!(result.category, BloatCategory::NewDependency);
        assert!(result.confidence >= 0.9);
    }
}
