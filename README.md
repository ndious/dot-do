# dot-do

A git-native todo CLI, written in Rust. Rust port of the
[tod-cli](https://github.com/timber-dev-society/tod-cli) concept.

## Main concepts

- A git branch is a **context**: switch branches and you switch todos.
- One task = one independent plain-text file, to avoid merge conflicts.
- Each task file is named by a **sha1 identifier**. Like git commits,
  you only need the first characters of an identifier to target a task.
- A **backlog** holds tasks that do not belong to any branch yet.
- Task files use the **dot document format** of the original lib:
  `Key: value` lines, tab-indented multi-line values.

## Monorepo layout

```
crates/dot     the dot library: document format (schema, parser,
               stringifier, typed fields), Task, git helpers, Store
crates/cli     the CLI (clap) built on top of the dot crate,
               binary name: tod
```

The `dot` crate is the Rust port of tod-cli's `src/core/dot` library:
a schema-based document format with typed fields (text, bool, ISO8601
date), Title Case file keys converted to camelCase, and multi-line
values indented with tabs.

## Task file format

One file per task, named by its sha1 uid:

```text
Description: Fix the login bug
Author: ndious
Created: 2026-10-06 18:12:44 +0000
Updated: 2026-10-06 18:12:44 +0000
Done: 0
Content:
    First line of a multi-line value
    Second line
```

## Install

Requires Rust (stable) and a git repository.

```bash
cargo install --path crates/cli
```

## Usage

```bash
tod init                    # create the .dot/ storage
tod add "Fix the login bug" # add a todo to the current branch
tod add -b "Refactor core" # add a todo to the backlog
tod ls                      # list todos of the current context
tod ls -b                   # list the backlog
tod x 919fe                 # mark task as complete
tod rm 919fe                # delete a task
tod rm -b 919fe             # delete a backlog task
tod mv 919fe                # backlog -> current context
tod mv -b 919fe             # current context -> backlog
tod ctx                     # list contexts with todos
tod resolve feature/login   # close a context
tod export todos.json       # export the whole database
```

## Storage layout

```
.dot/
  todo/<branch>/<sha1>       # tasks of each branch (context)
  backlog/<sha1>             # tasks waiting for a context
```

Add `.dot/` to your .gitignore if you do not want to share your todos,
or commit it to share a team backlog.

## Differences with tod-cli

- Rust + clap instead of Node.js + commander/redux.
- The dot document library is a standalone crate, usable without the CLI.
- No redux: plain, explicit functions. Errors are reported, never swallowed.
- Same command set: init, add, ls, x, rm, mv, ctx, resolve, export.

## License

MIT
