use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use crate::analysis;
use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::fossil::{Fossil, FossilConfig};
use crate::git;
use crate::manifest::{Environment, Manifest};
use crate::project::Project;
use crate::runner::Run;
use crate::ui::status;

pub fn create_fossil(
    project: &Project,
    name: &str,
    description: Option<&str>,
    iterations: Option<u32>,
) -> Result<(), FossilError> {
    let f =
        Fossil::create(&project.fossils_dir(), name, description, iterations)?;
    let rel = f
        .path
        .strip_prefix(&project.path)
        .map_err(|_| FossilError::InvalidConfig {
            context: f.path.display().to_string(),
            reason: "fossil path is not under project".into(),
        })?
        .to_path_buf();
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

    let env = Environment::capture(&project.path);
    let m = Manifest::new(fossil, project, &run, env);
    let run_dir = m.record(&fossil.records_dir(), &run.results_file())?;

    let rel = run_dir
        .strip_prefix(&project.path)
        .map_err(|_| FossilError::InvalidConfig {
            context: run_dir.display().to_string(),
            reason: "record path is not under project".into(),
        })?
        .to_path_buf();
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

pub fn bury_all(
    fossil: &Fossil,
    project: &Project,
    iterations: Option<u32>,
) -> Result<(), FossilError> {
    let variants: Vec<_> = fossil.config.variants.keys().cloned().collect();
    if variants.is_empty() {
        return Err(FossilError::NoCommand);
    }
    for vname in &variants {
        let v = fossil.resolve_variant(vname)?;
        bury(
            fossil,
            project,
            iterations,
            Some(v.name.to_string()),
            v.command.to_vec(),
        )?;
    }
    Ok(())
}

pub fn analyze(
    fossil: &Fossil,
    variant: Option<&str>,
    last: Option<usize>,
) -> Result<analysis::Summary, FossilError> {
    let parser = fossil
        .parser()
        .ok_or_else(|| FossilError::NoParser(fossil.config.name.clone()))?;

    if variant.is_some() || last.is_some() {
        let records = fossil.find_records(variant, last)?;
        if records.is_empty() {
            return Err(FossilError::NoRecords);
        }
        let mut columns = Vec::new();
        for r in &records {
            let metrics = parser.collect(&r.dir)?;
            let label = r.manifest.variant.clone().unwrap_or_else(|| r.id());
            columns.push((label, metrics));
        }
        return Ok(analysis::Summary { columns });
    }

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
            .and_modify(|prev| {
                if r.manifest.timestamp > prev.manifest.timestamp {
                    *prev = r;
                }
            })
            .or_insert(r);
    }

    let mut columns = Vec::new();
    for (name, record) in &latest {
        let metrics = parser.collect(&record.dir)?;
        columns.push((name.clone(), metrics));
    }
    Ok(analysis::Summary { columns })
}

pub fn dig(
    fossil: &Fossil,
    variant: Option<&str>,
    last: Option<usize>,
) -> Result<Vec<Value>, FossilError> {
    let records = fossil.find_records(variant, last)?;
    Ok(records
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
        .collect())
}

pub fn compare(
    project: &Project,
    left_fossil: &str,
    left_variant: &str,
    right_fossil: &str,
    right_variant: &str,
) -> Result<analysis::Summary, FossilError> {
    let resolve = |fname: &str,
                   vname: &str|
     -> Result<(String, analysis::MetricSet), FossilError> {
        let f = Fossil::load(&project.fossils_dir().join(fname))?;
        let parser = f
            .parser()
            .ok_or_else(|| FossilError::NoParser(fname.to_string()))?;
        let records = f.find_records(Some(vname), Some(1))?;
        let r = records.into_iter().next().ok_or(FossilError::NoRecords)?;
        let metrics = parser.collect(&r.dir)?;
        let label = if left_fossil == right_fossil {
            vname.to_string()
        } else {
            format!("{fname}/{vname}")
        };
        Ok((label, metrics))
    };

    let left = resolve(left_fossil, left_variant)?;
    let right = resolve(right_fossil, right_variant)?;
    Ok(analysis::Summary {
        columns: vec![left, right],
    })
}

pub fn import(project: &Project, toml_path: &Path) -> Result<(), FossilError> {
    let contents = std::fs::read_to_string(toml_path)?;
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
    let rel_fossil = fossil_dir.strip_prefix(&project.path).map_err(|_| {
        FossilError::InvalidConfig {
            context: fossil_dir.display().to_string(),
            reason: "fossil path is not under project".into(),
        }
    })?;
    let mut git_paths = vec![rel_fossil.join("fossil.toml")];

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
            git_paths.push(rel_fossil.join(script));
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
