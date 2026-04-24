use crate::analysis;
use crate::error::FossilError;
use crate::fossil::Fossil;
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
    let mut run = Run::new(args, n, variant, fossil.config.allow_failure)?;

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
) -> Result<(), FossilError> {
    let parser = fossil
        .parser()
        .ok_or_else(|| FossilError::NoParser(fossil.config.name.clone()))?;

    let records = fossil.find_records(variant, last)?;
    if records.is_empty() {
        return Err(FossilError::NoRecords);
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

pub fn dig(
    fossil: &Fossil,
    variant: Option<&str>,
    last: Option<usize>,
) -> Result<(), FossilError> {
    let records = fossil.find_records(variant, last)?;

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

    let cmp = analysis::Comparison {
        baseline: (baseline, &base_metrics),
        candidate: (candidate, &cand_metrics),
    };
    output!("{cmp}");
    Ok(())
}
