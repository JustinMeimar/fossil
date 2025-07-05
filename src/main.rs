pub mod cli;
pub mod config;
pub mod fossil;
pub mod tui;
pub mod utils;

use clap::Parser;
use cli::{Cli, Commands};

fn run_fossil_tui() -> Result<(), Box<dyn std::error::Error>> {
    tui::run_tui()
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
            let tag_string = tag.unwrap_or_default();
            match fossil::bury_files(files, tag_string) {
                Ok(()) => {}
                Err(e) => eprintln!("Error burying files: {}", e),
            }
        }
        Some(Commands::Dig { layer, tag, files }) => {
            match (layer, tag, files.is_empty()) {
                (Some(layer), None, true) => {
                    // Dig by layer
                    match fossil::dig_by_layer(layer) {
                        Ok(()) => {}
                        Err(e) => eprintln!("Error digging by layer: {}", e),
                    }
                }
                (None, Some(tag), true) => {
                    // Dig by tag
                    match fossil::dig_by_tag(&tag) {
                        Ok(()) => {}
                        Err(e) => eprintln!("Error digging by tag: {}", e),
                    }
                }
                (Some(layer), None, false) => {
                    // Dig by files
                    match fossil::dig_by_files(layer, &files) {
                        Ok(()) => {}
                        Err(e) => eprintln!("Error digging by files: {}", e),
                    }
                }
                _ => {
                    eprintln!("Error: Must specify one of --layer, --tag, or --files");
                }
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
