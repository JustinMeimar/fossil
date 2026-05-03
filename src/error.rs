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

impl FossilError {
    pub fn unknown(noun: &str, name: &str, available: &[&str]) -> Self {
        Self::InvalidArgs(format!(
            "unknown {noun} {name:?}, available: {}", available.join(", ")
        ))
    }

    pub fn load_toml<T: serde::de::DeserializeOwned>(
        path: &std::path::Path,
        not_found_msg: &str,
    ) -> Result<T, Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|_| Self::NotFound(not_found_msg.to_string()))?;
        toml::from_str(&contents).map_err(|e| {
            Self::InvalidConfig(format!("{}: {e}", path.display()))
        })
    }

    pub fn load_json<T: serde::de::DeserializeOwned>(
        path: &std::path::Path,
        not_found_msg: &str,
    ) -> Result<T, Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|_| Self::NotFound(not_found_msg.to_string()))?;
        serde_json::from_str(&contents).map_err(|e| {
            Self::InvalidConfig(format!("{}: {e}", path.display()))
        })
    }
}
