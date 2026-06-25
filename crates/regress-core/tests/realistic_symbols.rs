//! Integration tests using realistic Rust ecosystem symbol patterns.
//!
//! These verify that crate attribution, monomorphization detection, and the
//! full diff pipeline work correctly on real-world naming conventions from
//! crates like serde, tokio, axum, and ripgrep.

use regress_core::binary::SymbolEntry;
use regress_core::classify::monomorph;
use regress_core::diff::{SymbolDiff, compute_diff, group_by_crate};

// ── helpers ───────────────────────────────────────────────────────────────────

fn entry(name: &str, size: u64) -> SymbolEntry {
    SymbolEntry::new(name.to_string(), size, ".text".to_string(), 0)
}

fn sym_diff(demangled: &str, delta: i64) -> SymbolDiff {
    SymbolDiff {
        name: demangled.to_string(),
        demangled: demangled.to_string(),
        section: ".text".to_string(),
        size_before: 0,
        size_after: delta.unsigned_abs(),
        delta,
    }
}

// ── crate attribution ─────────────────────────────────────────────────────────

#[test]
fn plain_path_attributed_correctly() {
    // serde_json::de::Deserializer::parse_str → "serde_json", not "serde"
    let s = entry("serde_json::de::Deserializer::parse_str", 256);
    assert_eq!(s.crate_name(), "serde_json");
}

#[test]
fn trait_impl_attributed_to_implementing_type() {
    // <Vec<u8> as serde::ser::Serialize>::serialize → "Vec"
    let s = entry("<Vec<u8> as serde::ser::Serialize>::serialize", 128);
    assert_eq!(s.crate_name(), "Vec");
}

#[test]
fn tokio_trait_impl_attributed_to_tokio() {
    // <tokio::runtime::Handle as core::fmt::Debug>::fmt → "tokio"
    let s = entry("<tokio::runtime::Handle as core::fmt::Debug>::fmt", 64);
    assert_eq!(s.crate_name(), "tokio");
}

#[test]
fn generic_axum_router_attributed_correctly() {
    let s = entry("axum::routing::Router<S>::route", 512);
    assert_eq!(s.crate_name(), "axum");
}

#[test]
fn ripgrep_nested_path_attributed_correctly() {
    let s = entry("grep_searcher::searcher::Searcher::search_reader", 1024);
    assert_eq!(s.crate_name(), "grep_searcher");
}

#[test]
fn closure_symbol_attributed_correctly() {
    let s = entry("tokio::task::spawn::{{closure}}", 64);
    assert_eq!(s.crate_name(), "tokio");
}

// ── monomorphization detection ────────────────────────────────────────────────

/// Typical serde_json Deserializer instantiated over multiple reader types.
#[test]
fn serde_json_deserializer_monomorph_detected() {
    let types = [
        "std::io::BufReader<std::fs::File>",
        "&[u8]",
        "std::io::Cursor<Vec<u8>>",
        "std::io::Stdin",
    ];
    let syms: Vec<SymbolDiff> = types
        .iter()
        .map(|ty| {
            sym_diff(
                &format!("serde_json::de::Deserializer<{ty}>::parse_value"),
                2048,
            )
        })
        .collect();

    let groups = monomorph::detect(&syms);
    assert!(
        !groups.is_empty(),
        "expected monomorphization group for serde_json Deserializer"
    );
    assert!(
        groups[0]
            .base_name
            .contains("serde_json::de::Deserializer"),
        "base_name should be stripped: got {}",
        groups[0].base_name
    );
    assert_eq!(groups[0].instantiations.len(), 4);
}

/// tokio::sync::Mutex instantiated over various protected types.
#[test]
fn tokio_mutex_monomorph_detected() {
    let types = ["Vec<u8>", "HashMap<String, u64>", "Option<Connection>", "Cache"];
    let syms: Vec<SymbolDiff> = types
        .iter()
        .map(|ty| sym_diff(&format!("tokio::sync::Mutex<{ty}>::lock"), 1024))
        .collect();

    let groups = monomorph::detect(&syms);
    assert!(!groups.is_empty());
    assert!(groups[0].base_name.contains("tokio::sync::Mutex"));
}

/// A single instantiation is NOT monomorphization.
#[test]
fn single_instantiation_not_monomorph() {
    let syms = vec![sym_diff("hyper::client::Client<HttpConnector>::get", 512)];
    let groups = monomorph::detect(&syms);
    assert!(
        groups.is_empty(),
        "a single instantiation should not be flagged"
    );
}

/// Very small total delta below threshold is not reported.
#[test]
fn tiny_delta_monomorph_below_threshold_ignored() {
    let syms = vec![
        sym_diff("tiny::Foo<u8>::bar", 10),
        sym_diff("tiny::Foo<u16>::bar", 10),
        sym_diff("tiny::Foo<u32>::bar", 10),
    ];
    let groups = monomorph::detect(&syms);
    assert!(
        groups.is_empty(),
        "total delta 30 bytes should be below MIN_TOTAL_BYTES threshold"
    );
}

// ── full pipeline stress test ─────────────────────────────────────────────────

/// Simulate a realistic binary diff: 2 000 symbols from 10 crates,
/// ~15% regression.  The pipeline should complete without panic and produce
/// sane group counts.
#[test]
fn pipeline_handles_large_realistic_diff() {
    let crates = [
        "serde_json",
        "tokio",
        "axum",
        "hyper",
        "regex",
        "clap",
        "tracing",
        "bytes",
        "syn",
        "anyhow",
    ];
    const N: usize = 2_000;

    let from: Vec<SymbolEntry> = (0..N)
        .map(|i| {
            let krate = crates[i % crates.len()];
            entry(&format!("{krate}::mod{}::fn{}", i / crates.len(), i), (i as u64 % 1024) + 128)
        })
        .collect();

    // ~15% of symbols grow, ~5% are new
    let mut to = from.clone();
    let grow_n = N * 15 / 100;
    let new_n = N * 5 / 100;
    for sym in to.iter_mut().take(grow_n) {
        sym.size += 256;
    }
    for i in 0..new_n {
        let krate = crates[i % crates.len()];
        to.push(entry(&format!("{krate}::extra::new_fn{i}"), 512));
    }

    let diff = compute_diff(&from, &to);

    assert!(diff.total_delta() > 0, "expected positive total delta");
    assert!(
        diff.grown.len() >= grow_n,
        "expected at least {grow_n} grown symbols, got {}",
        diff.grown.len()
    );
    assert_eq!(diff.added.len(), new_n);

    let groups = group_by_crate(&diff.all_growing().cloned().collect::<Vec<_>>());
    assert_eq!(
        groups.len(),
        crates.len(),
        "expected one group per crate, got {}",
        groups.len()
    );

    // Groups are sorted by delta descending — first group must have the largest delta.
    assert!(
        groups[0].delta >= groups[groups.len() - 1].delta,
        "groups should be sorted by delta descending"
    );
}

/// Diff of identical symbol sets always yields zero delta regardless of size.
#[test]
fn pipeline_zero_delta_on_identical_large_input() {
    let syms: Vec<SymbolEntry> = (0..5_000)
        .map(|i| entry(&format!("some_crate::fn{i}"), (i as u64 % 512) + 64))
        .collect();

    let diff = compute_diff(&syms, &syms);
    assert_eq!(diff.total_delta(), 0);
    assert!(diff.added.is_empty());
    assert!(diff.removed.is_empty());
    assert!(diff.grown.is_empty());
    assert!(diff.shrunk.is_empty());
}
