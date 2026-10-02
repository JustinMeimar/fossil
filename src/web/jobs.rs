use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::{Web, missing, select};
use crate::analysis::{AnalysisScript, AnalyzedRecords};
use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::fossil::{ConfigurationKey, Fossil};

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct AnalysisRequest {
    project: String,
    fossil: String,
    analysis: ConfigurationKey,
    records: Vec<String>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Default, Serialize)]
struct Progress {
    completed: usize,
    total: usize,
    record: Option<String>,
}

#[derive(Clone, Serialize)]
pub(super) struct Job {
    id: u64,
    request: AnalysisRequest,
    state: JobState,
    progress: Progress,
    result: Option<String>,
    error: Option<String>,
    #[serde(skip)]
    output: Option<Arc<AnalyzedRecords>>,
}

#[derive(Serialize)]
struct JobView<'a> {
    #[serde(flatten)]
    job: &'a Job,
    variants: std::collections::BTreeSet<ConfigurationKey>,
}

impl Job {
    fn view(&self) -> JobView<'_> {
        JobView {
            job: self,
            variants: self
                .output
                .as_ref()
                .map(|output| output.variants())
                .unwrap_or_default(),
        }
    }

    fn active(&self) -> bool {
        matches!(self.state, JobState::Queued | JobState::Running)
    }
}

#[derive(Default)]
pub(super) struct JobStore {
    next_id: u64,
    jobs: BTreeMap<u64, Job>,
}

#[derive(Clone)]
pub(super) struct Jobs {
    pub store: Arc<Mutex<JobStore>>,
    pub queue: mpsc::Sender<u64>,
}

pub(super) async fn submit(
    State(web): State<Web>,
    Json(mut request): Json<AnalysisRequest>,
) -> Response {
    request.records.sort();
    request.records.dedup();
    if request.records.is_empty() {
        return (StatusCode::BAD_REQUEST, "Select at least one record")
            .into_response();
    }
    let mut store = web.jobs.store.lock().unwrap();
    if let Some(job) = store
        .jobs
        .values()
        .find(|job| job.active() && job.request == request)
    {
        return (StatusCode::ACCEPTED, Json(job.view())).into_response();
    }
    let permit = match web.jobs.queue.try_reserve() {
        Ok(permit) => permit,
        Err(_) => {
            return (StatusCode::SERVICE_UNAVAILABLE, "Job queue is full")
                .into_response();
        }
    };
    if store.jobs.len() >= 64 {
        let oldest = store
            .jobs
            .iter()
            .find(|(_, job)| !job.active())
            .map(|(id, _)| *id);
        if let Some(id) = oldest {
            store.jobs.remove(&id);
        }
    }
    store.next_id += 1;
    let job = Job {
        id: store.next_id,
        progress: Progress {
            total: request.records.len(),
            ..Default::default()
        },
        request,
        state: JobState::Queued,
        result: None,
        error: None,
        output: None,
    };
    store.jobs.insert(job.id, job.clone());
    permit.send(job.id);
    (StatusCode::ACCEPTED, Json(job.view())).into_response()
}

pub(super) async fn list(State(web): State<Web>) -> Response {
    let store = web.jobs.store.lock().unwrap();
    let jobs: Vec<_> = store.jobs.values().rev().map(Job::view).collect();
    ([(header::CACHE_CONTROL, "no-store")], Json(jobs)).into_response()
}

pub(super) async fn result(
    State(web): State<Web>,
    Path(id): Path<u64>,
) -> Response {
    let output = web
        .jobs
        .store
        .lock()
        .unwrap()
        .jobs
        .get(&id)
        .and_then(|job| job.output.clone());
    match output
        .map(|output| output.by_record().to_json())
        .transpose()
    {
        Ok(Some(output)) => (
            [(header::CACHE_CONTROL, "no-store")],
            super::preview::json_document(
                &output,
                &format!("/jobs/{id}/download"),
            ),
        )
            .into_response(),
        Ok(None) => {
            (StatusCode::NOT_FOUND, "Result not available").into_response()
        }
        Err(error) => super::error_response(error),
    }
}

pub(super) async fn download(
    State(web): State<Web>,
    Path(id): Path<u64>,
) -> Response {
    let output = web
        .jobs
        .store
        .lock()
        .unwrap()
        .jobs
        .get(&id)
        .and_then(|job| job.output.clone());
    match output
        .map(|output| output.by_record().to_json())
        .transpose()
    {
        Ok(Some(output)) => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=analysis.json",
                ),
                (header::CACHE_CONTROL, "no-store"),
            ],
            output,
        )
            .into_response(),
        Ok(None) => {
            (StatusCode::NOT_FOUND, "Result not available").into_response()
        }
        Err(error) => super::error_response(error),
    }
}

