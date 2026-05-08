use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::FossilError;

/// [Fossil Doc] `Repo`
/// -------------------------------------------------------------
/// A git repository. Each Project is backed by one of these.
/// Handles init-on-first-use, staging, and committing.
pub struct Repo(PathBuf);

impl Repo {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn commit(
        &self,
        paths: Vec<PathBuf>,
        message: impl AsRef<str>,
    ) -> Result<(), FossilError> {
        self.ensure_init()?;
        let strs: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let mut args: Vec<&str> = vec!["add"];
        args.extend(strs.iter().map(|s| s.as_str()));
        self.git(&args)?;
        self.git(&["commit", "-m", message.as_ref()])?;
        Ok(())
    }

    pub fn rm(
        &self,
        path: &Path,
        message: impl AsRef<str>,
    ) -> Result<(), FossilError> {
        self.ensure_init()?;
        self.git(&["rm", "-r", &path.to_string_lossy()])?;
        self.git(&["commit", "-m", message.as_ref()])?;
        Ok(())
    }

    fn ensure_init(&self) -> Result<(), FossilError> {
        if !self.0.join(".git").exists() {
            self.git(&["init"])?;
        }
        Ok(())
    }

    fn git(&self, args: &[&str]) -> Result<String, FossilError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.0)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FossilError::Git {
                args: args.join(" "),
                stderr: stderr.trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string())
    }
}
