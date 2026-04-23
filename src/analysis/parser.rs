use std::path::{Path, PathBuf};
use std::process::Stdio;
use serde_json::Value;
use super::quantity::{MetricSet, fold};
use crate::runner::Observation;

/// [Fossil Doc]
/// A parser is a script associated with a Fossil that describes how to transform
/// the coarse stdout/stderr of an `Observation` into a map of `Quantity`, which
/// can be manipulated algebraically.
pub struct Parser {
    path: PathBuf,
}

impl Parser {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn parse(&self, observation: &Observation) -> anyhow::Result<Value> {
        let mut child = std::process::Command::new(&self.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        serde_json::to_writer(child.stdin.take().unwrap(), observation)?;
        let output = child.wait_with_output()?;

        if !output.status.success() {
            anyhow::bail!(
                "parser {:?} failed: {}",
                self.path,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }

    pub fn collect(&self, run_dir: &Path) -> anyhow::Result<MetricSet> {
        let results: Value = serde_json::from_str(
            &std::fs::read_to_string(run_dir.join("results.json"))?,
        )?;
        let observations: Vec<Observation> =
            serde_json::from_value(results["observations"].clone())?;

        let parsed: Vec<Value> = observations
            .iter()
            .map(|obs| self.parse(obs))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(fold(parsed.into_iter().map(|v| MetricSet::from_json(&v))))
    }
}
