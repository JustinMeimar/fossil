use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::FossilError;

pub struct Commit {
    pub repo: PathBuf,
    pub paths: Vec<PathBuf>,
    pub message: String,
}

impl Commit {
    pub fn new(repo: &Path, paths: Vec<PathBuf>, message: String) -> Self {
        Self {
            repo: repo.to_path_buf(),
            paths,
            message,
        }
    }

    pub fn execute(&self) -> Result<(), FossilError> {
        ensure_repo(&self.repo)?;

        let path_strs: Vec<String> = self
            .paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let path_args: Vec<&str> =
            path_strs.iter().map(|s| s.as_str()).collect();
        git(&self.repo, &[&["add"], path_args.as_slice()].concat())?;
        git(&self.repo, &["commit", "-m", &self.message])?;
        Ok(())
    }
}

pub fn init(dir: &Path) -> Result<(), FossilError> {
    git(dir, &["init"])?;
    Ok(())
}

pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

fn ensure_repo(dir: &Path) -> Result<(), FossilError> {
    if !is_repo(dir) {
        init(dir)?;
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<String, FossilError> {
    let output = Command::new("git").args(args).current_dir(dir).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FossilError::Git {
            args: args.join(" "),
            stderr: stderr.trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
