use super::quantity::{Metric, fold};
use crate::environment::ExecutionContext;
use crate::error::FossilError;
use crate::runner::{Observation, Results};
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
    context: ExecutionContext,
}

impl AnalysisScript {
    pub fn new(path: PathBuf, context: ExecutionContext) -> Self {
        Self { path, context }
    }

    fn fail(&self, reason: impl fmt::Display) -> FossilError {
        FossilError::InvalidConfig(format!(
            "analysis script {} failed: {reason}",
            self.path.display()
        ))
    }

    pub fn parse(
        &self,
        observation: &Observation,
        run_dir: Option<&Path>,
    ) -> Result<Value, FossilError> {
        let mut cmd = std::process::Command::new(&self.path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.context.configure(&mut cmd);
        // Expose the record dir + variant name so analyzers can
        // self-identify without any stdin schema change.
        if let Some(dir) = run_dir {
            cmd.env("FOSSIL_RUN_DIR", dir);
            let manifest_path = dir.join("manifest.json");
            if let Ok(text) = std::fs::read_to_string(&manifest_path) {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    if let Some(name) =
                        v.get("variant").and_then(|s| s.as_str())
                    {
                        cmd.env("FOSSIL_VARIANT_NAME", name);
                    }
                }
            }
        }
        let mut child = cmd.spawn().map_err(|e| {
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
        let results: Results = serde_json::from_str(&raw).map_err(|e| {
            FossilError::InvalidConfig(format!(
                "corrupt data in {}: {e}",
                run_dir.display()
            ))
        })?;

        let parsed: Vec<Value> = results
            .observations
            .iter()
            .map(|obs| self.parse(obs, Some(run_dir)))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(fold(parsed.into_iter().map(|v| Metric::from_json(&v))))
    }
}
