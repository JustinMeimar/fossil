use std::io::{self, BufRead, Write};

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

pub fn pick<'a>(prompt: &str, options: &'a [&str]) -> Option<&'a str> {
    let stderr = io::stderr();
    let mut err = stderr.lock();
    let _ = writeln!(err, "{prompt}");
    for (i, opt) in options.iter().enumerate() {
        let _ = writeln!(err, "  [{i}] {opt}");
    }
    let _ = write!(err, "  > ");
    let _ = err.flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).ok()?;
    let idx: usize = line.trim().parse().ok()?;
    options.get(idx).copied()
}
