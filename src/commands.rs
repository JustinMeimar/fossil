use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use crate::analysis;
use crate::error::FossilError;
use crate::fossil::{Fossil, FossilConfig};
use crate::git;
use crate::manifest::{Environment, Manifest};
use crate::project::Project;
use crate::runner::Run;
use crate::ui::{output, status};

pub fn create_fossil(
    project: &Project,
    name: &str,
    description: Option<&str>,
    iterations: Option<u32>,
) -> Result<(), FossilError> {
    let f = Fossil::create(
        &project.fossils_dir(),
        name,
        description,
        iterations,
    )?;
    let rel = f.path.strip_prefix(&project.path).unwrap().to_path_buf();
    git::Commit::new(
        &project.path,
        vec![rel.join("fossil.toml")],
        format!("create fossil {name}"),
    )
    .execute()?;
    status!("created fossil {}", f.path.display());
    Ok(())
}

pub fn bury(
    fossil: &Fossil,
    project: &Project,
    iterations: Option<u32>,
    variant: Option<String>,
    args: Vec<String>,
) -> Result<(), FossilError> {
    let n = iterations.unwrap_or(fossil.config.default_iterations);
    let mut run = Run::new(
        args,
        n,
        variant,
        fossil.config.allow_failure,
        fossil.config.workdir.clone(),
    )?;

    for _ in 0..n {
        status!(
            "burying {}/{} ({}/{})",
            fossil.config.name,
            run.variant.as_deref().unwrap_or("untagged"),
            run.observations.len() + 1,
            n,
        );
        let obs = run.execute_one()?;
        status!("{}ms", obs.wall_time_us / 1000);
    }

    let env = Environment::capture();
    let m = Manifest::new(fossil, project, &run, env);
    let run_dir =
        m.record(&fossil.records_dir(), &run.observations_json())?;

    let rel = run_dir.strip_prefix(&project.path).unwrap().to_path_buf();
    git::Commit::new(
        &project.path,
        vec![rel.join("manifest.json"), rel.join("results.json")],
        format!(
            "bury {} {}",
            fossil.config.name,
            run.variant.as_deref().unwrap_or("untagged")
        ),
    )
    .execute()?;

    status!("{n} observations recorded → {}", run_dir.display());
    Ok(())
}

pub fn analyze(
    fossil: &Fossil,
    variant: Option<&str>,
    last: Option<usize>,
    as_json: bool,
) -> Result<(), FossilError> {
    let parser = fossil
        .parser()
        .ok_or_else(|| FossilError::NoParser(fossil.config.name.clone()))?;

    if variant.is_none() && last.is_none() {
        return analyze_summary(fossil, &parser, as_json);
    }

    let records = fossil.find_records(variant, last)?;
    if records.is_empty() {
        return Err(FossilError::NoRecords);
    }

    if as_json {
        let items: Vec<Value> = records
            .iter()
            .map(|r| {
                let metrics = parser.collect(&r.dir).ok();
                json!({
                    "id": r.id(),
                    "variant": r.manifest.variant,
                    "commit": r.manifest.git.commit,
                    "timestamp": r.manifest.timestamp,
                    "iterations": r.manifest.iterations,
                    "metrics": metrics.map(|m| m.to_json()),
                })
            })
            .collect();
        output!("{}", serde_json::to_string_pretty(&items).unwrap());
        return Ok(());
    }

    for r in &records {
        let metrics = parser.collect(&r.dir)?;
        output!(
            "--- {} [commit: {}{}] ---",
            r.id(),
            r.manifest.git.commit,
            r.manifest
                .variant
                .as_ref()
                .map(|v| format!(", variant: {v}"))
                .unwrap_or_default(),
        );
        output!("  ({} iterations):", r.manifest.iterations);
        output!("{metrics}");
    }
    Ok(())
}

fn analyze_summary(
    fossil: &Fossil,
    parser: &analysis::Parser,
    as_json: bool,
) -> Result<(), FossilError> {
    let all_records = fossil.find_records(None, None)?;
    if all_records.is_empty() {
        return Err(FossilError::NoRecords);
    }

    let mut latest: BTreeMap<String, &analysis::Record> = BTreeMap::new();
    for r in &all_records {
        let key = r
            .manifest
            .variant
            .clone()
            .unwrap_or_else(|| "untagged".to_string());
        latest
            .entry(key)
            .and_modify(|existing| {
                if r.manifest.timestamp > existing.manifest.timestamp {
                    *existing = r;
                }
            })
            .or_insert(r);
    }

    let mut columns = Vec::new();
    for (name, record) in &latest {
        let metrics = parser.collect(&record.dir)?;
        columns.push((name.clone(), metrics));
    }

    if as_json {
        let obj: serde_json::Map<String, Value> = columns
            .iter()
            .map(|(name, ms)| (name.clone(), ms.to_json()))
            .collect();
        output!("{}", serde_json::to_string_pretty(&obj).unwrap());
        return Ok(());
    }

    let summary = analysis::Summary { columns };
    output!("{summary}");
    Ok(())
}

