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
