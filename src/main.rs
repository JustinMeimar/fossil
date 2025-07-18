use fossil::{dispatch_command, cli::Cli};
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = dispatch_command(cli.command) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

