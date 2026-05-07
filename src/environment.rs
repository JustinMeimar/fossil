use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

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

/// [Fossil Doc] `Environment`
/// -------------------------------------------------------------
/// System state gathered at bury-time. Git info, CPU governor,
/// kernel version, timestamp. Passed into Manifest so the record
/// knows exactly what the machine looked like when it ran.
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
