mod analysis;
mod cli;
mod fossil;
mod git;
mod manifest;
mod runner;
mod project;
mod ui;

use clap::Parser;
use cli::{Cli, Cmd, ProjectCmd};
use fossil::Fossil;
use project::Project;
use ui::{status, info, error};

fn main() {
    if let Err(e) = run() {
        error!("{e}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let fossil_home = cli::resolve_fossil_home(cli.home.as_ref());

    match cli.command {
        Cmd::Init => {
            let pd = cli::projects_dir(&fossil_home);
            std::fs::create_dir_all(&pd)?;
            status!("initialized {}", pd.display());
            Ok(())
        }
        Cmd::Project { command } => match command {
            ProjectCmd::Create { name, desc } => {
                let pd = cli::projects_dir(&fossil_home);
                std::fs::create_dir_all(&pd)?;
                let project = Project::create(&pd, &name, desc.as_deref())?;
                status!("created project {}", project.path.display());
                Ok(())
            }
            ProjectCmd::List => {
                let pd = cli::projects_dir(&fossil_home);
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
            let project = cli::resolve_project(&fossil_home, cli.project.as_deref())?;
            let f = Fossil::create(&project.fossils_dir(), &name, desc.as_deref(), iterations)?;
            let rel = f.path.strip_prefix(&project.path).unwrap().to_path_buf();
            git::Commit::new(
                &project.path,
                vec![rel.join("fossil.toml")],
                format!("create fossil {name}"),
            ).execute()?;
            status!("created fossil {}", f.path.display());
            Ok(())
        }
        Cmd::Bury { fossil: fossil_name, iterations, variant, command } => {
            let project = cli::resolve_project(&fossil_home, cli.project.as_deref())?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;

            let (args, variant_name) = match (variant, command.is_empty()) {
                (Some(name), true) => {
                    let v = f.resolve_variant(&name)?;
                    (v.command, Some(v.name))
                }
                (Some(_), false) => anyhow::bail!("cannot specify both --variant and -- <command>"),
                (None, false) => (command, None),
                (None, true) => anyhow::bail!("specify --variant or -- <command>"),
            };

            f.bury(&project, iterations, variant_name, args)
        }
        Cmd::Analyze { fossil: fossil_name, variant, last } => {
            let project = cli::resolve_project(&fossil_home, cli.project.as_deref())?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;
            f.analyze(variant.as_deref(), last)
        }
        Cmd::List => {
            let project = cli::resolve_project(&fossil_home, cli.project.as_deref())?;
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
        Cmd::Dig { fossil: fossil_name, variant, last } => {
            let project = cli::resolve_project(&fossil_home, cli.project.as_deref())?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;
            f.dig(variant.as_deref(), last)
        }
        Cmd::Compare { fossil: fossil_name, baseline, candidate } => {
            let project = cli::resolve_project(&fossil_home, cli.project.as_deref())?;
            let f = Fossil::load(&project.fossils_dir().join(&fossil_name))?;
            f.compare(&baseline, &candidate)
        }
    }
}
