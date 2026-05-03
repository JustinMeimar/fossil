use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::analysis;
use crate::error::FossilError;
use crate::fossil::{Fossil, VizEntry};

/// [Fossil Doc] `Figure`
/// -------------------------------------------------------------
/// A visualization of analysis output. Resolves which viz script
/// to use, then pipes analysis metrics as JSON to the script's
/// stdin. The script produces the actual plot or chart.
pub struct Figure<'a> {
    pub name: &'a str,
    entry: &'a VizEntry,
    script_path: PathBuf,
}

impl<'a> Figure<'a> {
    pub fn resolve(
        fossil: &'a Fossil,
        name: Option<&'a str>,
    ) -> Result<Self, FossilError> {
        let map = fossil
            .config
            .visualize
            .as_ref()
            .ok_or_else(|| FossilError::NotFound(format!(
                "no visualizations configured for {:?}", fossil.config.name
            )))?;

        let (chosen_name, entry) = match name {
            Some(n) => {
                let entry = map.get(n).ok_or_else(|| {
                    let names: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
                    FossilError::unknown("visualization", n, &names)
                })?;
                (n, entry)
            }
            None if map.len() == 1 => {
                let (k, v) = map.iter().next().unwrap();
                (k.as_str(), v)
            }
            None => {
                let names: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
                let picked = crate::ui::pick("select visualization:", &names)
                    .ok_or_else(|| FossilError::InvalidArgs(format!(
                        "no visualization selected, available: {}", names.join(", ")
                    )))?;
                let (k, v) = map.get_key_value(picked).unwrap();
                (k.as_str(), v)
            }
        };

        let script_path = fossil.path.join(&entry.script);
        Ok(Self { name: chosen_name, entry, script_path })
    }

    pub fn analysis_name(&self) -> &str {
        &self.entry.analysis
    }

    pub fn run(
        &self,
        fossil: &Fossil,
        columns: &[(String, analysis::Metric)],
    ) -> Result<(), FossilError> {
        let result: BTreeMap<&str, &analysis::Metric> = columns
            .iter()
            .map(|(name, m)| (name.as_str(), m))
            .collect();
        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| FossilError::InvalidConfig(format!(
                "serializing analysis: {e}"
            )))?;

        crate::ui::status!("visualizing with {} ({})", self.name, self.script_path.display());

        let mut child = std::process::Command::new(&self.script_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .current_dir(&fossil.path)
            .env("FOSSIL_NAME", &fossil.config.name)
            .env("FOSSIL_DIR", &fossil.path)
            .env("FOSSIL_VIZ_NAME", self.name)
            .spawn()
            .map_err(|e| FossilError::InvalidConfig(format!(
                "viz script {} failed: {e} — is the script executable?",
                self.script_path.display()
            )))?;

        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, json.as_bytes())
                .map_err(FossilError::Io)?;
        }

        let exit = child.wait()?;
        if !exit.success() {
            return Err(FossilError::InvalidConfig(format!(
                "viz script {} exited with code {}",
                self.script_path.display(),
                exit.code().unwrap_or(-1),
            )));
        }

        Ok(())
    }
}
