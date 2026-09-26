use super::{Metric, ResolvedAnalysis};
use crate::environment::ExecutionContext;
use crate::error::FossilError;
use crate::record::Record;
use crate::runner::{Observation, Results};
use serde_json::Value;
use std::fmt;
use std::path::PathBuf;

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
    pub fn new(analysis: &ResolvedAnalysis, context: ExecutionContext) -> Self {
        Self {
            path: analysis.script.clone(),
            context,
        }
    }

    fn fail(&self, reason: impl fmt::Display) -> FossilError {
        FossilError::InvalidConfig(format!(
            "analysis script {} failed: {reason}",
            self.path.display()
        ))
    }

    fn parse(
        &self,
        observation: &Observation,
        record: &Record,
    ) -> Result<Value, FossilError> {
        let input =
            serde_json::to_vec(observation).map_err(|e| self.fail(e))?;
        let mut cmd = std::process::Command::new(&self.path);
        self.context.configure(&mut cmd);
        cmd.env("FOSSIL_RUN_DIR", &record.dir)
            .env("FOSSIL_VARIANT_NAME", record.manifest.variant.as_str());
        let output = crate::io::command_output(&mut cmd, Some(&input))
            .map_err(|e| self.fail(e))?;

        if !output.status.success() {
            return Err(
                self.fail(String::from_utf8_lossy(&output.stderr).trim())
            );
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|e| self.fail(format_args!("invalid JSON output: {e}")))
    }

    pub fn collect(&self, record: &Record) -> Result<Metric, FossilError> {
        let raw = std::fs::read_to_string(record.dir.join("results.json"))?;
        let results: Results = serde_json::from_str(&raw).map_err(|e| {
            FossilError::InvalidConfig(format!(
                "corrupt data in {}: {e}",
                record.dir.display()
            ))
        })?;

        let parse = |observation: &Observation| {
            let value = self.parse(observation, record)?;
            Metric::from_json(value).map_err(|error| {
                self.fail(format!(
                    "record {}, iteration {}: {error}",
                    record.dir.display(),
                    observation.iteration
                ))
            })
        };
        let mut observations = results.observations.iter();
        let first = observations.next().ok_or_else(|| {
            self.fail(format!(
                "record {} has no observations",
                record.dir.display()
            ))
        })?;
        let mut metric = parse(first)?;
        for observation in observations {
            metric.merge(parse(observation)?).map_err(|error| {
                self.fail(format!(
                    "record {}, iteration {}: {error}",
                    record.dir.display(),
                    observation.iteration
                ))
            })?;
        }
        Ok(metric)
    }
}
