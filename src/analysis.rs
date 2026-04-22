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

    let script_outputs: Vec<Value> = observations.iter()
        .map(|obs| run_script(script, obs))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(aggregate_metrics(&script_outputs))
}

pub fn aggregate_metrics(script_outputs: &[Value]) -> BTreeMap<String, Vec<f64>> {
    let mut metrics: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for result in script_outputs {
        if let Some(obj) = result.as_object() {
            for (k, v) in obj {
                if let Some(n) = v.as_f64() {
                    metrics.entry(k.clone()).or_default().push(n);
                }
            }
        }
    }
    metrics
}

pub fn format_run_summary(
    run_id: &str,
    manifest: &Manifest,
    metrics: &BTreeMap<String, Vec<f64>>,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("--- {run_id} [commit: {}{}] ---",
        manifest.git.commit,
        manifest.tag.as_ref().map(|t| format!(", tag: {t}")).unwrap_or_default(),
    ));
    lines.push(format!("  ({} iterations):", manifest.iterations));
    for (name, values) in metrics {
        lines.push(format!("    {name}: {:.1} ± {:.1}", mean(values), stddev(values)));
    }
    lines
}

pub fn format_comparison(
    baseline_name: &str,
    candidate_name: &str,
    base_metrics: &BTreeMap<String, Vec<f64>>,
    cand_metrics: &BTreeMap<String, Vec<f64>>,
) -> Vec<String> {
    let mut lines = Vec::new();

    let all_keys: BTreeMap<_, _> = base_metrics.keys()
        .chain(cand_metrics.keys())
        .map(|k| (k.clone(), ()))
        .collect();

    let base_w = baseline_name.len().max(10);
    let cand_w = candidate_name.len().max(10);

    lines.push(format!("  {:<20} {:>base_w$}   {:>cand_w$}   {:>8}",
        "metric", baseline_name, candidate_name, "delta"));
    lines.push(format!("  {}", "─".repeat(20 + base_w + cand_w + 14)));

    for key in all_keys.keys() {
        let b = base_metrics.get(key).map(|v| mean(v));
        let c = cand_metrics.get(key).map(|v| mean(v));

        let b_str = b.map(|v| format!("{v:.1}")).unwrap_or_else(|| "-".into());
        let c_str = c.map(|v| format!("{v:.1}")).unwrap_or_else(|| "-".into());
        let delta_str = match (b, c) {
            (Some(bv), Some(cv)) if bv != 0.0 => {
                let pct = (cv - bv) / bv * 100.0;
                let sign = if pct >= 0.0 { "+" } else { "" };
                format!("{sign}{pct:.1}%")
            }
            _ => "-".into(),
        };

        lines.push(format!("  {:<20} {:>base_w$}   {:>cand_w$}   {:>8}",
            key, b_str, c_str, delta_str));
    }
    lines
}

pub fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn stddev(values: &[f64]) -> f64 {
    let m = mean(values);
    let variance = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / values.len() as f64;
    variance.sqrt()
}
