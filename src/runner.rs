use std::io::{BufRead, BufReader};
use std::process::Command as ProcessCommand;
use std::time::Instant;

use serde::Serialize;
use serde_json::{json, Value};

use crate::ui::status;

#[derive(Debug, Serialize)]
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
    pub tag: Option<String>,
    pub observations: Vec<Observation>,
}

impl Run {
    pub fn new(args: Vec<String>, iterations: u32, tag: Option<String>) -> anyhow::Result<Self> {
        if args.is_empty() {
            anyhow::bail!("no command given — usage: fossil bury <name> -- <cmd...>");
        }
        Ok(Self {
            command: args.join(" "),
            iterations,
            tag,
            observations: Vec::new(),
        })
    }

    pub fn execute(&mut self, fossil_name: &str) -> anyhow::Result<()> {
        for i in 1..=self.iterations {
            status!("burying {}/{} ({i}/{})",
                fossil_name,
                self.tag.as_deref().unwrap_or("untagged"),
                self.iterations,
            );
            let obs = Observation::run(&self.command, i)?;
            if obs.exit_code != 0 {
                anyhow::bail!("command failed on iteration {i} (exit {})", obs.exit_code);
            }
            status!("{}ms", obs.wall_time_us / 1000);
            self.observations.push(obs);
        }
        Ok(())
    }

    pub fn results(&self, fossil_name: &str) -> Value {
        let observations: Vec<Value> = self.observations.iter()
            .map(|obs| serde_json::to_value(obs).unwrap())
            .collect();
        json!({
            "fossil": fossil_name,
            "observations": observations,
        })
    }
}
