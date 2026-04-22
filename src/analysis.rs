use std::collections::BTreeMap;
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

pub fn collect_metrics(
    script: &Path,
    run_dir: &Path,
) -> anyhow::Result<BTreeMap<String, Vec<f64>>> {
    let results: Value = serde_json::from_str(
        &std::fs::read_to_string(run_dir.join("results.json"))?,
    )?;
    let observations = results["observations"].as_array()
        .ok_or_else(|| anyhow::anyhow!("invalid results in {}", run_dir.display()))?;

    let mut metrics: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for obs in observations {
        let result = run_script(script, obs)?;
        if let Some(obj) = result.as_object() {
            for (k, v) in obj {
                if let Some(n) = v.as_f64() {
                    metrics.entry(k.clone()).or_default().push(n);
                }
            }
        }
    }
    Ok(metrics)
}

pub fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn stddev(values: &[f64]) -> f64 {
    let m = mean(values);
    let variance = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}
