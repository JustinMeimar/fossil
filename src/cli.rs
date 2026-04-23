use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
    Serve {
        #[arg(short, long, default_value = "3000")]
        port: u16,
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
