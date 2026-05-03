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
mod record;
mod runner;
mod ui;
mod tui;
mod figure;
mod web;

use clap::Parser;
use cli::{Cli, Cmd, ProjectCmd};
use entity::DirEntity;
use fossil::{Fossil, VariantName};
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
            project.create_fossil(&name, desc.as_deref(), iterations)
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

            let variant = variant.map(VariantName::new);
            match (variant, command.is_empty()) {
                (Some(ref name), true) => {
                    let v = f.resolve_variant(name, &project.config.constants)?;
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
                        None, command.join(" "), false,
                    )?;
                    Ok(())
                }
                (None, true) => {
                    Ok(commands::bury_all(&f, &project, iterations)?)
                }
            }
        }
        Cmd::Analyze {
            selectors,
            last,
            analysis,
            csv: _,
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
            let columns = commands::analyze(
                &project,
                &selectors,
                last,
                analysis.as_deref(),
            )?;
            let result: std::collections::BTreeMap<&str, &analysis::Metric> =
                columns.iter().map(|(n, m)| (n.as_str(), m)).collect();
            output!("{}", serde_json::to_string_pretty(&result).unwrap());
            Ok(())
        }
        Cmd::Figure {
            fossil: fname,
            last,
            variant,
            figure: fig_name,
        } => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(&fname),
            )?;
            let f = Fossil::load(&project.fossils_dir().join(&fname))?;
            let fig = figure::Figure::resolve(&f, fig_name.as_deref())?;

            let spec = match variant {
                Some(ref v) => format!("{fname}:{v}"),
                None => fname.to_string(),
            };
            let columns = commands::analyze(
                &project,
                &[spec],
                last,
                Some(fig.analysis_name()),
            )?;
            fig.run(&f, &columns)
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
        Cmd::Import { path } => {
            let project =
                Project::resolve(&projects_dir, cli.project.as_deref(), None)?;
            let abs = std::fs::canonicalize(&path)?;
            project.import(&abs)
        }
        Cmd::Serve { port } => web::run(fossil_home, port),
    }
}
