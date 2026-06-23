use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/regress-core  →  ../../  = workspace root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn build_fixture(name: &str) -> PathBuf {
    let path = workspace_root().join("tests/fixtures").join(name);
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&path)
        .status()
        .expect("failed to invoke cargo");
    assert!(status.success(), "cargo build failed for fixture '{name}'");
    path.join("target/release").join(name)
}

#[test]
fn parse_bloat_mono_symbols() {
    let bin = build_fixture("bloat-mono");
    let symbols = regress_core::binary::parse_symbols(&bin).expect("parse failed");
    assert!(!symbols.is_empty());
}

#[test]
fn diff_identical_binaries_has_zero_delta() {
    let bin = build_fixture("bloat-derive");
    let syms = regress_core::binary::parse_symbols(&bin).expect("parse failed");
    let diff = regress_core::diff::compute_diff(&syms, &syms);
    assert_eq!(diff.total_delta(), 0);
    assert!(diff.added.is_empty());
    assert!(diff.removed.is_empty());
    assert!(diff.grown.is_empty());
    assert!(diff.shrunk.is_empty());
}
