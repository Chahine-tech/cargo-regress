use std::collections::HashMap;

use crate::diff::SymbolDiff;

#[derive(Debug, Clone)]
pub struct MonomorphGroup {
    pub base_name: String,
    pub instantiations: Vec<MonomorphInstance>,
    pub total_delta: i64,
}

#[derive(Debug, Clone)]
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
        // Discard base names that lost their crate context (e.g. `::update` from
        // `<TypeA as Trait>::update`) — these group unrelated trait impls together.
        .filter(|(base_name, _)| !base_name.starts_with("::"))
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
            Some(MonomorphGroup {
                base_name,
                instantiations,
                total_delta,
            })
        })
        .collect();

    groups.sort_by_key(|b| std::cmp::Reverse(b.total_delta));
    groups
}

fn strip_type_params(demangled: &str) -> String {
    // Strip the MIR hash suffix (::h[16 hex chars]) before grouping.
    // Without this, two monomorphizations of the same function with different
    // concrete types (e.g. Core<BufReader, X> vs Core<Stdin, Y>) get different
    // hashes and are never recognised as the same generic function.
    let s = strip_hash_suffix(demangled);

    let mut result = String::with_capacity(s.len());
    let mut depth: i32 = 0;
    for ch in s.chars() {
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

/// Strip a trailing `::h[16 lowercase hex digits]` rustc hash suffix.
fn strip_hash_suffix(s: &str) -> &str {
    if let Some(idx) = s.rfind("::h") {
        let suffix = &s[idx + 3..];
        if suffix.len() == 16 && suffix.bytes().all(|b| b.is_ascii_hexdigit()) {
            return &s[..idx];
        }
    }
    s
}

fn extract_type_args(demangled: &str) -> String {
    demangled
        .find('<')
        .map(|i| demangled[i..].to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn hash_suffix_stripped_so_same_generic_fn_grouped() {
        // Two instantiations of the same function differ only by hash — must group.
        let syms = vec![
            sym(
                "grep_searcher::core::Core<M,S>::match_by_line::h2842b1982221f489",
                50_000,
            ),
            sym(
                "grep_searcher::core::Core<M,S>::match_by_line::hd397985c3d341c4f",
                50_000,
            ),
            sym(
                "grep_searcher::core::Core<M,S>::match_by_line::h604cba8603f3f0a0",
                50_000,
            ),
        ];
        let groups = detect(&syms);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].base_name.contains("match_by_line"));
        assert_eq!(groups[0].instantiations.len(), 3);
    }

    #[test]
    fn trait_impl_update_methods_not_grouped_as_monomorph() {
        // <TypeA as Trait>::update and <TypeB as Trait>::update are unrelated —
        // they must NOT be grouped (base name would be "::update").
        let syms: Vec<SymbolDiff> = (0..10)
            .map(|i| {
                sym(
                    &format!(
                        "<Type{i} as SomeTrait>::update::h{:016x}",
                        i as u64 * 0x111111111111u64
                    ),
                    5_000,
                )
            })
            .collect();
        let groups = detect(&syms);
        assert!(
            groups.is_empty(),
            "trait impls of different types should not be grouped as monomorphization"
        );
    }

    #[test]
    fn hash_suffix_stripping_does_not_affect_non_hash_suffixes() {
        // A suffix like ::handle (not 16 hex chars) must be preserved.
        let syms = vec![
            sym("my_crate::Foo<u8>::handle", 10_000),
            sym("my_crate::Foo<u16>::handle", 10_000),
        ];
        let groups = detect(&syms);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].base_name.contains("my_crate::Foo::handle"));
    }

    #[test]
    fn strip_hash_suffix_removes_exactly_16_hex_chars() {
        assert_eq!(strip_hash_suffix("foo::bar::h0123456789abcdef"), "foo::bar");
        // 15 hex chars — not a valid hash, preserved
        assert_eq!(
            strip_hash_suffix("foo::bar::h0123456789abcde"),
            "foo::bar::h0123456789abcde"
        );
        // Non-hex char — preserved
        assert_eq!(
            strip_hash_suffix("foo::bar::h0123456789abcdez"),
            "foo::bar::h0123456789abcdez"
        );
    }
}
