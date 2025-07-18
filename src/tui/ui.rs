use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::app::{App, AppMode, LayoutMode, CommandType};

struct LayoutChunks {
    controls: Rect,
    fossil_list: Rect,
    command_panel: Rect,
    preview: Option<Rect>,
}

impl LayoutChunks {
    fn new(area: Rect, mode: LayoutMode) -> Self { 
        match mode {
            LayoutMode::Preview => {
                let main_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .margin(1)
                    .constraints([Constraint::Percentage(15),
                                  Constraint::Percentage(42),
                                  Constraint::Percentage(43)])
                    .split(area);
                
                let middle_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(3), Constraint::Length(3)])
                    .split(main_chunks[1]);

                Self {
                    controls: main_chunks[0],
                    fossil_list: middle_chunks[0],
                    command_panel: middle_chunks[1],
                    preview: Some(main_chunks[2]),
                }
            },
            LayoutMode::Regular => {
                let main_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .margin(1)
                    .constraints([Constraint::Percentage(15), Constraint::Percentage(85)])
                    .split(area);
                
                let right_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(3), Constraint::Length(3)])
                    .split(main_chunks[1]);

                Self {
                    controls: main_chunks[0],
                    fossil_list: right_chunks[0],
                    command_panel: right_chunks[1],
                    preview: None,
                }
            }
        } 
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = LayoutChunks::new(f.area(), app.layout_mode.clone());
    
    draw_controls_panel(f, chunks.controls);
    draw_fossil_list(f, app, chunks.fossil_list);
    draw_command_panel(f, app, chunks.command_panel);
    
    if let Some(preview_rect) = chunks.preview {
        draw_preview_pane(f, app, preview_rect);
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
    let (title, input_text) = match app.mode {
        AppMode::Normal => {
            let title = "Ready";
            let text = if let Some(ref status) = app.status_message {
                status.clone()
            } else {
                String::new()
            };
            (title, text)
        }
        AppMode::Command => {
            match app.command_type {
                CommandType::General => ("Command Mode", format!(":{}", app.command_input)),
                CommandType::Bury => ("Bury - Enter tag (optional)", app.command_input.clone()),
                CommandType::Dig => ("Dig - Enter tag or version (optional)", app.command_input.clone()),
            }
        }
    };

    let style = match app.mode {
        AppMode::Normal => {
            if app.status_message.is_some() {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            }
        }
        AppMode::Command => Style::default().fg(Color::Yellow),
    };

    let input = Paragraph::new(input_text)
        .style(style)
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
        Line::from(Span::raw("b - Bury selected")),
        Line::from(Span::raw("d - Dig selected")),
        Line::from(Span::raw("s - Surface all")),
        Line::from(Span::raw("t - Track selected")),
        Line::from(Span::raw("u - Untrack selected")),
        Line::from(Span::raw("p - Toggle preview")),
        Line::from(Span::raw("r - Refresh data")),
        Line::from(Span::raw(": - Command mode")),
        Line::from(Span::raw("Esc - Clear status")),
        Line::from(Span::raw("q - Quit")),
        Line::from(Span::raw("Ctrl+C - Force quit")),
    ];

    let help = Paragraph::new(help_text)
        .block(Block::default().title("Help").borders(Borders::ALL))
        .style(Style::default().fg(Color::Gray));

    f.render_widget(help, area);
}
