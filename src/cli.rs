use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "fossil",
    about = "Bury and dig up benchmark results",
    long_about = "fossil tracks benchmark runs with full provenance: what was run, \
                  when, on which commit, under what CPU configuration. Results are \
                  stored in git-backed projects for reproducibility and comparison."
)]
pub struct Cli {
    #[arg(long, global = true, help = "Override ~/.fossil home directory")]
    pub home: Option<PathBuf>,
    #[arg(long, global = true, help = "Select project by name")]
    pub project: Option<String>,
    #[arg(long, global = true, help = "Output results as JSON")]
    pub json: bool,
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
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
        #[arg(long, help = "Run a specific variant (omit to run all)")]
        variant: Option<String>,
        #[arg(last = true)]
        command: Vec<String>,
    },
    #[command(about = "Show analyzed metrics for recorded runs")]
    Analyze {
        #[arg(help = "Fossil name (omit to list available fossils)")]
        fossil: Option<String>,
        #[arg(long, help = "Filter to a single variant")]
        variant: Option<String>,
        #[arg(long, help = "Show only the last N records")]
        last: Option<usize>,
        #[arg(short, long, help = "Named analysis script (for multi-analysis fossils)")]
        analysis: Option<String>,
    },
    #[command(about = "List fossils in a project")]
    List,
    #[command(about = "List recorded runs for a fossil")]
    Dig {
        fossil: String,
        #[arg(long, help = "Filter to a single variant")]
        variant: Option<String>,
        #[arg(long, help = "Show only the last N records")]
        last: Option<usize>,
    },
    #[command(
        about = "Compare metrics between two variants",
        long_about = "Compare metrics between two variants of the same fossil, or across \
                      fossils using fossil:variant syntax (e.g. fossil compare compile:O3 execute:O3)."
    )]
    Compare {
        #[arg(
            help = "Fossil name, or first fossil:variant pair for cross-fossil comparison"
        )]
        fossil: String,
        #[arg(help = "Baseline variant (or second fossil:variant pair)")]
        baseline: String,
        #[arg(help = "Candidate variant (omit for cross-fossil comparison)")]
        candidate: Option<String>,
        #[arg(short, long, help = "Named analysis script (for multi-analysis fossils)")]
        analysis: Option<String>,
    },
    #[command(about = "Import a fossil from a .toml file")]
    Import {
        #[arg(help = "Path to a fossil .toml config file")]
        path: PathBuf,
    },
    #[command(about = "Start the web UI")]
    Serve {
        #[arg(short, long, default_value = "3000")]
        port: u16,
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
