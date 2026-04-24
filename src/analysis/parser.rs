use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use serde_json::Value;
use super::quantity::{MetricSet, fold};
use crate::error::FossilError;
use crate::runner::Observation;

pub struct Parser {
    path: PathBuf,
}

impl Parser {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn fail(&self, reason: impl fmt::Display) -> FossilError {
        FossilError::ParserFailed {
            path: self.path.clone(),
            reason: reason.to_string(),
        }
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

        serde_json::to_writer(child.stdin.take().unwrap(), observation)
            .map_err(|e| self.fail(e))?;
        let output = child.wait_with_output().map_err(|e| self.fail(e))?;

        if !output.status.success() {
            return Err(self.fail(
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|e| self.fail(format_args!("invalid JSON output: {e}")))
    }

    pub fn collect(
        &self,
        run_dir: &Path,
    ) -> Result<MetricSet, FossilError> {
        let raw = std::fs::read_to_string(run_dir.join("results.json"))?;
        let results: Value =
            serde_json::from_str(&raw).map_err(|e| {
                FossilError::CorruptData {
                    path: run_dir.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
        let observations: Vec<Observation> =
            serde_json::from_value(results["observations"].clone())
                .map_err(|e| FossilError::CorruptData {
                    path: run_dir.display().to_string(),
                    reason: e.to_string(),
                })?;

        let parsed: Vec<Value> = observations
            .iter()
            .map(|obs| self.parse(obs))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(fold(parsed.into_iter().map(|v| MetricSet::from_json(&v))))
    }
}
