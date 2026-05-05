use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::analysis;
use crate::error::FossilError;
use crate::fossil::{Fossil, FigureEntry};

pub struct Figure<'a> {
    pub name: &'a str,
    entry: &'a FigureEntry,
}

impl<'a> Figure<'a> {
    pub fn resolve(
        fossil: &'a Fossil,
        name: Option<&'a str>,
    ) -> Result<Self, FossilError> {
        let map = fossil
            .config
            .figures
            .as_ref()
            .ok_or_else(|| FossilError::NotFound(format!(
                "no figures configured for {:?}", fossil.config.name
            )))?;

        let (chosen_name, entry) = match name {
            Some(n) => {
                let entry = map.get(n).ok_or_else(|| {
                    let names: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
                    FossilError::unknown("figure", n, &names)
                })?;
                (n, entry)
            }
            None if map.len() == 1 => {
                let (k, v) = map.iter().next().unwrap();
                (k.as_str(), v)
            }
            None => {
                let names: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
                let picked = crate::io::pick("select figure:", &names)
                    .ok_or_else(|| FossilError::InvalidArgs(format!(
                        "no figure selected, available: {}", names.join(", ")
                    )))?;
                let (k, v) = map.get_key_value(picked).unwrap();
                (k.as_str(), v)
            }
        };

        Ok(Self { name: chosen_name, entry })
    }

    pub fn analysis_name(&self) -> &str {
        self.entry.analysis.as_str()
    }

    pub fn output_path(&self, fossil: &Fossil) -> PathBuf {
        fossil.path.join("figures").join(format!("{}.png", self.name))
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

        let script_path = self.entry.script.resolve(&fossil.path);
        let out_path = self.output_path(fossil);

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut child = std::process::Command::new(&script_path)
            .arg(&out_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&fossil.path)
            .spawn()
            .map_err(|e| FossilError::InvalidConfig(format!(
                "figure script {} failed: {e} — is the script executable?",
                script_path.display()
            )))?;

        if let Some(mut stdin) = child.stdin.take() {
            std::io::Write::write_all(&mut stdin, json.as_bytes())
                .map_err(FossilError::Io)?;
        }

        let output = child.wait_with_output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FossilError::InvalidConfig(format!(
                "figure script {} failed: {}",
                script_path.display(),
                stderr.trim(),
            )));
        }

        Ok(())
    }
}
