use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

use crate::config::load_config;
use crate::tui::app::App;
use crate::tui::events::{AppEvent, handle_events};

pub mod app;
pub mod events;
pub mod ui;

pub type CrosstermTerminal = Terminal<CrosstermBackend<io::Stdout>>;

pub fn setup_terminal() -> Result<CrosstermTerminal, Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

pub fn cleanup_terminal(mut terminal: CrosstermTerminal) -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let mut app = App::new(config);
    let mut terminal = setup_terminal()?;

    let result = run_app(&mut terminal, &mut app);

    cleanup_terminal(terminal)?;

    result
}

fn run_app(
    terminal: &mut CrosstermTerminal,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| ui::render(app, f))?;

        if app.should_quit {
            break;
        }

        if let Some(event) = handle_events()? {
            handle_app_event(app, event)?;
        }
    }

    Ok(())
}

fn handle_app_event(app: &mut App, event: AppEvent) -> Result<(), Box<dyn std::error::Error>> {
    match event {
        AppEvent::Quit => app.quit(),

        // Navigation (only in normal mode)
        AppEvent::Up => {
            if app.input_mode == app::InputMode::Normal {
                app.previous();
            }
        }
        AppEvent::Down => {
            if app.input_mode == app::InputMode::Normal {
                app.next();
            }
        }
        AppEvent::Home => {
            if app.input_mode == app::InputMode::Normal {
                app.goto_first();
            }
        }
        AppEvent::End => {
            if app.input_mode == app::InputMode::Normal {
                app.goto_last();
            }
        }
        AppEvent::PageUp => {
            if app.input_mode == app::InputMode::Normal {
                app.page_up();
            }
        }
        AppEvent::PageDown => {
            if app.input_mode == app::InputMode::Normal {
                app.page_down();
            }
        }

        // File operations (only in normal mode)
        AppEvent::TrackFile => {
            if app.input_mode == app::InputMode::Normal {
                app.track_selected()?;
            }
        }
        AppEvent::BuryAll => {
            if app.input_mode == app::InputMode::Normal {
                app.bury_all()?;
            }
        }
        AppEvent::BuryWithTag => {
            if app.input_mode == app::InputMode::Normal {
                app.start_tag_input();
            }
        }
        AppEvent::Surface => {
            if app.input_mode == app::InputMode::Normal {
                app.surface()?;
            }
        }
        AppEvent::Refresh => {
            if app.input_mode == app::InputMode::Normal {
                app.refresh()?;
            }
        }

        // Selection (only in normal mode)
        AppEvent::ToggleSelect => {
            if app.input_mode == app::InputMode::Normal {
                app.toggle_selection();
            }
        }
        AppEvent::SelectAll => {
            if app.input_mode == app::InputMode::Normal {
                app.select_all();
            }
        }
        AppEvent::DeselectAll => {
            if app.input_mode == app::InputMode::Normal {
                app.deselect_all();
            }
        }

        // Layer operations (only in normal mode)
        AppEvent::QuickDig(layer) => {
            if app.input_mode == app::InputMode::Normal && app.layers.contains(&layer) {
                app.dig_to_layer(layer)?;
            }
        }
        AppEvent::DigByTag => {
            if app.input_mode == app::InputMode::Normal {
                app.start_tag_dig_input();
            }
        }

        // View operations (only in normal mode)
        AppEvent::TogglePreview => {
            if app.input_mode == app::InputMode::Normal {
                app.toggle_preview();
            }
        }
        AppEvent::ToggleHelp => {
            if app.input_mode == app::InputMode::Normal {
                app.toggle_help();
            }
        }

        // Command mode (only in normal mode)
        AppEvent::CommandMode => {
            if app.input_mode == app::InputMode::Normal {
                app.start_command_mode();
            }
        }

        // Input handling
        AppEvent::Char(c) => app.handle_char_input(c),
        AppEvent::Enter => app.handle_enter()?,
        AppEvent::Escape => app.handle_escape(),
        AppEvent::Backspace => app.handle_backspace(),

        // Ignore other events
        _ => {}
    }

    Ok(())
}