pub fn dig(
    fossil: &Fossil,
    variant: Option<&str>,
    last: Option<usize>,
    as_json: bool,
) -> Result<(), FossilError> {
    let records = fossil.find_records(variant, last)?;

    if as_json {
        let items: Vec<Value> = records
            .iter()
            .map(|r| {
                json!({
                    "id": r.id(),
                    "variant": r.manifest.variant,
                    "commit": r.manifest.git.commit,
                    "branch": r.manifest.git.branch,
                    "timestamp": r.manifest.timestamp,
                    "iterations": r.manifest.iterations,
                    "command": r.manifest.command,
                })
            })
            .collect();
        output!("{}", serde_json::to_string_pretty(&items).unwrap());
        return Ok(());
    }

    if records.is_empty() {
        output!("no records found for {:?}", fossil.config.name);
        return Ok(());
    }

    for r in &records {
        output!(
            "  {}  commit={} variant={} iters={}",
            r.id(),
            r.manifest.git.commit,
            r.manifest.variant.as_deref().unwrap_or("-"),
            r.manifest.iterations,
        );
    }
    Ok(())
}

pub fn compare(
    fossil: &Fossil,
    baseline: &str,
    candidate: &str,
    as_json: bool,
) -> Result<(), FossilError> {
    let parser = fossil
        .parser()
        .ok_or_else(|| FossilError::NoParser(fossil.config.name.clone()))?;

    let get_latest =
        |variant: &str| -> Result<analysis::MetricSet, FossilError> {
            let records =
                fossil.find_records(Some(variant), Some(1))?;
            let r = records.into_iter().next().ok_or(
                FossilError::NoRecords,
            )?;
            parser.collect(&r.dir)
        };

    let base_metrics = get_latest(baseline)?;
    let cand_metrics = get_latest(candidate)?;

    if as_json {
        output!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "baseline": { "name": baseline, "metrics": base_metrics.to_json() },
                "candidate": { "name": candidate, "metrics": cand_metrics.to_json() },
            }))
            .unwrap()
        );
        return Ok(());
    }

    let cmp = analysis::Comparison {
        baseline: (baseline, &base_metrics),
        candidate: (candidate, &cand_metrics),
    };
    output!("{cmp}");
    Ok(())
}

fn parse_fossil_variant(s: &str) -> Result<(&str, &str), FossilError> {
    s.split_once(':').ok_or_else(|| FossilError::InvalidConfig {
        context: s.to_string(),
        reason: "expected fossil:variant syntax (e.g. compile:O3)".to_string(),
    })
}

pub fn compare_across(
    project: &Project,
    left: &str,
    right: &str,
    as_json: bool,
) -> Result<(), FossilError> {
    let (lf, lv) = parse_fossil_variant(left)?;
    let (rf, rv) = parse_fossil_variant(right)?;

    let resolve = |fossil_name: &str,
                   variant: &str|
     -> Result<analysis::MetricSet, FossilError> {
        let f =
            Fossil::load(&project.fossils_dir().join(fossil_name))?;
        let parser = f
            .parser()
            .ok_or_else(|| FossilError::NoParser(fossil_name.to_string()))?;
        let records = f.find_records(Some(variant), Some(1))?;
        let r = records
            .into_iter()
            .next()
            .ok_or(FossilError::NoRecords)?;
        parser.collect(&r.dir)
    };

    let left_metrics = resolve(lf, lv)?;
    let right_metrics = resolve(rf, rv)?;

    let left_label = format!("{lf}/{lv}");
    let right_label = format!("{rf}/{rv}");

    if as_json {
        output!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "baseline": { "name": left_label, "metrics": left_metrics.to_json() },
                "candidate": { "name": right_label, "metrics": right_metrics.to_json() },
            }))
            .unwrap()
        );
        return Ok(());
    }

    let cmp = analysis::Comparison {
        baseline: (&left_label, &left_metrics),
        candidate: (&right_label, &right_metrics),
    };
    output!("{cmp}");
    Ok(())
}

pub fn import(
    project: &Project,
    toml_path: &Path,
) -> Result<(), FossilError> {
    let contents = std::fs::read_to_string(toml_path).map_err(|_| {
        FossilError::InvalidConfig {
            context: toml_path.display().to_string(),
            reason: "file not found".to_string(),
        }
    })?;
    let config: FossilConfig =
        toml::from_str(&contents).map_err(|e| FossilError::InvalidConfig {
            context: toml_path.display().to_string(),
            reason: e.to_string(),
        })?;

    let fossil_dir = project.fossils_dir().join(&config.name);
    if fossil_dir.exists() {
        return Err(FossilError::FossilExists(config.name.clone()));
    }
    std::fs::create_dir_all(&fossil_dir)?;
    std::fs::create_dir_all(fossil_dir.join("records"))?;

    std::fs::copy(toml_path, fossil_dir.join("fossil.toml"))?;

    let source_dir = toml_path.parent().unwrap_or(Path::new("."));
    let mut git_paths = vec![
        fossil_dir
            .strip_prefix(&project.path)
            .unwrap()
            .join("fossil.toml"),
    ];

    if let Some(ref script) = config.analyze {
        let src = source_dir.join(script);
        if src.exists() {
            let dest = fossil_dir.join(script);
            std::fs::copy(&src, &dest)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&dest)?.permissions();
                perms.set_mode(perms.mode() | 0o111);
                std::fs::set_permissions(&dest, perms)?;
            }
            git_paths.push(
                fossil_dir
                    .strip_prefix(&project.path)
                    .unwrap()
                    .join(script),
            );
        }
    }

    git::Commit::new(
        &project.path,
        git_paths,
        format!("import fossil {}", config.name),
    )
    .execute()?;

    status!("imported {} → {}", config.name, fossil_dir.display());
    Ok(())
}
