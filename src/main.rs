pub mod cli;
pub mod fossil;
pub mod utils;
pub mod config;
pub mod tui;

use clap::Parser;
use cli::{Cli, Commands};
use tui::list::ListApp;
use tui::{setup_terminal, cleanup_terminal};
use config::load_config;

fn run_fossil_tui() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let mut terminal = setup_terminal()?;
    let mut app = ListApp::new(config);
    
    let result = app.run(&mut terminal);
    cleanup_terminal(terminal)?;
    
    result
}

fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        None => {
            // No subcommand provided, launch TUI
            match run_fossil_tui() {
                Ok(()) => {},
                Err(e) => eprintln!("Error running TUI: {}", e),
            }
        },
        Some(Commands::Init) => {
            match fossil::init() {
                Ok(()) => println!("Fossil repository initialized successfully"),
                Err(e) => eprintln!("Error initializing repository: {}", e),
            }
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
        },
        Some(Commands::Burry { tag, files }) => {
            let files_option = if files.is_empty() { None } else { Some(files) };
            
            match fossil::burry(files_option, tag) {
                Ok(()) => {},
                Err(e) => eprintln!("Error burrying files: {}", e),
            }
        },
        Some(Commands::Dig { layer }) => {
            match fossil::dig(layer) {
                Ok(()) => {},
                Err(e) => eprintln!("Error digging: {}", e),
            }
        },
        Some(Commands::Surface) => {
            match fossil::surface() {
                Ok(()) => {},
                Err(e) => eprintln!("Error finding surface: {}", e),
            }
        },
        Some(Commands::List) => { 
            match fossil::list() {
                Ok(()) => {},
                Err(e) => eprintln!("Error listing fossils: {}", e),
            }
        },
    }
}

