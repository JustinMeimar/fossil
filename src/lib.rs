pub mod config;
pub mod fossil;
pub mod utils;
pub mod tui;

use std::sync::atomic::{AtomicBool, Ordering};
static ENABLE_LOG: AtomicBool = AtomicBool::new(true);

pub fn enable_log() {
    ENABLE_LOG.store(true, Ordering::Relaxed);
}

pub fn disable_log() {
    ENABLE_LOG.store(false, Ordering::Relaxed);
}

#[macro_export]
macro_rules! fossil_log {
   ($($arg:tt)*) => {
       println!("{}", format!($($arg)*));
   };
}

#[macro_export]
macro_rules! fossil_error {
   ($($arg:tt)*) => {
       eprintln!("[ERROR] {}", format!($($arg)*));
   };
}

