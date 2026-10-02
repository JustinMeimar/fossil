use crate::analysis::{AnalyzedRecords, ResolvedAnalysis};
use crate::error::FossilError;
use crate::fossil::{ConfigurationKey, Fossil, FossilPath};
use crate::project::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactConfig {
    pub script: FossilPath,
    pub analysis: Option<ConfigurationKey>,
    #[serde(
        default,
        rename = "required-variants",
        skip_serializing_if = "BTreeSet::is_empty"
    )]
    pub required_variants: BTreeSet<String>,
}

/// A configured generator and its optional analysis dependency.
pub struct ResolvedArtifact {
    pub(crate) key: ConfigurationKey,
    pub(crate) script: PathBuf,
    pub(crate) analysis: Option<ResolvedAnalysis>,
    pub(crate) required_variants: BTreeSet<ConfigurationKey>,
}

impl ResolvedArtifact {
    pub fn missing_variants(
        &self,
        records: &AnalyzedRecords,
    ) -> Vec<&ConfigurationKey> {
        let available = records.variants();
        self.required_variants
            .iter()
            .filter(|variant| !available.contains(*variant))
            .collect()
    }

    pub fn prepare(
        &self,
        records: Option<&AnalyzedRecords>,
    ) -> Result<PreparedArtifact<'_>, FossilError> {
        let input = match (&self.analysis, records) {
            (Some(_), Some(records)) => {
                let missing = self.missing_variants(records);
                if !missing.is_empty() {
                    return Err(FossilError::InvalidArgs(format!(
                        "artifact {} is missing required variants: {}",
                        self.key,
                        missing
                            .iter()
                            .map(|variant| variant.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }
                Some(records.by_variant()?.to_json()?)
            }
            (None, None) => None,
            (Some(_), None) => {
                return Err(FossilError::InvalidArgs(format!(
                    "artifact {} requires analysis results",
                    self.key
                )));
            }
            (None, Some(_)) => {
                return Err(FossilError::InvalidArgs(format!(
                    "artifact {} does not accept analysis results",
                    self.key
                )));
            }
        };
        Ok(PreparedArtifact {
            artifact: self,
            input,
        })
    }

    pub fn output_dir(
        &self,
        fossil: &Fossil,
        project: &Project,
    ) -> Result<PathBuf, FossilError> {
        project.artifact_path(
            PathBuf::from(&fossil.config.name).join(self.key.as_str()),
        )
    }
}

pub struct PreparedArtifact<'a> {
    artifact: &'a ResolvedArtifact,
    input: Option<String>,
}

impl PreparedArtifact<'_> {
    pub fn artifact(&self) -> &ResolvedArtifact {
        self.artifact
    }

    pub fn input(&self) -> Option<&str> {
        self.input.as_deref()
    }
}
