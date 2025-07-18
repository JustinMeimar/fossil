use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::app::{App, AppMode};

pub fn draw(f: &mut Frame, app: &App) {
    if app.preview_mode {
        // Three-column layout: controls | fossil list | preview
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([Constraint::Percentage(15), Constraint::Percentage(42), Constraint::Percentage(43)].as_ref())
            .split(f.area());

        let middle_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
            .split(chunks[1]);

        // Side panel with controls
        draw_controls_panel(f, chunks[0]);
        
        // Main fossil list panel
        draw_fossil_list(f, app, middle_chunks[0]);
        
        // Command input panel
        draw_command_panel(f, app, middle_chunks[1]);
        
        // Preview panel
        draw_preview_pane(f, app, chunks[2]);
    } else {
        // Two-column layout: controls | main area
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([Constraint::Percentage(15), Constraint::Percentage(85)].as_ref())
            .split(f.area());

        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
            .split(chunks[1]);

        // Side panel with controls
        draw_controls_panel(f, chunks[0]);
        
        // Main fossil list panel
        draw_fossil_list(f, app, right_chunks[0]);
        
        // Command input panel
        draw_command_panel(f, app, right_chunks[1]);
    }
}

fn draw_fossil_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .fossils
        .iter()
        .enumerate()
        .map(|(idx, fossil)| {
            let selected = app.select_fossils.contains(&idx);
            let checkbox = if selected { "[✓] " } else { "[ ] " };
            let content = format!(
                "{}{} | v{}/{} | {} tags | {}",
                checkbox,
                fossil.path,
                fossil.current_version,
                fossil.total_versions,
                fossil.tag_count,
                fossil.preview
            );
            ListItem::new(Line::from(Span::raw(content)))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.cursor_idx));

    let list = List::new(items)
        .block(
            Block::default()
                .title("Fossils")
                .borders(Borders::ALL)
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_command_panel(f: &mut Frame, app: &App, area: Rect) {
    let title = match app.mode {
        AppMode::Normal => "Ready",
        AppMode::Command => "Command Mode",
    };

    let input_text = if app.mode == AppMode::Command {
        format!(":{}", app.command_input)
    } else {
        String::new()
    };

    let input = Paragraph::new(input_text)
        .style(match app.mode {
            AppMode::Normal => Style::default(),
            AppMode::Command => Style::default().fg(Color::Yellow),
        })
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(input, area);
}

fn draw_preview_pane(f: &mut Frame, app: &App, area: Rect) {
    let content = if app.fossils.is_empty() {
        "No fossils available".to_string()
    } else {
        let current_fossil = &app.fossils[app.cursor_idx];
        format!("Preview: {}\n\n{}", current_fossil.path, current_fossil.preview)
    };

    let preview = Paragraph::new(content)
        .block(Block::default().title("Preview").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

    f.render_widget(preview, area);
}

fn draw_controls_panel(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::raw("Controls:")),
        Line::from(Span::raw("")),
        Line::from(Span::raw("j/k, ↑/↓ - Navigate")),
        Line::from(Span::raw("Space - Select")),
        Line::from(Span::raw("p - Toggle preview")),
        Line::from(Span::raw(": - Command mode")),
        Line::from(Span::raw("Esc - Exit cmd mode")),
        Line::from(Span::raw("q - Quit")),
        Line::from(Span::raw("Ctrl+C - Force quit")),
    ];

    let help = Paragraph::new(help_text)
        .block(Block::default().title("Help").borders(Borders::ALL))
        .style(Style::default().fg(Color::Gray));

    f.render_widget(help, area);
}
