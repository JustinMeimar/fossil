use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::analysis;
use crate::git;
use crate::manifest::{Environment, Manifest};
use crate::project::Project;
use crate::runner::Run;
use crate::ui::{info, status};

fn default_iterations() -> u32 {
    10
}

pub struct Variant {
    pub name: String,
    pub command: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FossilConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_iterations")]
    pub default_iterations: u32,
    pub analyze: Option<String>,
    #[serde(default)]
    pub variants: BTreeMap<String, Vec<String>>,
}

pub struct Fossil {
    pub config: FossilConfig,
    pub path: PathBuf,
}

impl Fossil {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(dir.join("fossil.toml"))?;
        let config: FossilConfig = toml::from_str(&contents)?;
        Ok(Self {
            config,
            path: dir.to_path_buf(),
        })
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
            variants: BTreeMap::new(),
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
            if !entry.file_type()?.is_dir() {
                continue;
            }
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

    pub fn analyses_dir(&self) -> PathBuf {
        self.path.join("analyses")
    }

    pub fn parser(&self) -> Option<analysis::Parser> {
        self.config
            .analyze
            .as_ref()
            .map(|s| analysis::Parser::new(self.path.join(s)))
    }

    pub fn find_records(
        &self,
        variant: Option<&str>,
        last: Option<usize>,
    ) -> anyhow::Result<Vec<analysis::Record>> {
        let mut records: Vec<_> = std::fs::read_dir(self.records_dir())?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|e| {
                let dir = e.path();
                let manifest = Manifest::load(&dir).ok()?;
                if variant.is_some()
                    && manifest.variant.as_deref() != variant
                {
                    return None;
                }
                Some(analysis::Record { dir, manifest })
            })
            .collect();

        records.sort_by(|a, b| {
            a.manifest.timestamp.cmp(&b.manifest.timestamp)
        });
        if let Some(n) = last {
            let skip = records.len().saturating_sub(n);
            records = records.into_iter().skip(skip).collect();
        }
        Ok(records)
    }

    pub fn resolve_variant(&self, name: &str) -> anyhow::Result<Variant> {
        let command = self.config.variants.get(name).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown variant {name:?}, available: {}",
                self.config
                    .variants
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        Ok(Variant {
            name: name.to_string(),
            command: command.clone(),
        })
    }

    pub fn bury(
        &self,
        project: &Project,
        iterations: Option<u32>,
        variant: Option<String>,
        args: Vec<String>,
    ) -> anyhow::Result<()> {
        let n = iterations.unwrap_or(self.config.default_iterations);
        let mut run = Run::new(args, n, variant)?;

        for _ in 0..n {
            status!(
                "burying {}/{} ({}/{})",
                self.config.name,
                run.variant.as_deref().unwrap_or("untagged"),
                run.observations.len() + 1,
                n,
            );
            let obs = run.execute_one()?;
            status!("{}ms", obs.wall_time_us / 1000);
        }

        let env = Environment::capture();
        let m = Manifest::new(self, project, &run, env);
        let run_dir =
            m.record(&self.records_dir(), &run.observations_json())?;

        let rel = run_dir.strip_prefix(&project.path).unwrap().to_path_buf();
        git::Commit::new(
            &project.path,
            vec![rel.join("manifest.json"), rel.join("results.json")],
            format!(
                "bury {} {}",
                self.config.name,
                run.variant.as_deref().unwrap_or("untagged")
            ),
        )
        .execute()?;

        status!("{n} observations recorded → {}", run_dir.display());
        Ok(())
    }

    pub fn analyze(
        &self,
        variant: Option<&str>,
        last: Option<usize>,
    ) -> anyhow::Result<()> {
        let parser = self.parser().ok_or_else(|| {
            anyhow::anyhow!(
                "no parser configured for {:?}",
                self.config.name
            )
        })?;

        let records = self.find_records(variant, last)?;
        if records.is_empty() {
            anyhow::bail!("no matching records found");
        }

        for r in &records {
            let metrics = parser.collect(&r.dir)?;
            info!(
                "--- {} [commit: {}{}] ---",
                r.id(),
                r.manifest.git.commit,
                r.manifest
                    .variant
                    .as_ref()
                    .map(|v| format!(", variant: {v}"))
                    .unwrap_or_default(),
            );
            info!("  ({} iterations):", r.manifest.iterations);
            info!("{metrics}");
        }
        Ok(())
    }

    pub fn dig(
        &self,
        variant: Option<&str>,
        last: Option<usize>,
    ) -> anyhow::Result<()> {
        let records = self.find_records(variant, last)?;

        if records.is_empty() {
            info!("no records found for {:?}", self.config.name);
            return Ok(());
        }

        for r in &records {
            info!(
                "  {}  commit={} variant={} iters={}",
                r.id(),
                r.manifest.git.commit,
                r.manifest.variant.as_deref().unwrap_or("-"),
                r.manifest.iterations,
            );
        }
        Ok(())
    }

    pub fn compare(
        &self,
        baseline: &str,
        candidate: &str,
    ) -> anyhow::Result<()> {
        let parser = self.parser().ok_or_else(|| {
            anyhow::anyhow!(
                "no parser configured for {:?}",
                self.config.name
            )
        })?;

        let get_latest =
            |variant: &str| -> anyhow::Result<analysis::MetricSet> {
                let records =
                    self.find_records(Some(variant), Some(1))?;
                let r =
                    records.into_iter().next().ok_or_else(|| {
                        anyhow::anyhow!(
                            "no records found for variant {variant:?}"
                        )
                    })?;
                parser.collect(&r.dir)
            };

        let base_metrics = get_latest(baseline)?;
        let cand_metrics = get_latest(candidate)?;

        let cmp = analysis::Comparison {
            baseline: (baseline, &base_metrics),
            candidate: (candidate, &cand_metrics),
        };
        info!("{cmp}");
        Ok(())
    }
}
