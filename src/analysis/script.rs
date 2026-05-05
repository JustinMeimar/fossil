use super::quantity::{Metric, fold};
use crate::error::FossilError;
use crate::runner::{Observation, ResultsFile};
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// [Fossil Doc] `AnalysisScript`
/// -------------------------------------------------------------
/// A script that turns raw observations into structured metrics.
/// Feeds each observation as JSON to the script's stdin, parses
/// the JSON output, and folds across iterations.
pub struct AnalysisScript {
    path: PathBuf,
}

impl AnalysisScript {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn fail(&self, reason: impl fmt::Display) -> FossilError {
        FossilError::InvalidConfig(format!(
            "analysis script {} failed: {reason}", self.path.display()
        ))
    }

    pub fn parse(
        &self,
        observation: &Observation,
    ) -> Result<Value, FossilError> {
        let mut child = std::process::Command::new(&self.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                self.fail(format_args!(
                    "{e} — is the script executable? (chmod +x {})",
                    self.path.display()
                ))
            })?;

        if let Some(stdin) = child.stdin.take() {
            serde_json::to_writer(stdin, observation)
                .map_err(|e| self.fail(e))?;
        }
        let output = child.wait_with_output().map_err(|e| self.fail(e))?;

        if !output.status.success() {
            return Err(
                self.fail(String::from_utf8_lossy(&output.stderr).trim())
            );
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|e| self.fail(format_args!("invalid JSON output: {e}")))
    }

    pub fn collect(&self, run_dir: &Path) -> Result<Metric, FossilError> {
        let raw = std::fs::read_to_string(run_dir.join("results.json"))?;
        let results: ResultsFile = serde_json::from_str(&raw).map_err(|e| {
            FossilError::InvalidConfig(format!(
                "corrupt data in {}: {e}", run_dir.display()
            ))
        })?;

        let parsed: Vec<Value> = results
            .observations
            .iter()
            .map(|obs| self.parse(obs))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(fold(
            parsed.into_iter().map(|v| Metric::from_json(&v)),
        ))
    }
}
