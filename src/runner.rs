use crate::environment::ExecutionContext;
use crate::error::FossilError;
use crate::fossil::FossilVariantKey;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Instant;

#[derive(Clone, Copy)]
pub enum OutputMode {
    Verbose,
    ProgressOnly,
    Quiet,
}

impl OutputMode {
    pub fn echoes_command(self) -> bool {
        matches!(self, Self::Verbose)
    }

    pub fn shows_progress(self) -> bool {
        !matches!(self, Self::Quiet)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Results {
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
        context: &ExecutionContext,
        output_mode: OutputMode,
    ) -> Result<Self, FossilError> {
        let mut cmd = ProcessCommand::new("sh");
        cmd.args(["-c", command]);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        context.configure(&mut cmd);
        if let Some(dir) = workdir {
            cmd.current_dir(dir);
        }

        let start = Instant::now();
        let mut child = cmd.spawn()?;

        let echo = output_mode.echoes_command();
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
    pub variant: Option<FossilVariantKey>,
    pub allow_failure: bool,
    pub workdir: Option<PathBuf>,
    pub context: ExecutionContext,
    pub output_mode: OutputMode,
    pub observations: Vec<Observation>,
}

impl Run {
    pub fn execute_one(&mut self) -> Result<&Observation, FossilError> {
        let i = self.observations.len() as u32 + 1;
        let workdir = self.workdir.as_deref();
        let obs = Observation::run(
            &self.command,
            i,
            workdir,
            &self.context,
            self.output_mode,
        )?;
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

    pub fn results(&self) -> Results {
        Results {
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
