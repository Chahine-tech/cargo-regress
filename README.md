# cargo-regress

> *"My binary grew by 400 KB between yesterday and today — why exactly?"*

A Rust CLI that answers that question: binary size diff between two git commits, with causal attribution, classified by bloat type, and actionable suggestions.

[![Crates.io](https://img.shields.io/crates/v/cargo-regress.svg)](https://crates.io/crates/cargo-regress)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

---

## Why another tool?

Rust's "zero-cost abstractions" are zero-cost at runtime — not in binary size. When a release binary silently grows, the existing tooling only tells you *what* is big, never *why it got bigger* or *what to do about it*.

```
cargo-bloat (snapshot)      cargo-regress (diff)

  Function           Size     HEAD~1 → HEAD
  ──────────────── ──────     ─────────────────────────────────────────
  serde_json::de    42 KB       +187 KB  serde_json  [monomorphization]
  regex::exec       38 KB                └─ 14 new Deserialize<T> copies
  std::fmt::write   21 KB                   → User, Post, Comment, …
  …                            +143 KB  regex       [new dependency]
                                        └─ via: your_crate → some_lib@0.4.2
  "What's big right now"                   → feature "unicode" adds ~140 KB
                                           → Suggestion: default-features = false
                               "What changed, why, and how to fix it"
```

### Ecosystem comparison

| Tool                | Snapshot | Commit diff | Causal attribution | Suggestions |
|---------------------|:--------:|:-----------:|:------------------:|:-----------:|
| `cargo-bloat`       | ✅        | ❌           | ❌                  | ❌           |
| `cargo-llvm-lines`  | ✅        | ❌           | ❌                  | ❌           |
| `cargo-bloat-action`| ✅        | ✅ CI only   | ❌                  | ❌           |
| `elf_bloat`         | ✅        | ✅ raw ELF   | ❌                  | ❌           |
| **cargo-regress**   | ✅        | ✅ git local | ✅                  | ✅           |

`cargo-bloat` (2 700 ⭐) has been in minimal maintenance since 2022. The official `wg-binary-size` working group was archived in June 2025. `cargo-regress` picks up where they left off.

---

## What it looks like

```
$ cargo regress --from v1.2.0 --to v1.3.0

Binary size regression: +412 KB (+18.3%)
v1.2.0 → v1.3.0

TOP REGRESSIONS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  +187 KB  serde_json  [monomorphization]
           └─ your_crate::models::User
           └─ your_crate::models::Post
           └─ your_crate::models::Comment
           → Consider miniserde for simpler types

  +143 KB  regex       [new dependency]
           ● new dependency (1.11.0)
           └─ import path: your_crate → some_lib → regex
           └─ features: [unicode, perf]
           → Disable unicode: default-features = false, features = ["std"]
              Estimated saving: 140 KB

   +82 KB  std         [derive support code]
           └─ <your_crate::config::AppConfig as core::fmt::Debug>::fmt
           └─ <your_crate::config::DbConfig as core::fmt::Debug>::fmt
           … and 1 more symbols

UNCHANGED / REMOVED: -0.2 KB saved across 1 symbols

Run `cargo regress explain <symbol>` for deeper analysis.
```

---

## Architecture

cargo-regress is organized as a Cargo workspace of three crates, each with a single responsibility:

| Crate             | Role                                                                                          |
|-------------------|-----------------------------------------------------------------------------------------------|
| `regress-core`    | Pure analysis library: binary parsing, symbol diff, bloat classification, causal attribution, suggestions |
| `regress-render`  | Output formatting: terminal, JSON, GitHub Markdown, SARIF, GitLab Code Quality, HTML treemap  |
| `cargo-regress`   | CLI binary and git orchestration: worktree management, cargo build invocation, clap interface |

```
┌──────────────────────────────────────────────────────────────────┐
│                      cargo-regress  (CLI)                        │
│                                                                  │
│  git worktrees  ·  cargo build  ·  clap CLI                      │
└─────────────────────────────┬────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                         regress-core                             │
│                                                                  │
│  binary ──▶ diff ──▶ classify ──▶ suggest                        │
│  ELF        added    monomorph    crate rules                    │
│  Mach-O     removed  hidden       feature flags                  │
│  PE         grown    derive                                      │
│  demangle   shrunk   new dep                                     │
│               │                                                  │
│             causal  ─  Cargo.lock diff  ·  dep graph             │
└─────────────────────────────┬────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                        regress-render                            │
│  terminal · JSON · GitHub Markdown · SARIF · GitLab · HTML       │
└──────────────────────────────────────────────────────────────────┘
```

### How the diff works

When you run `cargo regress --from A --to B`:

```
User                  cargo-regress              git                 cargo
  │                        │                      │                    │
  │  cargo regress         │                      │                    │
  │  --from HEAD~1         │                      │                    │
  │  --to HEAD             │                      │                    │
  │───────────────────────→│                      │                    │
  │                        │  rev-parse HEAD~1    │                    │
  │                        │─────────────────────→│                    │
  │                        │  worktree add /tmp/A │                    │
  │                        │─────────────────────→│                    │
  │                        │                      │  build --release   │
  │                        │──────────────────────────────────────────→│
  │                        │                      │  /tmp/A/target/… ←─┤
  │                        │                      │                    │
  │                        │  worktree add /tmp/B │                    │
  │                        │─────────────────────→│                    │
  │                        │                      │  build --release   │
  │                        │──────────────────────────────────────────→│
  │                        │                      │  /tmp/B/target/… ←─┤
  │                        │                      │                    │
  │                        │  parse ELF/Mach-O/PE symbols (object)     │
  │                        │  rustc-demangle → group by crate          │
  │                        │  classify: monomorph / hidden / derive    │
  │                        │  match suggest rules                      │
  │                        │                      │                    │
  │  render output         │                      │                    │
  │←───────────────────────│                      │                    │
  │                        │  worktree remove /tmp/A + /tmp/B          │
  │                        │─────────────────────→│                    │
```

Worktrees are used instead of checkout/stash — your working directory is never touched.

---

## Installation

```bash
cargo install cargo-regress
```

Requires Rust 1.85+ (edition 2024). Works on Linux (ELF) and macOS (Mach-O). Windows support is **experimental** — see [Windows](#windows-experimental) below.

---

## Usage

### Default: diff last two commits

```bash
cargo regress
```

### Specify commits, tags, or branches

```bash
cargo regress --from abc1234 --to def5678
cargo regress --from v1.2.0 --to v1.3.0
cargo regress --from main --to feature/new-parser
```

### Workspace with multiple binaries

```bash
cargo regress --bin my-service
cargo regress --bin cli-tool --from v2.0 --to v2.1
```

### Compare two pre-built binaries (no git required)

```bash
cargo regress --file-from ./old/my-service --file-to ./new/my-service
```

Useful for cross-compilation (comparing macOS vs Linux builds), CI artefact comparison, or any workflow where the binaries are already on disk. All output formats and `--fail-on` work the same way — only causal attribution is skipped since there is no `Cargo.lock` diff.

### CI / machine-readable output

```bash
# JSON output
cargo regress --format json

# GitHub Actions summary (Markdown for PR comments)
cargo regress --format github

# SARIF 2.1.0 — upload to GitHub Code Scanning
cargo regress --format sarif > results.sarif

# GitLab Code Quality JSON — for MR integration
cargo regress --format gitlab > gl-code-quality-report.json

# Interactive HTML treemap (open in browser)
cargo regress --format html > report.html

# Fail with exit code 1 if regression exceeds threshold
cargo regress --fail-on "+100kb"
cargo regress --fail-on "+1mb"
```

### Project setup

```bash
# Scaffold .cargo-regress.toml + .github/workflows/binary-size.yml in one command
cargo regress init

# With custom options
cargo regress init --bin my-service --fail-on 50000 --no-github
```

`cargo regress init` detects the binary name from `Cargo.toml` automatically and
generates a ready-to-use GitHub Actions workflow. It skips files that already
exist — use `--force` to overwrite.

### Config file

`cargo regress init` creates `.cargo-regress.toml` at the repo root. CLI flags
always take precedence over config values.

```toml
[defaults]
# Output format: terminal | github | json | sarif | gitlab | html
format = "terminal"

# Fail if total regression exceeds this threshold in bytes (0 = disabled)
fail_on_bytes = 10000

# Binary to analyse — override auto-detection if needed
# bin = "my-service"
```

### Baseline mode

Compare the current binary against a saved snapshot without needing a second commit:

```bash
# Save current HEAD as baseline
cargo regress baseline save
cargo regress baseline save --bin my-service

# Later: compare current HEAD against baseline
cargo regress baseline compare
cargo regress baseline compare --format github --fail-on "+50kb"
```

Baselines are stored at `~/.cargo/regress/baseline/<repo>-<binary>.json`.

### Secondary commands

```bash
# Deep analysis of a specific symbol
cargo regress explain "serde_json::de::Deserialize<my_crate::User>"

# Record current HEAD binary size to local history, then show trend
cargo regress watch
cargo regress watch --bin my-service

# Rebuild automatically every 30 seconds (Ctrl-C to stop)
cargo regress watch --interval 30

# Display size history without building
cargo regress watch --show

# Snapshot of current binary: top crates by size with category
cargo regress snapshot
cargo regress snapshot --top 30

# Interactive TUI for exploring regressions
cargo regress tui
cargo regress tui --from v1.0 --to v1.1
```

`cargo regress tui` runs the same diff as the default command, then opens
an interactive terminal UI. Left panel lists crates sorted by regression
size; right panel shows the symbols for the selected crate with their
category and confidence. Keybindings: `↑↓`/`jk` to navigate, `Tab` to
switch panels, `/` to filter crates by name, `q` to quit.

`cargo regress watch` builds HEAD in a clean worktree, appends
`{sha, branch, timestamp, size_bytes}` to
`~/.cargo/regress/watch/<repo>.jsonl`, and prints the last 10 entries
with size deltas. `--show` displays the history without triggering a build.
`--interval N` keeps rebuilding every N seconds until Ctrl-C.

`cargo regress snapshot` analyses the current HEAD binary and displays
all crates ranked by total symbol size, with bloat category when
detectable.

---

## Bloat categories

cargo-regress classifies every regression into one of four categories, derived from the [Tighten Rust's Belt](https://sing.stanford.edu/site/assets/publications/rust-lctes22.pdf) paper (Stanford/Google, LCTES'22):

| Category | What it looks like | Typical fix |
|----------|--------------------|-------------|
| **Monomorphization** | `Vec<String>::retain` + `Vec<u64>::retain` = two copies | `momo` crate, `Box<dyn Fn>`, or a shared non-generic inner function |
| **Derive support code** | `<AppConfig as fmt::Debug>::fmt` added 80 KB | Implement `Debug` manually, or gate it on `#[cfg(debug_assertions)]` |
| **Hidden data** | Panic strings, vtables, static initializers in `.rodata` | `strip = "symbols"`, LTO, fewer format strings |
| **New dependency** | `regex@1.11` appeared in `Cargo.lock`, feature `unicode` enabled | `default-features = false`, check transitive feature activation |

### Confidence model

Each classification carries a confidence score. `[monomorphization]` is high-confidence when N ≥ 2 instantiations of the same base function are found and their combined delta exceeds 4 KB. Unknown symbols fall through to `[unknown]` rather than being silently dropped.

---

## JSON output schema

```json
{
  "from": "abc1234",
  "to": "def5678",
  "from_total_bytes": 2048000,
  "to_total_bytes": 2469888,
  "total_delta_bytes": 421888,
  "total_delta_pct": 20.6,
  "regressions": [
    {
      "crate_name": "regex",
      "delta_bytes": 143360,
      "category": "new_dependency",
      "confidence": 0.95,
      "mono_group": null,
      "cause": {
        "type": "new_dependency",
        "version": "1.11.0"
      },
      "import_path": ["your_crate", "some_lib", "regex"],
      "active_features": ["unicode", "perf"],
      "symbols": ["regex::find::...", "regex::compile::..."],
      "suggestions": [
        "Disable unicode feature: regex = { version = \"...\", default-features = false, features = [\"std\"] }"
      ]
    }
  ],
  "profile_suggestions": [
    "[profile.release] panic = \"abort\"  — removes unwinding tables (~20–50 KB)",
    "[profile.release] lto = \"thin\"  — enables cross-crate dead code elimination"
  ]
}
```

---

## CI integration

### Exit codes

| Situation | Exit code |
|-----------|-----------|
| No regression, or regression below `--fail-on` threshold | `0` |
| Regression above `--fail-on` threshold | `1` |
| Build or analysis error | `2` (anyhow propagation) |

### Zero-config setup

Run `cargo regress init` in your project root — it auto-detects your binary and writes both files:

```
✔ .cargo-regress.toml
✔ .github/workflows/binary-size.yml
```

### GitHub Actions — official action

```yaml
# .github/workflows/binary-size.yml
name: Binary Size Regression
on:
  pull_request:
    branches: [main]

jobs:
  size-check:
    runs-on: ubuntu-latest
    permissions:
      pull-requests: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: Chahine-tech/cargo-regress@v0.5.2
        with:
          from: ${{ github.event.pull_request.base.sha }}
          to: ${{ github.event.pull_request.head.sha }}
          fail-on: "+100kb"
          format: github
```

The action posts a Markdown summary to the PR via `$GITHUB_STEP_SUMMARY` and exits with code 1 if the regression exceeds `fail-on`.

### GitHub Actions — SARIF / Code Scanning

Upload results to the GitHub Security tab (free for public repos):

```yaml
- uses: Chahine-tech/cargo-regress@v0.5.2
  with:
    from: ${{ github.event.pull_request.base.sha }}
    to: ${{ github.event.pull_request.head.sha }}
    format: sarif
  id: regress
- run: echo "${{ steps.regress.outputs.report }}" > results.sarif
- uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: results.sarif
```

### GitLab CI — Code Quality report

```yaml
binary-size:
  script:
    - cargo install cargo-regress --locked
    - cargo regress --from $CI_MERGE_REQUEST_DIFF_BASE_SHA --to $CI_COMMIT_SHA
        --format gitlab > gl-code-quality-report.json
  artifacts:
    reports:
      codequality: gl-code-quality-report.json
```

### GitHub Actions — manual install

```yaml
- name: Check binary size regression
  run: |
    cargo install cargo-regress --locked
    cargo regress \
      --from ${{ github.event.pull_request.base.sha }} \
      --to ${{ github.event.pull_request.head.sha }} \
      --format github \
      --fail-on "+100kb" >> $GITHUB_STEP_SUMMARY
```

---

## Windows (experimental)

Windows PE/COFF support is **experimental**. The `object` crate can parse PE binaries, but symbol availability depends heavily on the toolchain and build flags.

| Toolchain | Status | Notes |
|-----------|--------|-------|
| GNU/MinGW (`x86_64-pc-windows-gnu`) | ✅ Works | Symbols embedded in binary by default |
| MSVC debug builds | ✅ Works | COFF symbols present |
| MSVC release builds | ⚠️ Partial | Symbols stripped to `.pdb` by default — rebuild with `/debugtype:cv,pdata` to embed them |
| MSVC + LTO | ❌ No symbols | Symbols are fully stripped; no workaround |

If `cargo regress` reports an empty diff or no regressions on Windows, it likely means the binary has no embedded symbols. The tool will print a warning:

```
⚠ No symbols found in PE binary. MSVC release builds strip COFF symbols by default.
  Rebuild with /debugtype:cv,pdata or use the GNU/MinGW toolchain for embedded symbols.
```

For full analysis on Windows, use the MinGW toolchain or add to `.cargo/config.toml`:

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "link-arg=/debugtype:cv,pdata"]
```

---

## References

- [Tighten Rust's Belt: Shrinking Embedded Rust Binaries](https://sing.stanford.edu/site/assets/publications/rust-lctes22.pdf) — LCTES'22, Stanford/Google
- [Thoughts on Rust bloat](https://raphlinus.github.io/rust/2019/08/21/rust-bloat.html) — Raph Levien
- [Making Rust binaries smaller by default](https://kobzol.github.io/rust/cargo/2024/01/23/making-rust-binaries-smaller-by-default.html) — Kobzol (ex-wg-binary-size)
- [min-sized-rust](https://github.com/johnthagen/min-sized-rust) — comprehensive guide
- [cargo-bloat](https://github.com/RazrFalcon/cargo-bloat) — inspiration, 2 700 ⭐

---

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE) at your option.
