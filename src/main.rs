mod analysis;
mod cli;
mod commands;
mod entity;
mod environment;
mod error;
mod fossil;
mod git;
mod manifest;
mod project;
mod runner;
mod ui;
mod tui;
mod web;

use clap::Parser;
use cli::{Cli, Cmd, ProjectCmd};
use entity::DirEntity;
use fossil::Fossil;
use project::Project;
use ui::{error, output, status};

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

    let command = match cli.command {
        Some(cmd) => cmd,
        None => return tui::run(fossil_home),
    };

    match command {
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
                        output!(
                            "  {:<20} {}",
                            p.config.name,
                            p.config.description.as_deref().unwrap_or(""),
                        );
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
            Ok(commands::create_fossil(
                &project,
                &name,
                desc.as_deref(),
                iterations,
            )?)
        }
        Cmd::Bury {
            fossil: fname,
            iterations,
            variant,
            command,
        } => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(&fname),
            )?;
            let f = Fossil::load(&project.fossils_dir().join(&fname))?;

            match (variant, command.is_empty()) {
                (Some(name), true) => {
                    let v = f.resolve_variant(&name)?;
                    commands::bury(
                        &f, &project, iterations,
                        Some(v.name), v.command, false,
                    )?;
                    Ok(())
                }
                (Some(_), false) => Err(error::FossilError::InvalidArgs(
                    "cannot specify both --variant and -- <command>".into(),
                )),
                (None, false) => {
                    commands::bury(
                        &f, &project, iterations,
                        None, command, false,
                    )?;
                    Ok(())
                }
                (None, true) => {
                    Ok(commands::bury_all(&f, &project, iterations)?)
                }
            }
        }
        Cmd::Analyze {
            specs,
            last,
            analysis,
            csv,
        } => {
            if specs.is_empty() {
                let project = Project::resolve(
                    &projects_dir,
                    cli.project.as_deref(),
                    None,
                )?;
                return commands::list_fossil_info(&project);
            }
            let fossil_hint = specs[0].split(':').next().unwrap();
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(fossil_hint),
            )?;
            let summary = commands::analyze(
                &project,
                &specs,
                last,
                analysis.as_deref(),
            )?;
            emit(&summary, cli.json, csv);
            Ok(())
        }
        Cmd::Viz {
            fossil: fname,
            last,
            variant,
            viz,
        } => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(&fname),
            )?;
            commands::viz(
                &project,
                &fname,
                last,
                variant.as_deref(),
                viz.as_deref(),
            )
        }
        Cmd::List => {
            let project =
                Project::resolve(&projects_dir, cli.project.as_deref(), None)?;
            let fossils = Fossil::list_all(&project.fossils_dir())?;
            if fossils.is_empty() {
                output!("no fossils in project {:?}", project.config.name);
            } else {
                for f in &fossils {
                    output!(
                        "  {:<20} {}",
                        f.config.name,
                        f.config.description.as_deref().unwrap_or(""),
                    );
                }
            }
            Ok(())
        }
        Cmd::Dig {
            fossil: fname,
            variant,
            last,
        } => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(&fname),
            )?;
            let f = Fossil::load(&project.fossils_dir().join(&fname))?;
            let records = commands::dig(&f, variant.as_deref(), last)?;
            if cli.json {
                output!("{}", serde_json::to_string_pretty(&records).unwrap());
            } else if records.is_empty() {
                output!("no records found for {:?}", f.config.name);
            } else {
                for r in &records {
                    output!(
                        "  {}  commit={} variant={} iters={}",
                        r["id"].as_str().unwrap_or("-"),
                        r["commit"].as_str().unwrap_or("-"),
                        r["variant"].as_str().unwrap_or("-"),
                        r["iterations"].as_u64().unwrap_or(0),
                    );
                }
            }
            Ok(())
        }
        Cmd::Import { path } => {
            let project =
                Project::resolve(&projects_dir, cli.project.as_deref(), None)?;
            let abs = std::fs::canonicalize(&path)?;
            Ok(commands::import(&project, &abs)?)
        }
        Cmd::Serve { port } => web::run(fossil_home, port),
    }
}

fn emit(summary: &analysis::Summary, json: bool, csv: bool) {
    if csv {
        output!("{}", summary.to_csv());
    } else if json {
        output!(
            "{}",
            serde_json::to_string_pretty(&summary.to_json()).unwrap()
        );
    } else {
        output!("{summary}");
    }
}
