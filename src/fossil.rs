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
/// A configured variant with its invocation resolved.
pub struct ResolvedVariant {
    name: FossilVariantKey,
    runner: PathBuf,
}

impl ResolvedVariant {
    pub fn name(&self) -> &FossilVariantKey {
        &self.name
    }

    pub fn runner(&self) -> &Path {
        &self.runner
    }

    pub fn command(&self) -> String {
        format!(
            "{} {}",
            shell_quote(&self.runner.to_string_lossy()),
            shell_quote(self.name.as_str())
        )
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub type AnalysisMap = BTreeMap<AnalysisName, String>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FossilConfig {
    pub name: FossilName,
    pub description: Option<String>,
    pub default_iterations: u32,
    pub analyze: AnalysisMap,
    pub artifacts: BTreeMap<String, ArtifactEntry>,
    pub allow_failure: bool,
    pub workdir: Option<FossilPath>,
    pub variants: BTreeMap<FossilVariantKey, FossilPath>,
}

impl Default for FossilConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            default_iterations: 10,
            analyze: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            allow_failure: false,
            workdir: None,
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
        scripts.extend(self.analyze.values().map(|s| s.as_str()));
        scripts.extend(self.artifacts.values().map(|e| e.script.as_str()));
        scripts.extend(self.variants.values().map(FossilPath::as_str));
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
        let map = &self.config.analyze;

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
                let (name, script) = map.iter().next().ok_or_else(|| {
                    FossilError::NotFound(format!(
                        "no analysis script configured for {:?}",
                        self.config.name
                    ))
                })?;
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

    pub fn resolve_variant(
        &self,
        name: &FossilVariantKey,
    ) -> Result<ResolvedVariant, FossilError> {
        let (key, script) = self
            .config
            .variants
            .get_key_value(name)
            .ok_or_else(|| {
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
            runner: script.resolve(&self.path),
        })
    }

    pub fn resolve_bury_tasks(
        &self,
        variants: &[FossilVariantKey],
    ) -> Result<Vec<ResolvedVariant>, FossilError> {
        if !variants.is_empty() {
            return variants
                .iter()
                .map(|name| self.resolve_variant(name))
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
            .map(|name| self.resolve_variant(name))
            .collect()
    }
}
