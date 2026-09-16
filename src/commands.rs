use std::collections::BTreeMap;

use crate::analysis;
use crate::entity::DirEntity;
use crate::environment::{CpuInfo, ExecutionContext, GitInfo, Operation};
use crate::error::FossilError;
use crate::fossil::{Fossil, FossilVariantKey};
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
    tasks: Vec<(FossilVariantKey, String)>,
    output_mode: OutputMode,
) -> Result<String, FossilError> {
    if tasks.is_empty() {
        return Err(FossilError::InvalidArgs(
            "no variants given — usage: fossil bury <name> [--variant v]"
                .into(),
        ));
    }
    if tasks.iter().any(|(_, cmd)| cmd.is_empty()) {
        return Err(FossilError::InvalidArgs(
            "empty command for variant".into(),
        ));
    }

    let n = iterations.unwrap_or(fossil.config.default_iterations);
    let workdir = fossil
        .config
        .workdir
        .as_ref()
        .map(|p| p.resolve(&fossil.path));

    let mut runs: Vec<Run> = tasks
        .into_iter()
        .map(|(variant, command)| {
            let context = ExecutionContext::new(
                project,
                fossil,
                Operation::Variant(&variant),
            );
            Run {
                command,
                iterations: n,
                variant: Some(variant),
                allow_failure: fossil.config.allow_failure,
                workdir: workdir.clone(),
                context,
                output_mode,
                observations: Vec::new(),
            }
        })
        .collect();
    let git = GitInfo::current(&project.path);
    let cpu = CpuInfo::current();
    let mut record_dirs: Vec<Option<std::path::PathBuf>> =
        vec![None; runs.len()];
    let mut total_obs = 0usize;
    let mut total_us = 0u64;

    for i in 1..=n {
        for (run, record_dir) in runs.iter_mut().zip(&mut record_dirs) {
            let vname = run
                .variant
                .as_ref()
                .map(|v| v.as_str().to_string())
                .unwrap_or_else(|| "untagged".to_string());
            if output_mode.shows_progress() {
                status!(
                    "burying {}/{} ({}/{})",
                    fossil.config.name,
                    vname,
                    i,
                    n,
                );
            }
            let wall_time_us = run.execute_one()?.wall_time_us;
            let run_dir = match record_dir {
                Some(run_dir) => {
                    Manifest::update_results(run_dir, &run.results())?;
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
                    let run_dir = manifest
                        .record(&fossil.records_dir(), &run.results())?;
                    *record_dir = Some(run_dir.clone());
                    run_dir
                }
            };
            total_obs += 1;
            total_us += wall_time_us;
            if output_mode.shows_progress() {
                status!(
                    "{}ms recorded → {}",
                    wall_time_us / 1000,
                    run_dir.display(),
                );
            }
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
    let fossils = Fossil::list_all(project.fossils_dir())?;
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
    analysis: Option<&str>,
) -> Result<Vec<(String, analysis::Metric)>, FossilError> {
    let (fossil_name, variant) = match spec.split_once(':') {
        Some((f, v)) => (f, Some(v)),
        None => (spec, None),
    };

    let fossil = Fossil::load(&project.fossils_dir().join(fossil_name))?;
    let script = fossil.resolve_analysis(analysis, project)?;

    if let Some(vname) = variant {
        let records =
            fossil.find_records(Some(vname), Some(last.unwrap_or(1)))?;
        if records.is_empty() {
            return Err(FossilError::NotFound(
                "no matching records found".into(),
            ));
        }
        let mut cols = Vec::new();
        for r in &records {
            let metrics = script.collect(&r.dir)?;
            let label = if records.len() == 1 {
                vname.to_string()
            } else {
                r.id()
            };
            cols.push((label, metrics));
        }
        return Ok(cols);
    }

    let all = fossil.find_records(None, last)?;
    if all.is_empty() {
        return Err(FossilError::NotFound("no matching records found".into()));
    }

    if last.is_some() {
        let mut cols = Vec::new();
        for r in &all {
            let metrics = script.collect(&r.dir)?;
            let label = r
                .manifest
                .variant
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| r.id());
            cols.push((label, metrics));
        }
        return Ok(cols);
    }

    let mut latest: BTreeMap<String, &Record> = BTreeMap::new();
    for r in &all {
        let key = r
            .manifest
            .variant
            .as_ref()
            .map(FossilVariantKey::as_str)
            .unwrap_or("untagged")
            .to_string();
        latest
            .entry(key)
            .and_modify(|prev| {
                if r.manifest.timestamp > prev.manifest.timestamp {
                    *prev = r;
                }
            })
            .or_insert(r);
    }

    let mut cols = Vec::new();
    for (name, record) in &latest {
        let metrics = script.collect(&record.dir)?;
        cols.push((name.clone(), metrics));
    }
    Ok(cols)
}

pub fn analyze(
    project: &Project,
    selectors: &[String],
    last: Option<usize>,
    analysis: Option<&str>,
) -> Result<Vec<(String, analysis::Metric)>, FossilError> {
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
    let mut columns = Vec::new();
    for selector in selectors {
        columns.extend(resolve_spec(project, selector, last, analysis)?);
    }

    let mut merged: BTreeMap<String, analysis::Metric> = BTreeMap::new();
    for (label, metric) in columns {
        match merged.entry(label) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(metric);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let label = entry.key().clone();
                entry.get_mut().merge(metric).map_err(|error| {
                    FossilError::InvalidConfig(format!(
                        "analysis column {label:?}: {error}"
                    ))
                })?;
            }
        }
    }
    Ok(merged.into_iter().collect())
}

pub fn emit_artifact(
    fossil: &Fossil,
    project: &Project,
    artifact_name: Option<&str>,
    variant: Option<&str>,
    last: Option<usize>,
    force: bool,
) -> Result<std::path::PathBuf, FossilError> {
    let artifact = crate::artifact::Artifact::resolve(fossil, artifact_name)?;
    let columns = artifact
        .analysis_name()
        .map(|analysis| {
            let selector = match variant {
                Some(variant) => format!("{}:{variant}", fossil.config.name),
                None => fossil.config.name.clone(),
            };
            analyze(project, &[selector], last, Some(analysis))
        })
        .transpose()?;
    artifact.run(fossil, project, columns.as_deref(), force)
}
