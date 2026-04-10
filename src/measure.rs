use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Measure {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_iterations")]
    pub default_iterations: u32,
    pub cwd: Option<String>,
    pub binary: Option<String>,
    pub configs: BTreeMap<String, Config>,
    #[serde(skip)]
    pub root: PathBuf,
}

fn default_iterations() -> u32 { 10 }

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub command: String,
}

impl Measure {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let mut m: Self = toml::from_str(&contents)?;
        m.root = path.canonicalize()?
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        Ok(m)
    }

    pub fn resolve_cwd(&self) -> PathBuf {
        match &self.cwd {
            Some(cwd) => self.root.join(cwd),
            None => self.root.clone(),
        }
    }

    pub fn resolve_binary(&self) -> Option<PathBuf> {
        self.binary.as_ref().map(|b| self.root.join(b))
    }
}
