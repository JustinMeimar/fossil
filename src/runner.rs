use std::io::{BufRead, BufReader};
use std::process::Command as ProcessCommand;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Serialize, Deserialize)]
pub struct Observation {
    pub iteration: u32,
    pub wall_time_us: u64,
    pub exit_code: i32,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
}

impl Observation {
    fn run(command: &str, iteration: u32) -> anyhow::Result<Self> {
        let mut cmd = ProcessCommand::new("sh");
        cmd.args(["-c", command]);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

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
    pub observations: Vec<Observation>,
}

impl Run {
    pub fn new(
        args: Vec<String>,
        iterations: u32,
        variant: Option<String>,
    ) -> anyhow::Result<Self> {
        if args.is_empty() {
            anyhow::bail!(
                "no command given — usage: fossil bury <name> -- <cmd...>"
            );
        }
        Ok(Self {
            command: args.join(" "),
            iterations,
            variant,
            observations: Vec::new(),
        })
    }

    pub fn execute_one(&mut self) -> anyhow::Result<&Observation> {
        let i = self.observations.len() as u32 + 1;
        let obs = Observation::run(&self.command, i)?;
        if obs.exit_code != 0 {
            anyhow::bail!(
                "command failed on iteration {i} (exit {})",
                obs.exit_code
            );
        }
        self.observations.push(obs);
        Ok(self.observations.last().unwrap())
    }

    pub fn observations_json(&self) -> Value {
        json!({
            "observations": self.observations.iter()
                .map(|obs| serde_json::to_value(obs).unwrap())
                .collect::<Vec<Value>>(),
        })
    }
}
