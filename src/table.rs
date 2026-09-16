use std::path::PathBuf;

use crate::analysis;
use crate::environment::{ExecutionContext, Operation};
use crate::error::FossilError;
use crate::fossil::{Fossil, TableEntry};
use crate::project::Project;

pub struct Table<'a> {
    pub name: &'a str,
    entry: &'a TableEntry,
}

impl<'a> Table<'a> {
    pub fn resolve(
        fossil: &'a Fossil,
        name: Option<&'a str>,
    ) -> Result<Self, FossilError> {
        let map = fossil.config.tables.as_ref().ok_or_else(|| {
            FossilError::NotFound(format!(
                "no tables configured for {:?}",
                fossil.config.name
            ))
        })?;

        let (chosen_name, entry) = match name {
            Some(n) => {
                let entry = map.get(n).ok_or_else(|| {
                    let names: Vec<&str> =
                        map.keys().map(|k| k.as_str()).collect();
                    FossilError::unknown("table", n, &names)
                })?;
                (n, entry)
            }
            None if map.len() == 1 => {
                let (k, v) = map.iter().next().unwrap();
                (k.as_str(), v)
            }
            None => {
                let names: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
                return Err(FossilError::InvalidArgs(format!(
                    "multiple tables available, use --table: {}",
                    names.join(", ")
                )));
            }
        };

        Ok(Self {
            name: chosen_name,
            entry,
        })
    }

    pub fn analysis_name(&self) -> Option<&str> {
        self.entry.analysis.as_ref().map(|a| a.as_str())
    }

