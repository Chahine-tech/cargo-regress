use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use regress_core::binary::SymbolEntry;
use regress_core::classify::monomorph;
use regress_core::diff::{SymbolDiff, compute_diff, group_by_crate};

// Realistic crate names drawn from the Rust ecosystem
const CRATES: &[&str] = &[
    "serde",
    "serde_json",
    "tokio",
    "hyper",
    "axum",
    "regex",
    "ripgrep",
    "clap",
    "anyhow",
    "tracing",
    "bytes",
    "http",
    "tower",
    "futures",
    "syn",
    "quote",
    "proc_macro2",
    "unicode_normalization",
    "url",
    "rand",
];

/// Produce `count` symbols with realistic names spread across `CRATES`.
fn make_symbols(count: usize) -> Vec<SymbolEntry> {
    (0..count)
        .map(|i| {
            let krate = CRATES[i % CRATES.len()];
            let module = i / CRATES.len();
            let name = format!("{krate}::module{module}::function{i}");
            let size = (i as u64 % 4096) + 64;
            SymbolEntry::new(name, size, ".text".to_string(), i as u64 * 64)
        })
        .collect()
}

/// Apply a ~`pct`% regression to `from`: grow some symbols, add new ones.
fn make_regression(from: &[SymbolEntry], pct: usize) -> Vec<SymbolEntry> {
    let n = (from.len() * pct / 100).max(1);
    let mut to = from.to_vec();
    for sym in to.iter_mut().take(n) {
        sym.size += 256;
    }
    for i in 0..n {
        let krate = CRATES[i % CRATES.len()];
        to.push(SymbolEntry::new(
            format!("{krate}::new_module::new_fn{i}"),
            512,
            ".text".to_string(),
            i as u64,
        ));
    }
    to
}

// ── compute_diff ──────────────────────────────────────────────────────────────

fn bench_compute_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("compute_diff");
    for &size in &[1_000usize, 5_000, 10_000, 20_000] {
        let from = make_symbols(size);
        let to = make_regression(&from, 10);
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &(from, to),
            |b, (f, t)| {
                b.iter(|| compute_diff(black_box(f), black_box(t)));
            },
        );
    }
    group.finish();
}

// ── group_by_crate ────────────────────────────────────────────────────────────

fn bench_group_by_crate(c: &mut Criterion) {
    let mut group = c.benchmark_group("group_by_crate");
    for &size in &[1_000usize, 5_000, 10_000] {
        let from = make_symbols(size);
        let to = make_regression(&from, 20);
        let diff = compute_diff(&from, &to);
        let growing: Vec<_> = diff.all_growing().cloned().collect();
        group.bench_with_input(BenchmarkId::from_parameter(size), &growing, |b, syms| {
            b.iter(|| group_by_crate(black_box(syms)));
        });
    }
    group.finish();
}

// ── detect_monomorphization ───────────────────────────────────────────────────

fn bench_detect_monomorph(c: &mut Criterion) {
    // Multiple instantiations of the same generic function (serde-like pattern)
    let types = [
        "Vec<u8>",
        "String",
        "BufReader<File>",
        "&[u8]",
        "Cursor<Vec<u8>>",
    ];
    let syms: Vec<SymbolDiff> = (0..200)
        .flat_map(|fn_idx| {
            types.iter().map(move |ty| {
                let name = format!("serde_json::de::Deserializer<{ty}>::parse_fn{fn_idx}");
                SymbolDiff {
                    name: name.clone(),
                    demangled: name,
                    section: ".text".to_string(),
                    size_before: 512,
                    size_after: 768,
                    delta: 256,
                }
            })
        })
        .collect();

    c.bench_function("detect_monomorphization_1k", |b| {
        b.iter(|| monomorph::detect(black_box(&syms)));
    });
}

criterion_group!(
    benches,
    bench_compute_diff,
    bench_group_by_crate,
    bench_detect_monomorph
);
criterion_main!(benches);
