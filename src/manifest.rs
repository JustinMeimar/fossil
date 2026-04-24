use std::path::{Path, PathBuf};
use std::process::Command;
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::fossil::Fossil;
use crate::project::Project;
use crate::runner::Run;

#[derive(Debug, Deserialize, Serialize)]
pub struct GitInfo {
    pub commit: String,
    pub branch: String,
}

impl GitInfo {
    pub fn current(repo: &Path) -> Self {
        Self {
            commit: Self::git(repo, &["rev-parse", "--short", "HEAD"]),
            branch: Self::git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        }
    }

    fn git(repo: &Path, args: &[&str]) -> String {
        Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CpuInfo {
    pub pinned_core: String,
    pub governor: String,
    pub boost: bool,
}

impl CpuInfo {
    pub fn current() -> Self {
        let core = Self::bench_cpu();
        Self {
            governor: Self::read_sysfs(&format!(
                "/sys/devices/system/cpu/cpu{core}/cpufreq/scaling_governor"
            ))
            .unwrap_or_else(|| "unknown".into()),
            boost: Self::read_sysfs("/sys/devices/system/cpu/cpufreq/boost")
                .map(|s| s != "0")
                .unwrap_or(true),
            pinned_core: core,
        }
    }

    fn bench_cpu() -> String {
        std::env::var("BENCH_CPU").unwrap_or_else(|_| "2".into())
    }

    fn read_sysfs(path: &str) -> Option<String> {
        std::fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
    }
}


pub struct Environment {
    pub git: GitInfo,
    pub cpu: CpuInfo,
    pub kernel: String,
    pub timestamp: String,
}

impl Environment {
    pub fn capture(repo: &Path) -> Self {
        Self {
            git: GitInfo::current(repo),
            cpu: CpuInfo::current(),
            kernel: std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "unknown".into()),
            timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub version: u32,
    pub timestamp: String,
    pub fossil: String,
    pub project: String,
    pub command: String,
    pub description: Option<String>,
    pub iterations: u32,
    pub variant: Option<String>,
    pub git: GitInfo,
    pub cpu: CpuInfo,
    pub kernel: String,
}

impl Manifest {
    pub fn new(
        fossil: &Fossil,
        project: &Project,
        run: &Run,
        env: Environment,
    ) -> Self {
        Self {
            version: 3,
            timestamp: env.timestamp,
            fossil: fossil.config.name.clone(),
            project: project.config.name.clone(),
            command: run.command.clone(),
            description: fossil.config.description.clone(),
            iterations: run.iterations,
            variant: run.variant.clone(),
            git: env.git,
            cpu: env.cpu,
            kernel: env.kernel,
        }
    }

    pub fn load(run_dir: &Path) -> Result<Self, crate::error::FossilError> {
        let contents =
            std::fs::read_to_string(run_dir.join("manifest.json"))
                .map_err(|_| {
                    crate::error::FossilError::MissingManifest(
                        run_dir.to_path_buf(),
                    )
                })?;
        serde_json::from_str(&contents).map_err(|e| {
            crate::error::FossilError::CorruptData {
                path: run_dir.display().to_string(),
                reason: e.to_string(),
            }
        })
    }

    pub fn record(
        &self,
        records_dir: &Path,
        results: &Value,
    ) -> Result<PathBuf, crate::error::FossilError> {
        let ts = Local::now().format("%Y%m%d_%H%M%S_%3f");
        let mut parts = vec![ts.to_string()];
        if let Some(v) = &self.variant {
            parts.push(v.clone());
        }
        parts.push(self.git.commit.clone());
        let run_dir = records_dir.join(parts.join("_"));
        std::fs::create_dir_all(&run_dir)?;

        let manifest_json = serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::FossilError::CorruptData {
                path: run_dir.display().to_string(),
                reason: e.to_string(),
            })?;
        std::fs::write(run_dir.join("manifest.json"), manifest_json + "\n")?;

        let results_json = serde_json::to_string_pretty(results)
            .map_err(|e| crate::error::FossilError::CorruptData {
                path: run_dir.display().to_string(),
                reason: e.to_string(),
            })?;
        std::fs::write(run_dir.join("results.json"), results_json + "\n")?;

        Ok(run_dir)
    }
}
