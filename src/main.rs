mod commands;
mod git;
mod store;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// dot — a git-native todo CLI.
///
/// Each git branch is a context. Each task is a single file, named by
/// a sha1 identifier that you can abbreviate, just like git commits.
#[derive(Parser)]
#[command(name = "dot", version, about = "A git-native todo CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the dot storage in the current git repository
    Init,
    /// Create a new todo in the current context, or in the backlog (-b)
    Add {
        /// Create the todo in the backlog instead of the current context
        #[arg(short, long)]
        backlog: bool,
        /// Description of the todo
        description: String,
    },
    /// List todos of the current context, or of the backlog (-b)
    Ls {
        /// List the backlog instead of the current context
        #[arg(short, long)]
        backlog: bool,
    },
    /// Mark a task as complete (current context only)
    X {
        /// Full or partial task identifier
        identifier: String,
    },
    /// Delete a task
    Rm {
        /// Delete from the backlog instead of the current context
        #[arg(short, long)]
        backlog: bool,
        /// Full or partial task identifier
        identifier: String,
    },
    /// Move a task between the backlog and the current context
    Mv {
        /// Move from the current context to the backlog
        #[arg(short, long)]
        backlog: bool,
        /// Full or partial task identifier
        identifier: String,
    },
    /// List contexts (branches) that still hold todos
    Ctx,
    /// Close a context: delete all its todos
    Resolve {
        /// Context (branch) name
        context: String,
    },
    /// Export the whole dot database as a JSON file
    Export {
        /// Output file path
        output: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => store::init(),
        Commands::Add { backlog, description } => store::add(&description, backlog),
        Commands::Ls { backlog } => commands::ls(backlog),
        Commands::X { identifier } => commands::done(&identifier),
        Commands::Rm { backlog, identifier } => commands::rm(&identifier, backlog),
        Commands::Mv { backlog, identifier } => commands::mv(&identifier, backlog),
        Commands::Ctx => commands::ctx(),
        Commands::Resolve { context } => commands::resolve(&context),
        Commands::Export { output } => commands::export(&output),
    }
}
