use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde_json::Value;

use crate::manifest::Manifest;

pub fn run_script(script: &Path, observation: &Value) -> anyhow::Result<Value> {
    let mut child = std::process::Command::new(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    serde_json::to_writer(child.stdin.take().unwrap(), observation)?;
    let output = child.wait_with_output()?;

    if !output.status.success() {
        anyhow::bail!("analyze script failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn find_records(
    records_dir: &Path,
    tag: Option<&str>,
    last: Option<usize>,
) -> anyhow::Result<Vec<(PathBuf, Manifest)>> {
    let mut runs: Vec<_> = std::fs::read_dir(records_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let dir = e.path();
            let m = Manifest::load(&dir).ok()?;
            if tag.is_some() && m.tag.as_deref() != tag { return None; }
            Some((dir, m))
        })
        .collect();

    runs.sort_by(|a, b| a.1.timestamp.cmp(&b.1.timestamp));
    if let Some(n) = last {
        let skip = runs.len().saturating_sub(n);
        runs = runs.into_iter().skip(skip).collect();
    }
    Ok(runs)
}

pub fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn stddev(values: &[f64]) -> f64 {
    let m = mean(values);
    let variance = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}
