mod view;

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{
    Json, Router,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::artifact::{Artifact, ArtifactFormat};
use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::fossil::Fossil;
use crate::project::Project;

#[derive(Clone)]
struct Web {
    projects_dir: PathBuf,
    project: Option<PathBuf>,
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
    artifact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<u64>,
}

pub fn serve(
    projects_dir: PathBuf,
    project: Option<String>,
    port: u16,
) -> Result<(), FossilError> {
    let project = project
        .map(|name| {
            Project::list_all(&projects_dir)?
                .into_iter()
                .find(|p| p.config.name == name)
                .map(|p| p.path)
                .ok_or_else(|| {
                    FossilError::NotFound(format!("project {name:?} not found"))
                })
        })
        .transpose()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let listener =
            tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        crate::io::status!("browse http://{}", listener.local_addr()?);
        let app = Router::new()
            .route("/", get(index))
            .route("/output", get(output))
            .route("/analyze", post(analyze))
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
            .with_state(Web {
                projects_dir,
                project,
            });
        axum::serve(listener, app).await
    })?;
    Ok(())
}

// Filesystem work stays off the HTTP executor; rendering only consumes data.
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
            .map(|p| Fossil::list_all(&p.path))
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
                    let artifact = Artifact::resolve(f, Some(name))?;
                    if artifact.output_path(f, p)?.is_file() {
                        artifacts.push((name.as_str(), artifact.format()));
                    }
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

#[derive(Deserialize)]
struct AnalysisRequest {
    project: String,
    fossil: String,
    analysis: String,
    records: Vec<String>,
}

async fn analyze(
    State(web): State<Web>,
    Json(request): Json<AnalysisRequest>,
) -> Response {
    blocking(move || {
        if request.records.is_empty() {
            return Err(FossilError::InvalidArgs(
                "Select at least one record".into(),
            ));
        }
        let projects = web.load_projects()?;
        let project =
            select(&projects, Some(&request.project), |p| &p.config.name)?
                .ok_or_else(missing)?;
        let fossils = Fossil::list_all(&project.path)?;
        let fossil =
            select(&fossils, Some(&request.fossil), |f| &f.config.name)?
                .ok_or_else(missing)?;
        let records = fossil.find_records(None, None)?;
        // Resolve every ID before executing any scripts.
        let selected = request
            .records
            .iter()
            .map(|id| {
                records
                    .iter()
                    .find(|r| r.id() == *id)
                    .ok_or_else(missing)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let script =
            fossil.resolve_analysis(Some(&request.analysis), project)?;
        let columns = selected
            .into_iter()
            .map(|record| {
                script
                    .collect(record)
                    .map(|metric| (record.id(), metric))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((
            [(header::CACHE_CONTROL, "no-store")],
            crate::analysis::columns_to_json(&columns)?,
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
        let projects = web.load_projects()?;
        let project = select(&projects, selection.project.as_deref(), |p| {
            p.config.name.as_str()
        })?
        .ok_or_else(missing)?;
        let fossils = Fossil::list_all(&project.path)?;
        let fossil = select(&fossils, selection.fossil.as_deref(), |f| {
            f.config.name.as_str()
        })?
        .ok_or_else(missing)?;
        let (path, format) = match (&selection.record, &selection.artifact) {
            (Some(id), None) => {
                let record = fossil
                    .find_records(None, None)?
                    .into_iter()
                    .find(|r| r.id() == *id)
                    .ok_or_else(missing)?;
                (record.dir.join("results.json"), ArtifactFormat::Json)
            }
            (None, Some(name)) => {
                if !fossil.config.artifacts.contains_key(name) {
                    return Err(missing());
                }
                let artifact = Artifact::resolve(fossil, Some(name))?;
                (artifact.output_path(fossil, project)?, artifact.format())
            }
            _ => return Err(missing()),
        };
        if matches!(format, ArtifactFormat::Json) {
            let (text, pages) = read_chunk(&path, selection.page.unwrap_or(0))?;
            return Ok((
                [(header::CACHE_CONTROL, "no-store")],
                view::render_chunk(&selection, &text, pages),
            )
                .into_response());
        }
        let file = tokio::fs::File::from_std(std::fs::File::open(path)?);
        let body = axum::body::Body::from_stream(
            tokio_util::io::ReaderStream::new(file),
        );
        Ok((
            [
                (header::CONTENT_TYPE, "application/pdf"),
                (header::CACHE_CONTROL, "no-store"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            body,
        )
            .into_response())
    })
    .await
}

async fn blocking(
    work: impl FnOnce() -> Result<Response, FossilError> + Send + 'static,
) -> Response {
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => {
            let status = match &error {
                FossilError::NotFound(_) => StatusCode::NOT_FOUND,
                FossilError::InvalidArgs(_) => StatusCode::BAD_REQUEST,
                FossilError::Io(e)
                    if e.kind() == std::io::ErrorKind::NotFound =>
                {
                    StatusCode::NOT_FOUND
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, error.to_string()).into_response()
        }
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
            .into_response(),
    }
}

const CHUNK_BYTES: u64 = 64 * 1024;

fn read_chunk(path: &Path, page: u64) -> Result<(String, u64), FossilError> {
    let mut file = std::fs::File::open(path)?;
    let pages = file.metadata()?.len().div_ceil(CHUNK_BYTES).max(1);
    if page >= pages {
        return Err(FossilError::InvalidArgs("Page out of range".into()));
    }
    file.seek(SeekFrom::Start(page * CHUNK_BYTES))?;
    let mut bytes = Vec::new();
    file.take(CHUNK_BYTES + 4).read_to_end(&mut bytes)?;
    let continuation = |b: &u8| b & 0xc0 == 0x80;
    // Include a character crossing the end boundary in this page, and skip
    // its remaining bytes at the start of the next page.
    let start = bytes.iter().take_while(|b| continuation(b)).count();
    let mut end = (CHUNK_BYTES as usize).min(bytes.len());
    while bytes.get(end).is_some_and(continuation) {
        end += 1;
    }
    Ok((
        String::from_utf8_lossy(&bytes[start..end]).into_owned(),
        pages,
    ))
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
