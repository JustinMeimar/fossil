pub mod cli;
pub mod fossil;
pub mod utils;
pub mod config;
pub mod tui;

use cli::{CLIArgs, Actions};
use std::path::PathBuf;
use std::env;
use tui::list::ListApp;
use tui::{setup_terminal, cleanup_terminal};
use config::load_config;

fn run_fossil() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let mut terminal = setup_terminal()?;
    let mut app = ListApp::new(config);
    
    let result = app.run(&mut terminal);
    cleanup_terminal(terminal)?;
    
    result
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 { 
        match run_fossil() {
            Ok(()) => {},
            Err(e) => eprintln!("Error running TUI: {}", e),
        }
        return;
    }
    
    let action = match args[1].as_str() {
        "init" => Actions::Init,
        "track" => Actions::Track,
        "burry" => Actions::Burry,
        "dig" => Actions::Dig,
        "surface" => Actions::Surface,
        "list" => Actions::List,
        _ => {
            eprintln!("Invalid action. See fossil --help");
            return;
        }
    };
    
    let cli_args = CLIArgs {
        fossil_config: PathBuf::from(".fossil"),
        action,
    };
    
    match cli_args.action {
        Actions::Init => {
            match fossil::init() {
                Ok(()) => println!("Fossil repository initialized successfully"),
                Err(e) => eprintln!("Error initializing repository: {}", e),
            }
        },
        Actions::Track => {
            if args.len() < 3 {
                eprintln!("Usage: fossil track <files...>");
                return;
            }
            let files = args[2..].to_vec();
            match fossil::track(files) {
                Ok(()) => println!("Files tracked successfully"),
                Err(e) => eprintln!("Error tracking files: {}", e),
            }
        },
        Actions::Burry => {
            let files = if args.len() > 2 {
                Some(args[2..].to_vec())
            } else {
                None
            };
            match fossil::burry(files) {
                Ok(()) => {},
                Err(e) => eprintln!("Error burrying files: {}", e),
            }
        },
        Actions::Dig => {
            if args.len() < 3 {
                eprintln!("Usage: fossil dig <depth>");
                return;
            }
            let layer_str = &args[2];
            let layer = match layer_str.parse::<u32>() {
                Ok(d) => d,
                Err(_) => {
                    eprintln!("Error: depth must be a positive integer");
                    return;
                }
            };
            match fossil::dig(layer) {
                Ok(()) => {},
                Err(e) => eprintln!("Error digging: {}", e),
            }
        },
        Actions::Surface => {
            match fossil::surface() {
                Ok(()) => {},
                Err(e) => eprintln!("Error finding surface: {}", e),
            }
        }
        Actions::List => { 
            match fossil::list() {
                Ok(()) => {},
                Err(e) => eprintln!("Error listing fossils: {}", e),
            }
        },
    }
}

