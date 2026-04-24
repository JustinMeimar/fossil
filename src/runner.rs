use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command as ProcessCommand;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::FossilError;

#[derive(Debug, Serialize, Deserialize)]
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

        let child_stdout = child.stdout.take().unwrap();
        let child_stderr = child.stderr.take().unwrap();

        let stdout_handle = std::thread::spawn(move || {
            let reader = BufReader::new(child_stdout);
            let mut lines = Vec::new();
            for line in reader.lines() {
                let line = line.unwrap_or_default();
                println!("{line}");
                lines.push(line);
            }
            lines
        });

        let stderr_handle = std::thread::spawn(move || {
            let reader = BufReader::new(child_stderr);
            let mut lines = Vec::new();
            for line in reader.lines() {
                let line = line.unwrap_or_default();
                eprintln!("{line}");
                lines.push(line);
            }
            lines
        });

        let stdout_lines = stdout_handle.join().unwrap_or_default();
        let stderr_lines = stderr_handle.join().unwrap_or_default();

        let status = child.wait()?;
        let wall_time_us = start.elapsed().as_micros() as u64;

        Ok(Self {
            iteration,
            wall_time_us,
            exit_code: status.code().unwrap_or(-1),
            stdout: stdout_lines,
            stderr: stderr_lines,
        })
    }
}

pub struct Run {
    pub command: String,
    pub iterations: u32,
    pub variant: Option<String>,
    pub allow_failure: bool,
    pub workdir: Option<String>,
    pub observations: Vec<Observation>,
}

impl Run {
    pub fn new(
        args: Vec<String>,
        iterations: u32,
        variant: Option<String>,
        allow_failure: bool,
        workdir: Option<String>,
    ) -> Result<Self, FossilError> {
        if args.is_empty() {
            return Err(FossilError::NoCommand);
        }
        Ok(Self {
            command: args.join(" "),
            iterations,
            variant,
            allow_failure,
            workdir,
            observations: Vec::new(),
        })
    }

    pub fn execute_one(&mut self) -> Result<&Observation, FossilError> {
        let i = self.observations.len() as u32 + 1;
        let workdir = self.workdir.as_ref().map(|s| Path::new(s.as_str()));
        let obs = Observation::run(&self.command, i, workdir)?;
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

    pub fn observations_json(&self) -> Value {
        let obs: Vec<Value> = self
            .observations
            .iter()
            .filter_map(|obs| serde_json::to_value(obs).ok())
            .collect();
        json!({ "observations": obs })
    }
}
