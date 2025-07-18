pub mod cli;
pub mod config;
pub mod fossil;
pub mod utils;
pub mod tui;

use std::error;
use ::fossil::{fossil_log, fossil_error, enable_log, disable_log};
use clap::Parser;
use cli::{Cli, Commands};

pub fn dispatch_command(cmd: Option<Commands>) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        None => {
            disable_log();
            let result = tui::run_tui();
            enable_log();
            result?;
        }
        Some(Commands::Init) => {
            fossil::init()?;
        }
        Some(Commands::Track { files }) => {
            if files.is_empty() {
                return Err("No files specified to track".into());
            }
            fossil::track(files)?;
        }
        Some(Commands::Untrack { files }) => {
            if files.is_empty() {
                return Err("Must specify at least one file to untrack".into());
            }
            fossil::untrack(files)?;
        }
        Some(Commands::Bury { tag, files }) => {
            fossil::bury_files(files, tag)?
        }, 
        Some(Commands::Dig { tag, files, version }) => {
            fossil::dig_files(files, tag, version)?;
        } 
        Some(Commands::Surface) => {
            fossil::surface()?;
        } 
        Some(Commands::List) => {
            fossil::list()?;
        }
        Some(Commands::Reset) => {
            fossil_log!("Are you sure you want to reset all tracked fossils? (y/n)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => {
                    fossil::reset()?;
                    fossil_log!("Cleared all fossils.");
                }
                "n" | "no" => fossil_log!("Reset cancelled."),
                _ => fossil_log!("Invalid input. Reset cancelled."),
            }
        }
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    dispatch_command(cli.command);
}

