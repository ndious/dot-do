use anyhow::Result;

use crate::git;
use crate::store::Store;

pub fn init() -> Result<()> {
    let store = Store::discover()?;
    store.init()?;
    println!("Initialized dot in {}", store.dot_dir().display());
    Ok(())
}

pub fn add(description: &str, backlog: bool) -> Result<()> {
    let store = Store::discover()?;
    let author = git::user_name();
    let task = store.add(description, &author, backlog)?;
    println!("Added #{} {}", &task.uid[..5], task.description);
    Ok(())
}

pub fn ls(backlog: bool) -> Result<()> {
    let store = Store::discover()?;
    store.ensure_init()?;
    if backlog {
        let tasks = store.load_backlog()?;
        if tasks.is_empty() {
            println!("No backlog todos available");
            return Ok(());
        }
        println!("Backlog:");
        for task in &tasks {
            println!("  #{}  {}  ({})", &task.uid[..5], task.description, &task.created[..10]);
        }
    } else {
        let branch = store.branch()?;
        let tasks = store.load_context(&branch)?;
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
    let store = Store::discover()?;
    let task = store.complete(identifier)?;
    println!("Completed #{} {}", &task.uid[..5], task.description);
    Ok(())
}

pub fn rm(identifier: &str, backlog: bool) -> Result<()> {
    let store = Store::discover()?;
    let task = store.remove(identifier, backlog)?;
    println!("Deleted #{} {}", &task.uid[..5], task.description);
    Ok(())
}

pub fn mv(identifier: &str, to_backlog: bool) -> Result<()> {
    let store = Store::discover()?;
    let task = store.move_task(identifier, to_backlog)?;
    let destination = if to_backlog {
        "backlog".to_string()
    } else {
        format!("context '{}'", store.branch()?)
    };
    println!("Moved #{} {} to {destination}", &task.uid[..5], task.description);
    Ok(())
}

pub fn ctx() -> Result<()> {
    let store = Store::discover()?;
    store.ensure_init()?;
    let contexts = store.contexts()?;
    if contexts.is_empty() {
        println!("No contexts with todos");
        return Ok(());
    }
    for (name, tasks) in &contexts {
        println!("{}: {} todo(s)", name, tasks.len());
    }
    Ok(())
}

pub fn resolve(context: &str) -> Result<()> {
    let store = Store::discover()?;
    let count = store.resolve(context)?;
    println!("Resolved context '{context}': {count} todo(s) deleted");
    Ok(())
}

pub fn export(output: &str) -> Result<()> {
    let store = Store::discover()?;
    let (backlog, contexts) = store.export(output)?;
    println!("Exported {backlog} backlog todo(s) and {contexts} context(s) to {output}");
    Ok(())
}
