use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::app::{App, AppMode, CommandType};

struct LayoutChunks {
    controls: Rect,
    fossil_list: Rect,
    command_panel: Rect,
    detail: Rect,
    preview: Rect,
    stats: Rect,
}

impl LayoutChunks {
    fn new(area: Rect) -> Self { 
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([Constraint::Percentage(15), Constraint::Percentage(85)])
            .split(area);
        
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(main_chunks[1]);

        let fossil_area_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(right_chunks[0]);
        
        let fossil_detail_preview_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(fossil_area_chunks[1]);

        let controls_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(main_chunks[0]);

        Self {
            controls: controls_chunks[1],
            fossil_list: fossil_area_chunks[0],
            command_panel: right_chunks[1],
            detail: fossil_detail_preview_chunks[0],
            preview: fossil_detail_preview_chunks[1],
            stats: controls_chunks[0],
        }
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = LayoutChunks::new(f.area());
    
    draw_controls_panel(f, chunks.controls);
    draw_fossil_list(f, app, chunks.fossil_list);
    draw_command_panel(f, app, chunks.command_panel);
    draw_detail_pane(f, app, chunks.detail);
    draw_preview_pane(f, app, chunks.preview);
    draw_stats_panel(f, app, chunks.stats);
}

fn draw_fossil_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .fossils
        .iter()
        .enumerate()
        .map(|(idx, fossil)| {
            let selected = app.select_fossils.contains(&idx);
            let checkbox = if selected { "[✓] " } else { "[ ] " };
            // let content = format!("Versions: {}", fossil.versions.len());
            let content = format!(
                "{}{} | v{}/{}",
                checkbox,
                fossil.path.display(),
                fossil.cur_version,
                fossil.versions.len()
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
                CommandType::Track => ("Track - Enter filepath to track", app.command_input.clone()),
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


fn draw_detail_pane(f: &mut Frame, app: &App, area: Rect) {
    let content = if app.fossils.is_empty() {
        "No fossils available".to_string()
    } else {
        let current_fossil = &app.fossils[app.cursor_idx];
        
        let mut detail_lines = vec![
            format!("Path: {}", current_fossil.path.display()),
            format!("Current Version: {}/{}", current_fossil.cur_version, current_fossil.versions.len()),
            String::new(),
            "Versions:".to_string(),
        ];
        
        for (i, version) in current_fossil.versions.iter().enumerate() {
            let version_num = i + 1;
            let tag_info = match &version.tag {
                Some(tag) => format!(" ({})", tag),
                None => String::new(),
            };
            let current_marker = if version_num == current_fossil.cur_version {
                " ← current"
            } else {
                ""
            };
            
            detail_lines.push(format!("  v{}{}{}", version_num, tag_info, current_marker));
        }
        
        detail_lines.join("\n")
    };
    
    let detail = Paragraph::new(content)
        .block(Block::default().title("Fossil Detail").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));
    f.render_widget(detail, area);
}

fn draw_preview_pane(f: &mut Frame, app: &App, area: Rect) {
    let content = if app.fossils.is_empty() {
        "No fossils available".to_string()
    } else {
        let current_fossil = &app.fossils[app.cursor_idx];
        match current_fossil.get_version_content(current_fossil.cur_version) {
            Ok(current_content) => {
                let preview = String::from_utf8_lossy(&current_content);
                format!("Preview: {}\n\n{}", current_fossil.path.display(), preview)
            }
            Err(_) => "Preview not available".to_string()
        }
    };
    
    let preview = Paragraph::new(content)
        .block(Block::default().title("File Preview").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));
    f.render_widget(preview, area);
}

fn draw_stats_panel(f: &mut Frame, app: &App, area: Rect) {
    let stats_text = vec![
        Line::from(Span::raw("Statistics:")),
        Line::from(Span::raw("")),
        Line::from(Span::raw(format!("Fossils: {}", app.get_total_fossils()))),
        Line::from(Span::raw(format!("Versions: {}", app.get_total_versions()))),
        Line::from(Span::raw(format!("Tagged: {}", app.get_tagged_versions_count()))),
    ];

    let stats = Paragraph::new(stats_text)
        .block(Block::default().title("Stats").borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan));

    f.render_widget(stats, area);
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
