use std::path::PathBuf;

use crate::analysis;
use crate::environment::{ExecutionContext, Operation};
use crate::error::FossilError;
use crate::fossil::{FigureEntry, Fossil};
use crate::project::Project;

pub enum FigureOutput {
    Pdf(PathBuf),
    Json(PathBuf),
}

impl FigureOutput {
    pub fn detect(pdf_path: &std::path::Path) -> Option<Self> {
        if pdf_path.is_file() {
            return Some(Self::Pdf(pdf_path.to_path_buf()));
        }

        let json_path = pdf_path.with_extension("json");
        json_path.is_file().then_some(Self::Json(json_path))
    }
}

pub struct Figure<'a> {
    pub name: &'a str,
    entry: &'a FigureEntry,
}

impl<'a> Figure<'a> {
    pub fn resolve(
        fossil: &'a Fossil,
        name: Option<&'a str>,
    ) -> Result<Self, FossilError> {
        let map = fossil.config.figures.as_ref().ok_or_else(|| {
            FossilError::NotFound(format!(
                "no figures configured for {:?}",
                fossil.config.name
            ))
        })?;

        let (chosen_name, entry) = match name {
            Some(n) => {
                let entry = map.get(n).ok_or_else(|| {
                    let names: Vec<&str> =
                        map.keys().map(|k| k.as_str()).collect();
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
                return Err(FossilError::InvalidArgs(format!(
                    "multiple figures available, use --figure: {}",
                    names.join(", ")
                )));
            }
        };

        Ok(Self {
            name: chosen_name,
            entry,
        })
    }

    pub fn analysis_name(&self) -> &str {
        self.entry.analysis.as_str()
    }

    pub fn output_path(
        &self,
        fossil: &Fossil,
        project: &Project,
    ) -> Result<PathBuf, FossilError> {
        project.artifact_path(format!("{}-{}.pdf", fossil.prefix(), self.name))
    }

    pub fn run(
        &self,
        fossil: &Fossil,
        project: &Project,
        columns: &[(String, analysis::Metric)],
        force: bool,
    ) -> Result<PathBuf, FossilError> {
        let json = analysis::columns_to_json(columns)?;

        let script_path = self.entry.script.resolve(&fossil.path);
        let out_path = self.output_path(fossil, project)?;

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut cmd = std::process::Command::new(&script_path);
        let context = ExecutionContext::new(
            project,
            fossil,
            Operation::Figure(self.name),
        );
        context.configure(&mut cmd);
        cmd.arg(&out_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&fossil.path);
        if force {
            cmd.env("FOSSIL_FORCE", "1");
        }
        let mut child = cmd.spawn().map_err(|e| {
            FossilError::InvalidConfig(format!(
                "figure script {} failed: {e} — is the script executable?",
                script_path.display()
            ))
        })?;

        let write_result = match child.stdin.take() {
            Some(mut stdin) => {
                std::io::Write::write_all(&mut stdin, json.as_bytes())
                    .map_err(FossilError::Io)
            }
            None => Err(FossilError::InvalidConfig(format!(
                "figure script {} has no stdin pipe",
                script_path.display()
            ))),
        };

        let output = child.wait_with_output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FossilError::InvalidConfig(format!(
                "figure script {} failed: {}",
                script_path.display(),
                stderr.trim(),
            )));
        }

        write_result?;
        Ok(out_path)
    }

    //NOTE(Justin): hardcode xgd-open since it works on my machine,
    //seek a more portable solution.
    pub fn open(path: &std::path::Path) {
        let _ = std::process::Command::new("xdg-open")
            .arg(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    pub fn edit(path: &std::path::Path) -> Result<(), FossilError> {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
        let status = std::process::Command::new(&editor)
            .arg(path)
            .status()
            .map_err(|e| {
                FossilError::InvalidConfig(format!(
                    "failed to run {editor}: {e}"
                ))
            })?;

        if status.success() {
            Ok(())
        } else {
            Err(FossilError::InvalidConfig(format!(
                "{editor} exited with {}",
                status.code().unwrap_or(-1)
            )))
        }
    }
}
