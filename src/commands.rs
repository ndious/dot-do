use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::git;
use crate::store::{self, Task};

/// Load every task file (one JSON per file) from a directory.
fn load_dir(dir: &Path) -> Result<Vec<Task>> {
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
fn match_task<'a>(tasks: &'a [Task], identifier: &str) -> Result<&'a Task> {
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

pub fn ls(backlog: bool) -> Result<()> {
    store::ensure_init()?;
    if backlog {
        let tasks = load_dir(&store::backlog_dir()?)?;
        if tasks.is_empty() {
            println!("No backlog todos available");
            return Ok(());
        }
        println!("Backlog:");
        for task in &tasks {
            println!("  #{}  {}  ({})", &task.uid[..5], task.description, &task.created[..10]);
        }
    } else {
        let branch = git::current_branch()?;
        let tasks = load_dir(&store::context_dir(&branch)?)?;
        if tasks.is_empty() {
            println!("No todos available in context '{branch}'");
            return Ok(());
        }
        for task in &tasks {
            let mark = if task.done { "[x]" } else { "[ ]" };
            println!("{mark} #{}  {}", &task.uid[..5], task.description);
        }
    }
    Ok(())
}

pub fn done(identifier: &str) -> Result<()> {
    store::ensure_init()?;
    let branch = git::current_branch()?;
    let dir = store::context_dir(&branch)?;
    let tasks = load_dir(&dir)?;
    let task = match_task(&tasks, identifier)?.clone();
    if task.done {
        bail!("task #{} is already complete", &task.uid[..5]);
    }
    let mut task = task;
    task.done = true;
    task.updated = chrono::Utc::now().to_rfc3339();
    store::write_task(&dir, &task)?;
    println!("Completed #{} {}", &task.uid[..5], task.description);
    Ok(())
}

pub fn rm(identifier: &str, backlog: bool) -> Result<()> {
    store::ensure_init()?;
    let dir = if backlog {
        store::backlog_dir()?
    } else {
        store::context_dir(&git::current_branch()?)?
    };
    let tasks = load_dir(&dir)?;
    let task = match_task(&tasks, identifier)?;
    fs::remove_file(dir.join(format!("{}.json", task.uid)))?;
    println!("Deleted #{} {}", &task.uid[..5], task.description);
    Ok(())
}

/// Move a task between the backlog and the current context.
pub fn mv(identifier: &str, to_backlog: bool) -> Result<()> {
    store::ensure_init()?;
    let branch = git::current_branch()?;
    let (from, to) = if to_backlog {
        (store::context_dir(&branch)?, store::backlog_dir()?)
    } else {
        (store::backlog_dir()?, store::context_dir(&branch)?)
    };
    let tasks = load_dir(&from)?;
    let task = match_task(&tasks, identifier)?.clone();
    fs::create_dir_all(&to)?;
    fs::rename(
        from.join(format!("{}.json", task.uid)),
        to.join(format!("{}.json", task.uid)),
    )?;
    let destination = if to_backlog {
        "backlog".to_string()
    } else {
        format!("context '{branch}'")
    };
    println!("Moved #{} {} to {destination}", &task.uid[..5], task.description);
    Ok(())
}

/// List every context (branch) that still holds todos.
pub fn ctx() -> Result<()> {
    store::ensure_init()?;
    let mut found = false;
    if let Ok(entries) = fs::read_dir(store::todo_dir()?) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let count = load_dir(&entry.path())?.len();
                if count > 0 {
                    println!("{}: {} todo(s)", store::desanitize(&name), count);
                    found = true;
                }
            }
        }
    }
    if !found {
        println!("No contexts with todos");
    }
    Ok(())
}

/// Close a context: delete every task of a branch and its folder.
pub fn resolve(context: &str) -> Result<()> {
    store::ensure_init()?;
    let dir = store::context_dir(context)?;
    if !dir.is_dir() {
        bail!("unknown context '{context}'");
    }
    let count = load_dir(&dir)?.len();
    fs::remove_dir_all(&dir).with_context(|| format!("cannot remove {}", dir.display()))?;
    println!("Resolved context '{context}': {count} todo(s) deleted");
    Ok(())
}

/// Export the whole database (backlog + every context) as a JSON file.
pub fn export(output: &str) -> Result<()> {
    store::ensure_init()?;
    let backlog = load_dir(&store::backlog_dir()?)?;
    let mut contexts = serde_json::Map::new();
    if let Ok(entries) = fs::read_dir(store::todo_dir()?) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let name = store::desanitize(&entry.file_name().to_string_lossy());
                let tasks = load_dir(&entry.path())?;
                contexts.insert(name, serde_json::to_value(tasks)?);
            }
        }
    }
    let db = serde_json::json!({ "backlog": backlog, "contexts": contexts });
    fs::write(output, serde_json::to_string_pretty(&db)?)
        .with_context(|| format!("cannot write {output}"))?;
    println!(
        "Exported {} backlog todo(s) and {} context(s) to {output}",
        backlog.len(),
        contexts.len()
    );
    Ok(())
}
