mod analysis;
mod fossil;
mod manifest;
mod runner;
mod site;

use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use fossil::Fossil;
use manifest::GitInfo;
use site::Site;

#[derive(Parser)]
#[command(name = "fossil", about = "Bury and dig up benchmark results")]
struct Cli {
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[arg(long, global = true)]
    site: Option<String>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Init,
    Site {
        #[command(subcommand)]
        command: SiteCmd,
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
}

#[derive(Subcommand)]
enum SiteCmd {
    Create {
        name: String,
        #[arg(long)]
        desc: Option<String>,
    },
    List,
}

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

fn sites_dir(fossil_home: &PathBuf) -> PathBuf {
    fossil_home.join("sites")
}

fn resolve_site(fossil_home: &PathBuf, name: Option<&str>) -> anyhow::Result<Site> {
    let sd = sites_dir(fossil_home);
    if let Some(n) = name {
        return Site::load(&sd.join(n));
    }
    let sites = Site::list_all(&sd)?;
    match sites.len() {
        0 => anyhow::bail!("no sites found — create one with: fossil site create <name>"),
        1 => {
            let site = sites.into_iter().next().unwrap();
            Ok(site)
        }
        _ => {
            let names: Vec<_> = sites.iter().map(|s| s.config.name.as_str()).collect();
            anyhow::bail!(
                "multiple sites exist, specify one with --site: {}",
                names.join(", ")
            );
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let fossil_home = resolve_fossil_home(cli.home.as_ref());

    match cli.command {
        Cmd::Init => {
            let sd = sites_dir(&fossil_home);
            std::fs::create_dir_all(&sd)?;
            eprintln!("[fossil] initialized {}", sd.display());
            Ok(())
        }
        Cmd::Site { command } => match command {
            SiteCmd::Create { name, desc } => {
                let sd = sites_dir(&fossil_home);
                std::fs::create_dir_all(&sd)?;
                let site = Site::create(&sd, &name, desc.as_deref())?;
                eprintln!("[fossil] created site {}", site.path.display());
                Ok(())
            }
            SiteCmd::List => {
                let sd = sites_dir(&fossil_home);
                let sites = Site::list_all(&sd)?;
                if sites.is_empty() {
                    eprintln!("no sites");
                } else {
                    for s in &sites {
                        eprintln!("  {:<20} {}",
                            s.config.name,
                            s.config.description.as_deref().unwrap_or(""),
                        );
                    }
                }
                Ok(())
            }
        },
        Cmd::Create { name, desc, iterations } => {
            let site = resolve_site(&fossil_home, cli.site.as_deref())?;
            let f = Fossil::create(&site.fossils_dir(), &name, desc.as_deref(), iterations)?;
            eprintln!("[fossil] created fossil {}", f.path.display());
            Ok(())
        }
        Cmd::Bury { fossil: fossil_name, iterations, tag, command } => {
            if command.is_empty() {
                anyhow::bail!("no command given — usage: fossil bury <name> -- <cmd...>");
            }

            let site = resolve_site(&fossil_home, cli.site.as_deref())?;
            let f = Fossil::load(&site.fossils_dir().join(&fossil_name))?;
            let n = iterations.unwrap_or(f.config.default_iterations);
            let cmd_str = command.join(" ");
            let git = GitInfo::current();

            let mut observations: Vec<Value> = Vec::new();
            for i in 1..=n {
                eprintln!("[fossil] burying {}/{} ({i}/{n})",
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
                eprintln!("[fossil] {}ms", obs.wall_time_us / 1000);
                observations.push(serde_json::to_value(&obs)?);
            }

            let results = json!({
                "fossil": fossil_name,
                "observations": observations,
            });

            let run_dir = manifest::make_run_dir(
                &f.records_dir(), &git.commit, tag.as_deref(),
            )?;

            let m = manifest::Manifest {
                version: 3,
                timestamp: manifest::timestamp(),
                fossil: fossil_name.clone(),
                site: site.config.name.clone(),
                command: cmd_str,
                description: f.config.description.clone(),
                iterations: n,
                tag,
                git,
                cpu: manifest::CpuInfo::current(),
                kernel: manifest::kernel_version(),
            };
            m.write(&run_dir, &results)?;

            eprintln!("[fossil] {n} observations recorded → {}", run_dir.display());
            Ok(())
        }
        Cmd::Analyze { fossil: fossil_name, tag, last } => {
            let site = resolve_site(&fossil_home, cli.site.as_deref())?;
            let f = Fossil::load(&site.fossils_dir().join(&fossil_name))?;
            let script = f.resolve_analyze()
                .ok_or_else(|| anyhow::anyhow!("no analyze script configured for {fossil_name}"))?;

            let runs = analysis::find_records(&f.records_dir(), tag.as_deref(), last)?;
            if runs.is_empty() {
                anyhow::bail!("no matching records found");
            }

            for (run_dir, run_manifest) in &runs {
                let run_id = run_dir.file_name().unwrap().to_string_lossy();
                let results: Value = serde_json::from_str(
                    &std::fs::read_to_string(run_dir.join("results.json"))?,
                )?;
                let observations = results["observations"].as_array()
                    .ok_or_else(|| anyhow::anyhow!("invalid results in {run_id}"))?;

                eprintln!("--- {run_id} [commit: {}{}] ---",
                    run_manifest.git.commit,
                    run_manifest.tag.as_ref().map(|t| format!(", tag: {t}")).unwrap_or_default(),
                );

                let mut metrics: BTreeMap<String, Vec<f64>> = BTreeMap::new();
                for obs in observations {
                    let result = analysis::run_script(&script, obs)?;
                    if let Some(obj) = result.as_object() {
                        for (k, v) in obj {
                            if let Some(n) = v.as_f64() {
                                metrics.entry(k.clone()).or_default().push(n);
                            }
                        }
                    }
                }

                eprintln!("  ({} iterations):", observations.len());
                for (name, values) in &metrics {
                    eprintln!("    {name}: {:.1} ± {:.1}", analysis::mean(values), analysis::stddev(values));
                }
            }
            Ok(())
        }
        Cmd::List => {
            let site = resolve_site(&fossil_home, cli.site.as_deref())?;
            let fossils = Fossil::list_all(&site.fossils_dir())?;
            if fossils.is_empty() {
                eprintln!("no fossils in site {:?}", site.config.name);
            } else {
                for f in &fossils {
                    eprintln!("  {:<20} {}",
                        f.config.name,
                        f.config.description.as_deref().unwrap_or(""),
                    );
                }
            }
            Ok(())
        }
        Cmd::Dig { fossil: fossil_name, tag, last } => {
            let site = resolve_site(&fossil_home, cli.site.as_deref())?;
            let f = Fossil::load(&site.fossils_dir().join(&fossil_name))?;
            let runs = analysis::find_records(&f.records_dir(), tag.as_deref(), last)?;

            if runs.is_empty() {
                eprintln!("no records found for {fossil_name}");
                return Ok(());
            }

            for (run_dir, m) in &runs {
                let run_id = run_dir.file_name().unwrap().to_string_lossy();
                eprintln!("  {run_id}  commit={} tag={} iters={}",
                    m.git.commit,
                    m.tag.as_deref().unwrap_or("-"),
                    m.iterations,
                );
            }
            Ok(())
        }
    }
}
