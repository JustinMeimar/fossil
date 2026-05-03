use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::analysis;
use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::manifest::Manifest;

fn default_iterations() -> u32 { 10 }

pub struct Variant {
    pub name: String,
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
pub struct VizEntry {
    pub analysis: String,
    pub script: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FossilConfig {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_iterations")]
    pub default_iterations: u32,
    pub analyze: Option<AnalyzeSpec>,
    #[serde(default)]
    pub visualize: Option<BTreeMap<String, VizEntry>>,
    #[serde(default)]
    pub allow_failure: bool,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub variants: BTreeMap<String, String>,
}

impl FossilConfig {
    pub fn all_scripts(&self) -> Vec<&str> {
        let mut scripts = Vec::new();
        if let Some(ref spec) = self.analyze {
            scripts.extend(spec.scripts());
        }
        if let Some(ref viz_map) = self.visualize {
            scripts.extend(
                viz_map.values().map(|e| e.script.as_str()),
            );
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
        let contents = std::fs::read_to_string(dir.join("fossil.toml"))
            .map_err(|_| FossilError::NotFound(format!(
                "fossil {name:?} not found — run 'fossil list' to see available fossils"
            )))?;
        let config: FossilConfig = toml::from_str(&contents).map_err(|e| {
            FossilError::InvalidConfig(format!("fossil.toml in {name:?}: {e}"))
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
            visualize: None,
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
    ) -> Result<analysis::AnalysisScript, FossilError> {
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
                    return Err(FossilError::InvalidArgs(format!(
                        "unknown analysis {n:?}, available: {}", names.join(", ")
                    )));
                }
                Some(n)
            }
            None if names.len() > 1 => {
                let picked = crate::ui::pick("select analysis:", &names)
                    .ok_or_else(|| FossilError::InvalidArgs(format!(
                        "no analysis selected, available: {}", names.join(", ")
                    )))?;
                Some(picked)
            }
            None => None,
        };

        self.analyze_script(chosen)
            .map(analysis::AnalysisScript::new)
            .ok_or_else(|| FossilError::NotFound(format!(
                "no analysis script configured for {:?}", self.config.name
            )))
    }

    pub fn find_records(
        &self,
        variant: Option<&str>,
        last: Option<usize>,
    ) -> Result<Vec<analysis::Record>, FossilError> {
        let mut records: Vec<_> = std::fs::read_dir(self.records_dir())?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|e| {
                let dir = e.path();
                let manifest = Manifest::load(&dir).ok()?;
                if variant.is_some() && manifest.variant.as_deref() != variant {
                    return None;
                }
                Some(analysis::Record { dir, manifest })
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
        for (k, v) in project_constants {
            result = result.replace(&format!("${k}"), v);
        }
        for (k, v) in &self.config.variables {
            result = result.replace(&format!("${k}"), v);
        }
        result
    }

    pub fn resolve_variant(
        &self,
        name: &str,
        project_constants: &BTreeMap<String, String>,
    ) -> Result<Variant, FossilError> {
        let (key, command) =
            self.config.variants.get_key_value(name).ok_or_else(|| {
                let available: Vec<_> = self.config.variants.keys().cloned().collect();
                FossilError::InvalidArgs(format!(
                    "unknown variant {name:?}, available: {}", available.join(", ")
                ))
            })?;
        Ok(Variant {
            name: key.clone(),
            command: self.expand(command, project_constants),
        })
    }
}
