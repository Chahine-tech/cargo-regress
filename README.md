# cargo-regress

> *"My binary grew by 400 KB between yesterday and today — why exactly?"*

A Rust CLI that answers that question: binary size diff between two git commits, with causal attribution, classified by bloat type, and actionable suggestions.

[![Crates.io](https://img.shields.io/crates/v/cargo-regress.svg)](https://crates.io/crates/cargo-regress)
[![docs.rs](https://img.shields.io/docsrs/cargo-regress)](https://docs.rs/cargo-regress)
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
| `regress-render`  | Output formatting: colored terminal, JSON, GitHub Actions summary                             |
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
│      terminal (owo-colors)  ·  JSON  ·  GitHub Markdown          │
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

Requires Rust 1.85+ (edition 2024). Works on Linux (ELF), macOS (Mach-O), and Windows (PE).

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

### CI / machine-readable output

```bash
# JSON output
cargo regress --format json

# GitHub Actions summary (compatible with PR comments)
cargo regress --format github

# Fail with exit code 1 if regression exceeds threshold
cargo regress --fail-on "+100kb"
cargo regress --fail-on "+1mb"
```

### Secondary commands

```bash
# Deep analysis of a specific symbol
cargo regress explain "serde_json::de::Deserialize<my_crate::User>"

# Record current HEAD binary size to local history, then show trend
cargo regress watch
cargo regress watch --bin my-service

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

`cargo regress snapshot` analyses the current HEAD binary and displays
all crates ranked by total symbol size, with bloat category when
detectable. On Windows, MSVC release binaries must be built with
`/debugtype:cv,pdata` to embed COFF symbols; GNU/MinGW toolchains work
out of the box.

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

### GitHub Actions example

```yaml
- name: Check binary size regression
  run: |
    cargo install cargo-regress
    cargo regress \
      --from ${{ github.event.pull_request.base.sha }} \
      --to ${{ github.event.pull_request.head.sha }} \
      --format github \
      --fail-on "+100kb" >> $GITHUB_STEP_SUMMARY
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
