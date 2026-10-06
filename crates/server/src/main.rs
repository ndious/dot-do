//! tod-server: multi-repo project management server on top of the
//! dot store. Login/password (SQLite), git clone/pull, JSON API and
//! the Yew web console.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;

mod api;
mod db;
mod repos;

#[derive(Parser, Debug)]
#[command(name = "tod-server", about = "Multi-repo project management server for .tod repositories")]
struct Args {
    /// Port to listen on
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// SQLite database file
    #[arg(long, default_value = "tod-server.db")]
    db: PathBuf,

    /// Directory where remote repositories are cloned
    #[arg(long, default_value = ".tod-server")]
    workdir: PathBuf,

    /// Directory holding the built web console (crates/web/dist)
    #[arg(long, default_value = "dist")]
    static_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let database = db::Db::open(&args.db)?;
    database.bootstrap_from_env()?;

    let state = Arc::new(api::AppState {
        db: database,
        workdir: args.workdir.clone(),
    });

    let router = api::router(state).fallback_service(
        tower_http::services::ServeDir::new(&args.static_dir),
    );

    let addr = format!("0.0.0.0:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;
    println!(
        "tod-server listening on http://{addr} (db: {}, workdir: {}, static: {})",
        args.db.display(),
        args.workdir.display(),
        args.static_dir.display()
    );
    axum::serve(listener, router).await?;
    Ok(())
}
