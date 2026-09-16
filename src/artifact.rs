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
    use crate::commands;
    use crate::fossil::FossilConfig;
    use crate::runner::OutputMode;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture {
        root: PathBuf,
        project: Project,
        fossil: Fossil,
    }

    impl Fixture {
        fn new(analysis: bool) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "fossil-artifact-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let fossil_dir = root.join("3-3-experiment");
            std::fs::create_dir_all(fossil_dir.join("records")).unwrap();
            let project = Project {
                config: toml::from_str("name = 'project'\nartifact_dir = 'artifacts'\n[constants]\nVALUE = 'present'").unwrap(),
                path: root.clone(),
            };
            let analysis_line =
                if analysis { "analysis = 'measure'" } else { "" };
            let config: FossilConfig = toml::from_str(&format!(
                "name = '3-3-experiment'\n[variants]\nbaseline = 'true'\n[analyze]\nmeasure = 'analyze.sh'\n[artifacts.summary]\nscript = 'emit.sh'\nformat = 'json'\n{analysis_line}"
            )).unwrap();
            std::fs::write(
                fossil_dir.join("fossil.toml"),
                toml::to_string(&config).unwrap(),
            )
            .unwrap();
            Self {
                root,
                project,
                fossil: Fossil {
                    config,
                    path: fossil_dir,
                },
            }
        }

        fn script(&self, name: &str, body: &str) {
            use std::os::unix::fs::PermissionsExt;
            let path = self.fossil.path.join(name);
            std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n"))
                .unwrap();
            std::fs::set_permissions(
                path,
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn config_requires_format_and_rejects_legacy_sections() {
        assert!(
            toml::from_str::<FossilConfig>("[artifacts.a]\nscript = 'a.sh'")
                .is_err()
        );
        for section in ["figures", "tables", "visualize"] {
            let error = toml::from_str::<FossilConfig>(&format!("[{section}]"))
                .unwrap_err();
            assert!(error.to_string().contains(section));
            assert!(error.to_string().contains("artifacts"));
        }
        assert!(
            toml::from_str::<FossilConfig>("name = 'empty'")
                .unwrap()
                .artifacts
                .is_empty()
        );
    }

    #[test]
    fn selection_and_output_paths() {
        let mut fixture = Fixture::new(false);
        assert!(Artifact::resolve(&fixture.fossil, Some("missing")).is_err());
        let artifact = Artifact::resolve(&fixture.fossil, None).unwrap();
        assert_eq!(
            artifact
                .output_path(&fixture.fossil, &fixture.project)
                .unwrap(),
            fixture
                .root
                .join("artifacts/3-3-experiment-summary.json")
        );
        fixture.project.config.artifact_dir =
            Some(fixture.root.join("absolute"));
        fixture
            .fossil
            .config
            .artifacts
            .get_mut("summary")
            .unwrap()
            .format = ArtifactFormat::Pdf;
        let artifact = Artifact::resolve(&fixture.fossil, None).unwrap();
        assert_eq!(
            artifact
                .output_path(&fixture.fossil, &fixture.project)
                .unwrap(),
            fixture
                .root
                .join("absolute/3-3-experiment-summary.pdf")
        );
        let entry = fixture.fossil.config.artifacts["summary"].clone();
        fixture
            .fossil
            .config
            .artifacts
            .insert("other".into(), entry);
        assert!(Artifact::resolve(&fixture.fossil, None).is_err());
        fixture.fossil.config.artifacts.clear();
        assert!(Artifact::resolve(&fixture.fossil, None).is_err());
    }

    #[test]
    fn static_artifact_protocol_needs_no_records() {
        let fixture = Fixture::new(false);
        fixture.script(
            "emit.sh",
            r#"
[ "$PWD" = "$FOSSIL_PROJECT_DIR/3-3-experiment" ]
[ "$FOSSIL_NAME" = '3-3-experiment' ]
[ "$FOSSIL_ARTIFACT_NAME" = 'summary' ]
[ "$FOSSIL_CONST_VALUE" = 'present' ]
[ "$FOSSIL_FORCE" = '1' ]
[ -z "$(cat)" ]
printf '{"static":true}' > "$1"
"#,
        );
        let path = commands::emit_artifact(
            &fixture.fossil,
            &fixture.project,
            None,
            None,
            None,
            true,
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{\"static\":true}");
    }

    #[test]
    fn analyzed_artifact_computes_and_receives_metrics() {
        let fixture = Fixture::new(true);
        fixture.script("analyze.sh", "cat >/dev/null\nprintf '{\"value\":4}'");
        fixture.script("emit.sh", "cat >\"$1\"");
        let tasks = fixture
            .fossil
            .resolve_bury_tasks(&[], &fixture.project.config.project_scope())
            .unwrap();
        commands::bury(
            &fixture.fossil,
            &fixture.project,
            Some(2),
            tasks,
            OutputMode::Quiet,
        )
        .unwrap();
        let path = commands::emit_artifact(
            &fixture.fossil,
            &fixture.project,
            None,
            Some("baseline"),
            None,
            false,
        )
        .unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap())
                .unwrap();
        assert_eq!(value["baseline"]["value"]["mean"], 4.0);
        assert_eq!(value["baseline"]["value"]["stddev"], 0.0);
    }

    #[test]
    fn execution_errors_are_reported() {
        let fixture = Fixture::new(false);
        fixture.script("emit.sh", "echo broken >&2\nexit 7");
        let error = commands::emit_artifact(
            &fixture.fossil,
            &fixture.project,
            None,
            None,
            None,
            false,
        )
        .unwrap_err();
        assert!(error.to_string().contains("broken"));
        fixture.script("emit.sh", "true");
        let error = commands::emit_artifact(
            &fixture.fossil,
            &fixture.project,
            None,
            None,
            None,
            false,
        )
        .unwrap_err();
        assert!(error.to_string().contains("did not produce"));
        let artifact = Artifact::resolve(&fixture.fossil, None).unwrap();
        assert!(
            artifact
                .run(&fixture.fossil, &fixture.project, Some(&[]), false)
                .is_err()
        );
        let analyzed = Fixture::new(true);
        let artifact = Artifact::resolve(&analyzed.fossil, None).unwrap();
        assert!(
            artifact
                .run(&analyzed.fossil, &analyzed.project, None, false)
                .is_err()
        );
    }
}
