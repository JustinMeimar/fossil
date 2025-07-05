use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Quit,
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Backspace,
    Char(char),
    // Navigation
    Home,
    End,
    PageUp,
    PageDown,
    // File operations
    TrackFile,
    BuryAll,
    BuryWithTag,
    Surface,
    Refresh,
    ToggleSelect,
    SelectAll,
    DeselectAll,
    // Layer operations
    QuickDig(u32),
    DigByTag,
    // View operations
    TogglePreview,
    ToggleHelp,
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
        // Quit
        (KeyCode::Char('q'), KeyModifiers::NONE) => AppEvent::Quit,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => AppEvent::Quit,

        // Navigation
        (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
            AppEvent::Up
        }
        (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
            AppEvent::Down
        }
        (KeyCode::Left, KeyModifiers::NONE) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
            AppEvent::Left
        }
        (KeyCode::Right, KeyModifiers::NONE) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
            AppEvent::Right
        }
        (KeyCode::Home, KeyModifiers::NONE) | (KeyCode::Char('g'), KeyModifiers::NONE) => {
            AppEvent::Home
        }
        (KeyCode::End, KeyModifiers::NONE) | (KeyCode::Char('G'), KeyModifiers::NONE) => {
            AppEvent::End
        }
        (KeyCode::PageUp, KeyModifiers::NONE) => AppEvent::PageUp,
        (KeyCode::PageDown, KeyModifiers::NONE) => AppEvent::PageDown,

        // File operations
        (KeyCode::Char('t'), KeyModifiers::NONE) => AppEvent::TrackFile,
        (KeyCode::Char('b'), KeyModifiers::NONE) => AppEvent::BuryWithTag,
        (KeyCode::Char('B'), KeyModifiers::NONE) => AppEvent::BuryAll,
        (KeyCode::Char('s'), KeyModifiers::NONE) => AppEvent::Surface,
        (KeyCode::Char('s'), KeyModifiers::CONTROL) => AppEvent::Surface,
        (KeyCode::Char('r'), KeyModifiers::NONE) | (KeyCode::F(5), KeyModifiers::NONE) => {
            AppEvent::Refresh
        }

        // Selection
        (KeyCode::Char(' '), KeyModifiers::NONE) => AppEvent::ToggleSelect,
        (KeyCode::Char('a'), KeyModifiers::NONE) => AppEvent::SelectAll,
        (KeyCode::Char('A'), KeyModifiers::NONE) => AppEvent::DeselectAll,

        // Layer operations - quick dig with number keys
        (KeyCode::Char('0'), KeyModifiers::NONE) => AppEvent::QuickDig(0),
        (KeyCode::Char('1'), KeyModifiers::NONE) => AppEvent::QuickDig(1),
        (KeyCode::Char('2'), KeyModifiers::NONE) => AppEvent::QuickDig(2),
        (KeyCode::Char('3'), KeyModifiers::NONE) => AppEvent::QuickDig(3),
        (KeyCode::Char('4'), KeyModifiers::NONE) => AppEvent::QuickDig(4),
        (KeyCode::Char('5'), KeyModifiers::NONE) => AppEvent::QuickDig(5),
        (KeyCode::Char('6'), KeyModifiers::NONE) => AppEvent::QuickDig(6),
        (KeyCode::Char('7'), KeyModifiers::NONE) => AppEvent::QuickDig(7),
        (KeyCode::Char('8'), KeyModifiers::NONE) => AppEvent::QuickDig(8),
        (KeyCode::Char('9'), KeyModifiers::NONE) => AppEvent::QuickDig(9),

        // Tag and file operations
        (KeyCode::Char('d'), KeyModifiers::NONE) => AppEvent::DigByTag,

        // View operations
        (KeyCode::Char('p'), KeyModifiers::NONE) => AppEvent::TogglePreview,
        (KeyCode::Char('?'), KeyModifiers::NONE) | (KeyCode::F(1), KeyModifiers::NONE) => {
            AppEvent::ToggleHelp
        }


        // Input handling
        (KeyCode::Enter, KeyModifiers::NONE) => AppEvent::Enter,
        (KeyCode::Esc, KeyModifiers::NONE) => AppEvent::Escape,
        (KeyCode::Backspace, KeyModifiers::NONE) => AppEvent::Backspace,
        (KeyCode::Char(c), KeyModifiers::NONE) => AppEvent::Char(c),

        _ => AppEvent::Other,
    }
}
