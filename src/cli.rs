use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "fossil", about = "Bury and dig up benchmark results")]
pub struct Cli {
    #[arg(long, global = true, help = "Override ~/.fossil home directory")]
    pub home: Option<PathBuf>,
    #[arg(long, global = true, help = "Select project by name")]
    pub project: Option<String>,
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    #[command(about = "Browse records and artifacts in a local web interface")]
    Serve {
        #[arg(long, default_value_t = 8787)]
        port: u16,
    },
    #[command(about = "Initialize the fossil home directory")]
    Init,
    #[command(about = "Manage projects")]
    Project {
        #[command(subcommand)]
        command: ProjectCmd,
    },
    #[command(about = "Create a new fossil in a project")]
    Create {
        name: String,
        #[arg(long)]
        desc: Option<String>,
        #[arg(short = 'n', long)]
        iterations: Option<u32>,
    },
    #[command(about = "Run a benchmark and record observations")]
    Bury {
        fossil: String,
        #[arg(short = 'n', long, help = "Number of iterations per variant")]
        iterations: Option<u32>,
        #[arg(
            long,
            value_delimiter = ',',
            help = "Run specific variants (repeat or comma-separate; omit to run all)"
        )]
        variant: Vec<String>,
        #[arg(long, help = "Print the expanded command without running it")]
        dry_run: bool,
        #[arg(short, long, help = "Suppress subprocess output, show progress")]
        silent: bool,
    },
    #[command(about = "Analyze and compare metrics")]
    Analyze {
        #[arg(help = "Selectors: fossil or fossil:variant")]
        selectors: Vec<String>,
        #[arg(long, help = "Show only the last N records")]
        last: Option<usize>,
        #[arg(short, long, help = "Named analysis script")]
        analysis: Option<String>,
    },
    #[command(about = "Emit an artifact into the project's artifact_dir")]
    Emit {
        fossil: String,
        #[arg(long, help = "Show only the last N records")]
        last: Option<usize>,
        #[arg(long, help = "Filter to a specific variant")]
        variant: Option<String>,
        #[arg(long, help = "Named artifact to emit")]
        artifact: Option<String>,
        #[arg(
            short,
            long,
            help = "Set FOSSIL_FORCE=1 for the artifact script"
        )]
        force: bool,
    },
    #[command(about = "List fossils in a project")]
    List,
    #[command(about = "Import a fossil from a .toml file")]
    Import {
        #[arg(help = "Path to a fossil .toml config file")]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum ProjectCmd {
    #[command(about = "Create a new project")]
    Create {
        name: String,
        #[arg(long)]
        desc: Option<String>,
    },
    #[command(about = "List all projects")]
    List,
}

pub fn resolve_fossil_home(flag: Option<&PathBuf>) -> PathBuf {
    if let Some(p) = flag {
        return p.clone();
    }
    if let Ok(p) = std::env::var("FOSSIL_HOME") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME")
        .expect("HOME is not set — use --home or FOSSIL_HOME");
    PathBuf::from(home).join(".fossil")
}
