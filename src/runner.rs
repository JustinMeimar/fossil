use std::io::{BufRead, BufReader};
use std::process::Command;
use std::time::Instant;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Observation {
    pub iteration: u32,
    pub wall_time_us: u64,
    pub exit_code: i32,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
}

impl Observation {
    pub fn run(command: &str, iteration: u32) -> anyhow::Result<Self> {
        let mut cmd = Command::new("sh");
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
