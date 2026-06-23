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

fn fixture(name: &str) -> PathBuf {
    workspace_root().join("tests/fixtures").join(name)
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
fn dep_graph_finds_regex_in_bloat_dep() {
    let manifest = fixture("bloat-dep").join("Cargo.toml");
    let graph = regress_core::causal::DepGraph::from_manifest(&manifest)
        .expect("dep graph failed");

    let path = graph.path_to("regex").expect("regex not found in graph");
    // path should be ["bloat-dep", "regex"]
    assert_eq!(path.first().map(String::as_str), Some("bloat-dep"));
    assert_eq!(path.last().map(String::as_str), Some("regex"));
}

#[test]
fn dep_graph_unknown_crate_returns_none() {
    let manifest = fixture("bloat-dep").join("Cargo.toml");
    let graph = regress_core::causal::DepGraph::from_manifest(&manifest)
        .expect("dep graph failed");
    assert!(graph.path_to("this_crate_does_not_exist").is_none());
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
