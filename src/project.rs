use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::git;

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

impl DirEntity for Project {
    fn load(dir: &Path) -> Result<Self, FossilError> {
        let name = dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let contents = std::fs::read_to_string(dir.join("project.toml"))
            .map_err(|_| FossilError::NotFound(format!(
                "project {name:?} not found — run 'fossil project list' to see available projects"
            )))?;
        let config: ProjectConfig = toml::from_str(&contents).map_err(|e| {
            FossilError::InvalidConfig(format!("project.toml in {name:?}: {e}"))
        })?;
        Ok(Self {
            config,
            path: dir.to_path_buf(),
        })
    }

    fn sort_key(&self) -> &str {
        &self.config.name
    }
}

impl Project {
    pub fn create(
        projects_dir: &Path,
        name: &str,
        description: Option<&str>,
    ) -> Result<Self, FossilError> {
        let dir = projects_dir.join(name);
        if dir.exists() {
            return Err(FossilError::AlreadyExists(format!("project {name:?}")));
        }
        std::fs::create_dir_all(&dir)?;
        let config = ProjectConfig {
            name: name.to_string(),
            description: description.map(String::from),
        };
        let toml = toml::to_string_pretty(&config).map_err(|e| {
            FossilError::InvalidConfig(format!("serializing project {name:?}: {e}"))
        })?;
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

    pub fn fossils_dir(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn resolve(
        projects_dir: &Path,
        name: Option<&str>,
        fossil_hint: Option<&str>,
    ) -> Result<Self, FossilError> {
        if let Some(n) = name {
            return Self::load(&projects_dir.join(n));
        }
        let projects = Self::list_all(projects_dir)?;
        match projects.len() {
            0 => Err(FossilError::NotFound(
                "no projects found — create one with: fossil project create <name>".into(),
            )),
            1 => Ok(projects.into_iter().next().unwrap()),
            _ => {
                if let Some(fossil_name) = fossil_hint {
                    let matches: Vec<_> = projects
                        .into_iter()
                        .filter(|p| p.fossils_dir().join(fossil_name).exists())
                        .collect();
                    match matches.len() {
                        1 => return Ok(matches.into_iter().next().unwrap()),
                        0 => {
                            return Err(FossilError::NotFound(format!(
                                "no project contains fossil {fossil_name:?}"
                            )));
                        }
                        _ => {
                            let names: Vec<_> = matches
                                .iter()
                                .map(|p| p.config.name.clone())
                                .collect();
                            return Err(FossilError::InvalidArgs(format!(
                                "multiple projects exist, specify one with --project: {}",
                                names.join(", ")
                            )));
                        }
                    }
                }
                let names: Vec<_> =
                    projects.iter().map(|p| p.config.name.clone()).collect();
                Err(FossilError::InvalidArgs(format!(
                    "multiple projects exist, specify one with --project: {}",
                    names.join(", ")
                )))
            }
        }
    }
}
