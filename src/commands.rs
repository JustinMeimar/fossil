use crate::analysis;
use crate::fossil::Fossil;
use crate::git;
use crate::manifest::{Environment, Manifest};
use crate::project::Project;
use crate::runner::Run;
use crate::ui::{info, status};

pub fn create_fossil(
    project: &Project,
    name: &str,
    description: Option<&str>,
    iterations: Option<u32>,
) -> anyhow::Result<()> {
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
) -> anyhow::Result<()> {
    let n = iterations.unwrap_or(fossil.config.default_iterations);
    let mut run = Run::new(args, n, variant)?;

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
) -> anyhow::Result<()> {
    let parser = fossil.parser().ok_or_else(|| {
        anyhow::anyhow!(
            "no parser configured for {:?}",
            fossil.config.name
        )
    })?;

    let records = fossil.find_records(variant, last)?;
    if records.is_empty() {
        anyhow::bail!("no matching records found");
    }

    for r in &records {
        let metrics = parser.collect(&r.dir)?;
        info!(
            "--- {} [commit: {}{}] ---",
            r.id(),
            r.manifest.git.commit,
            r.manifest
                .variant
                .as_ref()
                .map(|v| format!(", variant: {v}"))
                .unwrap_or_default(),
        );
        info!("  ({} iterations):", r.manifest.iterations);
        info!("{metrics}");
    }
    Ok(())
}

pub fn dig(
    fossil: &Fossil,
    variant: Option<&str>,
    last: Option<usize>,
) -> anyhow::Result<()> {
    let records = fossil.find_records(variant, last)?;

    if records.is_empty() {
        info!("no records found for {:?}", fossil.config.name);
        return Ok(());
    }

    for r in &records {
        info!(
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
) -> anyhow::Result<()> {
    let parser = fossil.parser().ok_or_else(|| {
        anyhow::anyhow!(
            "no parser configured for {:?}",
            fossil.config.name
        )
    })?;

    let get_latest =
        |variant: &str| -> anyhow::Result<analysis::MetricSet> {
            let records =
                fossil.find_records(Some(variant), Some(1))?;
            let r = records.into_iter().next().ok_or_else(|| {
                anyhow::anyhow!(
                    "no records found for variant {variant:?}"
                )
            })?;
            parser.collect(&r.dir)
        };

    let base_metrics = get_latest(baseline)?;
    let cand_metrics = get_latest(candidate)?;

    let cmp = analysis::Comparison {
        baseline: (baseline, &base_metrics),
        candidate: (candidate, &cand_metrics),
    };
    info!("{cmp}");
    Ok(())
}
