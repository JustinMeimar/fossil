mod analysis;
mod artifact;
mod cli;
mod commands;
mod entity;
mod environment;
mod error;
mod fossil;
mod git;
mod io;
mod manifest;
mod project;
mod record;
mod runner;
mod web;

use clap::Parser;
use cli::{Cli, Cmd, ProjectCmd};
use entity::DirEntity;
use fossil::{ConfigurationKey, Fossil};
use io::{error, output, status};
use project::Project;
use runner::OutputMode;

fn main() {
    if let Err(e) = run() {
        error!("{e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), error::FossilError> {
    let cli = Cli::parse();
    let fossil_home = cli::resolve_fossil_home(cli.home.as_ref());
    let projects_dir = fossil_home.join("projects");

    match cli.command {
        Cmd::Serve { port } => web::serve(projects_dir, cli.project, port),
        Cmd::Init => {
            std::fs::create_dir_all(&projects_dir)?;
            status!("initialized {}", projects_dir.display());
            Ok(())
        }
        Cmd::Project { command } => match command {
            ProjectCmd::Create { name, desc } => {
                std::fs::create_dir_all(&projects_dir)?;
                let p = Project::create(&projects_dir, &name, desc.as_deref())?;
                status!("created project {}", p.path.display());
                Ok(())
            }
            ProjectCmd::List => {
                let projects = Project::list_all(&projects_dir)?;
                if projects.is_empty() {
                    output!("no projects");
                } else {
                    for p in &projects {
                        output!("  {:<20} {}", p.config.name, p.config.desc());
                    }
                }
                Ok(())
            }
        },
        Cmd::Create {
            name,
            desc,
            iterations,
        } => {
            let project =
                Project::resolve(&projects_dir, cli.project.as_deref(), None)?;
            project.create_fossil(&name, desc.as_deref(), iterations)
        }
        Cmd::Bury {
            fossil: fname,
            iterations,
            variant,
            dry_run,
            silent,
        } => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(&fname),
            )?;
            let f = Fossil::load(&project.fossils_dir().join(&fname))?;
            let variants: Vec<ConfigurationKey> = variant
                .into_iter()
                .map(ConfigurationKey::new)
                .collect();
            let tasks = commands::bury_tasks(&f, &variants)?;

            if dry_run {
                for variant in &tasks {
                    output!("[{}]\n{}\n", variant.name(), variant.command());
                }
                return Ok(());
            }

            let output_mode = if silent {
                OutputMode::ProgressOnly
            } else {
                OutputMode::Verbose
            };
            commands::bury(
                &f,
                &project,
                iterations,
                tasks,
                output_mode,
                |event| match event {
                    commands::BuryProgress::Running {
                        variant,
                        iteration,
                        iterations,
                        ..
                    } => {
                        status!(
                            "burying {}/{} ({}/{})",
                            f.config.name,
                            variant,
                            iteration,
                            iterations
                        );
                    }
                    commands::BuryProgress::Recorded {
                        wall_time_us,
                        record_dir,
                        ..
                    } => {
                        status!(
                            "{}ms recorded → {}",
                            wall_time_us / 1000,
                            record_dir.display()
                        );
                    }
                },
            )?;
            Ok(())
        }
        Cmd::Analyze {
            selectors,
            last,
            analysis,
        } => {
            if selectors.is_empty() {
                let project = Project::resolve(
                    &projects_dir,
                    cli.project.as_deref(),
                    None,
                )?;
                return commands::list_fossil_info(&project);
            }
            let fossil_hint = selectors[0].split(':').next().unwrap();
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(fossil_hint),
            )?;
            let analysis = analysis.map(ConfigurationKey::new);
            let analysis_result = commands::analyze(
                &project,
                &selectors,
                last,
                analysis.as_ref(),
            )?;
            output!("{}", analysis_result.to_json()?);
            Ok(())
        }
        Cmd::Emit {
            fossil: fname,
            last,
            variant,
            artifact,
        } => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(&fname),
            )?;
            let fossil = Fossil::load(&project.fossils_dir().join(&fname))?;
            let artifact = artifact.map(ConfigurationKey::new);
            let path = commands::emit_artifact(
                &fossil,
                &project,
                artifact.as_ref(),
                variant.as_deref(),
                last,
            )?;
            status!("wrote {}", path.display());
            Ok(())
        }
        Cmd::List => {
            let project =
                Project::resolve(&projects_dir, cli.project.as_deref(), None)?;
            let fossils = Fossil::list_all(&project.fossils_dir())?;
            if fossils.is_empty() {
                output!("no fossils in project {:?}", project.config.name);
            } else {
                for f in &fossils {
                    output!("  {:<20} {}", f.config.name, f.config.desc());
                }
            }
            Ok(())
        }
        Cmd::Import { path } => {
            let project =
                Project::resolve(&projects_dir, cli.project.as_deref(), None)?;
            let abs = std::fs::canonicalize(&path)?;
            project.import(&abs)
        }
    }
}
