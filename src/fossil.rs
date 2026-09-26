use crate::analysis::ResolvedAnalysis;
use crate::artifact::{ArtifactConfig, ResolvedArtifact};
use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::manifest::Manifest;
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

/// A name within a configuration namespace (variants, analyses, or artifacts).
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize,
)]
#[serde(transparent)]
pub struct ConfigurationKey(String);

impl ConfigurationKey {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConfigurationKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// [Fossil Doc] `ResolvedVariant`
/// A configured variant with its invocation resolved.
pub struct ResolvedVariant {
    name: ConfigurationKey,
    runner: PathBuf,
}

impl ResolvedVariant {
    pub fn name(&self) -> &ConfigurationKey {
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FossilConfig {
    pub name: FossilName,
    pub description: Option<String>,
    pub default_iterations: u32,
    pub analyses: BTreeMap<ConfigurationKey, FossilPath>,
    pub artifacts: BTreeMap<ConfigurationKey, ArtifactConfig>,
    pub allow_failure: bool,
    pub workdir: Option<FossilPath>,
    pub variants: BTreeMap<ConfigurationKey, FossilPath>,
}

impl Default for FossilConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            default_iterations: 10,
            analyses: BTreeMap::new(),
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
        scripts.extend(self.analyses.values().map(|s| s.as_str()));
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
    const CONFIG_FILE: &'static str = "fossil.toml";
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
        if config.name != name {
            return Err(FossilError::InvalidConfig(format!(
                "fossil name {:?} must match directory {name:?}",
                config.name
            )));
        }
        Ok(Self {
            config,
            path: dir.canonicalize()?,
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
        key: &ConfigurationKey,
    ) -> Result<ResolvedAnalysis, FossilError> {
        let script = lookup(&self.config.analyses, key, "analysis")?;
        Ok(ResolvedAnalysis {
            script: script.resolve(&self.path),
        })
    }

    pub fn resolve_artifact(
        &self,
        key: &ConfigurationKey,
    ) -> Result<ResolvedArtifact, FossilError> {
        let config = lookup(&self.config.artifacts, key, "artifact")?;
        Ok(ResolvedArtifact {
            key: key.clone(),
            script: config.script.resolve(&self.path),
            analysis: config
                .analysis
                .as_ref()
                .map(|key| self.resolve_analysis(key))
                .transpose()?,
        })
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
        key: &ConfigurationKey,
    ) -> Result<ResolvedVariant, FossilError> {
        let script = lookup(&self.config.variants, key, "variant")?;
        Ok(ResolvedVariant {
            name: key.clone(),
            runner: script.resolve(&self.path),
        })
    }
}

fn lookup<'a, T>(
    entries: &'a BTreeMap<ConfigurationKey, T>,
    key: &ConfigurationKey,
    kind: &str,
) -> Result<&'a T, FossilError> {
    entries.get(key).ok_or_else(|| {
        let available: Vec<_> =
            entries.keys().map(ConfigurationKey::as_str).collect();
        FossilError::unknown(kind, key.as_str(), &available)
    })
}
