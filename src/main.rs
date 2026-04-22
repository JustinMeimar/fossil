mod analysis;
mod fossil;
mod manifest;
mod runner;
mod project;
mod ui;

use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use fossil::Fossil;
use manifest::GitInfo;
use project::Project;
use ui::{status, error, info};

#[derive(Parser)]
#[command(name = "fossil", about = "Bury and dig up benchmark results")]
struct Cli {
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[arg(long, global = true)]
    project: Option<String>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Init,
    Project {
        #[command(subcommand)]
        command: ProjectCmd,
    },
    Create {
        name: String,
        #[arg(long)]
        desc: Option<String>,
        #[arg(short = 'n', long)]
        iterations: Option<u32>,
    },
    Bury {
        fossil: String,
        #[arg(short = 'n', long)]
        iterations: Option<u32>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(last = true)]
        command: Vec<String>,
    },
    Analyze {
        fossil: String,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        last: Option<usize>,
    },
    List,
    Dig {
        fossil: String,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        last: Option<usize>,
    },
    Compare {
        fossil: String,
        baseline: String,
        candidate: String,
    },
}

#[derive(Subcommand)]
enum ProjectCmd {
    Create {
        name: String,
        #[arg(long)]
        desc: Option<String>,
    },
    List,
}


// NOTE: we should create a cli.rs and move the above defs and these one off functions below to be
// factored int there. I like my main.rs to be as short as possible.
fn resolve_fossil_home(flag: Option<&PathBuf>) -> PathBuf {
    if let Some(p) = flag {
        return p.clone();
    }
    if let Ok(p) = std::env::var("FOSSIL_HOME") {
        return PathBuf::from(p);
    }
    dirs()
}

fn dirs() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".fossil")
}

fn projects_dir(fossil_home: &PathBuf) -> PathBuf {
    fossil_home.join("projects")
}

fn resolve_project(fossil_home: &PathBuf, name: Option<&str>) -> anyhow::Result<Project> {
    let pd = projects_dir(fossil_home);
    if let Some(n) = name {
        return Project::load(&pd.join(n));
    }
    let projects = Project::list_all(&pd)?;
    match projects.len() {
        0 => anyhow::bail!("no projects found — create one with: fossil project create <name>"),
        1 => {
            let project = projects.into_iter().next().unwrap();
            Ok(project)
        }
        _ => {
            let names: Vec<_> = projects.iter().map(|p| p.config.name.as_str()).collect();
            anyhow::bail!(
                "multiple projects exist, specify one with --project: {}",
                names.join(", ")
            );
        }
    }
}

