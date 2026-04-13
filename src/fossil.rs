use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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
}
