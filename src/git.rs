use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Root of the current git repository.
pub fn repo_root() -> Result<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git")?;
    if !out.status.success() {
        bail!("not inside a git repository");
    }
    let path = String::from_utf8(out.stdout)?.trim().to_string();
    Ok(PathBuf::from(path))
}

/// Current git branch of the repository rooted at 'dir': the "context" of dot.
pub fn current_branch_in(dir: &Path) -> Result<String> {
    let out = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(dir)
        .output()
        .context("failed to run git")?;
    let branch = String::from_utf8(out.stdout)?.trim().to_string();
    if branch.is_empty() {
        bail!("detached HEAD state: no current branch");
    }
    Ok(branch)
}

/// Current git branch of the current working directory.
pub fn current_branch() -> Result<String> {
    current_branch_in(&std::env::current_dir().context("cannot get current directory")?)
}

/// Git user name, used as task author.
pub fn user_name() -> String {
    Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
