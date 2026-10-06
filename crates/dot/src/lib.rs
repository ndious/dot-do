//! dot — the library behind the tod CLI.
//!
//! A Rust port of the "dot" library of
//! [tod-cli](https://github.com/timber-dev-society/tod-cli): a tiny
//! schema-based document format ("Key: value" lines, tab-indented
//! multi-line values) used to store one todo per plain-text file,
//! plus the git-native todo store and the project planning tools
//! built on top of it.
//!
//! ## Planning layer
//!
//! On top of the one-file-per-task storage, the library provides a
//! project-wide planning layer:
//!
//! - `.tod/plan.dot`: a single central document tracking the plan
//!   (micro-todo list) of every task that has one. Keys are task
//!   uids, values are the tab-indented steps (`[x] done` /
//!   `[ ] todo`), in the regular dot format.
//! - `.tod/md/<uid>.md`: the generated spec markdown of each task,
//!   the brief meant to be handed to a development agent.
//!
//! The plan file is the source of truth; markdowns are generated.

pub mod brief;
pub mod git;
pub mod plan;
pub mod schema;
pub mod store;
pub mod task;
pub mod types;
