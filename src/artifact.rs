use crate::analysis::ResolvedAnalysis;
use crate::error::FossilError;
use crate::fossil::{ConfigurationKey, Fossil, FossilPath};
use crate::project::Project;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactConfig {
    pub script: FossilPath,
    pub analysis: Option<ConfigurationKey>,
}

/// A configured generator and its optional analysis dependency.
pub struct ResolvedArtifact {
    pub(crate) key: ConfigurationKey,
    pub(crate) script: PathBuf,
    pub(crate) analysis: Option<ResolvedAnalysis>,
}

impl ResolvedArtifact {
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
