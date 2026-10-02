use std::collections::BTreeMap;

use crate::analysis::{
    AnalysisResult, AnalysisScript, AnalyzedRecords, ResolvedAnalysis,
};
use crate::artifact::PreparedArtifact;
use crate::entity::DirEntity;
use crate::environment::{CpuInfo, GitInfo};
use crate::error::FossilError;
use crate::fossil::{ConfigurationKey, Fossil, ResolvedVariant};
use crate::manifest::Manifest;
use crate::project::Project;
use crate::record::Record;
use crate::runner::{OutputMode, Run};

#[derive(Clone, serde::Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum BuryProgress {
    Running {
        variant: ConfigurationKey,
        iteration: u32,
        iterations: u32,
        completed: usize,
        total: usize,
    },
    Recorded {
        variant: ConfigurationKey,
        iteration: u32,
        completed: usize,
        total: usize,
        wall_time_us: u64,
        record_dir: std::path::PathBuf,
    },
}

#[derive(serde::Serialize)]
pub struct BuryResult {
    pub observations: usize,
    pub wall_time_us: u64,
    pub records: Vec<std::path::PathBuf>,
}

pub fn bury(
    fossil: &Fossil,
    project: &Project,
    iterations: Option<u32>,
    tasks: Vec<ResolvedVariant>,
    output_mode: OutputMode,
    mut progress: impl FnMut(BuryProgress),
) -> Result<BuryResult, FossilError> {
    if tasks.is_empty() {
        return Err(FossilError::InvalidArgs(
            "no variants given — usage: fossil bury <name> [--variant v]"
                .into(),
        ));
    }
    let n = iterations.unwrap_or(fossil.config.default_iterations);
    let mut runs: Vec<Run> = tasks
        .into_iter()
        .map(|variant| Run::new(variant, fossil, n, output_mode))
        .collect();
    let git = GitInfo::current(&project.path);
    let cpu = CpuInfo::current();
    let mut record_dirs: Vec<Option<std::path::PathBuf>> =
        vec![None; runs.len()];
    let mut total_obs = 0usize;
    let mut total_us = 0u64;

    let total = runs.len() * n as usize;
    for i in 1..=n {
        for (run, record_dir) in runs.iter_mut().zip(&mut record_dirs) {
            progress(BuryProgress::Running {
                variant: run.variant.name().clone(),
                iteration: i,
                iterations: n,
                completed: total_obs,
                total,
            });
            let wall_time_us = run.execute_one()?.wall_time_us;
            let run_dir = match record_dir {
                Some(run_dir) => {
                    Manifest::update_results(run_dir, &run.results)?;
                    run_dir.clone()
                }
                None => {
                    let manifest = Manifest::new(
                        fossil,
                        project,
                        run,
                        git.clone(),
                        cpu.clone(),
                    );
                    let run_dir =
                        manifest.record(&fossil.records_dir(), &run.results)?;
                    *record_dir = Some(run_dir.clone());
                    run_dir
                }
            };
            total_obs += 1;
            total_us += wall_time_us;
            progress(BuryProgress::Recorded {
                variant: run.variant.name().clone(),
                iteration: i,
                completed: total_obs,
                total,
                wall_time_us,
                record_dir: run_dir,
            });
        }
    }

    Ok(BuryResult {
        observations: total_obs,
        wall_time_us: total_us,
        records: record_dirs.into_iter().flatten().collect(),
    })
}

pub fn list_fossil_info(project: &Project) -> Result<(), FossilError> {
    let fossils = Fossil::list_all(&project.fossils_dir())?;
    if fossils.is_empty() {
        return Err(FossilError::NotFound("no matching records found".into()));
    }
    for f in &fossils {
        crate::io::output!("  {:<20} {}", f.config.name, f.config.desc());
    }
    Ok(())
}

fn resolve_spec(
    project: &Project,
    spec: &str,
    last: Option<usize>,
    analysis: Option<&ConfigurationKey>,
) -> Result<AnalysisResult, FossilError> {
    let (fossil_name, variant) = match spec.split_once(':') {
        Some((f, v)) => (f, Some(v)),
        None => (spec, None),
    };

    let fossil = Fossil::load(&project.fossils_dir().join(fossil_name))?;
    let key = select_key(&fossil.config.analyses, analysis, "analysis")?;
    let analysis = fossil.resolve_analysis(&key)?;
    let analyzed = analyze_records(&fossil, variant, last, &analysis)?;
    if variant.is_some() && analyzed.records.len() > 1 {
        Ok(analyzed.by_record())
    } else {
        analyzed.by_variant()
    }
}

