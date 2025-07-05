pub mod cli;
pub mod fossil;

use cli::{CLIArgs, Actions};
use std::path::PathBuf;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: fossil <action>");
        return;
    }
    
    let action = match args[1].as_str() {
        "init" => Actions::Init,
        "track" => Actions::Track,
        "burry" => Actions::Burry,
        "dig" => Actions::Dig,
        "list" => Actions::List,
        _ => {
            eprintln!("Unknown action: {}", args[1]);
            return;
        }
    };
    
    let cli_args = CLIArgs {
        fossil_config: PathBuf::from(".fossil"),
        action,
    };
    
    match cli_args.action {
        Actions::Init => println!("Initializing fossil repository..."),
        Actions::Track => println!("Tracking files..."),
        Actions::Burry => println!("Burrying files..."),
        Actions::Dig => println!("Digging up files..."),
        Actions::List => println!("Listing artifacts..."),
    }
}

