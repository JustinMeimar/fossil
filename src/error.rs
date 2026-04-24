use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FossilError {
    #[error("fossil {0:?} not found — run 'fossil list' to see available fossils")]
    FossilNotFound(String),

    #[error("project {0:?} not found — run 'fossil project list' to see available projects")]
    ProjectNotFound(String),

    #[error("fossil {0:?} already exists")]
    FossilExists(String),

    #[error("project {0:?} already exists")]
    ProjectExists(String),

    #[error("{context}: {reason}")]
    InvalidConfig { context: String, reason: String },

    #[error("missing manifest in {}", .0.display())]
    MissingManifest(PathBuf),

    #[error("corrupt manifest in {path}: {reason}")]
    CorruptData { path: String, reason: String },

    #[error("unknown variant {name:?}, available: {available}")]
    UnknownVariant { name: String, available: String },

    #[error("no matching records found")]
    NoRecords,

    #[error("{command:?} failed on iteration {iteration} (exit {exit_code})")]
    CommandFailed {
        command: String,
        iteration: u32,
        exit_code: i32,
    },

    #[error("no command given — usage: fossil bury <name> -- <cmd...>")]
    NoCommand,

    #[error("parser {} failed: {reason}", .path.display())]
    ParserFailed { path: PathBuf, reason: String },

    #[error("no parser configured for {0:?}")]
    NoParser(String),

    #[error("no projects found — create one with: fossil project create <name>")]
    NoProjects,

    #[error("multiple projects exist, specify one with --project: {0}")]
    AmbiguousProject(String),

    #[error("no project contains fossil {0:?}")]
    FossilOrphan(String),

    #[error("git {args}: {stderr}")]
    Git { args: String, stderr: String },

    #[error("{0}")]
    Io(#[from] std::io::Error),
}
