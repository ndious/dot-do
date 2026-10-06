//! The JSON API of tod-server, built on the dot crate.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use dot::brief;
use dot::plan::Project;
use dot::store::Store;
use dot::task::Task;

/// Shared state: the store of the managed repository.
pub struct App {
    pub store: Store,
}

impl App {
    pub fn new(root: PathBuf) -> Self {
        App {
            store: Store::new(root),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StepView {
    pub done: bool,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
    pub uid: String,
    pub description: String,
    pub author: String,
    pub done: bool,
    pub context: String,
    pub plan: Vec<StepView>,
    pub progress: (usize, usize),
}

#[derive(Debug, Serialize)]
pub struct ContextView {
    pub name: String,
    pub tasks: Vec<TaskView>,
}

#[derive(Debug, Serialize)]
pub struct Status {
    pub root: String,
    pub branch: String,
    pub backlog: Vec<TaskView>,
    pub contexts: Vec<ContextView>,
}

fn task_view(task: &Task, context: &str, project: &Project) -> TaskView {
    let plan = project
        .plan(&task.uid)
        .map(|p| {
            p.steps
                .iter()
                .map(|s| StepView {
                    done: s.done,
                    text: s.text.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let done_count = plan.iter().filter(|s| s.done).count();
    TaskView {
        uid: task.uid.clone(),
        description: task.description.clone(),
        author: task.author.clone(),
        done: task.done,
        context: context.to_string(),
        progress: (done_count, plan.len()),
        plan,
    }
}

fn err<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

async fn status(State(app): State<Arc<App>>) -> Result<Json<Status>, (StatusCode, String)> {
    let s = &app.store;
    let project = Project::load(s).map_err(err)?;
    let branch = s.branch().map_err(err)?;
    let backlog = s.load_backlog().map_err(err)?;
    let contexts = s.contexts().map_err(err)?;

    Ok(Json(Status {
        root: s.root().display().to_string(),
        branch,
        backlog: backlog
            .iter()
            .map(|t| task_view(t, "backlog", &project))
            .collect(),
        contexts: contexts
            .iter()
            .map(|(name, tasks)| ContextView {
                name: name.clone(),
                tasks: tasks
                    .iter()
                    .map(|t| task_view(t, name, &project))
                    .collect(),
            })
            .collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct AddTaskBody {
    description: String,
    #[serde(default)]
    context: Option<String>,
}

async fn add_task(
    State(app): State<Arc<App>>,
    Json(body): Json<AddTaskBody>,
) -> Result<Json<TaskView>, (StatusCode, String)> {
    if body.description.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty description".to_string()));
    }
    let task = app
        .store
        .add_in(body.context.as_deref(), body.description.trim(), "web")
        .map_err(err)?;
    let project = Project::load(&app.store).map_err(err)?;
    let context = body.context.unwrap_or_else(|| "backlog".to_string());
    Ok(Json(task_view(&task, &context, &project)))
}

#[derive(Debug, Deserialize)]
struct SubStepBody {
    uid: String,
    text: String,
}

/// Regenerate the spec markdown of a task, but only if the brief
/// already exists (the file is opt-in, created by 'tod brief').
fn refresh_brief(store: &Store, task: &Task, context: &str) -> anyhow::Result<()> {
    if !store.md_dir().join(format!("{}.md", task.uid)).is_file() {
        return Ok(());
    }
    let project = Project::load(store)?;
    let plan = project.plan(&task.uid).cloned().unwrap_or_default();
    brief::write_brief(store, task, context, &plan)?;
    Ok(())
}

async fn add_sub(
    State(app): State<Arc<App>>,
    Json(body): Json<SubStepBody>,
) -> Result<Json<TaskView>, (StatusCode, String)> {
    let (context, task) = app.store.find_task(&body.uid).map_err(err)?;
    let mut project = Project::load(&app.store).map_err(err)?;
    project.add_step(&task.uid, body.text.trim());
    project.save(&app.store).map_err(err)?;
    refresh_brief(&app.store, &task, &context).map_err(err)?;
    let project = Project::load(&app.store).map_err(err)?;
    Ok(Json(task_view(&task, &context, &project)))
}

#[derive(Debug, Deserialize)]
struct CheckStepBody {
    uid: String,
    number: usize,
}

async fn check_step(
    State(app): State<Arc<App>>,
    Json(body): Json<CheckStepBody>,
) -> Result<Json<TaskView>, (StatusCode, String)> {
    let (context, task) = app.store.find_task(&body.uid).map_err(err)?;
    let mut project = Project::load(&app.store).map_err(err)?;
    project.check_step(&task.uid, body.number).map_err(err)?;
    project.save(&app.store).map_err(err)?;
    refresh_brief(&app.store, &task, &context).map_err(err)?;
    let project = Project::load(&app.store).map_err(err)?;
    Ok(Json(task_view(&task, &context, &project)))
}

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/task", post(add_task))
        .route("/api/sub", post(add_sub))
        .route("/api/check", post(check_step))
        .with_state(app)
}
