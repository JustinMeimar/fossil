use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::analysis;
use crate::manifest::{GitInfo, Manifest};
use crate::runner;
use crate::ui::{status, info};

fn default_iterations() -> u32 { 10 }

#[derive(Debug, Deserialize, Serialize)]
pub struct FossilConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_iterations")]
    pub default_iterations: u32,
    pub analyze: Option<String>,
}

pub struct Fossil {
    pub config: FossilConfig,
    pub path: PathBuf,
}

impl Fossil {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(dir.join("fossil.toml"))?;
        let config: FossilConfig = toml::from_str(&contents)?;
        Ok(Self { config, path: dir.to_path_buf() })
    }

    pub fn create(
        fossils_dir: &Path,
        name: &str,
        description: Option<&str>,
        iterations: Option<u32>,
    ) -> anyhow::Result<Self> {
        let dir = fossils_dir.join(name);
        if dir.exists() {
            anyhow::bail!("fossil {name:?} already exists");
        }
        std::fs::create_dir_all(&dir)?;
        std::fs::create_dir_all(dir.join("records"))?;
        let config = FossilConfig {
            name: name.to_string(),
            description: description.map(String::from),
            default_iterations: iterations.unwrap_or(10),
            analyze: None,
        };
        let toml = toml::to_string_pretty(&config)?;
        std::fs::write(dir.join("fossil.toml"), toml)?;
        Ok(Self { config, path: dir })
    }

    pub fn list_all(fossils_dir: &Path) -> anyhow::Result<Vec<Self>> {
        let mut fossils = Vec::new();
        let entries = match std::fs::read_dir(fossils_dir) {
            Ok(e) => e,
            Err(_) => return Ok(fossils),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() { continue; }
            if let Ok(fossil) = Self::load(&entry.path()) {
                fossils.push(fossil);
            }
        }
        fossils.sort_by(|a, b| a.config.name.cmp(&b.config.name));
        Ok(fossils)
    }

    pub fn records_dir(&self) -> PathBuf {
        self.path.join("records")
    }

    pub fn resolve_analyze(&self) -> Option<PathBuf> {
        self.config.analyze.as_ref().map(|s| self.path.join(s))
    }

    pub fn bury(
        &self,
        project_name: &str,
        iterations: Option<u32>,
        tag: Option<String>,
        command: Vec<String>,
    ) -> anyhow::Result<()> {
        if command.is_empty() {
            anyhow::bail!("no command given — usage: fossil bury <name> -- <cmd...>");
        }

        let n = iterations.unwrap_or(self.config.default_iterations);
        let cmd_str = command.join(" ");
        let git = GitInfo::current();

        let mut observations: Vec<Value> = Vec::new();
        for i in 1..=n {
            status!("burying {}/{} ({i}/{n})",
                self.config.name,
                tag.as_deref().unwrap_or("untagged"),
            );
            let obs = runner::Observation::run(&cmd_str, i)?;
            if obs.exit_code != 0 {
                anyhow::bail!("command failed on iteration {i} (exit {})", obs.exit_code);
            }
            status!("{}ms", obs.wall_time_us / 1000);
            observations.push(serde_json::to_value(&obs)?);
        }

        let results = json!({
            "fossil": self.config.name,
            "observations": observations,
        });

        let m = Manifest::new(
            self.config.name.clone(),
            project_name.to_string(),
            cmd_str,
            self.config.description.clone(),
            n,
            tag,
            git,
        );
        let run_dir = m.record(&self.records_dir(), &results)?;

        status!("{n} observations recorded → {}", run_dir.display());
        Ok(())
    }

    pub fn analyze(&self, tag: Option<&str>, last: Option<usize>) -> anyhow::Result<()> {
        let script = self.resolve_analyze()
            .ok_or_else(|| anyhow::anyhow!("no analyze script configured for {:?}", self.config.name))?;

        let runs = analysis::find_records(&self.records_dir(), tag, last)?;
        if runs.is_empty() {
            anyhow::bail!("no matching records found");
        }

        for (run_dir, run_manifest) in &runs {
            let run_id = run_dir.file_name().unwrap().to_string_lossy();
            info!("--- {run_id} [commit: {}{}] ---",
                run_manifest.git.commit,
                run_manifest.tag.as_ref().map(|t| format!(", tag: {t}")).unwrap_or_default(),
            );

            let metrics = analysis::collect_metrics(&script, run_dir)?;
            info!("  ({} iterations):", run_manifest.iterations);
            for (name, values) in &metrics {
                info!("    {name}: {:.1} ± {:.1}", analysis::mean(values), analysis::stddev(values));
            }
        }
        Ok(())
    }

    pub fn dig(&self, tag: Option<&str>, last: Option<usize>) -> anyhow::Result<()> {
        let runs = analysis::find_records(&self.records_dir(), tag, last)?;

        if runs.is_empty() {
            info!("no records found for {:?}", self.config.name);
            return Ok(());
        }

        for (run_dir, m) in &runs {
            let run_id = run_dir.file_name().unwrap().to_string_lossy();
            info!("  {run_id}  commit={} tag={} iters={}",
                m.git.commit,
                m.tag.as_deref().unwrap_or("-"),
                m.iterations,
            );
        }
        Ok(())
    }

    pub fn compare(&self, baseline: &str, candidate: &str) -> anyhow::Result<()> {
        let script = self.resolve_analyze()
            .ok_or_else(|| anyhow::anyhow!("no analyze script configured for {:?}", self.config.name))?;

        let get_latest = |tag: &str| -> anyhow::Result<_> {
            let runs = analysis::find_records(&self.records_dir(), Some(tag), Some(1))?;
            let (run_dir, _) = runs.into_iter().next()
                .ok_or_else(|| anyhow::anyhow!("no records found for tag {tag:?}"))?;
            analysis::collect_metrics(&script, &run_dir)
        };

        let base_metrics = get_latest(baseline)?;
        let cand_metrics = get_latest(candidate)?;

        let all_keys: BTreeMap<_, _> = base_metrics.keys()
            .chain(cand_metrics.keys())
            .map(|k| (k.clone(), ()))
            .collect();

        let base_w = baseline.len().max(10);
        let cand_w = candidate.len().max(10);

        info!("  {:<20} {:>base_w$}   {:>cand_w$}   {:>8}",
            "metric", baseline, candidate, "delta");
        info!("  {}", "─".repeat(20 + base_w + cand_w + 14));

        for key in all_keys.keys() {
            let b = base_metrics.get(key).map(|v| analysis::mean(v));
            let c = cand_metrics.get(key).map(|v| analysis::mean(v));

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

            info!("  {:<20} {:>base_w$}   {:>cand_w$}   {:>8}",
                key, b_str, c_str, delta_str);
        }
        Ok(())
    }
}
