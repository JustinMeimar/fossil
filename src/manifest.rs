use crate::environment::{CpuInfo, GitInfo};
use crate::error::FossilError;
use crate::fossil::{Fossil, FossilVariantKey};
use crate::project::Project;
use crate::runner::{Results, Run};

use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// [Fossil Doc] `Manifest`
/// -------------------------------------------------------------
/// Metadata snapshot captured at bury-time. Records what was run,
/// which variant, the git state, CPU config, and kernel version.
/// Stored as manifest.json alongside the results.
#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub version: u32,
    pub timestamp: String,
    pub fossil: String,
    pub project: String,
    pub command: String,
    pub description: Option<String>,
    pub iterations: u32,
    pub variant: Option<FossilVariantKey>,
    pub git: GitInfo,
    pub cpu: CpuInfo,
    pub kernel: String,
}

impl Manifest {
    pub fn new(
        fossil: &Fossil,
        project: &Project,
        run: &Run,
        git: GitInfo,
        cpu: CpuInfo,
    ) -> Self {
        Self {
            version: 3,
            timestamp: Local::now()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            fossil: fossil.config.name.clone(),
            project: project.config.name.clone(),
            command: run.command.clone(),
            description: fossil.config.description.clone(),
            iterations: run.iterations,
            variant: run.variant.clone(),
            git,
            cpu,
            kernel: std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "unknown".into()),
        }
    }

    pub fn load(run_dir: &Path) -> Result<Self, FossilError> {
        FossilError::load_json(
            &run_dir.join("manifest.json"),
            &format!("missing manifest in {}", run_dir.display()),
        )
    }

    pub fn record(
        &self,
        records_dir: &Path,
        results: &Results,
    ) -> Result<PathBuf, FossilError> {
        let ts = Local::now().format("%Y%m%d_%H%M%S_%3f");
        let mut parts = vec![ts.to_string()];
        if let Some(v) = &self.variant {
            parts.push(v.to_string());
        }
        parts.push(self.git.commit.clone());
        let run_dir = records_dir.join(parts.join("_"));
        std::fs::create_dir_all(&run_dir)?;

        let manifest_json =
            serde_json::to_string_pretty(self).map_err(|e| {
                FossilError::InvalidConfig(format!(
                    "serializing manifest in {}: {e}",
                    run_dir.display()
                ))
            })?;
        std::fs::write(run_dir.join("manifest.json"), manifest_json + "\n")?;

        let results_json =
            serde_json::to_string_pretty(results).map_err(|e| {
                FossilError::InvalidConfig(format!(
                    "serializing results in {}: {e}",
                    run_dir.display()
                ))
            })?;
        std::fs::write(run_dir.join("results.json"), results_json + "\n")?;

        Ok(run_dir)
    }
}
