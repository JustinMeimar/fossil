use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{Router, routing::get};
use serde::{Deserialize, Serialize};

use crate::artifact::{Artifact, ArtifactFormat};
use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::fossil::Fossil;
use crate::project::Project;
use crate::record::Record;

#[derive(Clone)]
struct Web {
    projects_dir: PathBuf,
    project: Option<String>,
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
        let projects = Project::list_all(&web.projects_dir)?;
        let project = select(
            &projects,
            selection.project.as_deref().or(web.project.as_deref()),
            |p| p.config.name.as_str(),
        )?;
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
        Ok(Html(render(
            &projects, project, &fossils, fossil, &records, &artifacts,
        ))
        .into_response())
    })
    .await
}

async fn output(
    State(web): State<Web>,
    Query(selection): Query<Selection>,
) -> Response {
    blocking(move || {
        let projects = Project::list_all(&web.projects_dir)?;
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
                Html(render_chunk(&selection, &text, pages)),
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

fn render_chunk(selection: &Selection, text: &str, pages: u64) -> String {
    let page = selection.page.unwrap_or(0);
    let mut html = String::from(concat!(
        "<!doctype html><html lang=en><meta charset=utf-8>",
        "<title>Output</title><link rel=stylesheet href=/style.css>",
        "<body class=output><nav aria-label=\"Output pages\">",
    ));
    if page > 0 {
        html.push_str(&link(
            "Previous",
            Selection {
                page: Some(page - 1),
                ..selection.clone()
            },
            true,
        ));
    }
    html.push_str(&format!("<span>Chunk {} of {pages}</span>", page + 1));
    if page + 1 < pages {
        html.push_str(&link(
            "Next",
            Selection {
                page: Some(page + 1),
                ..selection.clone()
            },
            true,
        ));
    }
    html.push_str(&format!("</nav><pre>{}</pre></body></html>", escape(text)));
    html
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

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn link(label: &str, selection: Selection, output: bool) -> String {
    let query = serde_urlencoded::to_string(selection).unwrap();
    let (path, target) = if output {
        ("/output", " target=output")
    } else {
        ("/", "")
    };
    format!(
        "<a href=\"{path}?{}\"{target}>{}</a>",
        escape(&query),
        escape(label)
    )
}

fn render(
    projects: &[Project],
    project: Option<&Project>,
    fossils: &[Fossil],
    fossil: Option<&Fossil>,
    records: &[Record],
    artifacts: &[(&str, ArtifactFormat)],
) -> String {
    let mut html = String::from(concat!(
        "<!doctype html><html lang=en><meta charset=utf-8>",
        "<meta name=viewport content=\"width=device-width, initial-scale=1\">",
        "<title>Fossil</title><link rel=stylesheet href=/style.css>",
        "<header><a href=/>fossil</a><span>records &amp; artifacts</span></header>",
        "<main><aside><h2>Projects</h2>",
    ));
    for p in projects {
        html.push_str(&link(
            &p.config.name,
            Selection {
                project: Some(p.config.name.clone()),
                ..Default::default()
            },
            false,
        ));
    }
    if projects.is_empty() {
        html.push_str("<p>No projects yet.</p>");
    }
    html.push_str("<h2>Fossils</h2>");
    for f in fossils {
        html.push_str(&link(
            &f.config.name,
            Selection {
                project: project.map(|p| p.config.name.clone()),
                fossil: Some(f.config.name.clone()),
                ..Default::default()
            },
            false,
        ));
    }
    if fossils.is_empty() {
        html.push_str("<p>No fossils yet.</p>");
    }
    html.push_str("</aside><section>");
    if let (Some(p), Some(f)) = (project, fossil) {
        let selection = || Selection {
            project: Some(p.config.name.clone()),
            fossil: Some(f.config.name.clone()),
            ..Default::default()
        };
        html.push_str(&format!(
            "<p class=muted>{}</p><h1>{}</h1><p>{}</p>",
            escape(&p.config.name),
            escape(&f.config.name),
            escape(f.config.desc())
        ));
        html.push_str("<h2>Records</h2>");
        if records.is_empty() {
            html.push_str("<p>No records yet.</p>");
        } else {
            html.push_str("<div class=scroll><table><thead><tr><th>Variant</th><th>Recorded</th><th>Commit</th><th>Iterations</th></tr></thead><tbody>");
            for r in records {
                let m = &r.manifest;
                html.push_str(&format!("<tr><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
                    link(m.variant.as_str(), Selection {
                        record: Some(r.id()), ..selection()
                    }, true), escape(&m.timestamp.to_string()), escape(&m.git.commit), m.iterations));
            }
            html.push_str("</tbody></table></div>");
        }
        html.push_str("<h2>Artifacts</h2><nav>");
        for (name, format) in artifacts {
            html.push_str(&link(
                &format!("{name}.{}", format.extension()),
                Selection {
                    artifact: Some((*name).into()),
                    ..selection()
                },
                true,
            ));
        }
        if artifacts.is_empty() {
            html.push_str("<p>No generated artifacts yet.</p>");
        }
        html.push_str("</nav><h2>Output</h2><p class=muted>Select a record or artifact to view it below.</p><iframe name=output title=\"Record or artifact output\"></iframe>");
    }
    html.push_str("</section></main></html>");
    html
}