    /// Select the analysis payload this table consumes from the TUI's last
    /// analysis. Static tables deliberately ignore stale analysis state.
    pub(crate) fn columns_from_last_analysis<'b>(
        &self,
        last_analysis: Option<&'b (String, Vec<(String, analysis::Metric)>)>,
    ) -> Result<Option<&'b [(String, analysis::Metric)]>, FossilError> {
        let Some(required) = self.analysis_name() else {
            return Ok(None);
        };
        let Some((actual, columns)) = last_analysis else {
            return Err(FossilError::InvalidArgs(format!(
                "table {:?} requires analysis {required:?}; run it first",
                self.name
            )));
        };
        if actual != required {
            return Err(FossilError::InvalidArgs(format!(
                "table {:?} requires analysis {required:?}, but the current analysis is {actual:?}",
                self.name
            )));
        }
        Ok(Some(columns))
    }

    fn validate_columns(
        &self,
        columns: Option<&[(String, analysis::Metric)]>,
    ) -> Result<(), FossilError> {
        match (self.analysis_name(), columns.is_some()) {
            (None, false) | (Some(_), true) => Ok(()),
            (None, true) => Err(FossilError::InvalidArgs(format!(
                "table {:?} does not consume analysis output",
                self.name
            ))),
            (Some(required), false) => Err(FossilError::InvalidArgs(format!(
                "table {:?} requires analysis {required:?}",
                self.name
            ))),
        }
    }

    /// Where the emitted JSON table should land. Tables target the
    /// project's `artifact_dir` and are named `<fossil-name>-<name>.json`
    /// so that a downstream typst library can consume them by convention.
    pub fn output_path(
        &self,
        fossil: &Fossil,
        project: &Project,
    ) -> Result<PathBuf, FossilError> {
        project
            .artifact_path(format!("{}-{}.json", fossil.config.name, self.name))
    }

    pub fn run(
        &self,
        fossil: &Fossil,
        project: &Project,
        columns: Option<&[(String, analysis::Metric)]>,
        force: bool,
    ) -> Result<PathBuf, FossilError> {
        self.validate_columns(columns)?;
        let script_path = self.entry.script.resolve(&fossil.path);
        let out_path = self.output_path(fossil, project)?;

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = columns.map(analysis::columns_to_json).transpose()?;
        let stdin_cfg = if json.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        };

        let mut cmd = std::process::Command::new(&script_path);
        let context =
            ExecutionContext::new(project, fossil, Operation::Table(self.name));
        context.configure(&mut cmd);
        cmd.arg(&out_path)
            .stdin(stdin_cfg)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&fossil.path);
        if force {
            cmd.env("FOSSIL_FORCE", "1");
        }
        let mut child = cmd.spawn().map_err(|e| {
            FossilError::InvalidConfig(format!(
                "table script {} failed: {e} — is the script executable?",
                script_path.display()
            ))
        })?;

        let write_result = match (json, child.stdin.take()) {
            (Some(json), Some(mut stdin)) => {
                std::io::Write::write_all(&mut stdin, json.as_bytes())
                    .map_err(FossilError::Io)
            }
            (Some(_), None) => Err(FossilError::InvalidConfig(format!(
                "table script {} has no stdin pipe",
                script_path.display()
            ))),
            (None, _) => Ok(()),
        };

        let output = child.wait_with_output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FossilError::InvalidConfig(format!(
                "table script {} failed: {}",
                script_path.display(),
                stderr.trim(),
            )));
        }

        write_result?;
        Ok(out_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::Figure;
    use crate::fossil::FossilConfig;
    use crate::project::ProjectConfig;

    fn fossil_with_table(analysis: Option<&str>) -> Fossil {
        let analysis_line = analysis
            .map(|name| format!("analysis = {name:?}\n"))
            .unwrap_or_default();
        let source = format!(
            r#"
name = "3-3-inter-workload"
[figures.chart]
analysis = "artifact-sets"
script = "figure.py"
[tables.coverage]
{analysis_line}script = "table.py"
"#
        );
        Fossil {
            config: toml::from_str::<FossilConfig>(&source).unwrap(),
            path: PathBuf::from("/tmp/fossil-table-test"),
        }
    }

    fn columns() -> Vec<(String, analysis::Metric)> {
        vec![(
            "amazon".into(),
            analysis::Metric::from_json(&serde_json::json!({"count": 1})),
        )]
    }

    #[test]
    fn static_table_does_not_consume_stale_analysis() {
        let fossil = fossil_with_table(None);
        let table = Table::resolve(&fossil, Some("coverage")).unwrap();
        let last_analysis = ("artifact-sets".into(), columns());

        assert!(
            table
                .columns_from_last_analysis(Some(&last_analysis))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn analysis_table_consumes_matching_analysis() {
        let fossil = fossil_with_table(Some("artifact-sets"));
        let table = Table::resolve(&fossil, Some("coverage")).unwrap();
        let last_analysis = ("artifact-sets".into(), columns());

        let selected = table
            .columns_from_last_analysis(Some(&last_analysis))
            .unwrap()
            .unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0, "amazon");
    }

    #[test]
    fn analysis_table_rejects_missing_or_different_analysis() {
        let fossil = fossil_with_table(Some("artifact-sets"));
        let table = Table::resolve(&fossil, Some("coverage")).unwrap();
        let different = ("allocation".into(), columns());

        let missing = table.columns_from_last_analysis(None).err().unwrap();
        assert!(missing.to_string().contains("run it first"));

        let mismatch = table
            .columns_from_last_analysis(Some(&different))
            .err()
            .unwrap();
        assert!(mismatch.to_string().contains("current analysis"));
    }

    #[test]
    fn run_contract_rejects_invalid_payload_shapes() {
        let static_fossil = fossil_with_table(None);
        let static_table =
            Table::resolve(&static_fossil, Some("coverage")).unwrap();
        let data = columns();
        assert!(static_table.validate_columns(Some(&data)).is_err());

        let analyzed_fossil = fossil_with_table(Some("artifact-sets"));
        let analyzed_table =
            Table::resolve(&analyzed_fossil, Some("coverage")).unwrap();
        assert!(analyzed_table.validate_columns(None).is_err());
    }

    #[test]
    fn artifacts_share_the_project_artifact_directory() {
        let fossil = fossil_with_table(None);
        let table = Table::resolve(&fossil, Some("coverage")).unwrap();
        let figure = Figure::resolve(&fossil, Some("chart")).unwrap();
        let project = Project {
            config: toml::from_str::<ProjectConfig>(
                "name = \"test\"\nartifact_dir = \"artifacts\"\n",
            )
            .unwrap(),
            path: PathBuf::from("/tmp/fossil-project-test"),
        };

        assert_eq!(
            table.output_path(&fossil, &project).unwrap(),
            PathBuf::from(
                "/tmp/fossil-project-test/artifacts/3-3-inter-workload-coverage.json"
            )
        );
        assert_eq!(
            figure.output_path(&fossil, &project).unwrap(),
            PathBuf::from(
                "/tmp/fossil-project-test/artifacts/3-3-inter-workload-chart.pdf"
            )
        );
    }
}