pub(super) async fn run(web: Web, mut queue: mpsc::Receiver<u64>) {
    while let Some(id) = queue.recv().await {
        let request = {
            let mut store = web.jobs.store.lock().unwrap();
            let job = store.jobs.get_mut(&id).unwrap();
            job.state = JobState::Running;
            job.request.clone()
        };
        let worker = web.clone();
        let result = tokio::task::spawn_blocking(move || {
            analyze(&worker, request, |progress| {
                worker
                    .jobs
                    .store
                    .lock()
                    .unwrap()
                    .jobs
                    .get_mut(&id)
                    .unwrap()
                    .progress = progress;
            })
        })
        .await;
        let result = result
            .map_err(|error| error.to_string())
            .and_then(|result| result.map_err(|error| error.to_string()));
        let mut store = web.jobs.store.lock().unwrap();
        let job = store.jobs.get_mut(&id).unwrap();
        match result {
            Ok(output) => {
                job.state = JobState::Succeeded;
                job.result = Some(format!("/jobs/{id}/result"));
                job.output = Some(Arc::new(output));
            }
            Err(error) => {
                job.state = JobState::Failed;
                job.error = Some(error);
            }
        }
    }
}

fn analyze(
    web: &Web,
    request: AnalysisRequest,
    mut progress: impl FnMut(Progress),
) -> Result<AnalyzedRecords, FossilError> {
    let projects = web.load_projects()?;
    let project =
        select(&projects, Some(&request.project), |p| &p.config.name)?
            .ok_or_else(missing)?;
    let fossils = Fossil::list_all(&project.fossils_dir())?;
    let fossil = select(&fossils, Some(&request.fossil), |f| &f.config.name)?
        .ok_or_else(missing)?;
    let records = fossil.find_records(None, None)?;
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
    let analysis = fossil.resolve_analysis(&request.analysis)?;
    let script = AnalysisScript::new(&analysis, &fossil.path);
    let mut analyzed = AnalyzedRecords::default();
    let total = selected.len();
    for (completed, record) in selected.into_iter().enumerate() {
        progress(Progress {
            completed,
            total,
            record: Some(record.id()),
        });
        analyzed.records.push(script.collect(record)?);
        progress(Progress {
            completed: completed + 1,
            total,
            record: Some(record.id()),
        });
    }
    Ok(analyzed)
}

#[derive(Deserialize)]
pub(super) struct ArtifactRequest {
    project: String,
    fossil: String,
    artifact: ConfigurationKey,
    job: Option<u64>,
}

pub(super) async fn generate(
    State(web): State<Web>,
    Json(request): Json<ArtifactRequest>,
) -> Response {
    super::blocking(move || {
        let _generation = web.generation.try_lock().map_err(|_| {
            FossilError::InvalidArgs("Artifact generation is already running".into())
        })?;
        let projects = web.load_projects()?;
        let project = select(&projects, Some(&request.project), |p| &p.config.name)?
            .ok_or_else(missing)?;
        let fossils = Fossil::list_all(&project.fossils_dir())?;
        let fossil = select(&fossils, Some(&request.fossil), |f| &f.config.name)?
            .ok_or_else(missing)?;
        let artifact = fossil.resolve_artifact(&request.artifact)?;
        let analyzed = match &artifact.analysis {
            Some(analysis) => {
                let store = web.jobs.store.lock().unwrap();
                let job = request.job.and_then(|id| store.jobs.get(&id))
                    .filter(|job| job.request.project == request.project
                        && job.request.fossil == request.fossil
                        && job.request.analysis == analysis.key);
                let output = job.and_then(|job| job.output.clone()).ok_or_else(|| {
                    FossilError::InvalidArgs(format!("View a completed {} analysis for this fossil first", analysis.key))
                })?;
                drop(store);
                Some(output)
            }
            None => None,
        };
        let prepared = artifact.prepare(analyzed.as_deref())?;
        let destination = crate::commands::generate_artifact(fossil, project, &prepared)?;
        let files: Vec<_> = crate::io::artifact_files(&destination)?
            .into_iter().map(|file| file.to_string_lossy().into_owned()).collect();
        Ok(Json(files).into_response())
    }).await
}
