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
