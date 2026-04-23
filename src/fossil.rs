use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::analysis;
use crate::manifest::Manifest;

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
}
