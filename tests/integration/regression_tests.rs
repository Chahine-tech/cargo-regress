use std::path::PathBuf;
use std::process::Command;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/fixtures")
        .join(name)
}

fn build_fixture(name: &str) -> PathBuf {
    let path = fixture_path(name);
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&path)
        .status()
        .expect("cargo build failed");
    assert!(status.success(), "Build failed for fixture: {name}");
    path.join("target/release").join(name)
}

#[test]
fn parse_bloat_mono_symbols() {
    let bin = build_fixture("bloat-mono");
    let symbols = regress_core::binary::parse_symbols(&bin).expect("parse failed");
    assert!(!symbols.is_empty(), "Expected at least some symbols");
}

#[test]
fn diff_identical_binaries_is_empty() {
    let bin = build_fixture("bloat-derive");
    let syms = regress_core::binary::parse_symbols(&bin).expect("parse failed");
    let diff = regress_core::diff::compute_diff(&syms, &syms);
    assert_eq!(diff.total_delta(), 0);
    assert!(diff.added.is_empty());
    assert!(diff.removed.is_empty());
    assert!(diff.grown.is_empty());
    assert!(diff.shrunk.is_empty());
}
