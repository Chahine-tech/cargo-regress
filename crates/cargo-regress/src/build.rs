use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use git2::Repository;

/// Resolve a git ref spec to a full 40-char SHA.
pub fn resolve_commit(repo_path: &Path, spec: &str) -> Result<String> {
    let repo = Repository::open(repo_path)?;
    let obj = repo
        .revparse_single(spec)
        .with_context(|| format!("Cannot resolve git ref '{spec}'"))?;
    Ok(obj.id().to_string())
}

/// Find the root of the git repository (workdir path).
pub fn find_repo_root() -> Result<PathBuf> {
    let repo = Repository::discover(".")
        .context("Not inside a git repository")?;
    repo.workdir()
        .map(|p| p.to_path_buf())
        .context("Bare repositories are not supported")
}

/// A temporary git worktree that is cleaned up on drop.
pub struct Worktree {
    path: PathBuf,
    repo: PathBuf,
}

impl Worktree {
    /// Create a detached worktree at `commit_sha` inside a system temp dir.
    pub fn create(repo: &Path, commit_sha: &str) -> Result<Self> {
        let short = commit_sha.chars().take(8).collect::<String>();
        let path = std::env::temp_dir().join(format!("cargo-regress-{short}"));

        if path.exists() {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("Cannot remove old worktree: {}", path.display()))?;
        }

        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Non-UTF-8 path: {}", path.display()))?;

        let ok = Command::new("git")
            .args(["worktree", "add", "--detach", path_str, commit_sha])
            .current_dir(repo)
            .status()
            .context("Failed to run git worktree add")?
            .success();

        if !ok {
            bail!("git worktree add failed for commit {commit_sha}");
        }

        Ok(Self { path, repo: repo.to_path_buf() })
    }

    /// Root directory of this worktree (contains Cargo.toml and Cargo.lock).
    pub fn root(&self) -> &Path {
        &self.path
    }

    /// Run `cargo build --release` and return the path to the produced binary.
    pub fn build_release(&self, bin: Option<&str>, lib: bool) -> Result<PathBuf> {
        let mut cmd = Command::new("cargo");
        cmd.args(["build", "--release"]);

        match (bin, lib) {
            (Some(b), _) => { cmd.args(["--bin", b]); }
            (None, true) => { cmd.arg("--lib"); }
            _ => {}
        }

        let ok = cmd
            .current_dir(&self.path)
            .status()
            .context("Failed to invoke cargo")?
            .success();

        if !ok {
            bail!("cargo build --release failed in {}", self.path.display());
        }

        find_binary(&self.path.join("target/release"), bin)
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        // Drop cannot propagate errors; best-effort cleanup.
        let path_str = self.path.to_str().unwrap_or_default();
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force", path_str])
            .current_dir(&self.repo)
            .status();
    }
}

fn find_binary(dir: &Path, hint: Option<&str>) -> Result<PathBuf> {
    if let Some(name) = hint {
        let p = dir.join(name);
        if p.exists() {
            return Ok(p);
        }
        let p = dir.join(format!("{name}.exe"));
        if p.exists() {
            return Ok(p);
        }
        bail!("Binary '{name}' not found in {}", dir.display());
    }

    // Heuristic: largest file without an extension in target/release
    let mut candidates: Vec<(PathBuf, u64)> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file()
                && matches!(
                    p.extension().and_then(|x| x.to_str()),
                    None | Some("exe")
                )
        })
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.len())))
        .collect();

    candidates.sort_by_key(|(_, sz)| std::cmp::Reverse(*sz));

    candidates
        .into_iter()
        .next()
        .map(|(p, _)| p)
        .ok_or_else(|| anyhow::anyhow!("No binary found in {}", dir.display()))
}
