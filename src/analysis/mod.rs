pub mod quantity;
mod script;
pub use quantity::Metric;
pub use script::AnalysisScript;

use crate::error::FossilError;
use std::collections::BTreeMap;

pub type AnalysisName = String;

pub fn columns_to_json(
    columns: &[(String, Metric)],
) -> Result<String, FossilError> {
    let map: BTreeMap<&str, &Metric> =
        columns.iter().map(|(n, m)| (n.as_str(), m)).collect();
    serde_json::to_string_pretty(&map).map_err(|e| {
        FossilError::InvalidConfig(format!("serializing analysis: {e}"))
    })
}
