use crate::fossil::{Fossil, FossilVariantKey};
use crate::project::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

pub enum Operation<'a> {
    Variant(&'a FossilVariantKey),
    Analysis(&'a str),
    Artifact(&'a str),
}

/// The project, fossil, and operation metadata exposed to a subprocess.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    variables: BTreeMap<String, String>,
}

impl ExecutionContext {
    pub fn new(
        project: &Project,
        fossil: &Fossil,
        operation: Operation<'_>,
    ) -> Self {
        let mut variables = BTreeMap::from([
            (
                "FOSSIL_PROJECT_DIR".into(),
                project.path.to_string_lossy().into_owned(),
            ),
            ("FOSSIL_NAME".into(), fossil.config.name.clone()),
        ]);
        variables.extend(project.config.constants.iter().map(
            |(name, value)| (format!("FOSSIL_CONST_{name}"), value.clone()),
        ));
        let (name, value) = match operation {
            Operation::Variant(variant) => {
                ("FOSSIL_VARIANT_NAME", variant.as_str())
            }
            Operation::Analysis(analysis) => ("FOSSIL_ANALYSIS_NAME", analysis),
            Operation::Artifact(name) => ("FOSSIL_ARTIFACT_NAME", name),
        };
        variables.insert(name.into(), value.into());
        Self { variables }
    }

    pub fn configure(&self, command: &mut Command) {
        command.envs(&self.variables);
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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
