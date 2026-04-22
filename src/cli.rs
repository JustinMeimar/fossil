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

pub fn resolve_project(fossil_home: &PathBuf, name: Option<&str>) -> anyhow::Result<Project> {
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
