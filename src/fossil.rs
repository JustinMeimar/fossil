use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::analysis::AnalysisScript;
use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::manifest::Manifest;
use crate::record::Record;

fn default_iterations() -> u32 { 10 }

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

/// A key referencing a named analysis in an AnalyzeSpec.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AnalysisRef(String);

impl AnalysisRef {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A variant name, keying into a fossil's variant map.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord,
         Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub struct VariantName(String);

impl VariantName {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for VariantName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::ops::Deref for VariantName {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

pub struct Variant {
    pub name: VariantName,
    pub command: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AnalyzeSpec {
    Single(String),
    Multi(BTreeMap<String, String>),
}

impl AnalyzeSpec {
    pub fn resolve(&self, name: Option<&str>) -> Option<&str> {
        match self {
            AnalyzeSpec::Single(s) => Some(s.as_str()),
            AnalyzeSpec::Multi(map) => {
                if let Some(name) = name {
                    map.get(name).map(|v| v.as_str())
                } else {
                    map.values().next().map(|v| v.as_str())
                }
            }
        }
    }

    fn stem(path: &str) -> &str {
        let name = path.rsplit('/').next().unwrap_or(path);
        name.strip_suffix(".py")
            .or_else(|| name.strip_suffix(".sh"))
            .unwrap_or(name)
    }

    pub fn names(&self) -> Vec<&str> {
        match self {
            AnalyzeSpec::Single(s) => vec![Self::stem(s)],
            AnalyzeSpec::Multi(map) => map.keys().map(|k| k.as_str()).collect(),
        }
    }

    pub fn scripts(&self) -> Vec<&str> {
        match self {
            AnalyzeSpec::Single(s) => vec![s.as_str()],
            AnalyzeSpec::Multi(map) => map.values().map(|v| v.as_str()).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FigureEntry {
    pub analysis: AnalysisRef,
    pub script: FossilPath,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FossilConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_iterations")]
    pub default_iterations: u32,
    pub analyze: Option<AnalyzeSpec>,
    #[serde(default, alias = "visualize")]
    pub figures: Option<BTreeMap<String, FigureEntry>>,
    #[serde(default)]
    pub allow_failure: bool,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub variants: BTreeMap<VariantName, String>,
}

impl FossilConfig {
    pub fn all_scripts(&self) -> Vec<&str> {
        let mut scripts = Vec::new();
        if let Some(ref spec) = self.analyze {
            scripts.extend(spec.scripts());
        }
        if let Some(ref fig_map) = self.figures {
            scripts.extend(
                fig_map.values().map(|e| e.script.as_str()),
            )
        }
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
            analyze: None,
            figures: None,
            allow_failure: false,
            workdir: None,
            variables: BTreeMap::new(),
            variants: BTreeMap::new(),
        };
        let toml = toml::to_string_pretty(&config).map_err(|e| {
            FossilError::InvalidConfig(format!("serializing fossil {name:?}: {e}"))
        })?;
        std::fs::write(dir.join("fossil.toml"), toml)?;
        Ok(Self { config, path: dir })
    }

    pub fn records_dir(&self) -> PathBuf {
        self.path.join("records")
    }

    pub fn analyze_script(&self, name: Option<&str>) -> Option<PathBuf> {
        self.config
            .analyze
            .as_ref()
            .and_then(|spec| spec.resolve(name))
            .map(|script| self.path.join(script))
    }

    pub fn resolve_analysis(
        &self,
        name: Option<&str>,
    ) -> Result<AnalysisScript, FossilError> {
        let spec = self
            .config
            .analyze
            .as_ref()
            .ok_or_else(|| FossilError::NotFound(format!(
                "no analysis script configured for {:?}", self.config.name
            )))?;

        let names = spec.names();
        let chosen = match name {
            Some(n) => {
                if spec.resolve(Some(n)).is_none() {
                    return Err(FossilError::unknown("analysis", n, &names));
                }
                Some(n)
            }
            None if names.len() > 1 => {
                return Err(FossilError::InvalidArgs(format!(
                    "multiple analyses available, use --analysis: {}", names.join(", ")
                )));
            }
            None => None,
        };

        self.analyze_script(chosen)
            .map(AnalysisScript::new)
            .ok_or_else(|| FossilError::NotFound(format!(
                "no analysis script configured for {:?}", self.config.name
            )))
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
                if variant.is_some() && manifest.variant.as_deref() != variant {
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

    pub fn expand(&self, template: &str, project_constants: &BTreeMap<String, String>) -> String {
        let mut result = template.to_string();
        for (k, v) in &self.config.variables {
            result = result.replace(&format!("${k}"), v);
        }
        for (k, v) in project_constants {
            result = result.replace(&format!("${k}"), v);
        }
        result
    }

    pub fn resolve_variant(
        &self,
        name: &VariantName,
        project_constants: &BTreeMap<String, String>,
    ) -> Result<Variant, FossilError> {
        let (key, command) =
            self.config.variants.get_key_value(name).ok_or_else(|| {
                let available: Vec<&str> = self.config.variants.keys().map(|k| k.as_str()).collect();
                FossilError::unknown("variant", name.as_str(), &available)
            })?;
        Ok(Variant {
            name: key.clone(),
            command: self.expand(command, project_constants),
        })
    }
}
