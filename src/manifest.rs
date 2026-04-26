use crate::environment::{CpuInfo, Environment, GitInfo};
use crate::error::FossilError;
use crate::fossil::Fossil;
use crate::project::Project;
use crate::runner::{ResultsFile, Run};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

    pub fn load(run_dir: &Path) -> Result<Self, FossilError> {
        let contents = std::fs::read_to_string(run_dir.join("manifest.json"))
            .map_err(|_| {
            FossilError::NotFound(format!("missing manifest in {}", run_dir.display()))
        })?;
        serde_json::from_str(&contents).map_err(|e| {
            FossilError::InvalidConfig(format!(
                "corrupt manifest in {}: {e}", run_dir.display()
            ))
        })
    }

    pub fn record(
        &self,
        records_dir: &Path,
        results: &ResultsFile,
    ) -> Result<PathBuf, FossilError> {
        let ts = Local::now().format("%Y%m%d_%H%M%S_%3f");
        let mut parts = vec![ts.to_string()];
        if let Some(v) = &self.variant {
            parts.push(v.clone());
        }
        parts.push(self.git.commit.clone());
        let run_dir = records_dir.join(parts.join("_"));
        std::fs::create_dir_all(&run_dir)?;

        let manifest_json =
            serde_json::to_string_pretty(self).map_err(|e| {
                FossilError::InvalidConfig(format!(
                    "serializing manifest in {}: {e}", run_dir.display()
                ))
            })?;
        std::fs::write(run_dir.join("manifest.json"), manifest_json + "\n")?;

        let results_json =
            serde_json::to_string_pretty(results).map_err(|e| {
                FossilError::InvalidConfig(format!(
                    "serializing results in {}: {e}", run_dir.display()
                ))
            })?;
        std::fs::write(run_dir.join("results.json"), results_json + "\n")?;

        Ok(run_dir)
    }
}
