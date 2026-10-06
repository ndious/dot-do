//! tod-server: a project management server on top of the dot store.
//!
//! Clones (or opens) a git repository, then serves a small JSON API
//! and the Yew web console to browse and manage its .tod database.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::Parser;

mod api;

#[derive(Parser, Debug)]
#[command(name = "tod-server", about = "Project management server for .tod repositories")]
struct Args {
    /// Git URL (https://, git@, ssh://) or local path of the repository to manage
    #[arg(long)]
    repo: String,

    /// Port to listen on
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// Directory where remote repositories are cloned
    #[arg(long, default_value = ".tod-server")]
    workdir: PathBuf,
}

fn is_remote(repo: &str) -> bool {
    repo.starts_with("http://")
        || repo.starts_with("https://")
        || repo.starts_with("git@")
        || repo.starts_with("ssh://")
}

/// Last path segment of a git URL, without the .git suffix.
fn repo_dir_name(repo: &str) -> String {
    let trimmed = repo.trim_end_matches('/').trim_end_matches(".git");
    trimmed
        .rsplit(['/', ':'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("repo")
        .to_string()
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

/// Clone (or update) the repository, and return its local root.
fn prepare_repo(repo: &str, workdir: &Path) -> Result<PathBuf> {
    if !is_remote(repo) {
        let root = PathBuf::from(repo);
        require_dot_dir(&root)?;
        return Ok(root);
    }

    std::fs::create_dir_all(workdir)?;
    let root = workdir.join(repo_dir_name(repo));
    if root.is_dir() {
        git(Some(&root), &["pull", "--ff-only"])?;
        println!("updated {}", root.display());
    } else {
        git(None, &["clone", repo, &root.to_string_lossy()])?;
        println!("cloned {} into {}", repo, root.display());
    }
    require_dot_dir(&root)?;
    Ok(root)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let root = prepare_repo(&args.repo, &args.workdir)?;
    let app = Arc::new(api::App::new(root));

    let router = api::router(app).fallback_service(tower_http::services::ServeDir::new(
        "crates/web/dist",
    ));

    let addr = format!("0.0.0.0:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;
    println!("tod-server listening on http://{addr}");
    axum::serve(listener, router).await?;
    Ok(())
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
    fn repo_dir_name_strips_git_suffix() {
        assert_eq!(repo_dir_name("https://github.com/ndious/dot-do.git"), "dot-do");
        assert_eq!(repo_dir_name("git@github.com:ndious/dot-do.git"), "dot-do");
        assert_eq!(repo_dir_name("https://host/group/proj/"), "proj");
    }
}
