use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

pub enum AppEvent {
    Quit,
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Other,
}

pub fn handle_events() -> Result<Option<AppEvent>, Box<dyn std::error::Error>> {
    if event::poll(Duration::from_millis(250))? {
        match event::read()? {
            Event::Key(key) => Ok(Some(map_key_event(key))),
            _ => Ok(Some(AppEvent::Other)),
        }
    } else {
        Ok(None)
    }
}

fn map_key_event(key: KeyEvent) -> AppEvent {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) => AppEvent::Quit,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => AppEvent::Quit,
        (KeyCode::Up, KeyModifiers::NONE) => AppEvent::Up,
        (KeyCode::Down, KeyModifiers::NONE) => AppEvent::Down,
        (KeyCode::Left, KeyModifiers::NONE) => AppEvent::Left,
        (KeyCode::Right, KeyModifiers::NONE) => AppEvent::Right,
        (KeyCode::Enter, KeyModifiers::NONE) => AppEvent::Enter,
        (KeyCode::Esc, KeyModifiers::NONE) => AppEvent::Escape,
        _ => AppEvent::Other,
    }
}
