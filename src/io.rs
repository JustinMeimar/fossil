macro_rules! status {
    ($($arg:tt)*) => {
        eprintln!("[fossil] {}", format_args!($($arg)*))
    };
}
pub(crate) use status;

macro_rules! error {
    ($($arg:tt)*) => {
        eprintln!("error: {}", format_args!($($arg)*))
    };
}
pub(crate) use error;

macro_rules! output {
    ($($arg:tt)*) => {
        println!($($arg)*)
    };
}
pub(crate) use output;

use crate::error::FossilError;

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
            FossilError::InvalidConfig(format!("failed to run {editor}: {e}"))
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

use std::io::{self, Write};
use std::process::{Command, Output, Stdio};

/// Run a configured command with optional input and capture both output streams.
/// Writing stdin must overlap with draining stdout/stderr: scripts may produce
/// output before consuming their input. Always close stdin and reap the child,
/// including when it stops reading early.
pub fn command_output(
    command: &mut Command,
    input: Option<&[u8]>,
) -> io::Result<Output> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let Some(mut stdin) = child.stdin.take() else {
        return child.wait_with_output();
    };

    std::thread::scope(|scope| {
        let writer = match std::thread::Builder::new()
            .spawn_scoped(scope, move || stdin.write_all(input.unwrap()))
        {
            Ok(writer) => writer,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        // wait_with_output drains stdout and stderr concurrently.
        let output = child.wait_with_output();
        let written = writer.join().map_err(|_| {
            io::Error::other("subprocess stdin writer panicked")
        })?;
        let output = output?;
        // Preserve a failed script's exit status and stderr even if its early
        // exit also broke the input pipe. Successful scripts must accept input.
        if output.status.success() {
            written?;
        }
        Ok(output)
    })
}
