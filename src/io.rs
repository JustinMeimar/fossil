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

/// Discover regular artifact files, relative to their output directory.
/// Symlinks are excluded so generated links cannot expose unrelated files.
pub fn artifact_files(
    root: &std::path::Path,
) -> io::Result<Vec<std::path::PathBuf>> {
    fn collect(
        root: &std::path::Path,
        dir: &std::path::Path,
        files: &mut Vec<std::path::PathBuf>,
    ) -> io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                collect(root, &entry.path(), files)?;
            } else if kind.is_file() {
                files.push(
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                );
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    match std::fs::symlink_metadata(root) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(files);
        }
        Err(error) => return Err(error),
        Ok(metadata) if !metadata.is_dir() => return Ok(files),
        Ok(_) => {}
    }
    collect(root, root, &mut files)?;
    files.sort();
    Ok(files)
}
