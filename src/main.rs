pub mod cli;
pub mod config;
pub mod fossil;
// pub mod tui;
pub mod utils;

use clap::Parser;
use cli::{Cli, Commands};

fn run_fossil_tui() -> Result<(), Box<dyn std::error::Error>> {
    // tui::run_tui()
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            // No subcommand provided, launch TUI
            match run_fossil_tui() {
                Ok(()) => {}
                Err(e) => eprintln!("Error running TUI: {}", e),
            }
        }
        Some(Commands::Init) => match fossil::init() {
            Ok(()) => println!("Fossil repository initialized successfully"),
            Err(e) => eprintln!("Error initializing repository: {}", e),
        },
        Some(Commands::Track { files }) => {
            if files.is_empty() {
                eprintln!("Error: No files specified to track");
                return;
            }
            match fossil::track(files) {
                Ok(()) => println!("Files tracked successfully"),
                Err(e) => eprintln!("Error tracking files: {}", e),
            }
        }
        Some(Commands::Untrack { files }) => {
            if files.is_empty() {
                eprintln!("Error: Must specify at least one file to untrack.");
                return;
            }
            match fossil::untrack(files) {
                Ok(()) => println!("Files untracked successfully"),
                Err(e) => eprintln!("Error untracking files: {}", e),
            }
        }
        Some(Commands::Bury { tag, files }) => {
            match fossil::bury_files(files, tag) {
                Ok(()) => {}
                Err(e) => eprintln!("Error burying files: {}", e),
            }
        }
        Some(Commands::Dig { tag, files, version }) => {
            match fossil::dig_files(files, tag, version) {
                Ok(()) => {}
                Err(e) => eprintln!("Error digging files: {}", e),
            } 
        }
        Some(Commands::Surface) => match fossil::surface() {
            Ok(()) => {}
            Err(e) => eprintln!("Error finding surface: {}", e),
        },
        Some(Commands::List) => match fossil::list() {
            Ok(()) => {}
            Err(e) => eprintln!("Error listing fossils: {}", e),
        },
        Some(Commands::Reset) => {
            println!("Are you sure you want to reset all tracked fossils? (y/n)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap_or_default();
            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => match fossil::reset() {
                    Ok(()) => {
                        println!("Cleared all fossils.")
                    }
                    Err(e) => eprintln!("Error listing fossils: {}", e),
                },
                "n" | "no" => {
                    println!("Reset cancelled.");
                }
                _ => {
                    println!("Invalid input. Reset cancelled.");
                }
            }
        }
    }
}
