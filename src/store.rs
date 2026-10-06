use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::git;

/// Hidden directory holding the dot database.
pub const DOT_DIR: &str = ".dot";

/// A single todo. One task = one JSON file named by its uid (sha1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub uid: String,
    pub description: String,
    pub author: String,
    pub created: String,
    pub updated: String,
    pub done: bool,
}

pub fn dot_dir() -> Result<PathBuf> {
    Ok(git::repo_root()?.join(DOT_DIR))
}

pub fn todo_dir() -> Result<PathBuf> {
    Ok(dot_dir()?.join("todo"))
}

pub fn backlog_dir() -> Result<PathBuf> {
    Ok(dot_dir()?.join("backlog"))
}

/// Branch names may contain '/' which cannot be a single directory name.
pub fn sanitize(branch: &str) -> String {
    branch.replace('/', "--")
}

pub fn desanitize(dir: &str) -> String {
    dir.replace("--", "/")
}

pub fn ensure_init() -> Result<()> {
    if !todo_dir()?.is_dir() {
        bail!("dot is not initialized here. Run 'dot init' first.");
    }
    Ok(())
}

pub fn init() -> Result<()> {
    for dir in [todo_dir()?, backlog_dir()?] {
        fs::create_dir_all(&dir)
            .with_context(|| format!("cannot create {}", dir.display()))?;
    }
    println!("Initialized dot in {}", dot_dir()?.display());
    Ok(())
}

pub fn context_dir(branch: &str) -> Result<PathBuf> {
    Ok(todo_dir()?.join(sanitize(branch)))
}

fn new_uid(description: &str) -> String {
    let now = chrono::Utc::now().to_rfc3339();
    let mut hasher = Sha1::new();
    hasher.update(description.as_bytes());
    hasher.update(now.as_bytes());
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Create a new task in the current context, or in the backlog.
pub fn add(description: &str, backlog: bool) -> Result<()> {
    ensure_init()?;
    let now = chrono::Utc::now().to_rfc3339();
    let task = Task {
        uid: new_uid(description),
        description: description.to_string(),
        author: git::user_name(),
        created: now.clone(),
        updated: now,
        done: false,
    };
    let dir = if backlog {
        backlog_dir()?
    } else {
        context_dir(&git::current_branch()?)?
    };
    fs::create_dir_all(&dir)?;
    write_task(&dir, &task)?;
    println!("Added #{} {}", &task.uid[..5], task.description);
    Ok(())
}

pub fn write_task(dir: &std::path::Path, task: &Task) -> Result<()> {
    let path = dir.join(format!("{}.json", task.uid));
    fs::write(&path, serde_json::to_string_pretty(task)?)
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_round_trip() {
        assert_eq!(sanitize("feature/fix"), "feature--fix");
        assert_eq!(desanitize(&sanitize("feature/fix")), "feature/fix");
    }

    #[test]
    fn task_round_trip() {
        let task = Task {
            uid: "abc".to_string(),
            description: "hello".to_string(),
            author: "me".to_string(),
            created: "2021-01-01T00:00:00Z".to_string(),
            updated: "2021-01-01T00:00:00Z".to_string(),
            done: false,
        };
        let json = serde_json::to_string(&task).unwrap();
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(back.description, "hello");
    }
}
