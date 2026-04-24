mod analysis;
mod cli;
mod commands;
mod error;
mod fossil;
mod git;
mod manifest;
mod project;
mod runner;
mod ui;
mod web;

use clap::Parser;
use cli::{Cli, Cmd, ProjectCmd};
use fossil::Fossil;
use project::Project;
use ui::{error, output, status};

fn main() {
    if let Err(e) = run() {
        error!("{e}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let fossil_home = cli::resolve_fossil_home(cli.home.as_ref());
    let projects_dir = fossil_home.join("projects");

    match cli.command {
        Cmd::Init => {
            std::fs::create_dir_all(&projects_dir)?;
            status!("initialized {}", projects_dir.display());
            Ok(())
        }
        Cmd::Project { command } => match command {
            ProjectCmd::Create { name, desc } => {
                std::fs::create_dir_all(&projects_dir)?;
                let project =
                    Project::create(&projects_dir, &name, desc.as_deref())?;
                status!("created project {}", project.path.display());
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
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                None,
            )?;
            Ok(commands::create_fossil(
                &project,
                &name,
                desc.as_deref(),
                iterations,
            )?)
        }
        Cmd::Bury {
            fossil: fossil_name,
            iterations,
            variant,
            command,
        } => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(&fossil_name),
            )?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;

            match (variant, command.is_empty()) {
                (Some(name), true) => {
                    let v = f.resolve_variant(&name)?;
                    Ok(commands::bury(
                        &f, &project, iterations, Some(v.name), v.command,
                    )?)
                }
                (Some(_), false) => anyhow::bail!(
                    "cannot specify both --variant and -- <command>"
                ),
                (None, false) => {
                    Ok(commands::bury(
                        &f, &project, iterations, None, command,
                    )?)
                }
                (None, true) => {
                    let variants: Vec<_> = f
                        .config
                        .variants
                        .keys()
                        .cloned()
                        .collect();
                    if variants.is_empty() {
                        anyhow::bail!(
                            "no variants defined — specify --variant or -- <command>"
                        );
                    }
                    for vname in &variants {
                        let v = f.resolve_variant(vname)?;
                        commands::bury(
                            &f,
                            &project,
                            iterations,
                            Some(v.name),
                            v.command,
                        )?;
                    }
                    Ok(())
                }
            }
        }
        Cmd::Analyze {
            fossil: fossil_name,
            variant,
            last,
        } => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(&fossil_name),
            )?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;
            Ok(commands::analyze(&f, variant.as_deref(), last, cli.json)?)
        }
        Cmd::List => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                None,
            )?;
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
            fossil: fossil_name,
            variant,
            last,
        } => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                Some(&fossil_name),
            )?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;
            Ok(commands::dig(&f, variant.as_deref(), last, cli.json)?)
        }
        Cmd::Compare {
            fossil: first,
            baseline: second,
            candidate,
        } => {
            if let Some(cand) = candidate {
                let project = Project::resolve(
                    &projects_dir,
                    cli.project.as_deref(),
                    Some(&first),
                )?;
                let f = Fossil::load(
                    &project.fossils_dir().join(&first),
                )?;
                Ok(commands::compare(
                    &f, &second, &cand, cli.json,
                )?)
            } else {
                let project = Project::resolve(
                    &projects_dir,
                    cli.project.as_deref(),
                    None,
                )?;
                Ok(commands::compare_across(
                    &project, &first, &second, cli.json,
                )?)
            }
        }
        Cmd::Import { path } => {
            let project = Project::resolve(
                &projects_dir,
                cli.project.as_deref(),
                None,
            )?;
            let abs = std::fs::canonicalize(&path)?;
            Ok(commands::import(&project, &abs)?)
        }
        Cmd::Serve { port } => web::run(fossil_home, port),
    }
}
