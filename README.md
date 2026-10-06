# dot-do

A git-native todo CLI, written in Rust. Rust port of the
[tod-cli](https://github.com/timber-dev-society/tod-cli) concept.

## Main concepts

- A git branch is a **context**: switch branches and you switch todos.
- One task = one independent JSON file, to avoid merge conflicts.
- Each task file is named by a **sha1 identifier**. Like git commits,
  you only need the first characters of an identifier to target a task.
- A **backlog** holds tasks that do not belong to any branch yet.

## Install

Requires Rust (stable) and a git repository.

```bash
cargo install --path .
```

## Usage

```bash
dot init                    # create the .dot/ storage
dot add "Fix the login bug" # add a todo to the current branch
dot add -b "Refactor core" # add a todo to the backlog
dot ls                      # list todos of the current context
dot ls -b                   # list the backlog
dot x 919fe                 # mark task as complete
dot rm 919fe                # delete a task
dot rm -b 919fe             # delete a backlog task
dot mv 919fe                # backlog -> current context
dot mv -b 919fe             # current context -> backlog
dot ctx                     # list contexts with todos
dot resolve feature/login   # close a context
dot export todos.json       # export the whole database
```

## Storage layout

```
.dot/
  todo/<branch>/<sha1>.json   # tasks of each branch (context)
  backlog/<sha1>.json         # tasks waiting for a context
```

Add `.dot/` to your .gitignore if you do not want to share your todos,
or commit it to share a team backlog.

## Differences with tod-cli

- Rust + clap instead of Node.js + commander/redux.
- Tasks are stored as JSON files (human readable, easy to merge).
- No redux: plain, explicit functions. Errors are reported, never swallowed.
- Same command set: init, add, ls, x, rm, mv, ctx, resolve, export.

## License

MIT
