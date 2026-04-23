use std::path::{Path, PathBuf};
use std::process::Command;

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

    pub fn execute(&self) -> anyhow::Result<()> {
        ensure_repo(&self.repo)?;

        let path_args: Vec<&str> =
            self.paths.iter().map(|p| p.to_str().unwrap()).collect();
        git(&self.repo, &[&["add"], path_args.as_slice()].concat())?;
        git(&self.repo, &["commit", "-m", &self.message])?;
        Ok(())
    }
}

pub fn init(dir: &Path) -> anyhow::Result<()> {
    git(dir, &["init"])?;
    Ok(())
}

pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

fn ensure_repo(dir: &Path) -> anyhow::Result<()> {
    if !is_repo(dir) {
        init(dir)?;
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git").args(args).current_dir(dir).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {}: {}", args.join(" "), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
