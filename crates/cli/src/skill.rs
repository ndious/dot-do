//! The agent skill: a SKILL.md installed in the repository so that
//! an AI coding agent knows how to read and update the tod database.

use std::path::PathBuf;

use anyhow::{Context, Result};

use dot::store::Store;

/// Folder where the skill is installed (agent convention: one
/// folder per skill, with a SKILL.md entrypoint).
pub const SKILL_DIR: &str = ".agents/skills/tod";

pub fn skill_path(store: &Store) -> PathBuf {
    store.root().join(SKILL_DIR).join("SKILL.md")
}

/// The skill content. Kept in sync with the README.
pub const SKILL_MD: &str = r#"---
name: tod
description: Manage the todos, plans and briefs of this repository with the tod CLI. Use it before starting work (pick the next step), and after completing work (check the step, read the brief).
---

# tod — git-native todo & planning for this repository

This repository tracks its work with **tod**: one plain-text file per
task in `.tod/`, a central plan document `.tod/plan.dot`, and agent
briefs in `.tod/md/<uid>.md`.

## Concepts

- A git **branch is a context**: tasks live per branch, plus a
  shared **backlog**.
- Each task has a **sha1 uid**; you can abbreviate it, git-style
  (`919fe` for `919fea...`).
- Complex tasks have a **plan**: a numbered list of micro todos.
- The **brief** (`tod brief <id>`) is the up-to-date spec markdown
  of a task: objective, context, status, plan with progress.

## Workflow for an agent

1. **Pick work**: run `tod status` (overview) or `tod ls` (current
   context). Choose the next task, or ask the human which one.
2. **Read the brief**: `tod brief <id>` — it is the source of truth
   for what the task is about.
3. **Execute the next undone step** of the plan, exactly one
   micro-todo at a time. Do not skip steps; if a step is wrong or
   blocked, say so instead of marking it done.
4. **Mark the step done**: `tod check <id> <number>` (1-based, as
   shown by status). Only check steps you actually completed.
5. **When a step reveals new work**, propose it to the human; the
   human adds it with `tod sub <id> "text"`. Never rewrite
   `.tod/plan.dot` by hand.
6. **When the whole task is done**, ask the human to run
   `tod x <id>`; do not complete a task on your own.

## Commands

```bash
tod status              # backlog + contexts, with [done total] progress
tod ls / ls -b          # todos of the current context / of the backlog
tod brief <id>          # the agent-facing spec markdown of a task
tod check <id> <n>      # toggle step n of a task's plan
tod add "text"          # (human) new todo in the current context
tod plan "text"         # (human) new task with a plan + brief
tod sub <id> "text"     # (human) add a step to a plan
tod x <id>              # (human) mark a task complete
tod rm / mv / ctx / resolve / export   # (human) maintenance
```

## Storage (read-only for agents)

```
.tod/plan.dot              # every plan, one document — do not hand-edit
.tod/md/<sha1>.md          # generated briefs — do not hand-edit
.tod/todo/<branch>/<sha1> # task files (format dot) — do not hand-edit
.tod/backlog/<sha1>        # backlog tasks — do not hand-edit
```

Agents interact with tod **only through the CLI**. The files above
are the human's storage; the briefs in `.tod/md/` are generated.

## Rules

- Never mark a step or task done unless you finished it.
- One step at a time: small, verifiable changes.
- If the plan is stale (the code moved on), tell the human instead
  of improvising changes to the plan.
"#;

/// Install (or overwrite) the skill. Returns the SKILL.md path.
pub fn install(store: &Store) -> Result<PathBuf> {
    let path = skill_path(store);
    let dir = path.parent().expect("skill path has a parent");
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    std::fs::write(&path, SKILL_MD)
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(path)
}

/// Remove the skill. Ok(false) when it was not installed.
pub fn remove(store: &Store) -> Result<bool> {
    let path = skill_path(store);
    if !path.is_file() {
        return Ok(false);
    }
    std::fs::remove_file(&path).with_context(|| format!("cannot remove {}", path.display()))?;
    // drop the folder too when it is now empty
    let _ = std::fs::remove_dir(path.parent().expect("skill path has a parent"));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_then_remove_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let path = install(&store).unwrap();
        assert!(path.is_file());
        assert!(path.ends_with(".agents/skills/tod/SKILL.md"));
        assert!(std::fs::read_to_string(&path).unwrap().contains("# tod"));
        assert!(remove(&store).unwrap());
        assert!(!path.exists());
        assert!(!remove(&store).unwrap()); // already gone
    }
}
