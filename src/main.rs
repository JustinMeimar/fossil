pub mod cli;
pub mod config;
pub mod fossil;
pub mod utils;

use ::fossil::{fossil_log, fossil_error, enable_log, disable_log};
use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            disable_log();
            // Start TUI application
            enable_log(); 
        }
        Some(Commands::Init) => match fossil::init() {
            Ok(()) =>fossil_log!("Fossil repository initialized successfully"),
            Err(e) => fossil_error!("Error initializing repository: {}", e),
        },
        Some(Commands::Track { files }) => {
            if files.is_empty() {
                fossil_error!("Error: No files specified to track");
                return;
            }
            match fossil::track(files) {
                Ok(()) =>fossil_log!("Files tracked successfully"),
                Err(e) => fossil_error!("Error tracking files: {}", e),
            }
        }
        Some(Commands::Untrack { files }) => {
            if files.is_empty() {
                fossil_error!("Error: Must specify at least one file to untrack.");
                return;
            }
            match fossil::untrack(files) {
                Ok(()) =>fossil_log!("Files untracked successfully"),
                Err(e) => fossil_error!("Error untracking files: {}", e),
            }
        }
        Some(Commands::Bury { tag, files }) => {
            match fossil::bury_files(files, tag) {
                Ok(()) => {}
                Err(e) => fossil_error!("Error burying files: {}", e),
            }
        }
        Some(Commands::Dig { tag, files, version }) => {
            match fossil::dig_files(files, tag, version) {
                Ok(()) => {}
                Err(e) => fossil_error!("Error digging files: {}", e),
            } 
        }
        Some(Commands::Surface) => match fossil::surface() {
            Ok(()) => {}
            Err(e) => fossil_error!("Error finding surface: {}", e),
        },
        Some(Commands::List) => match fossil::list() {
            Ok(()) => {}
            Err(e) => fossil_error!("Error listing fossils: {}", e),
        },
        Some(Commands::Reset) => {
           fossil_log!("Are you sure you want to reset all tracked fossils? (y/n)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap_or_default();
            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => match fossil::reset() {
                    Ok(()) => {
                       fossil_log!("Cleared all fossils.")
                    }
                    Err(e) => fossil_error!("Error listing fossils: {}", e),
                },
                "n" | "no" => {
                   fossil_log!("Reset cancelled.");
                }
                _ => {
                   fossil_log!("Invalid input. Reset cancelled.");
                }
            }
        }
    }
}
