use std::collections::BTreeMap;

use crate::analysis::{AnalysisResult, AnalysisScript, ResolvedAnalysis};
use crate::entity::DirEntity;
use crate::environment::{CpuInfo, GitInfo};
use crate::error::FossilError;
use crate::fossil::{ConfigurationKey, Fossil, ResolvedVariant};
use crate::io::status;
use crate::manifest::Manifest;
use crate::project::Project;
use crate::record::Record;
use crate::runner::{OutputMode, Run};

/// Bury one or more variants, interleaving iterations across variants.
///
/// Outer loop is the iteration counter; inner loop cycles through every
/// variant. This averages out system-load and thermal drift across variants
/// rather than concentrating it in whichever variant ran last.
pub fn bury(
    fossil: &Fossil,
    project: &Project,
    iterations: Option<u32>,
    tasks: Vec<ResolvedVariant>,
    output_mode: OutputMode,
) -> Result<String, FossilError> {
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

    for i in 1..=n {
        for (run, record_dir) in runs.iter_mut().zip(&mut record_dirs) {
            status!(
                "burying {}/{} ({}/{})",
                fossil.config.name,
                run.variant.name(),
                i,
                n,
            );
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
            status!(
                "{}ms recorded → {}",
                wall_time_us / 1000,
                run_dir.display(),
            );
        }
    }

    let avg_ms = if total_obs == 0 {
        0
    } else {
        total_us / total_obs as u64 / 1000
    };
    Ok(format!(
        "{total_obs} observations recorded ({avg_ms}ms avg)"
    ))
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
    analyze_records(&fossil, variant, last, &analysis)
}

fn analyze_records(
    fossil: &Fossil,
    variant: Option<&str>,
    last: Option<usize>,
    analysis: &ResolvedAnalysis,
) -> Result<AnalysisResult, FossilError> {
    let script = AnalysisScript::new(analysis, &fossil.path);

    if let Some(vname) = variant {
        let records =
            fossil.find_records(Some(vname), Some(last.unwrap_or(1)))?;
        if records.is_empty() {
            return Err(FossilError::NotFound(
                "no matching records found".into(),
            ));
        }
        let mut analysis_result = AnalysisResult::default();
        for r in &records {
            let metrics = script.collect(r)?;
            let label = if records.len() == 1 {
                vname.to_string()
            } else {
                r.id()
            };
            analysis_result.merge_metric(label, metrics)?;
        }
        return Ok(analysis_result);
    }

    let all = fossil.find_records(None, last)?;
    if all.is_empty() {
        return Err(FossilError::NotFound("no matching records found".into()));
    }

    if last.is_some() {
        let mut analysis_result = AnalysisResult::default();
        for r in &all {
            let metrics = script.collect(r)?;
            let label = r.manifest.variant.to_string();
            analysis_result.merge_metric(label, metrics)?;
        }
        return Ok(analysis_result);
    }

    let mut latest: BTreeMap<String, &Record> = BTreeMap::new();
    for r in &all {
        let key = r.manifest.variant.to_string();
        latest
            .entry(key)
            .and_modify(|prev| {
                if r.manifest.timestamp > prev.manifest.timestamp {
                    *prev = r;
                }
            })
            .or_insert(r);
    }

    let mut analysis_result = AnalysisResult::default();
    for (name, record) in &latest {
        let metrics = script.collect(record)?;
        analysis_result.merge_metric(name.clone(), metrics)?;
    }
    Ok(analysis_result)
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
    let destination = artifact.output_dir(fossil, project)?;
    let input = match &artifact.analysis {
        Some(analysis) => {
            let analysis_result =
                analyze_records(fossil, variant, last, analysis)?;
            Some(analysis_result.to_json()?)
        }
        None => None,
    };

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
        input.as_deref().map(str::as_bytes),
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
