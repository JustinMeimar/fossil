mod app;
pub mod theme;
mod views;
use crate::error::FossilError;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use std::path::PathBuf;

pub fn run(fossil_home: PathBuf) -> Result<(), FossilError> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stderr(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        original_hook(panic);
    }));

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = ratatui::DefaultTerminal::new(
        ratatui::backend::CrosstermBackend::new(stdout),
    )?;

    let mut app = app::App::new(fossil_home)?;
    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(std::io::stderr(), LeaveAlternateScreen, DisableMouseCapture)?;

    result
}
