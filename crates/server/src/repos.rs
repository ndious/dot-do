//! Clone / update the managed git repositories.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::db::Repo;

pub fn is_remote(url: &str) -> bool {
    url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("git@")
        || url.starts_with("ssh://")
}

fn git(dir: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    let out = cmd.output().context("failed to run git")?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn require_dot_dir(root: &Path) -> Result<()> {
    if !root.join(dot::store::DOT_DIR).is_dir() {
        bail!(
            "{} has no .tod directory. Run 'tod init' inside it first.",
            root.display()
        );
    }
    Ok(())
}

/// Clone (or update) the repository 'repo' into the server workdir,
/// and return its local root. A local path is used in place.
/// The clone directory is always named after the repo id, so two
/// registered repos can never collide on disk.
pub fn prepare(repo: &Repo, workdir: &Path) -> Result<PathBuf> {
    if !is_remote(&repo.url) {
        let root = PathBuf::from(&repo.url);
        require_dot_dir(&root)?;
        return Ok(root);
    }

    std::fs::create_dir_all(workdir)?;
    let root = workdir.join(format!("{}-{}", repo.id, repo.name));
    if root.is_dir() {
        git(Some(&root), &["pull", "--ff-only"])?;
    } else {
        git(None, &["clone", &repo.url, &root.to_string_lossy()])?;
    }
    require_dot_dir(&root)?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_remote_and_local_repos() {
        assert!(is_remote("https://github.com/u/r.git"));
        assert!(is_remote("git@github.com:u/r.git"));
        assert!(is_remote("ssh://git@host/r"));
        assert!(!is_remote("/home/user/project"));
        assert!(!is_remote("../project"));
    }

    #[test]
    fn local_repo_without_dot_dir_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repo { id: 1, name: "p".into(), url: dir.path().to_string_lossy().into_owned() };
        assert!(prepare(&repo, Path::new("/tmp/tod-test-workdir")).is_err());
    }
}
