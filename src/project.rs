use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

pub struct Project {
    pub config: ProjectConfig,
    pub path: PathBuf,
}

impl Project {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(dir.join("project.toml"))?;
        let config: ProjectConfig = toml::from_str(&contents)?;
        Ok(Self { config, path: dir.to_path_buf() })
    }

    pub fn create(projects_dir: &Path, name: &str, description: Option<&str>) -> anyhow::Result<Self> {
        let dir = projects_dir.join(name);
        if dir.exists() {
            anyhow::bail!("project {name:?} already exists");
        }
        std::fs::create_dir_all(&dir)?;
        std::fs::create_dir_all(dir.join("fossils"))?;
        let config = ProjectConfig {
            name: name.to_string(),
            description: description.map(String::from),
        };
        let toml = toml::to_string_pretty(&config)?;
        std::fs::write(dir.join("project.toml"), toml)?;
        Ok(Self { config, path: dir })
    }

    pub fn list_all(projects_dir: &Path) -> anyhow::Result<Vec<Self>> {
        let mut projects = Vec::new();
        let entries = match std::fs::read_dir(projects_dir) {
            Ok(e) => e,
            Err(_) => return Ok(projects),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() { continue; }
            if let Ok(project) = Self::load(&entry.path()) {
                projects.push(project);
            }
        }
        projects.sort_by(|a, b| a.config.name.cmp(&b.config.name));
        Ok(projects)
    }

    pub fn fossils_dir(&self) -> PathBuf {
        self.path.join("fossils")
    }
}
