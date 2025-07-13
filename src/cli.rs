use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fossil")]
#[command(about = "A file tracking and versioning tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new fossil repository
    Init,

    /// Track files for versioning
    Track {
        /// Files or patterns to track
        files: Vec<String>,
    },

    /// Remove a tracked file from the config
    Untrack { files: Vec<String> },

    /// Bury tracked files in a new layer
    Bury {
        /// Optional tag for this layer
        #[arg(short, long)]
        tag: Option<String>,

        /// Specific files to bury (if none specified, buries all tracked files)
        files: Vec<String>,
    },

    /// Dig to a specific layer, or dig specific files, or dig by tag
    Dig {
        /// Dig files with specific tag
        #[arg(short, long)]
        tag: Option<String>,
        
        #[arg(short, long)]
        version: Option<usize>,

        /// Dig specific files by path
        files: Vec<String>,
    },

    /// Return to surface layer, replacing symbolic links with original files.
    Surface,

    /// List tracked files and layers
    List,

    /// Remove .fossil
    Reset,
}
