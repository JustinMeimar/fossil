use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::project::Project;

#[derive(Parser)]
#[command(name = "fossil", about = "Bury and dig up benchmark results")]
pub struct Cli {
    #[arg(long, global = true)]
    pub home: Option<PathBuf>,
    #[arg(long, global = true)]
    pub project: Option<String>,
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
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
        variant: Option<String>,
        #[arg(last = true)]
        command: Vec<String>,
    },
    Analyze {
        fossil: String,
        #[arg(long)]
        variant: Option<String>,
        #[arg(long)]
        last: Option<usize>,
    },
    List,
    Dig {
        fossil: String,
        #[arg(long)]
        variant: Option<String>,
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
pub enum ProjectCmd {
    Create {
        name: String,
        #[arg(long)]
        desc: Option<String>,
    },
    List,
}

pub fn resolve_fossil_home(flag: Option<&PathBuf>) -> PathBuf {
    if let Some(p) = flag {
        return p.clone();
    }
    if let Ok(p) = std::env::var("FOSSIL_HOME") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".fossil")
}

pub fn projects_dir(fossil_home: &PathBuf) -> PathBuf {
    fossil_home.join("projects")
}

pub fn resolve_project(
    fossil_home: &PathBuf,
    name: Option<&str>,
    fossil_hint: Option<&str>,
) -> anyhow::Result<Project> {
    let pd = projects_dir(fossil_home);
    if let Some(n) = name {
        return Project::load(&pd.join(n));
    }
    let projects = Project::list_all(&pd)?;
    match projects.len() {
        0 => anyhow::bail!(
            "no projects found — create one with: fossil project create <name>"
        ),
        1 => Ok(projects.into_iter().next().unwrap()),
        _ => {
            if let Some(fossil_name) = fossil_hint {
                let matches: Vec<_> = projects
                    .into_iter()
                    .filter(|p| p.fossils_dir().join(fossil_name).exists())
                    .collect();
                match matches.len() {
                    1 => return Ok(matches.into_iter().next().unwrap()),
                    0 => anyhow::bail!(
                        "no project contains fossil {fossil_name:?}"
                    ),
                    _ => {}
                }
            }
            let pd = projects_dir(fossil_home);
            let names: Vec<_> = Project::list_all(&pd)?
                .iter()
                .map(|p| p.config.name.clone())
                .collect();
            anyhow::bail!(
                "multiple projects exist, specify one with --project: {}",
                names.join(", ")
            );
        }
    }
}
