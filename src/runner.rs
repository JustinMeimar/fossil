use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::Command as ProcessCommand;
use std::time::Instant;
use serde::{Deserialize, Serialize};
use crate::error::FossilError;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResultsFile {
    pub observations: Vec<Observation>,
}

/// [Fossil Doc] `Observation`
/// -------------------------------------------------------------
/// A single iteration of running the command. Captures stdout,
/// stderr, exit code, and wall time. A Record contains many of
/// these, one per iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub iteration: u32,
    pub wall_time_us: u64,
    pub exit_code: i32,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
}

impl Observation {
    fn run(
        command: &str,
        iteration: u32,
        workdir: Option<&Path>,
        silent: bool,
    ) -> Result<Self, FossilError> {
        let mut cmd = ProcessCommand::new("sh");
        cmd.args(["-c", command]);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if let Some(dir) = workdir {
            cmd.current_dir(dir);
        }

        let start = Instant::now();
        let mut child = cmd.spawn()?;

        let echo = !silent;
        let stdout_handle =
            drain_lines(child.stdout.take().unwrap(), echo, false);
        let stderr_handle =
            drain_lines(child.stderr.take().unwrap(), echo, true);

        let status = child.wait()?;
        let wall_time_us = start.elapsed().as_micros() as u64;

        Ok(Self {
            iteration,
            wall_time_us,
            exit_code: status.code().unwrap_or(-1),
            stdout: stdout_handle.join().unwrap_or_default(),
            stderr: stderr_handle.join().unwrap_or_default(),
        })
    }
}

/// [Fossil Doc] `Run`
/// -------------------------------------------------------------
/// An in-progress execution. Holds the command, config, and the
/// observations collected so far. Once finished, a Run becomes
/// a Record on disk.
pub struct Run {
    pub command: String,
    pub iterations: u32,
    pub variant: Option<crate::fossil::VariantName>,
    pub allow_failure: bool,
    pub workdir: Option<String>,
    pub silent: bool,
    pub observations: Vec<Observation>,
}

impl Run {
    pub fn new(
        command: String,
        iterations: u32,
        variant: Option<crate::fossil::VariantName>,
        allow_failure: bool,
        workdir: Option<String>,
        silent: bool,
    ) -> Result<Self, FossilError> {
        if command.is_empty() {
            return Err(FossilError::InvalidArgs(
                "no command given — usage: fossil bury <name> -- <cmd...>".into(),
            ));
        }
        Ok(Self {
            command,
            iterations,
            variant,
            allow_failure,
            workdir,
            silent,
            observations: Vec::new(),
        })
    }

    pub fn execute_one(&mut self) -> Result<&Observation, FossilError> {
        let i = self.observations.len() as u32 + 1;
        let workdir = self.workdir.as_ref().map(|s| Path::new(s.as_str()));
        let obs = Observation::run(&self.command, i, workdir, self.silent)?;
        if obs.exit_code != 0 && !self.allow_failure {
            return Err(FossilError::CommandFailed {
                command: self.command.clone(),
                iteration: i,
                exit_code: obs.exit_code,
            });
        }
        self.observations.push(obs);
        Ok(self.observations.last().unwrap())
    }

    pub fn results_file(&self) -> ResultsFile {
        ResultsFile {
            observations: self.observations.clone(),
        }
    }
}

fn drain_lines(
    stream: impl Read + Send + 'static,
    echo: bool,
    to_stderr: bool,
) -> std::thread::JoinHandle<Vec<String>> {
    std::thread::spawn(move || {
        BufReader::new(stream)
            .lines()
            .map(|l| l.unwrap_or_default())
            .inspect(|l| {
                if echo {
                    if to_stderr {
                        eprintln!("{l}");
                    } else {
                        println!("{l}");
                    }
                }
            })
            .collect()
    })
}

