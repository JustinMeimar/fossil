use std::collections::BTreeMap;

use crate::analysis::{self, quantity::Quantity};
use crate::entity::DirEntity;
use crate::environment::{CpuInfo, GitInfo};
use crate::error::FossilError;
use crate::fossil::{Fossil, FossilVariantKey};
use crate::io::status;
use crate::manifest::Manifest;
use crate::project::Project;
use crate::record::Record;
use crate::runner::Run;

pub fn bury(
    fossil: &Fossil,
    project: &Project,
    iterations: Option<u32>,
    variant: Option<FossilVariantKey>,
    command: String,
    silent: bool,
) -> Result<String, FossilError> {
    if command.is_empty() {
        return Err(FossilError::InvalidArgs(
            "no command given — usage: fossil bury <name> -- <cmd...>".into(),
        ));
    }

    let n = iterations.unwrap_or(fossil.config.default_iterations);
    let mut run = Run {
        command,
        iterations: n,
        variant,
        allow_failure: fossil.config.allow_failure,
        workdir: fossil
            .config
            .workdir
            .as_ref()
            .map(|p| p.resolve(&fossil.path)),
        silent,
        observations: Vec::new(),
    };

    for _ in 0..n {
        if !silent {
            let vname = run.variant.as_ref().map(FossilVariantKey::as_str);
            status!(
                "burying {}/{} ({}/{})",
                fossil.config.name,
                vname.unwrap_or("untagged"),
                run.observations.len() + 1,
                n,
            );
        }
        let obs = run.execute_one()?;
        if !silent {
            status!("{}ms", obs.wall_time_us / 1000);
        }
    }

    let m = Manifest::new(
        fossil,
        project,
        &run,
        GitInfo::current(&project.path),
        CpuInfo::current(),
    );
    let run_dir = m.record(&fossil.records_dir(), &run.results())?;

    let rel = run_dir
        .strip_prefix(&project.path)
        .map_err(|_| {
            FossilError::InvalidConfig(format!(
                "{}: record path is not under project",
                run_dir.display()
            ))
        })?
        .to_path_buf();
    let vname = run.variant.as_ref().map(FossilVariantKey::as_str);
    project.commit(
        vec![rel.join("manifest.json"), rel.join("results.json")],
        format!(
            "bury {} {}",
            fossil.config.name,
            vname.unwrap_or("untagged"),
        ),
    )?;

    let avg_ms = if run.observations.is_empty() {
        0
    } else {
        let total: u64 = run.observations.iter().map(|o| o.wall_time_us).sum();
        total / run.observations.len() as u64 / 1000
    };

    if !silent {
        status!("{n} observations recorded → {}", run_dir.display());
    }

    Ok(format!("{n} observations recorded ({avg_ms}ms avg)"))
}

pub fn bury_all(
    fossil: &Fossil,
    project: &Project,
    iterations: Option<u32>,
) -> Result<(), FossilError> {
    let variants: Vec<_> = fossil.config.variants.keys().cloned().collect();
    if variants.is_empty() {
        return Err(FossilError::InvalidArgs(
            "no variants configured — define variants in fossil.toml or use -- <cmd>".into(),
        ));
    }
    for vname in &variants {
        let v = fossil.resolve_variant(vname, &project.config.constants)?;
        bury(fossil, project, iterations, Some(v.name), v.command, false)?;
    }
    Ok(())
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
    let script = fossil.resolve_analysis(analysis)?;

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
        merged
            .entry(label)
            .and_modify(|acc| *acc = acc.combine(&metric))
            .or_insert(metric);
    }
    Ok(merged.into_iter().collect())
}
