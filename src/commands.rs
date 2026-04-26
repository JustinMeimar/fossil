use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use crate::analysis;
use crate::entity::DirEntity;
use crate::environment::Environment;
use crate::error::FossilError;
use crate::fossil::{Fossil, FossilConfig};
use crate::git;
use crate::manifest::Manifest;
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
        .map_err(|_| FossilError::InvalidConfig(format!(
            "{}: fossil path is not under project", f.path.display()
        )))?
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
        .map_err(|_| FossilError::InvalidConfig(format!(
            "{}: record path is not under project", run_dir.display()
        )))?
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
        return Err(FossilError::InvalidArgs(
            "no variants configured — define variants in fossil.toml or use -- <cmd>".into(),
        ));
    }
    for vname in &variants {
        let v = fossil.resolve_variant(vname)?;
        bury(
            fossil,
            project,
            iterations,
            Some(v.name),
            v.command,
        )?;
    }
    Ok(())
}

pub fn list_fossil_info(project: &Project) -> Result<(), FossilError> {
    let fossils = Fossil::list_all(&project.fossils_dir())?;
    if fossils.is_empty() {
        return Err(FossilError::NotFound("no matching records found".into()));
    }
    for f in &fossils {
        let desc = f.config.description.as_deref().unwrap_or("");
        crate::ui::output!("  {:<20} {desc}", f.config.name);
    }
    Ok(())
}

pub fn resolve_analysis<'a>(
    fossil: &'a Fossil,
    analysis: Option<&str>,
) -> Result<analysis::Parser, FossilError> {
    let spec = fossil
        .config
        .analyze
        .as_ref()
        .ok_or_else(|| FossilError::NotFound(format!(
            "no parser configured for {:?}", fossil.config.name
        )))?;

    let names = spec.names();
    let chosen = match analysis {
        Some(name) => {
            if spec.resolve(Some(name)).is_none() {
                return Err(FossilError::InvalidArgs(format!(
                    "unknown analysis {name:?}, available: {}", names.join(", ")
                )));
            }
            Some(name.to_string())
        }
        None if names.len() > 1 => {
            let picked = crate::ui::pick("select analysis:", &names)
                .ok_or_else(|| FossilError::InvalidArgs(format!(
                    "no analysis selected, available: {}", names.join(", ")
                )))?;
            Some(picked.to_string())
        }
        None => None,
    };

    fossil
        .parser(chosen.as_deref())
        .ok_or_else(|| FossilError::NotFound(format!(
            "no parser configured for {:?}", fossil.config.name
        )))
}

fn resolve_spec(
    project: &Project,
    spec: &str,
    last: Option<usize>,
    analysis: Option<&str>,
) -> Result<Vec<(String, analysis::AnalysisResult)>, FossilError> {
    let (fossil_name, variant) = match spec.split_once(':') {
        Some((f, v)) => (f, Some(v)),
        None => (spec, None),
    };

    let fossil = Fossil::load(&project.fossils_dir().join(fossil_name))?;
    let parser = resolve_analysis(&fossil, analysis)?;

    if let Some(vname) = variant {
        let records = fossil.find_records(Some(vname), Some(last.unwrap_or(1)))?;
        if records.is_empty() {
            return Err(FossilError::NotFound("no matching records found".into()));
        }
        let mut cols = Vec::new();
        for r in &records {
            let metrics = parser.collect(&r.dir)?;
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
            let metrics = parser.collect(&r.dir)?;
            let label = r.manifest.variant.clone().unwrap_or_else(|| r.id());
            cols.push((label, metrics));
        }
        return Ok(cols);
    }

    let mut latest: BTreeMap<String, &analysis::Record> = BTreeMap::new();
    for r in &all {
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

    let mut cols = Vec::new();
    for (name, record) in &latest {
        let metrics = parser.collect(&record.dir)?;
        cols.push((name.clone(), metrics));
    }
    Ok(cols)
}

fn fossil_name_from_spec(spec: &str) -> &str {
    spec.split_once(':').map_or(spec, |(f, _)| f)
}

pub fn analyze(
    project: &Project,
    specs: &[String],
    last: Option<usize>,
    analysis: Option<&str>,
) -> Result<analysis::Summary, FossilError> {
    if specs.len() > 1 {
        let names: Vec<_> = specs.iter().map(|s| fossil_name_from_spec(s)).collect();
        if !names.windows(2).all(|w| w[0] == w[1]) {
            let unique: Vec<_> = names.into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            return Err(FossilError::InvalidArgs(format!(
                "all specs must refer to the same fossil, got: {}", unique.join(", ")
            )));
        }
    }
    let mut columns = Vec::new();
    for spec in specs {
        columns.extend(resolve_spec(project, spec, last, analysis)?);
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

pub fn import(project: &Project, toml_path: &Path) -> Result<(), FossilError> {
    let contents = std::fs::read_to_string(toml_path)?;
    let config: FossilConfig =
        toml::from_str(&contents).map_err(|e| {
            FossilError::InvalidConfig(format!("{}: {e}", toml_path.display()))
        })?;

    let fossil_dir = project.fossils_dir().join(&config.name);
    if fossil_dir.exists() {
        return Err(FossilError::AlreadyExists(format!("fossil {:?}", config.name)));
    }
    std::fs::create_dir_all(&fossil_dir)?;
    std::fs::create_dir_all(fossil_dir.join("records"))?;
    std::fs::copy(toml_path, fossil_dir.join("fossil.toml"))?;

    let source_dir = toml_path.parent().unwrap_or(Path::new("."));
    let rel_fossil = fossil_dir.strip_prefix(&project.path).map_err(|_| {
        FossilError::InvalidConfig(format!(
            "{}: fossil path is not under project", fossil_dir.display()
        ))
    })?;
    let mut git_paths = vec![rel_fossil.join("fossil.toml")];

    if let Some(ref spec) = config.analyze {
        for script in spec.scripts() {
            let src = source_dir.join(script);
            if src.exists() {
                let dest = fossil_dir.join(script);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
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
