use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use serde_json::{Value, json};

use crate::fossil::Fossil;
use crate::manifest::Manifest;
use crate::project::Project;

type AppState = Arc<PathBuf>;
type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

const INDEX_HTML: &str = include_str!("index.html");

fn not_found(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_FOUND, Json(json!({ "error": msg })))
}

fn projects_dir(state: &AppState) -> PathBuf {
    state.join("projects")
}

async fn list_projects(State(state): State<AppState>) -> Json<Value> {
    let projects = Project::list_all(&projects_dir(&state)).unwrap_or_default();
    let items: Vec<Value> = projects
        .iter()
        .map(|p| {
            json!({
                "name": p.config.name,
                "description": p.config.description,
            })
        })
        .collect();
    Json(json!(items))
}

async fn get_project(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult {
    let project = Project::load(&projects_dir(&state).join(&name))
        .map_err(|_| not_found(format!("project {name:?} not found")))?;
    Ok(Json(json!({
        "name": project.config.name,
        "description": project.config.description,
    })))
}

async fn list_fossils(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult {
    let project = Project::load(&projects_dir(&state).join(&name))
        .map_err(|_| not_found(format!("project {name:?} not found")))?;
    let fossils = Fossil::list_all(&project.fossils_dir()).unwrap_or_default();
    let items: Vec<Value> = fossils
        .iter()
        .map(|f| {
            json!({
                "name": f.config.name,
                "description": f.config.description,
                "default_iterations": f.config.default_iterations,
                "analyze": f.config.analyze,
                "variants": f.config.variants,
            })
        })
        .collect();
    Ok(Json(json!(items)))
}

async fn get_fossil(
    State(state): State<AppState>,
    Path((project_name, fossil_name)): Path<(String, String)>,
) -> ApiResult {
    let project = Project::load(&projects_dir(&state).join(&project_name))
        .map_err(|_| not_found(format!("project {project_name:?} not found")))?;
    let fossil = Fossil::load(&project.fossils_dir().join(&fossil_name))
        .map_err(|_| not_found(format!("fossil {fossil_name:?} not found")))?;
    Ok(Json(json!({
        "name": fossil.config.name,
        "description": fossil.config.description,
        "default_iterations": fossil.config.default_iterations,
        "analyze": fossil.config.analyze,
        "variants": fossil.config.variants,
    })))
}

async fn list_records(
    State(state): State<AppState>,
    Path((project_name, fossil_name)): Path<(String, String)>,
) -> ApiResult {
    let project = Project::load(&projects_dir(&state).join(&project_name))
        .map_err(|_| not_found(format!("project {project_name:?} not found")))?;
    let fossil = Fossil::load(&project.fossils_dir().join(&fossil_name))
        .map_err(|_| not_found(format!("fossil {fossil_name:?} not found")))?;
    let runs = crate::analysis::find_records(&fossil.records_dir(), None, None)
        .unwrap_or_default();
    let items: Vec<Value> = runs
        .iter()
        .map(|(dir, m)| {
            let id = dir.file_name().unwrap().to_string_lossy().to_string();
            json!({
                "id": id,
                "timestamp": m.timestamp,
                "variant": m.variant,
                "iterations": m.iterations,
                "commit": m.git.commit,
                "branch": m.git.branch,
            })
        })
        .collect();
    Ok(Json(json!(items)))
}

async fn get_record(
    State(state): State<AppState>,
    Path((project_name, fossil_name, record_id)): Path<(String, String, String)>,
) -> ApiResult {
    let project = Project::load(&projects_dir(&state).join(&project_name))
        .map_err(|_| not_found(format!("project {project_name:?} not found")))?;
    let fossil = Fossil::load(&project.fossils_dir().join(&fossil_name))
        .map_err(|_| not_found(format!("fossil {fossil_name:?} not found")))?;
    let record_dir = fossil.records_dir().join(&record_id);
    let manifest = Manifest::load(&record_dir)
        .map_err(|_| not_found(format!("record {record_id:?} not found")))?;
    let results: Value = std::fs::read_to_string(record_dir.join("results.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!(null));
    Ok(Json(json!({
        "manifest": manifest,
        "results": results,
    })))
}

pub fn run(fossil_home: PathBuf, port: u16) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let state: AppState = Arc::new(fossil_home);
        let app = Router::new()
            .route("/", get(|| async { Html(INDEX_HTML) }))
            .route("/api/projects", get(list_projects))
            .route("/api/projects/{name}", get(get_project))
            .route("/api/projects/{name}/fossils", get(list_fossils))
            .route("/api/projects/{name}/fossils/{fossil}", get(get_fossil))
            .route(
                "/api/projects/{name}/fossils/{fossil}/records",
                get(list_records),
            )
            .route(
                "/api/projects/{name}/fossils/{fossil}/records/{record}",
                get(get_record),
            )
            .with_state(state);

        let listener =
            tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
        eprintln!("[fossil] serving on http://localhost:{port}");
        axum::serve(listener, app).await?;
        Ok(())
    })
}
