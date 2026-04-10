mod manifest;
mod measure;
mod runner;

use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use manifest::GitInfo;
use measure::Measure;

#[derive(Parser)]
#[command(name = "measure", about = "SpiderMonkey measurement tool")]
struct Cli {
    #[arg(long, default_value = "./measures", global = true)]
    measures_dir: PathBuf,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run {
        config: PathBuf,
        #[arg(short = 'n', long)]
        iterations: Option<u32>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        experiment: Option<String>,
        #[arg(long)]
        allow_dirty: bool,
    },
    List {
        #[arg(default_value = "./benchmarks")]
        dir: PathBuf,
    },
}

struct Session {
    measure: Measure,
    git: GitInfo,
    iterations: u32,
    tag: Option<String>,
    experiment: Option<String>,
    measures_dir: PathBuf,
    json: bool,
}

impl Session {
    fn run(&self) -> anyhow::Result<()> {
        if !self.json {
            eprintln!("=== {} (n={}) ===", self.measure.name, self.iterations);
            eprintln!("  commit: {} ({})", self.git.commit, self.git.branch);
            eprintln!();
        }

        let results = self.execute()?;
        let run_dir = manifest::make_run_dir(
            &self.measures_dir, &self.measure.name, &self.git.commit, self.tag.as_deref(),
        )?;
        let manifest = manifest::build_manifest(
            &self.measure, self.iterations, self.tag.as_deref(), self.experiment.as_deref(),
        );
        manifest::write_run(&run_dir, &manifest, &results)?;

        if self.json {
            let output = json!({
                "run_id": run_dir.file_name().unwrap().to_string_lossy(),
                "status": "complete",
                "path": run_dir,
                "results": results,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            eprintln!("results: {}", run_dir.display());
        }
        Ok(())
    }

    fn execute(&self) -> anyhow::Result<Value> {
        let configs: BTreeMap<String, Value> = self.measure.configs.iter()
            .map(|(label, config)| {
                let observations: Vec<Value> = (1..=self.iterations)
                    .map(|i| {
                        if !self.json { eprint!("  {label} [{i}/{}]... ", self.iterations); }
                        let obs = runner::run_iteration(
                            &config.command, &self.measure.resolve_cwd(), i,
                        )?;

                        if !self.json {
                            eprintln!("{}ms (exit {})", obs.wall_time_us / 1000, obs.exit_code);
                        }
                        serde_json::to_value(&obs).map_err(Into::into)
                    })
                    .collect::<anyhow::Result<_>>()?;

                Ok((label.clone(), json!(observations)))
            })
            .collect::<anyhow::Result<_>>()?;

        Ok(json!({ "measure": self.measure.name, "configs": configs }))
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { config, iterations, tag, experiment, allow_dirty } => {
            let measure = Measure::load(&config)?;
            let git = manifest::git_info();
            if git.dirty && !allow_dirty {
                anyhow::bail!("working tree is dirty (use --allow-dirty to override)");
            }
            Session {
                iterations: iterations.unwrap_or(measure.default_iterations),
                measure, git, tag, experiment,
                measures_dir: cli.measures_dir,
                json: cli.json,
            }.run()
        }
        Command::List { dir } => {
            let mut defs: Vec<_> = std::fs::read_dir(&dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
                .filter_map(|e| Measure::load(&e.path()).ok())
                .collect();
            defs.sort_by(|a, b| a.name.cmp(&b.name));

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&defs)?);
            } else {
                for m in &defs {
                    eprintln!("  {:<20} {}", m.name, m.description.as_deref().unwrap_or(""));
                }
            }
            Ok(())
        }
    }
}
