use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, Wrap},
};
use std::collections::HashSet;

use crate::tui::app::{App, InputMode};

pub fn render(app: &mut App, f: &mut Frame) {
    if app.show_help {
        render_help(f, f.area());
        return;
    }

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(f.area());

    let content_chunks = if app.show_preview {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(30),
                Constraint::Min(0),
                Constraint::Length(40),
            ])
            .split(main_chunks[0])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(0)])
            .split(main_chunks[0])
    };

    render_sidebar(app, f, content_chunks[0]);
    render_main_table(app, f, content_chunks[1]);

    if app.show_preview && content_chunks.len() > 2 {
        render_preview_panel(app, f, content_chunks[2]);
    }

    render_bottom_bar(app, f, main_chunks[1]);
}

fn render_sidebar(app: &App, f: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(std::cmp::max(app.layers.len() as u16 + 2, 5)),
            Constraint::Length(8),
            Constraint::Min(0),
        ])
        .split(area);

    // Layers panel
    let layers_items: Vec<ListItem> = app
        .layers
        .iter()
        .map(|layer| {
            let current_marker = if *layer == app.config.current_layer {
                " (current)"
            } else {
                ""
            };
            ListItem::new(format!("Layer {}{}", layer, current_marker)).style(
                if *layer == app.config.current_layer {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            )
        })
        .collect();

    let layers_list = List::new(layers_items)
        .block(Block::default().borders(Borders::ALL).title("Layers"))
        .style(Style::default().fg(Color::White));

    f.render_widget(layers_list, chunks[0]);

    // Stats panel
    let stats_text = vec![
        format!("Tracked: {}", app.fossils.len()),
        format!("Untracked: {}", app.untracked_files.len()),
        format!("Selected: {}", app.selected_fossils.len()),
        format!("Current Layer: {}", app.config.current_layer),
        format!("Surface Layer: {}", app.config.surface_layer),
    ];

    let stats = Paragraph::new(stats_text.join("\n"))
        .block(Block::default().borders(Borders::ALL).title("Stats"))
        .style(Style::default().fg(Color::White));

    f.render_widget(stats, chunks[1]);

    // Help panel
    let help_text = vec![
        "j/k,↑/↓ - Navigate",
        "Space - Select/Deselect",
        "t - Track file",
        "b - Bury with tag",
        "B - Bury all",
        "s,Ctrl+S - Surface",
        "d - Dig by tag",
        "r - Refresh",
        "p - Toggle preview",
        "? - Help",
        "q - Quit",
    ];

    let help = Paragraph::new(help_text.join("\n"))
        .block(Block::default().borders(Borders::ALL).title("Controls"))
        .style(Style::default().fg(Color::Gray));

    f.render_widget(help, chunks[2]);
}

