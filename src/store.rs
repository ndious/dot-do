use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::git;

/// Hidden directory holding the dot database.
pub const DOT_DIR: &str = ".dot";

/// A single todo. One task = one JSON file named by its uid (sha1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub uid: String,
    pub description: String,
    pub author: String,
    pub created: String,
    pub updated: String,
    pub done: bool,
}

/// The dot database of one git repository.
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Discover the store of the git repository containing the current directory.
    pub fn discover() -> Result<Self> {
        Ok(Self::new(git::repo_root()?))
    }

    /// Open the store of an explicit repository root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Store { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dot_dir(&self) -> PathBuf {
        self.root.join(DOT_DIR)
    }

    pub fn todo_dir(&self) -> PathBuf {
        self.dot_dir().join("todo")
    }

    pub fn backlog_dir(&self) -> PathBuf {
        self.dot_dir().join("backlog")
    }

    /// Directory holding the tasks of one branch (context).
    pub fn context_dir(&self, branch: &str) -> PathBuf {
        self.todo_dir().join(sanitize(branch))
    }

    /// Current branch of the repository behind this store.
    pub fn branch(&self) -> Result<String> {
        git::current_branch_in(&self.root)
    }

    pub fn is_initialized(&self) -> bool {
        self.todo_dir().is_dir() && self.backlog_dir().is_dir()
    }

    pub fn ensure_init(&self) -> Result<()> {
        if !self.is_initialized() {
            bail!("dot is not initialized here. Run 'dot init' first.");
        }
        Ok(())
    }

    /// Create the .dot storage layout.
    pub fn init(&self) -> Result<()> {
        for dir in [self.todo_dir(), self.backlog_dir()] {
            fs::create_dir_all(&dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }
        Ok(())
    }

    /// Create a new task in the current context, or in the backlog.
    pub fn add(&self, description: &str, author: &str, backlog: bool) -> Result<Task> {
        self.ensure_init()?;
        let now = chrono::Utc::now().to_rfc3339();
        let task = Task {
            uid: new_uid(description),
            description: description.to_string(),
            author: author.to_string(),
            created: now.clone(),
            updated: now,
            done: false,
        };
        let dir = if backlog {
            self.backlog_dir()
        } else {
            self.context_dir(&self.branch()?)
        };
        fs::create_dir_all(&dir)?;
        write_task(&dir, &task)?;
        Ok(task)
    }

    /// Load the backlog.
    pub fn load_backlog(&self) -> Result<Vec<Task>> {
        load_dir(&self.backlog_dir())
    }

    /// Load the tasks of one context.
    pub fn load_context(&self, branch: &str) -> Result<Vec<Task>> {
        load_dir(&self.context_dir(branch))
    }

    /// Every context (branch) holding at least one task, sorted by name.
    pub fn contexts(&self) -> Result<Vec<(String, Vec<Task>)>> {
        let mut out = Vec::new();
        if !self.todo_dir().is_dir() {
            return Ok(out);
        }
        for entry in fs::read_dir(self.todo_dir())?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let tasks = load_dir(&path)?;
                if !tasks.is_empty() {
                    let name = desanitize(&entry.file_name().to_string_lossy());
                    out.push((name, tasks));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }

    /// Mark a task of the current context as complete.
    pub fn complete(&self, identifier: &str) -> Result<Task> {
        self.ensure_init()?;
        let dir = self.context_dir(&self.branch()?);
        let tasks = load_dir(&dir)?;
        let mut task = match_task(&tasks, identifier)?.clone();
        if task.done {
            bail!("task #{} is already complete", &task.uid[..5]);
        }
        task.done = true;
        task.updated = chrono::Utc::now().to_rfc3339();
        write_task(&dir, &task)?;
        Ok(task)
    }

    /// Delete a task from the current context, or from the backlog.
    pub fn remove(&self, identifier: &str, backlog: bool) -> Result<Task> {
        self.ensure_init()?;
        let dir = if backlog {
            self.backlog_dir()
        } else {
            self.context_dir(&self.branch()?)
        };
        let tasks = load_dir(&dir)?;
        let task = match_task(&tasks, identifier)?.clone();
        fs::remove_file(dir.join(format!("{}.json", task.uid)))?;
        Ok(task)
    }

    /// Move a task between the backlog and the current context.
    pub fn move_task(&self, identifier: &str, to_backlog: bool) -> Result<Task> {
        self.ensure_init()?;
        let branch = self.branch()?;
        let (from, to) = if to_backlog {
            (self.context_dir(&branch), self.backlog_dir())
        } else {
            (self.backlog_dir(), self.context_dir(&branch))
        };
        let tasks = load_dir(&from)?;
        let task = match_task(&tasks, identifier)?.clone();
        fs::create_dir_all(&to)?;
        fs::rename(
            from.join(format!("{}.json", task.uid)),
            to.join(format!("{}.json", task.uid)),
        )?;
        Ok(task)
    }

    /// Close a context: delete every task of the branch and its folder.
    pub fn resolve(&self, context: &str) -> Result<usize> {
        self.ensure_init()?;
        let dir = self.context_dir(context);
        if !dir.is_dir() {
            bail!("unknown context '{context}'");
        }
        let count = load_dir(&dir)?.len();
        fs::remove_dir_all(&dir).with_context(|| format!("cannot remove {}", dir.display()))?;
        Ok(count)
    }

    /// Export the whole database (backlog + every context) as a JSON file.
    /// Returns (backlog size, context count).
    pub fn export(&self, output: &str) -> Result<(usize, usize)> {
        self.ensure_init()?;
        let backlog = self.load_backlog()?;
        let contexts = self.contexts()?;
        let mut map = serde_json::Map::new();
        for (name, tasks) in &contexts {
            map.insert(name.clone(), serde_json::to_value(tasks)?);
        }
        let db = serde_json::json!({ "backlog": backlog, "contexts": map });
        fs::write(output, serde_json::to_string_pretty(&db)?)
            .with_context(|| format!("cannot write {output}"))?;
        Ok((backlog.len(), contexts.len()))
    }
}

/// Load every task file (one JSON per file) from a directory.
/// A missing directory is an empty task list; a corrupted file is skipped
/// with a warning instead of failing the whole command.
pub fn load_dir(dir: &Path) -> Result<Vec<Task>> {
    let mut tasks = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(tasks),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("cannot read {}", path.display()))?;
            match serde_json::from_str::<Task>(&content) {
                Ok(task) => tasks.push(task),
                Err(_) => eprintln!("warning: skipping invalid task file {}", path.display()),
            }
        }
    }
    tasks.sort_by(|a, b| a.created.cmp(&b.created));
    Ok(tasks)
}

