use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct SiteConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

pub struct Site {
    pub config: SiteConfig,
    pub path: PathBuf,
}

impl Site {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(dir.join("site.toml"))?;
        let config: SiteConfig = toml::from_str(&contents)?;
        Ok(Self { config, path: dir.to_path_buf() })
    }

    pub fn create(sites_dir: &Path, name: &str, description: Option<&str>) -> anyhow::Result<Self> {
        let dir = sites_dir.join(name);
        if dir.exists() {
            anyhow::bail!("site {name:?} already exists");
        }
        std::fs::create_dir_all(&dir)?;
        std::fs::create_dir_all(dir.join("fossils"))?;
        let config = SiteConfig {
            name: name.to_string(),
            description: description.map(String::from),
        };
        let toml = toml::to_string_pretty(&config)?;
        std::fs::write(dir.join("site.toml"), toml)?;
        Ok(Self { config, path: dir })
    }

    pub fn list_all(sites_dir: &Path) -> anyhow::Result<Vec<Self>> {
        let mut sites = Vec::new();
        let entries = match std::fs::read_dir(sites_dir) {
            Ok(e) => e,
            Err(_) => return Ok(sites),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() { continue; }
            if let Ok(site) = Self::load(&entry.path()) {
                sites.push(site);
            }
        }
        sites.sort_by(|a, b| a.config.name.cmp(&b.config.name));
        Ok(sites)
    }

    pub fn fossils_dir(&self) -> PathBuf {
        self.path.join("fossils")
    }
}
