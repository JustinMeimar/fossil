use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::fossil::Fossil;
use crate::project::Project;
use crate::runner::Run;

#[derive(Debug, Deserialize, Serialize)]
pub struct GitInfo {
    pub commit: String,
    pub branch: String,
}

impl GitInfo {
    pub fn current() -> Self {
        Self {
            commit: Self::git(&["rev-parse", "--short", "HEAD"]),
            branch: Self::git(&["rev-parse", "--abbrev-ref", "HEAD"]),
        }
    }

    fn git(args: &[&str]) -> String {
        Command::new("git")
            .args(args)
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
            )).unwrap_or_else(|| "unknown".into()),
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
        std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
pub struct BuildInfo {
    pub path: PathBuf,
    pub sha256: String,
}

#[allow(dead_code)]
impl BuildInfo {
    pub fn from_path(binary: Option<PathBuf>) -> Option<Self> {
        let path = binary?;
        let resolved = std::fs::canonicalize(&path).ok()?;
        let sha256 = Self::sha256_file(&resolved).ok()?;
        Some(Self { path: resolved, sha256 })
    }

    fn sha256_file(path: &Path) -> anyhow::Result<String> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
        }
        Ok(format!("{:x}", hasher.finalize()))
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
    pub tag: Option<String>,
    pub git: GitInfo,
    pub cpu: CpuInfo,
    pub kernel: String,
}

impl Manifest {
    pub fn new(fossil: &Fossil, project: &Project, run: &Run) -> Self {
        Self {
            version: 3,
            timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
            fossil: fossil.config.name.clone(),
            project: project.config.name.clone(),
            command: run.command.clone(),
            description: fossil.config.description.clone(),
            iterations: run.iterations,
            tag: run.tag.clone(),
            git: GitInfo::current(),
            cpu: CpuInfo::current(),
            kernel: std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "unknown".into()),
        }
    }

    pub fn load(run_dir: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(run_dir.join("manifest.json"))?;
        Ok(serde_json::from_str(&contents)?)
    }

    pub fn record(&self, records_dir: &Path, results: &Value) -> anyhow::Result<PathBuf> {
        let ts = Local::now().format("%Y%m%d_%H%M%S");
        let mut parts = vec![ts.to_string()];
        if let Some(t) = &self.tag { parts.push(t.clone()); }
        parts.push(self.git.commit.clone());
        let run_dir = records_dir.join(parts.join("_"));
        std::fs::create_dir_all(&run_dir)?;

        std::fs::write(
            run_dir.join("manifest.json"),
            serde_json::to_string_pretty(self)? + "\n",
        )?;
        std::fs::write(
            run_dir.join("results.json"),
            serde_json::to_string_pretty(results)? + "\n",
        )?;
        Ok(run_dir)
    }
}