fn render_main_table(app: &mut App, f: &mut Frame, area: Rect) {
    let header_cells = [
        "Sel",
        "Hash",
        "Path",
        "Status",
        "Layer",
        "Tags",
        "Versions",
        "Last Tracked",
    ]
    .iter()
    .map(|h| {
        ratatui::widgets::Cell::from(*h).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    });

    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app
        .fossils
        .iter()
        .enumerate()
        .map(|(i, (hash, tracked_file))| {
            let is_selected = app.selected_fossils.contains(&i);
            let file_exists = std::path::Path::new(&tracked_file.file_path).exists();

            let status = if file_exists {
                if app.file_has_changes(tracked_file).unwrap_or(false) {
                    "Modified"
                } else {
                    "Clean"
                }
            } else {
                "Missing"
            };

            let status_style = match status {
                "Modified" => Style::default().fg(Color::Yellow),
                "Missing" => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::Green),
            };

            // Get current layer for this file
            let current_layer = app
                .config
                .file_current_layers
                .get(hash as &str)
                .copied()
                .unwrap_or(app.config.current_layer);

            // Get tags for this file (collect unique tags from all versions)
            let tags: HashSet<String> = tracked_file
                .layer_versions
                .iter()
                .filter_map(|lv| {
                    if lv.tag.is_empty() {
                        None
                    } else {
                        Some(lv.tag.clone())
                    }
                })
                .collect();
            let tags_str = if tags.is_empty() {
                "-".to_string()
            } else {
                tags.into_iter().collect::<Vec<_>>().join(",")
            };

            let cells = vec![
                ratatui::widgets::Cell::from(if is_selected { "●" } else { " " })
                    .style(Style::default().fg(Color::Cyan)),
                ratatui::widgets::Cell::from(hash[..8.min(hash.len())].to_string()),
                ratatui::widgets::Cell::from(
                    tracked_file.file_path.to_string_lossy().to_string(),
                ),
                ratatui::widgets::Cell::from(status).style(status_style),
                ratatui::widgets::Cell::from(current_layer.to_string())
                    .style(Style::default().fg(Color::Magenta)),
                ratatui::widgets::Cell::from(tags_str).style(Style::default().fg(Color::Cyan)),
                ratatui::widgets::Cell::from(tracked_file.versions.to_string()),
                ratatui::widgets::Cell::from(
                    tracked_file.last_tracked.format("%m-%d %H:%M").to_string(),
                ),
            ];
            Row::new(cells).height(1).bottom_margin(1)
        });

    let title = if !app.selected_fossils.is_empty() {
        format!("Tracked Fossils ({} selected)", app.selected_fossils.len())
    } else {
        "Tracked Fossils".to_string()
    };

    let table = Table::new(
        rows,
        &[
            Constraint::Length(3),  // Sel
            Constraint::Length(10), // Hash
            Constraint::Min(20),    // Path
            Constraint::Length(8),  // Status
            Constraint::Length(6),  // Layer
            Constraint::Length(12), // Tags
            Constraint::Length(8),  // Versions
            Constraint::Length(12), // Last Tracked
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title))
    .row_highlight_style(Style::default().bg(Color::Gray).fg(Color::Black))
    .highlight_symbol(">> ");

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn render_bottom_bar(app: &App, f: &mut Frame, area: Rect) {
    let text = match app.input_mode {
        InputMode::TagInput => format!("Tag: {}", app.input_buffer),
        InputMode::TagDigInput => format!("Dig by tag: {}", app.input_buffer),
        InputMode::Normal => {
            if let Some(ref message) = app.status_message {
                message.clone()
            } else {
                format!(
                    "Layer: {} | {} tracked, {} untracked | ? for help",
                    app.config.current_layer,
                    app.fossils.len(),
                    app.untracked_files.len()
                )
            }
        }
    };

    let style = if app.status_message.is_some() {
        Style::default().fg(Color::Yellow)
    } else if app.input_mode != InputMode::Normal {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL))
        .style(style);

    f.render_widget(paragraph, area);
}

fn render_preview_panel(app: &App, f: &mut Frame, area: Rect) {
    let content = if let Some(selected) = app.selected_fossil {
        if selected < app.fossils.len() {
            let (_, tracked_file) = &app.fossils[selected];
            let path = &tracked_file.file_path;

            if std::path::Path::new(path).exists() {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let lines: Vec<&str> = content.lines().take(50).collect();
                        lines.join("\n")
                    }
                    Err(_) => "Binary file or read error".to_string(),
                }
            } else {
                "File not found".to_string()
            }
        } else {
            "No file selected".to_string()
        }
    } else {
        "No file selected".to_string()
    };

    let preview = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Preview"))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White));

    f.render_widget(preview, area);
}

fn render_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(vec![Span::styled(
            "Fossil TUI Help",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Navigation:"),
        Line::from("  j/k, ↑/↓     - Move up/down"),
        Line::from("  h/l, ←/→     - Move left/right"),
        Line::from("  g/G          - Go to first/last"),
        Line::from("  PgUp/PgDn    - Page up/down"),
        Line::from(""),
        Line::from("File Operations:"),
        Line::from("  t            - Track selected file/untracked files"),
        Line::from("  b            - Bury with tag"),
        Line::from("  B            - Bury all changes"),
        Line::from("  s, Ctrl+S    - Surface to latest layer"),
        Line::from("  r            - Refresh file status"),
        Line::from(""),
        Line::from("Selection:"),
        Line::from("  Space        - Toggle selection"),
        Line::from("  a            - Select all"),
        Line::from("  A            - Deselect all"),
        Line::from(""),
        Line::from("Layer Operations:"),
        Line::from("  0-9          - Quick dig to layer"),
        Line::from("  d            - Dig by tag"),
        Line::from(""),
        Line::from("View:"),
        Line::from("  p            - Toggle preview panel"),
        Line::from("  ?            - Toggle this help"),
        Line::from(""),
        Line::from("Other:"),
        Line::from("  q            - Quit"),
        Line::from("  Esc          - Cancel current operation"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press ? again to close help",
            Style::default().fg(Color::Green),
        )]),
    ];

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Left);

    f.render_widget(help, area);
}
