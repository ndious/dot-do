//! tod web console — a Yew (CSR) app served by the tod project server.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepView {
    pub done: bool,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskView {
    pub uid: String,
    pub description: String,
    pub author: String,
    pub done: bool,
    pub context: String,
    pub plan: Vec<StepView>,
    pub progress: (usize, usize),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextView {
    pub name: String,
    pub tasks: Vec<TaskView>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub root: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub backlog: Vec<TaskView>,
    #[serde(default)]
    pub contexts: Vec<ContextView>,
}

async fn api_get_status() -> Option<Status> {
    let resp = Request::get("/api/status").send().await.ok()?;
    resp.json::<Status>().await.ok()
}

async fn api_post(path: &str, body: &serde_json::Value) -> bool {
    match Request::post(path).json(body).send().await {
        Ok(resp) => resp.ok(),
        Err(_) => false,
    }
}

/// Fetch the whole status and update the state.
fn refresh(status: &UseStateHandle<Status>) -> Callback<()> {
    let status = status.clone();
    Callback::from(move |_| {
        let status = status.clone();
        spawn_local(async move {
            if let Some(s) = api_get_status().await {
                status.set(s);
            }
        });
    })
}

#[derive(Properties, PartialEq)]
pub struct TaskCardProps {
    pub task: TaskView,
    pub on_refresh: Callback<()>,
}

#[function_component(TaskCard)]
pub fn task_card(props: &TaskCardProps) -> Html {
    let task = &props.task;
    let new_step = use_state(String::new);

    let on_check = {
        let uid = task.uid.clone();
        let on_refresh = props.on_refresh.clone();
        Callback::from(move |n: usize| {
            let uid = uid.clone();
            let on_refresh = on_refresh.clone();
            spawn_local(async move {
                let body = serde_json::json!({ "uid": uid, "number": n });
                api_post("/api/check", &body).await;
                on_refresh.emit(());
            });
        })
    };

    let steps = task
        .plan
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let n = i + 1;
            let on_check = on_check.clone();
            let onclick = Callback::from(move |_| on_check.emit(n));
            let class = if s.done { "step done" } else { "step" };
            let mark = if s.done { "[x]" } else { "[ ]" };
            html! { <li class={class} {onclick}>{ format!("{mark} {}", s.text) }</li> }
        })
        .collect::<Html>();

    let on_submit_step = {
        let uid = task.uid.clone();
        let new_step = new_step.clone();
        let on_refresh = props.on_refresh.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let text = (*new_step).clone();
            if text.trim().is_empty() {
                return;
            }
            let uid = uid.clone();
            let on_refresh = on_refresh.clone();
            spawn_local(async move {
                let body = serde_json::json!({ "uid": uid, "text": text });
                api_post("/api/sub", &body).await;
                on_refresh.emit(());
            });
        })
    };

    let class = if task.done { "task done" } else { "task" };
    let short = &task.uid[..task.uid.len().min(7)];

    html! {
        <li class={class}>
            <span class="desc">{ task.description.clone() }</span>
            <span class="uid">{ short }</span>
            if !task.plan.is_empty() {
                <span class="progress">{ format!("{}/{}", task.progress.0, task.progress.1) }</span>
            }
            if !task.plan.is_empty() {
                <ol class="plan">{ steps }</ol>
            }
            <form class="sub" onsubmit={on_submit_step}>
                <input
                    type="text"
                    placeholder="new micro todo…"
                    value={(*new_step).clone()}
                    oninput={Callback::from(move |e: InputEvent| new_step.set(e.data_concat()))}
                />
                <button type="submit">{ "+" }</button>
            </form>
        </li>
    }
}

#[derive(Properties, PartialEq)]
pub struct AddFormProps {
    pub contexts: Vec<String>,
    pub on_refresh: Callback<()>,
}

#[function_component(AddForm)]
pub fn add_form(props: &AddFormProps) -> Html {
    let description = use_state(String::new);
    let context = use_state(|| "backlog".to_string());

    let on_submit = {
        let description = description.clone();
        let context = context.clone();
        let on_refresh = props.on_refresh.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let desc = (*description).clone();
            if desc.trim().is_empty() {
                return;
            }
            let ctx = (*context).clone();
            let on_refresh = on_refresh.clone();
            spawn_local(async move {
                let body = serde_json::json!({
                    "description": desc,
                    "context": if ctx == "backlog" {
                        serde_json::Value::Null
                    } else {
                        serde_json::Value::String(ctx)
                    },
                });
                api_post("/api/task", &body).await;
                on_refresh.emit(());
            });
        })
    };

    let options = std::iter::once("backlog".to_string())
        .chain(props.contexts.iter().cloned())
        .map(|c| {
            let selected = (*context).clone() == c;
            html! { <option value={c.clone()} {selected}>{ c }</option> }
        })
        .collect::<Html>();

    let ctx = context.clone();
    html! {
        <form class="add" onsubmit={on_submit}>
            <input
                type="text"
                placeholder="new task…"
                value={(*description).clone()}
                oninput={Callback::from(move |e: InputEvent| description.set(e.data_concat()))}
            />
            <select oninput={Callback::from(move |e: InputEvent| ctx.set(e.data_concat()))}>
                { options }
            </select>
            <button type="submit">{ "add" }</button>
        </form>
    }
}

#[function_component(App)]
pub fn app() -> Html {
    let status = use_state(Status::default);
    let on_refresh = refresh(&status);

    {
        let on_refresh = on_refresh.clone();
        use_effect_with((), move |_| {
            on_refresh.emit(());
            || {}
        });
    }

    let s = (*status).clone();

    let backlog = s
        .backlog
        .iter()
        .map(|t| html! { <TaskCard task={t.clone()} on_refresh={on_refresh.clone()} /> })
        .collect::<Html>();

    let contexts = s
        .contexts
        .iter()
        .map(|c| {
            let tasks = c
                .tasks
                .iter()
                .map(|t| html! { <TaskCard task={t.clone()} on_refresh={on_refresh.clone()} /> })
                .collect::<Html>();
            html! {
                <section class="context">
                    <h2>{ format!("context: {}", c.name) }</h2>
                    <ul>{ tasks }</ul>
                </section>
            }
        })
        .collect::<Html>();

    let ctx_names: Vec<String> = s.contexts.iter().map(|c| c.name.clone()).collect();
    let root = if s.root.is_empty() { "?" } else { s.root.as_str() };

    html! {
        <main>
            <header>
                <h1>{ "tod" }</h1>
                <p class="meta">{ format!("{} — branch {}", root, s.branch) }</p>
            </header>
            <AddForm contexts={ctx_names} on_refresh={on_refresh.clone()} />
            <section class="backlog">
                <h2>{ "backlog" }</h2>
                <ul>{ backlog }</ul>
            </section>
            { contexts }
        </main>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
