use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::analysis::ResolvedAnalysis;
use crate::error::FossilError;
use crate::fossil::{ConfigurationKey, Fossil, FossilPath};
use crate::project::Project;

/// The supported file types for artifact generation. PDF composes
/// much better than PNG for type-setting.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactFormat {
    Pdf,
    Json,
}

impl ArtifactFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactConfig {
    pub script: FossilPath,
    pub analysis: Option<ConfigurationKey>,
    pub format: ArtifactFormat,
}

/// A configured artifact with its script and analysis dependency resolved.
pub struct ResolvedArtifact {
    pub(crate) key: ConfigurationKey,
    pub(crate) script: PathBuf,
    pub(crate) format: ArtifactFormat,
    pub(crate) analysis: Option<ResolvedAnalysis>,
}

impl ResolvedArtifact {
    pub fn format(&self) -> ArtifactFormat {
        self.format
    }

    pub fn output_path(
        &self,
        fossil: &Fossil,
        project: &Project,
    ) -> Result<PathBuf, FossilError> {
        project.artifact_path(format!(
            "{}-{}.{}",
            fossil.config.name,
            self.key,
            self.format.extension()
        ))
    }
}
