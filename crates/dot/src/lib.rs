//! dot — the library behind the dot-do CLI.
//!
//! A Rust port of the "dot" library of
//! [tod-cli](https://github.com/timber-dev-society/tod-cli): a tiny
//! schema-based document format ("Key: value" lines, tab-indented
//! multi-line values) used to store one todo per plain-text file, plus
//! the git-native todo store built on top of it.
//!
//! Layout of a task file:
//!
//! ```text
//! Description: Fix the login bug
//! Author: ndious
//! Created: 2026-10-06 18:12:44 +0000
//! Updated: 2026-10-06 18:12:44 +0000
//! Done: 0
//! Content:
//!     First line of a multi-line value
//!     Second line
//! ```

pub mod git;
pub mod schema;
pub mod store;
pub mod task;
pub mod types;
