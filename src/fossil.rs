use crate::analysis::{AnalysisName, AnalysisScript};
use crate::artifact::ArtifactEntry;
use crate::entity::DirEntity;
use crate::environment::{ExecutionContext, Operation};
use crate::error::FossilError;
use crate::manifest::Manifest;
use crate::project::Project;
use crate::record::Record;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type FossilName = String;

/// A path relative to a fossil's root directory.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct FossilPath(String);

impl FossilPath {
    pub fn resolve(&self, root: &Path) -> PathBuf {
        root.join(&self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A variant name, keying into a fossil's variant map.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize,
)]
#[serde(transparent)]
pub struct FossilVariantKey(String);

impl FossilVariantKey {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FossilVariantKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// [Fossil Doc] `ResolvedVariant`
/// To give a fossil variant a type. Produced when a
/// variant we ask for matches one declared in the fossil.toml
pub struct ResolvedVariant {
    pub name: FossilVariantKey,
    pub command: String,
}

pub type AnalysisMap = BTreeMap<AnalysisName, String>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FossilConfig {
    pub name: FossilName,
    pub description: Option<String>,
    pub default_iterations: u32,
    pub analyze: Option<AnalysisMap>,
    pub artifacts: BTreeMap<String, ArtifactEntry>,
    pub allow_failure: bool,
    pub workdir: Option<FossilPath>,
    pub variables: BTreeMap<String, String>,
    pub variants: BTreeMap<FossilVariantKey, String>,
}

impl Default for FossilConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            default_iterations: 10,
            analyze: None,
            artifacts: BTreeMap::new(),
            allow_failure: false,
            workdir: None,
            variables: BTreeMap::new(),
            variants: BTreeMap::new(),
        }
    }
}

impl FossilConfig {
    pub fn desc(&self) -> &str {
        self.description.as_deref().unwrap_or("")
    }

    pub fn all_scripts(&self) -> Vec<&str> {
        let mut scripts = Vec::new();
        if let Some(ref map) = self.analyze {
            scripts.extend(map.values().map(|s| s.as_str()));
        }
        scripts.extend(self.artifacts.values().map(|e| e.script.as_str()));
        scripts
    }
}

/// [Fossil Doc] `Fossil`
/// -------------------------------------------------------------
/// A Fossil is the core type of the program. It represents a
/// benchmark, profile, test-run - what we can generally call a
/// "measurement" of the subject program.
#[derive(Clone)]
pub struct Fossil {
    pub config: FossilConfig,
    pub path: PathBuf,
}

// NOTE(Justin): Is it better convention to impl traits in the file
// containing the trait definition? Or in the struct being impl'ds file.
impl DirEntity for Fossil {
    fn load(dir: &Path) -> Result<Self, FossilError> {
        let name = dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let config: FossilConfig = FossilError::load_toml(
            &dir.join("fossil.toml"),
            &format!("fossil {name:?} not found"),
        )?;
        Ok(Self {
            config,
            path: dir.to_path_buf(),
        })
    }

    fn sort_key(&self) -> &str {
        &self.config.name
    }
}

impl Fossil {
    pub fn create(
        fossils_dir: &Path,
        name: &str,
        description: Option<&str>,
        iterations: Option<u32>,
    ) -> Result<Self, FossilError> {
        let dir = fossils_dir.join(name);
        if dir.exists() {
            return Err(FossilError::AlreadyExists(format!("fossil {name:?}")));
        }
        std::fs::create_dir_all(&dir)?;
        std::fs::create_dir_all(dir.join("records"))?;
        let config = FossilConfig {
            name: name.to_string(),
            description: description.map(String::from),
            default_iterations: iterations.unwrap_or(10),
            ..Default::default()
        };
        let toml = toml::to_string_pretty(&config).map_err(|e| {
            FossilError::InvalidConfig(format!(
                "serializing fossil {name:?}: {e}"
            ))
        })?;
        std::fs::write(dir.join("fossil.toml"), toml)?;
        Ok(Self { config, path: dir })
    }

