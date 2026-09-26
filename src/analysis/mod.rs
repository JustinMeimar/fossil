pub mod quantity;
mod script;
pub use quantity::Metric;
pub use script::AnalysisScript;

use crate::error::FossilError;
use std::collections::BTreeMap;

/// A configured analysis with its script path resolved.
pub struct ResolvedAnalysis {
    pub(crate) script: std::path::PathBuf,
}

/// Analyzed metrics grouped by variant name or record identifier.
#[derive(Default)]
pub struct AnalysisResult {
    pub metrics_by_label: BTreeMap<String, Metric>,
}

impl AnalysisResult {
    pub fn to_json(&self) -> Result<String, FossilError> {
        serde_json::to_string_pretty(&self.metrics_by_label).map_err(|e| {
            FossilError::InvalidConfig(format!("serializing analysis: {e}"))
        })
    }

    /// Combine samples with the same label. Discard the result on failure.
    pub fn merge_metric(
        &mut self,
        label: String,
        metric: Metric,
    ) -> Result<(), FossilError> {
        match self.metrics_by_label.entry(label) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(metric);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let label = entry.key().clone();
                entry.get_mut().merge(metric).map_err(|error| {
                    FossilError::InvalidConfig(format!(
                        "analysis result for {label:?}: {error}"
                    ))
                })?;
            }
        }
        Ok(())
    }
}
