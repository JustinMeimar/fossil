use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::analysis::{self, AnalysisName, Metric};
use crate::environment::{ExecutionContext, Operation};
use crate::error::FossilError;
use crate::fossil::{Fossil, FossilPath};
use crate::project::Project;

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
pub struct ArtifactEntry {
    pub script: FossilPath,
    pub analysis: Option<AnalysisName>,
    pub format: ArtifactFormat,
}

pub struct Artifact<'a> {
    pub name: &'a str,
    entry: &'a ArtifactEntry,
}

impl<'a> Artifact<'a> {
    pub fn resolve(
        fossil: &'a Fossil,
        name: Option<&str>,
    ) -> Result<Self, FossilError> {
        let entries = &fossil.config.artifacts;
        let available: Vec<_> = entries.keys().map(String::as_str).collect();
        let (name, entry) = match name {
            Some(name) => entries.get_key_value(name).ok_or_else(|| {
                FossilError::unknown("artifact", name, &available)
            })?,
            None if entries.is_empty() => {
                return Err(FossilError::NotFound(format!(
                    "no artifacts configured for {:?}",
                    fossil.config.name
                )));
            }
            None if entries.len() == 1 => entries.iter().next().unwrap(),
            None => {
                return Err(FossilError::InvalidArgs(format!(
                    "multiple artifacts available, use --artifact: {}",
                    available.join(", ")
                )));
            }
        };
        Ok(Self { name, entry })
    }

    pub fn analysis_name(&self) -> Option<&str> {
        self.entry.analysis.as_deref()
    }

    pub fn format(&self) -> ArtifactFormat {
        self.entry.format
    }

    pub fn output_path(
        &self,
        fossil: &Fossil,
        project: &Project,
    ) -> Result<PathBuf, FossilError> {
        project.artifact_path(format!(
            "{}-{}.{}",
            fossil.config.name,
            self.name,
            self.format().extension()
        ))
    }

    pub fn run(
        &self,
        fossil: &Fossil,
        project: &Project,
        columns: Option<&[(String, Metric)]>,
        force: bool,
    ) -> Result<PathBuf, FossilError> {
        if self.analysis_name().is_some() != columns.is_some() {
            return Err(FossilError::InvalidArgs(format!(
                "artifact {:?} {} analysis output",
                self.name,
                if self.analysis_name().is_some() {
                    "requires"
                } else {
                    "does not consume"
                }
            )));
        }
        let script = self.entry.script.resolve(&fossil.path);
        let destination = self.output_path(fossil, project)?;
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = columns.map(analysis::columns_to_json).transpose()?;
        let mut command = Command::new(&script);
        ExecutionContext::new(project, fossil, Operation::Artifact(self.name))
            .configure(&mut command);
        command
            .arg(&destination)
            .current_dir(&fossil.path)
            .stdin(if json.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if force {
            command.env("FOSSIL_FORCE", "1");
        }
        let fail = |reason: String| {
            FossilError::InvalidConfig(format!(
                "artifact script {} failed: {reason}",
                script.display()
            ))
        };
        let mut child = command.spawn().map_err(|e| fail(e.to_string()))?;
        let written = match (json, child.stdin.take()) {
            (Some(json), Some(mut stdin)) => {
                std::io::Write::write_all(&mut stdin, json.as_bytes())
            }
            (Some(_), None) => Err(std::io::Error::other("missing stdin pipe")),
            (None, _) => Ok(()),
        };
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(fail(format!(
                "{}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        written?;
        if !destination.is_file() {
            return Err(fail(format!(
                "did not produce {}",
                destination.display()
            )));
        }
        Ok(destination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{commands, fossil::FossilConfig, runner::OutputMode};
    use std::{fs, os::unix::fs::PermissionsExt};

    const CONFIG: &str = include_str!("../tests/fixtures/fossil.toml");
    const PROJECT: &str = include_str!("../tests/fixtures/project.toml");
    struct Fixture {
        project: Project,
        fossil: Fossil,
    }
    impl Fixture {
        fn new(analyzed: bool) -> Self {
            let nonce = chrono::Utc::now().timestamp_nanos_opt().unwrap();
            let root = std::env::temp_dir().join(format!("fossil-{nonce}"));
            let project = Project {
                config: toml::from_str(PROJECT).unwrap(),
                path: root.clone(),
            };
            let mut config: FossilConfig = toml::from_str(CONFIG).unwrap();
            config.artifacts.get_mut("summary").unwrap().analysis =
                analyzed.then(|| "measure".into());
            let fossil = Fossil {
                config,
                path: root.join("3-3-experiment"),
            };
            fs::create_dir_all(fossil.records_dir()).unwrap();
            fs::write(fossil.path.join("fossil.toml"), CONFIG).unwrap();
            let fixture = Self { project, fossil };
            fixture.script("run.sh", "true");
            fixture
        }
        fn script(&self, name: &str, body: &str) {
            let path = self.fossil.path.join(name);
            fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        fn emit(&self, force: bool) -> Result<PathBuf, FossilError> {
            let (f, p) = (&self.fossil, &self.project);
            commands::emit_artifact(f, p, None, None, None, force)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.project.path).unwrap();
        }
    }

    #[test]
    fn static_artifact_protocol_needs_no_records() {
        let f = Fixture::new(false);
        f.script("emit.sh", include_str!("../tests/fixtures/static.sh"));
        let output = fs::read_to_string(f.emit(true).unwrap()).unwrap();
        assert_eq!(output, r#"{"static":true}"#);
    }

    #[test]
    fn analyzed_artifact_computes_and_receives_metrics() {
        let fixture = Fixture::new(true);
        fixture.script("analyze.sh", r#"cat >/dev/null; printf '{"value":4}'"#);
        fixture.script("emit.sh", "cat >\"$1\"");
        let (f, p) = (&fixture.fossil, &fixture.project);
        let tasks = f.resolve_bury_tasks(&[]).unwrap();
        commands::bury(f, p, Some(2), tasks, OutputMode::Quiet).unwrap();
        let output = fs::read_to_string(fixture.emit(false).unwrap()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(
            value["baseline"]["value"],
            serde_json::json!({"mean":4.0,"stddev":0.0})
        );
    }

    #[test]
    fn execution_errors_are_reported() {
        let fixture = Fixture::new(false);
        for (script, error) in [
            ("echo broken >&2; exit 7", "broken"),
            ("true", "did not produce"),
        ] {
            fixture.script("emit.sh", script);
            assert!(
                fixture
                    .emit(false)
                    .unwrap_err()
                    .to_string()
                    .contains(error)
            );
        }
    }
}
