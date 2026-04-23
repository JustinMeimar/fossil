use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::git;

#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// [Fossil Doc]
/// A project encapsulates a collection of Fossils. Conceptually like
/// an archaeological site with a perimeter marked for digging.
pub struct Project {
    pub config: ProjectConfig,
    pub path: PathBuf,
}

impl Project {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(dir.join("project.toml"))?;
        let config: ProjectConfig = toml::from_str(&contents)?;
        Ok(Self {
            config,
            path: dir.to_path_buf(),
        })
    }

    pub fn create(
        projects_dir: &Path,
        name: &str,
        description: Option<&str>,
    ) -> anyhow::Result<Self> {
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

        git::init(&dir)?;
        git::Commit::new(
            &dir,
            vec![PathBuf::from("project.toml")],
            format!("init project {name}"),
        )
        .execute()?;

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
            if !entry.file_type()?.is_dir() {
                continue;
            }
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

    pub fn resolve(
        projects_dir: &Path,
        name: Option<&str>,
        fossil_hint: Option<&str>,
    ) -> anyhow::Result<Self> {
        if let Some(n) = name {
            return Self::load(&projects_dir.join(n));
        }
        let projects = Self::list_all(projects_dir)?;
        match projects.len() {
            0 => anyhow::bail!(
                "no projects found — create one with: fossil project create <name>"
            ),
            1 => Ok(projects.into_iter().next().unwrap()),
            _ => {
                if let Some(fossil_name) = fossil_hint {
                    let matches: Vec<_> = projects
                        .into_iter()
                        .filter(|p| {
                            p.fossils_dir().join(fossil_name).exists()
                        })
                        .collect();
                    match matches.len() {
                        1 => {
                            return Ok(
                                matches.into_iter().next().unwrap()
                            )
                        }
                        0 => anyhow::bail!(
                            "no project contains fossil {fossil_name:?}"
                        ),
                        _ => {}
                    }
                }
                let names: Vec<_> = Self::list_all(projects_dir)?
                    .iter()
                    .map(|p| p.config.name.clone())
                    .collect();
                anyhow::bail!(
                    "multiple projects exist, specify one with --project: {}",
                    names.join(", ")
                );
            }
        }
    }
}
