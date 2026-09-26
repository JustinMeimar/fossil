use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::{Web, missing, select};
use crate::analysis::{AnalysisResult, AnalysisScript, Metric};
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

struct AnalyzedRecords {
    json: String,
    variants: Vec<(String, Metric)>,
}

impl AnalyzedRecords {
    fn artifact_input(&self) -> Result<String, FossilError> {
        let mut result = AnalysisResult::default();
        for (variant, metric) in &self.variants {
            result.merge_metric(variant.clone(), metric.clone())?;
        }
        result.to_json()
    }
}

impl Job {
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
        return (StatusCode::ACCEPTED, Json(job.clone())).into_response();
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
    (StatusCode::ACCEPTED, Json(job)).into_response()
}

pub(super) async fn list(State(web): State<Web>) -> Response {
    let jobs: Vec<_> = web
        .jobs
        .store
        .lock()
        .unwrap()
        .jobs
        .values()
        .rev()
        .cloned()
        .collect();
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
    match output {
        Some(output) => (
            [(header::CACHE_CONTROL, "no-store")],
            super::preview::json_document(
                &output.json,
                &format!("/jobs/{id}/download"),
            ),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "Result not available").into_response(),
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
    match output {
        Some(output) => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=analysis.json",
                ),
                (header::CACHE_CONTROL, "no-store"),
            ],
            output.json.clone(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "Result not available").into_response(),
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
    let mut result = AnalysisResult::default();
    let mut variants = Vec::with_capacity(selected.len());
    let total = selected.len();
    for (completed, record) in selected.into_iter().enumerate() {
        progress(Progress {
            completed,
            total,
            record: Some(record.id()),
        });
        let metric = script.collect(record)?;
        result
            .metrics_by_label
            .insert(record.id(), metric.clone());
        variants.push((record.manifest.variant.to_string(), metric));
        progress(Progress {
            completed: completed + 1,
            total,
            record: Some(record.id()),
        });
    }
    Ok(AnalyzedRecords {
        json: result.to_json()?,
        variants,
    })
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
        let input = match &fossil.config.artifacts[&request.artifact].analysis {
            Some(analysis) => {
                let store = web.jobs.store.lock().unwrap();
                let job = request.job.and_then(|id| store.jobs.get(&id))
                    .filter(|job| job.request.project == request.project
                        && job.request.fossil == request.fossil
                        && &job.request.analysis == analysis);
                let output = job.and_then(|job| job.output.clone()).ok_or_else(|| {
                    FossilError::InvalidArgs(format!("View a completed {analysis} analysis for this fossil first"))
                })?;
                drop(store);
                Some(output.artifact_input()?)
            }
            None => None,
        };
        let destination = crate::commands::generate_artifact(
            fossil, project, &artifact, input.as_deref(),
        )?;
        let files: Vec<_> = crate::io::artifact_files(&destination)?
            .into_iter().map(|file| file.to_string_lossy().into_owned()).collect();
        Ok(Json(files).into_response())
    }).await
}
