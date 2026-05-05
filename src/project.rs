use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::entity::DirEntity;
use crate::record::Record;
use crate::error::FossilError;
use crate::fossil::{Fossil, FossilConfig};
use crate::git;
use crate::io::status;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub constants: BTreeMap<String, String>,
}

impl ProjectConfig {
    pub fn resolve_constants(&mut self) {
        for _ in 0..self.constants.len() {
            let snapshot = self.constants.clone();
            let mut changed = false;
            for value in self.constants.values_mut() {
                for (k, v) in &snapshot {
                    let placeholder = format!("${k}");
                    if value.contains(&placeholder) {
                        *value = value.replace(&placeholder, v);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }
}

/// [Fossil Doc] `Project`
/// -------------------------------------------------------------
/// A Project is a collection of fossils and the git boundary.
/// All records under a project are version-controlled together.
/// Think of it as a repository of related benchmarks.
#[derive(Clone)]
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
        let mut config: ProjectConfig = FossilError::load_toml(
            &dir.join("project.toml"),
            &format!("project {name:?} not found"),
        )?;
        config.resolve_constants();
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
            constants: BTreeMap::new(),
        };
        let toml = toml::to_string_pretty(&config).map_err(|e| {
            FossilError::InvalidConfig(format!("serializing project {name:?}: {e}"))
        })?;
        std::fs::write(dir.join("project.toml"), toml)?;

        git::Repo::at(&dir).commit(
            vec![PathBuf::from("project.toml")],
            format!("init project {name}"),
        )?;

        Ok(Self { config, path: dir })
    }

    pub fn fossils_dir(&self) -> &Path {
        &self.path
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
                let candidates = if let Some(fossil_name) = fossil_hint {
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
                        _ => matches,
                    }
                } else {
                    projects
                };
                let names: Vec<_> =
                    candidates.iter().map(|p| p.config.name.clone()).collect();
                Err(FossilError::InvalidArgs(format!(
                    "multiple projects exist, specify one with --project: {}",
                    names.join(", ")
                )))
            }
        }
    }

    fn rel_path(&self, abs: &Path) -> Result<PathBuf, FossilError> {
        abs.strip_prefix(&self.path)
            .map(|p| p.to_path_buf())
            .map_err(|_| FossilError::InvalidConfig(format!(
                "{}: path is not under project", abs.display()
            )))
    }

    pub fn create_fossil(
        &self,
        name: &str,
        description: Option<&str>,
        iterations: Option<u32>,
    ) -> Result<(), FossilError> {
        let f = Fossil::create(self.fossils_dir(), name, description, iterations)?;
        let rel = self.rel_path(&f.path)?;
        git::Repo::at(&self.path).commit(
            vec![rel.join("fossil.toml")],
            format!("create fossil {name}"),
        )?;
        status!("created fossil {}", f.path.display());
        Ok(())
    }

    pub fn delete_record(
        &self,
        record: &Record,
    ) -> Result<(), FossilError> {
        let rel = self.rel_path(&record.dir)?;
        git::Repo::at(&self.path).rm(
            &rel,
            format!("delete record {}", record.id()),
        )
    }

    pub fn import(&self, toml_path: &Path) -> Result<(), FossilError> {
        let contents = std::fs::read_to_string(toml_path)?;
        let config: FossilConfig = toml::from_str(&contents).map_err(|e| {
            FossilError::InvalidConfig(format!("{}: {e}", toml_path.display()))
        })?;

        let fossil_dir = self.fossils_dir().join(&config.name);
        if fossil_dir.exists() {
            return Err(FossilError::AlreadyExists(format!("fossil {:?}", config.name)));
        }
        std::fs::create_dir_all(&fossil_dir)?;
        std::fs::create_dir_all(fossil_dir.join("records"))?;
        std::fs::copy(toml_path, fossil_dir.join("fossil.toml"))?;

        let source_dir = toml_path.parent().unwrap_or(Path::new("."));
        let rel_fossil = self.rel_path(&fossil_dir)?;

        let mut git_paths = vec![rel_fossil.join("fossil.toml")];
        for script in config.all_scripts() {
            let src = source_dir.join(script);
            if src.exists() {
                copy_executable(&src, &fossil_dir.join(script))?;
                git_paths.push(rel_fossil.join(script));
            }
        }

        git::Repo::at(&self.path).commit(
            git_paths,
            format!("import fossil {}", config.name),
        )?;
        status!("imported {} → {}", config.name, fossil_dir.display());
        Ok(())
    }
}

fn copy_executable(src: &Path, dest: &Path) -> Result<(), FossilError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src, dest)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(dest)?.permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(dest, perms)?;
    }
    Ok(())
}