fn analyze_records(
    fossil: &Fossil,
    variant: Option<&str>,
    last: Option<usize>,
    analysis: &ResolvedAnalysis,
) -> Result<AnalyzedRecords, FossilError> {
    let limit = if variant.is_some() {
        Some(last.unwrap_or(1))
    } else {
        last
    };
    let records = fossil.find_records(variant, limit)?;
    if records.is_empty() {
        return Err(FossilError::NotFound("no matching records found".into()));
    }
    let records = if variant.is_none() && last.is_none() {
        latest_per_variant(records)
    } else {
        records
    };
    let script = AnalysisScript::new(analysis, &fossil.path);
    Ok(AnalyzedRecords {
        records: records
            .iter()
            .map(|record| script.collect(record))
            .collect::<Result<_, _>>()?,
    })
}

fn latest_per_variant(records: Vec<Record>) -> Vec<Record> {
    let mut latest: BTreeMap<ConfigurationKey, Record> = BTreeMap::new();
    for record in records {
        let key = record.manifest.variant.clone();
        match latest.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(record);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if record.manifest.timestamp > entry.get().manifest.timestamp {
                    entry.insert(record);
                }
            }
        }
    }
    latest.into_values().collect()
}

pub fn analyze(
    project: &Project,
    selectors: &[String],
    last: Option<usize>,
    analysis: Option<&ConfigurationKey>,
) -> Result<AnalysisResult, FossilError> {
    let unique_names: std::collections::BTreeSet<_> = selectors
        .iter()
        .map(|s| s.split_once(':').map_or(s.as_str(), |(f, _)| f))
        .collect();
    if unique_names.len() > 1 {
        let names: Vec<_> = unique_names.into_iter().collect();
        return Err(FossilError::InvalidArgs(format!(
            "all selectors must refer to the same fossil, got: {}",
            names.join(", ")
        )));
    }
    let mut analysis_result = AnalysisResult::default();
    for selector in selectors {
        let selected_result = resolve_spec(project, selector, last, analysis)?;
        for (label, metric) in selected_result.metrics_by_label {
            analysis_result.merge_metric(label, metric)?;
        }
    }
    Ok(analysis_result)
}

pub fn emit_artifact(
    fossil: &Fossil,
    project: &Project,
    artifact_name: Option<&ConfigurationKey>,
    variant: Option<&str>,
    last: Option<usize>,
) -> Result<std::path::PathBuf, FossilError> {
    let key = select_key(&fossil.config.artifacts, artifact_name, "artifact")?;
    let artifact = fossil.resolve_artifact(&key)?;
    let analyzed = artifact
        .analysis
        .as_ref()
        .map(|analysis| analyze_records(fossil, variant, last, analysis))
        .transpose()?;
    let prepared = artifact.prepare(analyzed.as_ref())?;
    generate_artifact(fossil, project, &prepared)
}

pub(crate) fn generate_artifact(
    fossil: &Fossil,
    project: &Project,
    prepared: &PreparedArtifact<'_>,
) -> Result<std::path::PathBuf, FossilError> {
    let artifact = prepared.artifact();
    let destination = artifact.output_dir(fossil, project)?;
    let script = &artifact.script;
    let mut command = std::process::Command::new(script);
    std::fs::create_dir_all(&destination)?;
    let destination = destination.canonicalize()?;
    command.arg(&destination).current_dir(&fossil.path);
    let fail = |reason: String| {
        FossilError::InvalidConfig(format!(
            "artifact script {} failed: {reason}",
            script.display()
        ))
    };
    let output = crate::io::command_output(
        &mut command,
        prepared.input().map(str::as_bytes),
    )
    .map_err(|e| fail(e.to_string()))?;
    if !output.status.success() {
        return Err(fail(format!(
            "{}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    if crate::io::artifact_files(&destination)?.is_empty() {
        return Err(fail(format!("did not produce {}", destination.display())));
    }
    Ok(destination)
}

/// Command policy: an omitted key selects the sole configured entry.
pub fn select_key<T>(
    entries: &BTreeMap<ConfigurationKey, T>,
    requested: Option<&ConfigurationKey>,
    kind: &str,
) -> Result<ConfigurationKey, FossilError> {
    if let Some(key) = requested {
        return Ok(key.clone());
    }
    match entries.len() {
        0 => Err(FossilError::NotFound(format!("no {kind} configured"))),
        1 => Ok(entries.keys().next().unwrap().clone()),
        _ => {
            let available: Vec<_> =
                entries.keys().map(ConfigurationKey::as_str).collect();
            Err(FossilError::InvalidArgs(format!(
                "multiple entries available, use --{kind}: {}",
                available.join(", ")
            )))
        }
    }
}

/// Command policy: omitted variant keys select all configured variants.
pub fn bury_tasks(
    fossil: &Fossil,
    requested: &[ConfigurationKey],
) -> Result<Vec<ResolvedVariant>, FossilError> {
    let keys: Vec<_> = if requested.is_empty() {
        fossil.config.variants.keys().collect()
    } else {
        requested.iter().collect()
    };
    if keys.is_empty() {
        return Err(FossilError::InvalidArgs(
            "no variants configured — define variants in fossil.toml".into(),
        ));
    }
    keys.into_iter()
        .map(|key| fossil.resolve_variant(key))
        .collect()
}
