use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Local;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::measure::Measure;

#[derive(Debug, Serialize)]
pub struct GitInfo {
    pub commit: String,
    pub branch: String,
    pub dirty: bool,
}

#[derive(Debug, Serialize)]
struct CpuInfo {
    pinned_core: String,
    governor: String,
    boost: bool,
}

#[derive(Debug, Serialize)]
struct BuildInfo {
    path: PathBuf,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct ManifestData {
    version: u32,
    timestamp: String,
    measure: String,
    description: Option<String>,
    iterations: u32,
    tag: Option<String>,
    experiment: Option<String>,
    git: GitInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    build: Option<BuildInfo>,
    cpu: CpuInfo,
    kernel: String,
    configs: Value,
}

fn git(args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub fn git_info() -> GitInfo {
    GitInfo {
        commit: git(&["rev-parse", "--short", "HEAD"]),
        branch: git(&["rev-parse", "--abbrev-ref", "HEAD"]),
        dirty: !git(&["status", "--porcelain"]).is_empty(),
    }
}

fn bench_cpu() -> String {
    std::env::var("BENCH_CPU").unwrap_or_else(|_| "2".into())
}

fn read_sysfs(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn cpu_info() -> CpuInfo {
    let core = bench_cpu();
    CpuInfo {
        governor: read_sysfs(&format!(
            "/sys/devices/system/cpu/cpu{core}/cpufreq/scaling_governor"
        )).unwrap_or_else(|| "unknown".into()),
        boost: read_sysfs("/sys/devices/system/cpu/cpufreq/boost")
            .map(|s| s != "0")
            .unwrap_or(true),
        pinned_core: core,
    }
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

fn build_info(binary: Option<PathBuf>) -> Option<BuildInfo> {
    let path = binary?;
    let resolved = std::fs::canonicalize(&path).ok()?;
    let sha256 = sha256_file(&resolved).ok()?;
    Some(BuildInfo { path: resolved, sha256 })
}

fn kernel_version() -> String {
    Command::new("uname").arg("-r")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

pub fn make_run_dir(
    measures_dir: &Path,
    name: &str,
    commit: &str,
    tag: Option<&str>,
) -> anyhow::Result<PathBuf> {
    let ts = Local::now().format("%Y%m%d_%H%M%S");
    let mut parts = vec![ts.to_string(), name.to_string()];
    if let Some(t) = tag { parts.push(t.to_string()); }
    parts.push(commit.to_string());
    let dir = measures_dir.join(parts.join("_"));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn build_manifest(
    measure: &Measure,
    iterations: u32,
    tag: Option<&str>,
    experiment: Option<&str>,
) -> Value {
    let data = ManifestData {
        version: 2,
        timestamp: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        measure: measure.name.clone(),
        description: measure.description.clone(),
        iterations,
        tag: tag.map(Into::into),
        experiment: experiment.map(Into::into),
        git: git_info(),
        build: build_info(measure.resolve_binary()),
        cpu: cpu_info(),
        kernel: kernel_version(),
        configs: serde_json::to_value(&measure.configs).unwrap_or_default(),
    };
    serde_json::to_value(data).unwrap_or_default()
}

pub fn write_run(run_dir: &Path, manifest: &Value, results: &Value) -> anyhow::Result<()> {
    let write_json = |name: &str, val: &Value| -> anyhow::Result<()> {
        std::fs::write(run_dir.join(name), serde_json::to_string_pretty(val)? + "\n")?;
        Ok(())
    };
    write_json("manifest.json", manifest)?;
    write_json("results.json", results)?;
    Ok(())
}