/// Match a (possibly partial) identifier against task uids, git-style.
/// Returns the unique match, or an error when nothing or several match.
pub fn match_task<'a>(tasks: &'a [Task], identifier: &str) -> Result<&'a Task> {
    let matches: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.uid.starts_with(identifier))
        .collect();
    match matches.len() {
        0 => bail!("no task found with identifier '{identifier}'"),
        1 => Ok(matches[0]),
        n => bail!("identifier '{identifier}' is ambiguous ({n} matches)"),
    }
}

/// Write a task to its file inside 'dir'.
pub fn write_task(dir: &Path, task: &Task) -> Result<()> {
    let path = dir.join(format!("{}.json", task.uid));
    fs::write(&path, serde_json::to_string_pretty(task)?)
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

/// Branch names may contain '/' which cannot be a single directory name.
pub fn sanitize(branch: &str) -> String {
    branch.replace('/', "--")
}

pub fn desanitize(dir: &str) -> String {
    dir.replace("--", "/")
}

fn new_uid(description: &str) -> String {
    let now = chrono::Utc::now().to_rfc3339();
    let mut hasher = Sha1::new();
    hasher.update(description.as_bytes());
    hasher.update(now.as_bytes());
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(uid: &str) -> Task {
        Task {
            uid: uid.to_string(),
            description: "sample".to_string(),
            author: "tester".to_string(),
            created: "2021-01-01T00:00:00Z".to_string(),
            updated: "2021-01-01T00:00:00Z".to_string(),
            done: false,
        }
    }

    #[test]
    fn sanitize_round_trip() {
        assert_eq!(sanitize("feature/fix"), "feature--fix");
        assert_eq!(sanitize("main"), "main");
        assert_eq!(desanitize(&sanitize("feature/fix")), "feature/fix");
    }

    #[test]
    fn task_json_round_trip() {
        let task = sample("abc");
        let json = serde_json::to_string(&task).unwrap();
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(back, task);
    }

    #[test]
    fn uids_are_unique() {
        // Same description, consecutive calls: uids must still differ.
        assert_ne!(new_uid("same"), new_uid("same"));
    }

    #[test]
    fn match_task_accepts_prefix_and_full_id() {
        let tasks = vec![sample("abcdef1234"), sample("abz4567890")];
        assert_eq!(match_task(&tasks, "abcdef1234").unwrap().uid, "abcdef1234");
        assert_eq!(match_task(&tasks, "abc").unwrap().uid, "abcdef1234");
        assert_eq!(match_task(&tasks, "abz").unwrap().uid, "abz4567890");
    }

    #[test]
    fn match_task_rejects_unknown_identifier() {
        let tasks = vec![sample("abcdef1234")];
        assert!(match_task(&tasks, "zzz").is_err());
    }

    #[test]
    fn match_task_rejects_ambiguous_identifier() {
        let tasks = vec![sample("abcdef1234"), sample("abc4567890")];
        assert!(match_task(&tasks, "abc").is_err());
    }

    #[test]
    fn load_dir_on_missing_directory_is_empty() {
        let missing = Path::new("/nonexistent-dot-dir");
        assert!(load_dir(missing).unwrap().is_empty());
    }

    #[test]
    fn init_creates_layout_and_ensure_init_guards() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        assert!(!store.is_initialized());
        assert!(store.ensure_init().is_err());
        store.init().unwrap();
        assert!(store.is_initialized());
        assert!(store.todo_dir().is_dir());
        assert!(store.backlog_dir().is_dir());
        store.ensure_init().unwrap();
    }
}
