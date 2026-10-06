use std::fs;
use std::io::{IsTerminal, Write};

use anyhow::Result;

use dot::brief;
use dot::git;
use dot::plan::Project;
use dot::store::Store;
use dot::task::Task;

pub fn init(skill: bool, no_skill: bool) -> Result<()> {
    let store = Store::discover()?;
    store.init()?;
    println!("Initialized tod in {}", store.dot_dir().display());

    // --skill / --no-skill decide directly; otherwise ask (default
    // yes) when running interactively, and stay silent in scripts.
    let install = if skill {
        true
    } else if no_skill {
        false
    } else if std::io::stdin().is_terminal() {
        ask_install_skill()?
    } else {
        false
    };
    if install {
        commands::install_skill(&store)?;
    }
    Ok(())
}

fn ask_install_skill() -> Result<bool> {
    print!(
        "\nInstall the tod skill for your AI agent? [Y/n] "
    );
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    let answer = answer.trim().to_lowercase();
    Ok(answer.is_empty() || answer == "y" || answer == "yes")
}

pub fn skill(remove: bool) -> Result<()> {
    let store = Store::discover()?;
    if remove {
        if skill_lib::remove(&store)? {
            println!("Removed the agent skill");
        } else {
            println!("No agent skill installed");
        }
    } else {
        commands::install_skill(&store)?;
    }
    Ok(())
}

mod commands {
    use super::*;

    pub fn install_skill(store: &Store) -> Result<()> {
        let path = crate::skill::install(store)?;
        println!("Installed the agent skill: {}", path.display());
        println!("Commit it so your agent picks it up in every checkout.");
        Ok(())
    }
}
// re-export for init()
mod skill_lib {
    pub use crate::skill::*;
}

pub fn add(description: &str, backlog: bool) -> Result<()> {
    let store = Store::discover()?;
    let author = git::user_name();
    let task = store.add(description, &author, backlog)?;
    println!("Added #{} {}", &task.uid[..5], task.description);
    Ok(())
}

pub fn plan(description: &str, backlog: bool) -> Result<()> {
    let store = Store::discover()?;
    let author = git::user_name();
    let task = store.add(description, &author, backlog)?;
    let context = if backlog {
        "backlog".to_string()
    } else {
        store.branch()?
    };

    // an empty plan in the central project file
    let mut project = Project::load(&store)?;
    project.ensure_plan(&task.uid);
    project.save(&store)?;

    // the spec markdown skeleton
    let plan = project.plan(&task.uid).cloned().unwrap_or_default();
    let path = brief::write_brief(&store, &task, &context, &plan)?;
    println!("Planned #{} {} — {}", &task.uid[..5], task.description, path.display());
    Ok(())
}

pub fn sub(identifier: &str, step: &str) -> Result<()> {
    let store = Store::discover()?;
    store.ensure_init()?;
    let (context, task) = store.find_task(identifier)?;

    let mut project = Project::load(&store)?;
    let number = project.add_step(&task.uid, step);
    project.save(&store)?;
    refresh_brief(&store, &task, &context, &project)?;

    println!("Added step {number} to #{}: {step}", &task.uid[..5]);
    Ok(())
}

pub fn check(identifier: &str, number: usize) -> Result<()> {
    let store = Store::discover()?;
    store.ensure_init()?;
    let (context, task) = store.find_task(identifier)?;

    let mut project = Project::load(&store)?;
    let step = project.check_step(&task.uid, number)?;
    project.save(&store)?;
    refresh_brief(&store, &task, &context, &project)?;

    let mark = if step.done { 'x' } else { ' ' };
    println!("[{mark}] step {number} of #{}: {}", &task.uid[..5], step.text);
    Ok(())
}

pub fn brief(identifier: &str) -> Result<()> {
    let store = Store::discover()?;
    store.ensure_init()?;
    let (context, task) = store.find_task(identifier)?;
    let project = Project::load(&store)?;
    let plan = project.plan(&task.uid).cloned().unwrap_or_default();

    let path = brief::write_brief(&store, &task, &context, &plan)?;
    let content = fs::read_to_string(&path)?;
    println!("{content}");
    Ok(())
}

pub fn status() -> Result<()> {
    let store = Store::discover()?;
    store.ensure_init()?;
    let project = Project::load(&store)?;

    let print = |label: &str, tasks: &[Task]| {
        println!("{label}:");
        if tasks.is_empty() {
            println!("  (none)");
            return;
        }
        for task in tasks {
            let mark = if task.done { 'x' } else { ' ' };
            let progress = match project.plan(&task.uid) {
                Some(plan) if !plan.is_empty() => {
                    format!(" [{} {}]", plan.done_count(), plan.steps.len())
                }
                _ => String::new(),
            };
            println!("  [{mark}] #{} {}{progress}", &task.uid[..5], task.description);
        }
    };

    print("Backlog", &store.load_backlog()?);
    for (name, tasks) in store.contexts()? {
        print(&name, &tasks);
    }
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
            let created = task.created.format("%Y-%m-%d");
            println!("  #{}  {}  ({})", &task.uid[..5], task.description, created);
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
    forget_task(&store, &[task.uid])?;
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
    store.ensure_init()?;
    // collect the uids before deleting, to clean up plans and briefs
    let tasks = store.load_context(context)?;
    let count = store.resolve(context)?;
    let uids: Vec<String> = tasks.iter().map(|t| t.uid.clone()).collect();
    forget_task(&store, &uids)?;
    println!("Resolved context '{context}': {count} todo(s) deleted");
    Ok(())
}

pub fn export(output: &str) -> Result<()> {
    let store = Store::discover()?;
    let (backlog, contexts) = store.export(output)?;
    println!("Exported {backlog} backlog todo(s) and {contexts} context(s) to {output}");
    Ok(())
}

/// Remove the plan entries and spec markdowns of deleted tasks.
fn forget_task(store: &Store, uids: &[String]) -> Result<()> {
    let mut project = Project::load(store)?;
    let mut changed = false;
    for uid in uids {
        project.remove(uid);
        changed = true;
        let _ = fs::remove_file(store.md_dir().join(format!("{uid}.md")));
    }
    if changed {
        project.save(store)?;
    }
    Ok(())
}

/// Regenerate the spec markdown, but only if it already exists:
/// 'tod brief' is the explicit way to (re)generate a brief.
fn refresh_brief(store: &Store, task: &Task, context: &str, project: &Project) -> Result<()> {
    let path = store.md_dir().join(format!("{}.md", task.uid));
    if path.exists() {
        let plan = project.plan(&task.uid).cloned().unwrap_or_default();
        brief::write_brief(store, task, context, &plan)?;
    }
    Ok(())
}
