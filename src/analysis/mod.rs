pub mod quantity;
mod script;
pub use quantity::Metric;
pub use script::AnalysisScript;

use crate::error::FossilError;
use crate::fossil::ConfigurationKey;
use std::collections::{BTreeMap, BTreeSet};

pub struct AnalyzedRecord {
    pub record_id: String,
    pub variant: ConfigurationKey,
    pub metrics: Metric,
}

#[derive(Default)]
pub struct AnalyzedRecords {
    pub records: Vec<AnalyzedRecord>,
}

impl AnalyzedRecords {
    pub fn variants(&self) -> BTreeSet<ConfigurationKey> {
        self.records
            .iter()
            .map(|record| record.variant.clone())
            .collect()
    }

    pub fn by_record(&self) -> AnalysisResult {
        AnalysisResult {
            metrics_by_label: self
                .records
                .iter()
                .map(|record| {
                    (record.record_id.clone(), record.metrics.clone())
                })
                .collect(),
        }
    }

    pub fn by_variant(&self) -> Result<AnalysisResult, FossilError> {
        let mut result = AnalysisResult::default();
        for record in &self.records {
            result.merge_metric(
                record.variant.to_string(),
                record.metrics.clone(),
            )?;
        }
        Ok(result)
    }
}

/// A configured analysis with its script path resolved.
pub struct ResolvedAnalysis {
    pub(crate) key: ConfigurationKey,
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
