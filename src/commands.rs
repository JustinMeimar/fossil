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
    silent: bool,
) -> Result<String, FossilError> {
    let n = iterations.unwrap_or(fossil.config.default_iterations);
    let mut run = Run::new(
        args,
        n,
        variant,
        fossil.config.allow_failure,
        fossil.config.workdir.clone(),
        silent,
    )?;

    for _ in 0..n {
        if !silent {
            status!(
                "burying {}/{} ({}/{})",
                fossil.config.name,
                run.variant.as_deref().unwrap_or("untagged"),
                run.observations.len() + 1,
                n,
            );
        }
        let obs = run.execute_one()?;
        if !silent {
            status!("{}ms", obs.wall_time_us / 1000);
        }
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

    let avg_ms = if run.observations.is_empty() {
        0
    } else {
        let total: u64 =
            run.observations.iter().map(|o| o.wall_time_us).sum();
        total / run.observations.len() as u64 / 1000
    };

    if !silent {
        status!("{n} observations recorded → {}", run_dir.display());
    }

    Ok(format!(
        "{n} observations recorded ({avg_ms}ms avg)"
    ))
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
            false,
        )?;
    }
    Ok(())
}

pub fn delete_record(
    project: &Project,
    record: &analysis::Record,
) -> Result<(), FossilError> {
    let rel = record
        .dir
        .strip_prefix(&project.path)
        .map_err(|_| FossilError::InvalidConfig(format!(
            "{}: record path is not under project",
            record.dir.display()
        )))?
        .to_path_buf();
    git::rm(&project.path, &rel)?;
    git::commit(
        &project.path,
        &format!("delete record {}", record.id()),
    )?;
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
) -> Result<analysis::AnalysisScript, FossilError> {
    let spec = fossil
        .config
        .analyze
        .as_ref()
        .ok_or_else(|| FossilError::NotFound(format!(
            "no analysis script configured for {:?}", fossil.config.name
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
        .analysis_script(chosen.as_deref())
        .ok_or_else(|| FossilError::NotFound(format!(
            "no analysis script configured for {:?}", fossil.config.name
        )))
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
    let script = resolve_analysis(&fossil, analysis)?;

    if let Some(vname) = variant {
        let records = fossil.find_records(Some(vname), Some(last.unwrap_or(1)))?;
        if records.is_empty() {
            return Err(FossilError::NotFound("no matching records found".into()));
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
        let metrics = script.collect(&record.dir)?;
        cols.push((name.clone(), metrics));
    }
    Ok(cols)
}

fn fossil_name_from_selector(selector: &str) -> &str {
    selector.split_once(':').map_or(selector, |(f, _)| f)
}

pub fn analyze(
    project: &Project,
    selectors: &[String],
    last: Option<usize>,
    analysis: Option<&str>,
) -> Result<Vec<(String, analysis::Metric)>, FossilError> {
    let unique_names: std::collections::BTreeSet<_> = selectors
        .iter()
        .map(|s| fossil_name_from_selector(s))
        .collect();
    if unique_names.len() > 1 {
        let names: Vec<_> = unique_names.into_iter().collect();
        return Err(FossilError::InvalidArgs(format!(
            "all selectors must refer to the same fossil, got: {}", names.join(", ")
        )));
    }
    let mut columns = Vec::new();
    for selector in selectors {
        columns.extend(resolve_spec(project, selector, last, analysis)?);
    }
    Ok(columns)
}

pub fn resolve_viz<'a>(
    fossil: &'a Fossil,
    viz_name: Option<&'a str>,
) -> Result<(&'a str, &'a crate::fossil::VizEntry), FossilError> {
    let map = fossil
        .config
        .visualize
        .as_ref()
        .ok_or_else(|| FossilError::NotFound(format!(
            "no visualizations configured for {:?}", fossil.config.name
        )))?;

    match viz_name {
        Some(name) => {
            let entry = map.get(name).ok_or_else(|| {
                let names: Vec<_> =
                    map.keys().map(|k| k.as_str()).collect();
                FossilError::InvalidArgs(format!(
                    "unknown visualization {name:?}, available: {}",
                    names.join(", ")
                ))
            })?;
            Ok((name, entry))
        }
        None if map.len() == 1 => {
            let (name, entry) = map.iter().next().unwrap();
            Ok((name.as_str(), entry))
        }
        None => {
            let names: Vec<&str> =
                map.keys().map(|k| k.as_str()).collect();
            let picked =
                crate::ui::pick("select visualization:", &names)
                    .ok_or_else(|| FossilError::InvalidArgs(format!(
                        "no visualization selected, available: {}",
                        names.join(", ")
                    )))?;
            let (k, entry) = map.get_key_value(picked).unwrap();
            Ok((k.as_str(), entry))
        }
    }
}

pub fn viz(
    project: &Project,
    fossil_name: &str,
    last: Option<usize>,
    variant: Option<&str>,
    viz_name: Option<&str>,
) -> Result<(), FossilError> {
    let fossil = Fossil::load(&project.fossils_dir().join(fossil_name))?;
    let (vname, entry) = resolve_viz(&fossil, viz_name)?;

    let spec = match variant {
        Some(v) => format!("{fossil_name}:{v}"),
        None => fossil_name.to_string(),
    };
    let columns = analyze(
        project,
        &[spec],
        last,
        Some(&entry.analysis),
    )?;

    let result: BTreeMap<&str, &analysis::Metric> = columns
        .iter()
        .map(|(name, m)| (name.as_str(), m))
        .collect();
    let json = serde_json::to_string_pretty(&result)
        .map_err(|e| FossilError::InvalidConfig(format!(
            "serializing analysis: {e}"
        )))?;

    let script_path = fossil.viz_script(vname).ok_or_else(|| {
        FossilError::NotFound(format!(
            "viz script not found for {vname:?}"
        ))
    })?;

    status!("visualizing with {vname} ({})", script_path.display());

    let mut child = std::process::Command::new(&script_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .current_dir(&fossil.path)
        .env("FOSSIL_NAME", &fossil.config.name)
        .env("FOSSIL_DIR", &fossil.path)
        .env("FOSSIL_VIZ_NAME", vname)
        .spawn()
        .map_err(|e| FossilError::InvalidConfig(format!(
            "viz script {} failed: {e} — is the script executable?",
            script_path.display()
        )))?;

    if let Some(mut stdin) = child.stdin.take() {
        std::io::Write::write_all(&mut stdin, json.as_bytes())
            .map_err(FossilError::Io)?;
    }

    let exit = child.wait()?;
    if !exit.success() {
        return Err(FossilError::InvalidConfig(format!(
            "viz script {} exited with code {}",
            script_path.display(),
            exit.code().unwrap_or(-1),
        )));
    }

    Ok(())
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

    if let Some(ref viz_map) = config.visualize {
        for entry in viz_map.values() {
            let src = source_dir.join(&entry.script);
            if src.exists() {
                let dest = fossil_dir.join(&entry.script);
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
                git_paths.push(rel_fossil.join(&entry.script));
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
