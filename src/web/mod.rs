mod jobs;
mod preview;
mod view;

use std::path::{Path, PathBuf};

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{
    Router,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::fossil::{ConfigurationKey, Fossil};
use crate::project::Project;

#[derive(Clone)]
struct Web {
    projects_dir: PathBuf,
    project: Option<PathBuf>,
    jobs: jobs::Jobs,
    generation: std::sync::Arc<std::sync::Mutex<()>>,
}

impl Web {
    fn load_projects(&self) -> Result<Vec<Project>, FossilError> {
        match &self.project {
            Some(path) => Ok(vec![Project::load(path)?]),
            None => Project::list_all(&self.projects_dir),
        }
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct Selection {
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fossil: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact: Option<ConfigurationKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
}

pub fn serve(
    projects_dir: PathBuf,
    project: Option<String>,
    port: u16,
) -> Result<(), FossilError> {
    let project = project
        .map(|name| {
            Project::resolve(&projects_dir, Some(&name), None)
                .map(|project| project.path)
        })
        .transpose()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let listener =
            tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        crate::io::status!("browse http://{}", listener.local_addr()?);
        let (queue, receiver) = tokio::sync::mpsc::channel(16);
        let web = Web {
            projects_dir,
            project,
            generation: Default::default(),
            jobs: jobs::Jobs {
                store: Default::default(),
                queue,
            },
        };
        tokio::spawn(jobs::run(web.clone(), receiver));
        let app = Router::new()
            .route("/", get(index))
            .route("/output", get(output))
            .route("/analyze", post(jobs::submit))
            .route("/generate", post(jobs::generate))
            .route("/jobs", get(jobs::list))
            .route("/jobs/{id}/result", get(jobs::result))
            .route("/jobs/{id}/download", get(jobs::download))
            .route("/file", get(raw_file))
            .route(
                "/app.js",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "text/javascript")],
                        include_str!("app.js"),
                    )
                }),
            )
            .route(
                "/style.css",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "text/css")],
                        include_str!("style.css"),
                    )
                }),
            )
            .with_state(web);
        axum::serve(listener, app).await
    })?;
    Ok(())
}

async fn index(
    State(web): State<Web>,
    Query(selection): Query<Selection>,
) -> Response {
    blocking(move || {
        let projects = web.load_projects()?;
        let project = select(&projects, selection.project.as_deref(), |p| {
            p.config.name.as_str()
        })?;
        let fossils = project
            .map(|p| Fossil::list_all(&p.fossils_dir()))
            .transpose()?
            .unwrap_or_default();
        let fossil = select(&fossils, selection.fossil.as_deref(), |f| {
            f.config.name.as_str()
        })?;
        let mut records = match fossil {
            Some(f) if f.records_dir().exists() => {
                f.find_records(None, None)?
            }
            _ => Vec::new(),
        };
        records.reverse();
        let mut artifacts = Vec::new();
        if let (Some(p), Some(f)) = (project, fossil) {
            if p.config.artifact_dir.is_some() {
                for name in f.config.artifacts.keys() {
                    let definition = f.resolve_artifact(name)?;
                    let files = crate::io::artifact_files(
                        &definition.output_dir(f, p)?,
                    )?
                    .into_iter()
                    .map(|file| file.to_string_lossy().into_owned())
                    .collect();
                    artifacts.push(view::Artifact { definition, files });
                }
            }
        }
        Ok(view::render(
            &projects, project, &fossils, fossil, &records, &artifacts,
        )
        .into_response())
    })
    .await
}

async fn output(
    State(web): State<Web>,
    Query(selection): Query<Selection>,
) -> Response {
    blocking(move || {
        let path = resolve_file(&web, &selection)?;
        Ok((
            [(header::CACHE_CONTROL, "no-store")],
            preview::file(&path, &selection)?,
        )
            .into_response())
    })
    .await
}

async fn raw_file(
    State(web): State<Web>,
    Query(selection): Query<Selection>,
    request: axum::extract::Request,
) -> Response {
    let download = selection.download.unwrap_or(false);
    let resolved =
        tokio::task::spawn_blocking(move || resolve_file(&web, &selection))
            .await;
    let path = match resolved {
        Ok(Ok(path)) => path,
        Ok(Err(error)) => return error_response(error),
        Err(error) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
                .into_response();
        }
    };
    match tower_http::services::ServeFile::new(path)
        .try_call(request)
        .await
    {
        Ok(response) => {
            let mut response = response.into_response();
            let headers = response.headers_mut();
            headers.insert(
                header::CONTENT_DISPOSITION,
                if download { "attachment" } else { "inline" }
                    .parse()
                    .unwrap(),
            );
            headers.insert(
                header::CONTENT_SECURITY_POLICY,
                "sandbox".parse().unwrap(),
            );
            headers.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                "nosniff".parse().unwrap(),
            );
            headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
            response
        }
        Err(error) => error_response(error.into()),
    }
}

fn resolve_file(
    web: &Web,
    selection: &Selection,
) -> Result<PathBuf, FossilError> {
    let projects = web.load_projects()?;
    let project = select(&projects, selection.project.as_deref(), |p| {
        p.config.name.as_str()
    })?
    .ok_or_else(missing)?;
    let fossils = Fossil::list_all(&project.fossils_dir())?;
    let fossil = select(&fossils, selection.fossil.as_deref(), |f| {
        f.config.name.as_str()
    })?
    .ok_or_else(missing)?;
    let path = match (&selection.record, &selection.artifact) {
        (Some(id), None) => {
            let record = fossil
                .find_records(None, None)?
                .into_iter()
                .find(|r| r.id() == *id)
                .ok_or_else(missing)?;
            record.dir.join("results.json")
        }
        (None, Some(name)) => {
            if !fossil.config.artifacts.contains_key(name) {
                return Err(missing());
            }
            let artifact = fossil.resolve_artifact(name)?;
            let root = artifact.output_dir(fossil, project)?;
            let file = selection.file.as_deref().ok_or_else(missing)?;
            let relative = Path::new(file);
            if !crate::io::artifact_files(&root)?
                .iter()
                .any(|p| p == relative)
            {
                return Err(missing());
            }
            let root = root.canonicalize()?;
            let path = root.join(relative).canonicalize()?;
            if !path.starts_with(&root) {
                return Err(missing());
            }
            path
        }
        _ => return Err(missing()),
    };
    Ok(path)
}

async fn blocking(
    work: impl FnOnce() -> Result<Response, FossilError> + Send + 'static,
) -> Response {
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => error_response(error),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
            .into_response(),
    }
}

fn error_response(error: FossilError) -> Response {
    let status = match &error {
        FossilError::NotFound(_) => StatusCode::NOT_FOUND,
        FossilError::InvalidArgs(_) => StatusCode::BAD_REQUEST,
        FossilError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => {
            StatusCode::NOT_FOUND
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, error.to_string()).into_response()
}

fn missing() -> FossilError {
    FossilError::NotFound("Selection not found".into())
}

fn select<'a, T>(
    items: &'a [T],
    name: Option<&str>,
    key: impl Fn(&T) -> &str,
) -> Result<Option<&'a T>, FossilError> {
    match name {
        Some(name) => items
            .iter()
            .find(|item| key(item) == name)
            .map(Some)
            .ok_or_else(missing),
        None => Ok(items.first()),
    }
}