    pub fn records_dir(&self) -> PathBuf {
        self.path.join("records")
    }

    pub fn resolve_analysis(
        &self,
        name: Option<&str>,
        project: &Project,
    ) -> Result<AnalysisScript, FossilError> {
        let map = self.config.analyze.as_ref().ok_or_else(|| {
            FossilError::NotFound(format!(
                "no analysis script configured for {:?}",
                self.config.name
            ))
        })?;

        let available: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
        let (analysis_name, script) = match name {
            Some(n) => (
                n,
                map.get(n).ok_or_else(|| {
                    FossilError::unknown("analysis", n, &available)
                })?,
            ),
            None if map.len() > 1 => {
                return Err(FossilError::InvalidArgs(format!(
                    "multiple analyses available, use --analysis: {}",
                    available.join(", ")
                )));
            }
            None => {
                let (name, script) = map.iter().next().unwrap();
                (name.as_str(), script)
            }
        };

        Ok(AnalysisScript::new(
            self.path.join(script),
            ExecutionContext::new(
                project,
                self,
                Operation::Analysis(analysis_name),
            ),
        ))
    }

    pub fn find_records(
        &self,
        variant: Option<&str>,
        last: Option<usize>,
    ) -> Result<Vec<Record>, FossilError> {
        let mut records: Vec<_> = std::fs::read_dir(self.records_dir())?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|e| {
                let dir = e.path();
                let manifest = Manifest::load(&dir).ok()?;
                if variant.is_some()
                    && Some(manifest.variant.as_str()) != variant
                {
                    return None;
                }
                Some(Record { dir, manifest })
            })
            .collect();

        records.sort_by(|a, b| a.manifest.timestamp.cmp(&b.manifest.timestamp));
        if let Some(n) = last {
            let skip = records.len().saturating_sub(n);
            records.drain(..skip);
        }
        Ok(records)
    }

    pub fn expand(
        &self,
        template: &str,
        project_scope: &BTreeMap<String, String>,
    ) -> String {
        let mut result = template.to_string();
        let mut local_keys: Vec<_> = self.config.variables.keys().collect();
        local_keys.sort_by(|a, b| b.len().cmp(&a.len()));
        for k in local_keys {
            result =
                result.replace(&format!("${k}"), &self.config.variables[k]);
        }
        let mut proj_keys: Vec<_> = project_scope.keys().collect();
        proj_keys.sort_by(|a, b| b.len().cmp(&a.len()));
        for k in proj_keys {
            result = result.replace(&format!("@{k}"), &project_scope[k]);
        }
        result
    }

    pub fn resolve_variant(
        &self,
        name: &FossilVariantKey,
        project_scope: &BTreeMap<String, String>,
    ) -> Result<ResolvedVariant, FossilError> {
        let (key, command) = self
            .config
            .variants
            .get_key_value(name)
            .ok_or_else(|| {
                // Find variants registered in the fossil.toml
                let available: Vec<&str> = self
                    .config
                    .variants
                    .keys()
                    .map(|k| k.as_str())
                    .collect();
                FossilError::unknown("variant", name.as_str(), &available)
            })?;
        Ok(ResolvedVariant {
            name: key.clone(),
            command: self.expand(command, project_scope),
        })
    }

    pub fn resolve_bury_tasks(
        &self,
        variants: &[FossilVariantKey],
        project_scope: &BTreeMap<String, String>,
    ) -> Result<Vec<(FossilVariantKey, String)>, FossilError> {
        if !variants.is_empty() {
            return variants
                .iter()
                .map(|name| {
                    self.resolve_variant(name, project_scope)
                        .map(|v| (v.name, v.command))
                })
                .collect();
        }
        if self.config.variants.is_empty() {
            return Err(FossilError::InvalidArgs(
                "no variants configured — define variants in fossil.toml"
                    .into(),
            ));
        }
        self.config
            .variants
            .keys()
            .map(|k| {
                self.resolve_variant(k, project_scope)
                    .map(|v| (v.name, v.command))
            })
            .collect()
    }
}
