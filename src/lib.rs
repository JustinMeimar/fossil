pub mod config;
pub mod fossil;
pub mod utils;
pub mod tui;

use std::sync::atomic::{AtomicBool, Ordering};
pub static ENABLE_LOG: AtomicBool = AtomicBool::new(true);

pub fn enable_log() {
    ENABLE_LOG.store(true, Ordering::Relaxed);
}

pub fn disable_log() {
    ENABLE_LOG.store(false, Ordering::Relaxed);
}

#[macro_export]
macro_rules! fossil_log {
   ($($arg:tt)*) => {
        if ($crate::ENABLE_LOG.load(std::sync::atomic::Ordering::Relaxed)) {
            println!("{}", format!($($arg)*));
        }
   };
}

#[macro_export]
macro_rules! fossil_error {
   ($($arg:tt)*) => {
        if ($crate::ENABLE_LOG.load(std::sync::atomic::Ordering::Relaxed)) {
            eprintln!("[ERROR] {}", format!($($arg)*));
        } 
   };
}

