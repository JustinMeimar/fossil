use std::path::Path;
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

fn lines(bytes: &[u8]) -> Vec<String> {
    let s = String::from_utf8_lossy(bytes);
    let trimmed = s.trim_end_matches('\n');
    if trimmed.is_empty() { return vec![]; }
    trimmed.split('\n').map(String::from).collect()
}

pub fn run_iteration(command: &str, cwd: &Path, iteration: u32) -> anyhow::Result<Observation> {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", command]);
    cmd.current_dir(cwd);

    let start = Instant::now();
    let output = cmd.output()?;
    let wall_time_us = start.elapsed().as_micros() as u64;

    Ok(Observation {
        iteration,
        wall_time_us,
        exit_code: output.status.code().unwrap_or(-1),
        stdout: lines(&output.stdout),
        stderr: lines(&output.stderr),
    })
}