fn main() {
    if let Err(e) = run() {
        error!("{e}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let fossil_home = resolve_fossil_home(cli.home.as_ref());

    match cli.command {
        Cmd::Init => {
            let pd = projects_dir(&fossil_home);
            std::fs::create_dir_all(&pd)?;
            status!("initialized {}", pd.display());
            Ok(())
        }
        Cmd::Project { command } => match command {
            ProjectCmd::Create { name, desc } => {
                let pd = projects_dir(&fossil_home);
                std::fs::create_dir_all(&pd)?;
                let project = Project::create(&pd, &name, desc.as_deref())?;
                status!("created project {}", project.path.display());
                Ok(())
            }
            ProjectCmd::List => {
                let pd = projects_dir(&fossil_home);
                let projects = Project::list_all(&pd)?;
                if projects.is_empty() {
                    info!("no projects");
                } else {
                    for p in &projects {
                        info!("  {:<20} {}",
                            p.config.name,
                            p.config.description.as_deref().unwrap_or(""),
                        );
                    }
                }
                Ok(())
            }
        },
        Cmd::Create { name, desc, iterations } => {
            let project = resolve_project(&fossil_home, cli.project.as_deref())?;
            let f = Fossil::create(&project.fossils_dir(), &name, desc.as_deref(), iterations)?;
            status!("created fossil {}", f.path.display());
            Ok(())
        }
        Cmd::Bury { fossil: fossil_name, iterations, tag, command } => {
            if command.is_empty() {
                anyhow::bail!("no command given — usage: fossil bury <name> -- <cmd...>");
            }

            let project = resolve_project(&fossil_home, cli.project.as_deref())?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;
            let n = iterations.unwrap_or(f.config.default_iterations);
            let cmd_str = command.join(" ");
            let git = GitInfo::current();

            let mut observations: Vec<Value> = Vec::new();
            for i in 1..=n {
                status!("burying {}/{} ({i}/{n})",
                    fossil_name,
                    tag.as_deref().unwrap_or("untagged"),
                );
                let obs = runner::Observation::run(&cmd_str, i)?;
                if obs.exit_code != 0 {
                    anyhow::bail!(
                        "command failed on iteration {i} (exit {})",
                        obs.exit_code,
                    );
                }
                status!("{}ms", obs.wall_time_us / 1000);
                observations.push(serde_json::to_value(&obs)?);
            }

            let results = json!({
                "fossil": fossil_name,
                "observations": observations,
            });

            let m = manifest::Manifest::new(
                fossil_name.clone(),
                project.config.name.clone(),
                cmd_str,
                f.config.description.clone(),
                n,
                tag,
                git,
            );
            let run_dir = m.record(&f.records_dir(), &results)?;

            status!("{n} observations recorded → {}", run_dir.display());
            Ok(())
        }
        Cmd::Analyze { fossil: fossil_name, tag, last } => {
            let project = resolve_project(&fossil_home, cli.project.as_deref())?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;
            let script = f.resolve_analyze()
                .ok_or_else(|| anyhow::anyhow!("no analyze script configured for {fossil_name}"))?;

            let runs = analysis::find_records(&f.records_dir(), tag.as_deref(), last)?;
            if runs.is_empty() {
                anyhow::bail!("no matching records found");
            }

            for (run_dir, run_manifest) in &runs {
                let run_id = run_dir.file_name().unwrap().to_string_lossy();
                info!("--- {run_id} [commit: {}{}] ---",
                    run_manifest.git.commit,
                    run_manifest.tag.as_ref().map(|t| format!(", tag: {t}")).unwrap_or_default(),
                );

                let metrics = analysis::collect_metrics(&script, &run_dir)?;
                info!("  ({} iterations):", run_manifest.iterations);
                for (name, values) in &metrics {
                    info!("    {name}: {:.1} ± {:.1}", analysis::mean(values), analysis::stddev(values));
                }
            }
            Ok(())
        }
        Cmd::List => {
            let project = resolve_project(&fossil_home, cli.project.as_deref())?;
            let fossils = Fossil::list_all(&project.fossils_dir())?;
            if fossils.is_empty() {
                info!("no fossils in project {:?}", project.config.name);
            } else {
                for f in &fossils {
                    info!("  {:<20} {}",
                        f.config.name,
                        f.config.description.as_deref().unwrap_or(""),
                    );
                }
            }
            Ok(())
        }
        Cmd::Dig { fossil: fossil_name, tag, last } => {
            let project = resolve_project(&fossil_home, cli.project.as_deref())?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;
            let runs = analysis::find_records(&f.records_dir(), tag.as_deref(), last)?;

            if runs.is_empty() {
                info!("no records found for {fossil_name}");
                return Ok(());
            }

            for (run_dir, m) in &runs {
                let run_id = run_dir.file_name().unwrap().to_string_lossy();
                info!("  {run_id}  commit={} tag={} iters={}",
                    m.git.commit,
                    m.tag.as_deref().unwrap_or("-"),
                    m.iterations,
                );
            }
            Ok(())
        }
        Cmd::Compare { fossil: fossil_name, baseline, candidate } => {
            let project = resolve_project(&fossil_home, cli.project.as_deref())?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;
            let script = f.resolve_analyze()
                .ok_or_else(|| anyhow::anyhow!("no analyze script configured for {fossil_name}"))?;

            let get_latest = |tag: &str| -> anyhow::Result<_> {
                let runs = analysis::find_records(&f.records_dir(), Some(tag), Some(1))?;
                let (run_dir, _) = runs.into_iter().next()
                    .ok_or_else(|| anyhow::anyhow!("no records found for tag {tag:?}"))?;
                analysis::collect_metrics(&script, &run_dir)
            };

            let base_metrics = get_latest(&baseline)?;
            let cand_metrics = get_latest(&candidate)?;

            let all_keys: BTreeMap<_, _> = base_metrics.keys()
                .chain(cand_metrics.keys())
                .map(|k| (k.clone(), ()))
                .collect();

            let base_w = baseline.len().max(10);
            let cand_w = candidate.len().max(10);

            info!("  {:<20} {:>base_w$}   {:>cand_w$}   {:>8}",
                "metric", baseline, candidate, "delta");
            info!("  {}", "─".repeat(20 + base_w + cand_w + 14));

            for key in all_keys.keys() {
                let b = base_metrics.get(key).map(|v| analysis::mean(v));
                let c = cand_metrics.get(key).map(|v| analysis::mean(v));

                let b_str = b.map(|v| format!("{v:.1}")).unwrap_or_else(|| "-".into());
                let c_str = c.map(|v| format!("{v:.1}")).unwrap_or_else(|| "-".into());
                let delta_str = match (b, c) {
                    (Some(bv), Some(cv)) if bv != 0.0 => {
                        let pct = (cv - bv) / bv * 100.0;
                        let sign = if pct >= 0.0 { "+" } else { "" };
                        format!("{sign}{pct:.1}%")
                    }
                    _ => "-".into(),
                };

                info!("  {:<20} {:>base_w$}   {:>cand_w$}   {:>8}",
                    key, b_str, c_str, delta_str);
            }
            Ok(())
        }
    }
}
