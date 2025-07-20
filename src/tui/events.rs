use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use super::app::{App, AppMode};

pub fn handle_events(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            handle_key_event(app, key)?;
        }
    }
    
    if app.should_auto_refresh() {
        app.refresh_data();
    }
    
    Ok(())
}

fn handle_key_event(app: &mut App, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
    match app.mode {
        AppMode::Normal => handle_normal_mode(app, key),
        AppMode::Command => handle_command_mode(app, key),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Char('b') => {
            app.enter_bury_mode();
        }
        KeyCode::Char('d') => {
            app.enter_dig_mode();
        }
        KeyCode::Char('s') => {
            app.execute_surface();
        }
        KeyCode::Char('t') => {
            app.execute_track();
        }
        KeyCode::Char('u') => {
            app.execute_untrack();
        }
        KeyCode::Char('q') => {
            app.quit();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.move_down();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.move_up();
        }
        KeyCode::Char(' ') => {
            app.select_fossil();
        }
        KeyCode::Char(':') => {
            app.enter_command_mode();
        }
        KeyCode::Char('r') => {
            app.refresh_data();
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.quit();
        }
        KeyCode::Esc => {
            app.clear_status();
        }
        _ => {}
    }
    Ok(())
}

fn handle_command_mode(app: &mut App, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Esc => {
            app.exit_command_mode();
        }
        KeyCode::Enter => {
            app.execute_command();
        }
        KeyCode::Backspace => {
            app.remove_char_from_command();
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.quit();
        }
        KeyCode::Char(c) => {
            app.add_char_to_command(c);
        }
        _ => {}
    }
    Ok(())
}
