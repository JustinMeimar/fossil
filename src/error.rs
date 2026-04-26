use thiserror::Error;

#[derive(Debug, Error)]
pub enum FossilError {
    #[error("{0}")]
    NotFound(String),

    #[error("{0} already exists")]
    AlreadyExists(String),

    #[error("{0}")]
    InvalidConfig(String),

    #[error("{0}")]
    InvalidArgs(String),

    #[error("{command:?} failed on iteration {iteration} (exit {exit_code})")]
    CommandFailed {
        command: String,
        iteration: u32,
        exit_code: i32,
    },

    #[error("git {args}: {stderr}")]
    Git { args: String, stderr: String },

    #[error("{0}")]
    Io(#[from] std::io::Error),
}
