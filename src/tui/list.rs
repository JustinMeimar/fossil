use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::fs;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, TableState, Wrap},
    text::{Span, Spans},
    Frame,
};

use crate::config::{Config, TrackedFile, load_config};
use crate::tui::events::{handle_events, AppEvent};
use crate::fossil;

pub struct ListApp {
    pub config: Config,
    pub fossils: Vec<(String, TrackedFile)>,
    pub layers: Vec<u32>,
    pub table_state: TableState,
    pub selected_fossil: Option<usize>,
    pub selected_fossils: HashSet<usize>,
    pub input_buffer: String,
    pub input_mode: InputMode,
    pub status_message: Option<String>,
    pub show_preview: bool,
    pub show_help: bool,
    pub current_dir: PathBuf,
    pub untracked_files: Vec<PathBuf>,
}

#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    Command,
    TagInput,
}

impl ListApp {
    pub fn new(config: Config) -> Self {
        let fossils: Vec<(String, TrackedFile)> = config.fossils.iter()
            .map(|(hash, file)| (hash.clone(), file.clone()))
            .collect();
        
        let mut all_layers: BTreeSet<u32> = BTreeSet::new();
        for tracked_file in config.fossils.values() {
            for layer_version in &tracked_file.layer_versions {
                all_layers.insert(layer_version.layer);
            }
        }
        let layers: Vec<u32> = all_layers.into_iter().rev().collect();
        
        let mut table_state = TableState::default();
        if !fossils.is_empty() {
            table_state.select(Some(0));
        }
        
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let untracked_files = Self::find_untracked_files(&current_dir, &config);
        
        let selected_fossil = if fossils.is_empty() { None } else { Some(0) };
        
        Self {
            config,
            fossils,
            layers,
            table_state,
            selected_fossil,
            selected_fossils: HashSet::new(),
            input_buffer: String::new(),
            input_mode: InputMode::Normal,
            status_message: None,
            show_preview: false,
            show_help: false,
            current_dir,
            untracked_files,
        }
    }
    
    fn find_untracked_files(dir: &PathBuf, config: &Config) -> Vec<PathBuf> {
        let mut untracked = Vec::new();
        
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let path_str = path.to_string_lossy().to_string();
                    let is_tracked = config.fossils.values()
                        .any(|f| f.original_path == path_str);
                    
                    if !is_tracked && !path.starts_with(".fossil") {
                        untracked.push(path);
                    }
                }
            }
        }
        
        untracked.sort();
        untracked
    }
    
    pub fn run<B: Backend>(&mut self, terminal: &mut tui::Terminal<B>) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|f| self.draw(f))?;
            
            if let Some(event) = handle_events()? {
                match event {
                    AppEvent::Quit => break,
                    
                    // Navigation
                    AppEvent::Up => {
                        if self.input_mode == InputMode::Normal {
                            self.previous();
                        }
                    },
                    AppEvent::Down => {
                        if self.input_mode == InputMode::Normal {
                            self.next();
                        }
                    },
                    AppEvent::Home => {
                        if self.input_mode == InputMode::Normal {
                            self.goto_first();
                        }
                    },
                    AppEvent::End => {
                        if self.input_mode == InputMode::Normal {
                            self.goto_last();
                        }
                    },
                    AppEvent::PageUp => {
                        if self.input_mode == InputMode::Normal {
                            self.page_up();
                        }
                    },
                    AppEvent::PageDown => {
                        if self.input_mode == InputMode::Normal {
                            self.page_down();
                        }
                    },
                    
                    // File operations
                    AppEvent::TrackFile => {
                        if self.input_mode == InputMode::Normal {
                            self.track_selected()?;
                        }
                    },
                    AppEvent::BuryAll => {
                        if self.input_mode == InputMode::Normal {
                            self.bury_all()?;
                        }
                    },
                    AppEvent::BuryWithTag => {
                        if self.input_mode == InputMode::Normal {
                            self.start_tag_input();
                        }
                    },
                    AppEvent::Surface => {
                        if self.input_mode == InputMode::Normal {
                            self.surface()?;
                        }
                    },
                    AppEvent::Refresh => {
                        if self.input_mode == InputMode::Normal {
                            self.refresh()?;
                        }
                    },
                    
                    // Selection
                    AppEvent::ToggleSelect => {
                        if self.input_mode == InputMode::Normal {
                            self.toggle_selection();
                        }
                    },
                    AppEvent::SelectAll => {
                        if self.input_mode == InputMode::Normal {
                            self.select_all();
                        }
                    },
                    AppEvent::DeselectAll => {
                        if self.input_mode == InputMode::Normal {
                            self.deselect_all();
                        }
                    },
                    
                    // Layer operations
                    AppEvent::QuickDig(layer) => {
                        if self.input_mode == InputMode::Normal && self.layers.contains(&layer) {
                            self.dig_to_layer(layer)?;
                        }
                    },
                    
                    // View operations
                    AppEvent::TogglePreview => {
                        if self.input_mode == InputMode::Normal {
                            self.show_preview = !self.show_preview;
                        }
                    },
                    AppEvent::ToggleHelp => {
                        if self.input_mode == InputMode::Normal {
                            self.show_help = !self.show_help;
                        }
                    },
                    
                    // Command mode
                    AppEvent::CommandMode => {
                        if self.input_mode == InputMode::Normal {
                            self.start_command_mode();
                        }
                    },
                    
                    // Input handling
                    AppEvent::Char(c) => self.handle_char_input(c),
                    AppEvent::Enter => self.handle_enter()?,
                    AppEvent::Escape => self.handle_escape(),
                    
                    _ => {}
                }
            }
        }
        Ok(())
    }
    
    fn draw<B: Backend>(&mut self, f: &mut Frame<B>) {
        if self.show_help {
            self.draw_help(f, f.size());
            return;
        }
        
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
            .split(f.size());
        
        let content_chunks = if self.show_preview {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(30), Constraint::Min(0), Constraint::Length(40)].as_ref())
                .split(main_chunks[0])
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(30), Constraint::Min(0)].as_ref())
                .split(main_chunks[0])
        };
        
        self.draw_sidebar(f, content_chunks[0]);
        self.draw_main_table(f, content_chunks[1]);
        
        if self.show_preview && content_chunks.len() > 2 {
            self.draw_preview_panel(f, content_chunks[2]);
        }
        
        self.draw_bottom_bar(f, main_chunks[1]);
    }
    
    fn draw_sidebar<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(self.layers.len() as u16 + 2),
                Constraint::Length(8),
                Constraint::Min(0)
            ].as_ref())
            .split(area);
        
        // Layers panel
        let layers_items: Vec<ListItem> = self.layers.iter()
            .map(|layer| {
                let current_marker = if *layer == self.config.current_layer { 
                    " (current)" 
                } else { 
                    "" 
                };
                ListItem::new(format!("Layer {}{}", layer, current_marker))
                    .style(if *layer == self.config.current_layer {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    })
            })
            .collect();
        
        let layers_list = List::new(layers_items)
            .block(Block::default().borders(Borders::ALL).title("Layers"))
            .style(Style::default().fg(Color::White));
        
        f.render_widget(layers_list, chunks[0]);
        
        // Stats panel
        let stats_text = vec![
            format!("Tracked: {}", self.fossils.len()),
            format!("Untracked: {}", self.untracked_files.len()),
            format!("Selected: {}", self.selected_fossils.len()),
            format!("Current Layer: {}", self.config.current_layer),
            format!("Surface Layer: {}", self.config.surface_layer),
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
            "b - Bury all",
            "s - Surface",
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
    
    fn draw_main_table<B: Backend>(&mut self, f: &mut Frame<B>, area: Rect) {
        let header_cells = ["Sel", "Hash", "Path", "Status", "Versions", "Last Tracked"]
            .iter()
            .map(|h| tui::widgets::Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        
        let header = Row::new(header_cells).height(1).bottom_margin(1);
        
        let rows = self.fossils.iter().enumerate().map(|(i, (hash, tracked_file))| {
            let is_selected = self.selected_fossils.contains(&i);
            let file_exists = std::path::Path::new(&tracked_file.original_path).exists();
            
            let status = if file_exists {
                if self.file_has_changes(tracked_file).unwrap_or(false) {
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
            
            let cells = vec![
                tui::widgets::Cell::from(if is_selected { "●" } else { " " })
                    .style(Style::default().fg(Color::Cyan)),
                tui::widgets::Cell::from(hash[..8.min(hash.len())].to_string()),
                tui::widgets::Cell::from(tracked_file.original_path.clone()),
                tui::widgets::Cell::from(status).style(status_style),
                tui::widgets::Cell::from(tracked_file.versions.to_string()),
                tui::widgets::Cell::from(tracked_file.last_tracked.format("%m-%d %H:%M").to_string()),
            ];
            Row::new(cells).height(1).bottom_margin(1)
        });
        
        let title = if !self.selected_fossils.is_empty() {
            format!("Tracked Fossils ({} selected)", self.selected_fossils.len())
        } else {
            "Tracked Fossils".to_string()
        };
        
        let table = Table::new(rows)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().bg(Color::Gray).fg(Color::Black))
            .highlight_symbol(">> ")
            .widths(&[
                Constraint::Length(3),
                Constraint::Length(10),
                Constraint::Min(20),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(12),
            ]);
        
        f.render_stateful_widget(table, area, &mut self.table_state);
    }
    
    fn draw_bottom_bar<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let text = match self.input_mode {
            InputMode::Command => format!("Command: {}", self.input_buffer),
            InputMode::TagInput => format!("Tag: {}", self.input_buffer),
            InputMode::Normal => {
                if let Some(ref message) = self.status_message {
                    message.clone()
                } else {
                    format!("Layer: {} | {} tracked, {} untracked | ? for help", 
                        self.config.current_layer, self.fossils.len(), self.untracked_files.len())
                }
            }
        };
        
        let style = if self.status_message.is_some() {
            Style::default().fg(Color::Yellow)
        } else if self.input_mode != InputMode::Normal {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        };
        
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .style(style);
        
        f.render_widget(paragraph, area);
    }
    
    fn draw_preview_panel<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let content = if let Some(selected) = self.selected_fossil {
            if selected < self.fossils.len() {
                let (_, tracked_file) = &self.fossils[selected];
                let path = &tracked_file.original_path;
                
                if std::path::Path::new(path).exists() {
                    match std::fs::read_to_string(path) {
                        Ok(content) => {
                            let lines: Vec<&str> = content.lines().take(50).collect();
                            lines.join("\n")
                        },
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
    
    fn draw_help<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let help_text = vec![
            Spans::from(vec![Span::styled("Fossil TUI Help", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Spans::from(""),
            Spans::from("Navigation:"),
            Spans::from("  j/k, ↑/↓     - Move up/down"),
            Spans::from("  h/l, ←/→     - Move left/right"),
            Spans::from("  g/G          - Go to first/last"),
            Spans::from("  PgUp/PgDn    - Page up/down"),
            Spans::from(""),
            Spans::from("File Operations:"),
            Spans::from("  t            - Track selected file/untracked files"),
            Spans::from("  b            - Bury all changes"),
            Spans::from("  B            - Bury with tag"),
            Spans::from("  s            - Surface to latest layer"),
            Spans::from("  r            - Refresh file status"),
            Spans::from(""),
            Spans::from("Selection:"),
            Spans::from("  Space        - Toggle selection"),
            Spans::from("  a            - Select all"),
            Spans::from("  A            - Deselect all"),
            Spans::from(""),
            Spans::from("Layer Operations:"),
            Spans::from("  0-9          - Quick dig to layer"),
            Spans::from(""),
            Spans::from("View:"),
            Spans::from("  p            - Toggle preview panel"),
            Spans::from("  ?            - Toggle this help"),
            Spans::from(""),
            Spans::from("Other:"),
            Spans::from("  q            - Quit"),
            Spans::from("  Esc          - Cancel current operation"),
            Spans::from(""),
            Spans::from(vec![Span::styled("Press ? again to close help", Style::default().fg(Color::Green))]),
        ];
        
        let help = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White));
        
        f.render_widget(help, area);
    }
    
    fn next(&mut self) {
        if self.fossils.is_empty() {
            return;
        }
        
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.fossils.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.selected_fossil = Some(i);
    }
    
    fn previous(&mut self) {
        if self.fossils.is_empty() {
            return;
        }
        
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.fossils.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.selected_fossil = Some(i);
    }
    
    // Navigation methods
    fn goto_first(&mut self) {
        if !self.fossils.is_empty() {
            self.table_state.select(Some(0));
            self.selected_fossil = Some(0);
        }
    }
    
    fn goto_last(&mut self) {
        if !self.fossils.is_empty() {
            let last = self.fossils.len() - 1;
            self.table_state.select(Some(last));
            self.selected_fossil = Some(last);
        }
    }
    
    fn page_up(&mut self) {
        if !self.fossils.is_empty() {
            let current = self.table_state.selected().unwrap_or(0);
            let new_pos = if current >= 10 { current - 10 } else { 0 };
            self.table_state.select(Some(new_pos));
            self.selected_fossil = Some(new_pos);
        }
    }
    
    fn page_down(&mut self) {
        if !self.fossils.is_empty() {
            let current = self.table_state.selected().unwrap_or(0);
            let new_pos = std::cmp::min(current + 10, self.fossils.len() - 1);
            self.table_state.select(Some(new_pos));
            self.selected_fossil = Some(new_pos);
        }
    }
    
    // Selection methods
    fn toggle_selection(&mut self) {
        if let Some(selected) = self.selected_fossil {
            if self.selected_fossils.contains(&selected) {
                self.selected_fossils.remove(&selected);
            } else {
                self.selected_fossils.insert(selected);
            }
        }
    }
    
    fn select_all(&mut self) {
        self.selected_fossils = (0..self.fossils.len()).collect();
    }
    
    fn deselect_all(&mut self) {
        self.selected_fossils.clear();
    }
    
    // Input handling methods
    fn handle_char_input(&mut self, c: char) {
        match self.input_mode {
            InputMode::Normal => {
                // Normal mode - characters are handled by events
            },
            InputMode::Command | InputMode::TagInput => {
                if c.is_alphanumeric() || c == ' ' || c == '_' || c == '-' {
                    self.input_buffer.push(c);
                }
            }
        }
    }
    
    fn handle_enter(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.input_mode {
            InputMode::Command => {
                // Handle command input
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            },
            InputMode::TagInput => {
                let tag = if self.input_buffer.is_empty() { 
                    None 
                } else { 
                    Some(self.input_buffer.clone()) 
                };
                self.bury_with_tag(tag)?;
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            },
            InputMode::Normal => {
                // No special action in normal mode
            }
        }
        Ok(())
    }
    
    fn handle_escape(&mut self) {
        if self.input_mode != InputMode::Normal {
            self.input_mode = InputMode::Normal;
            self.input_buffer.clear();
            self.status_message = None;
        }
    }
    
    fn start_command_mode(&mut self) {
        self.input_mode = InputMode::Command;
        self.input_buffer.clear();
    }
    
    fn start_tag_input(&mut self) {
        self.input_mode = InputMode::TagInput;
        self.input_buffer.clear();
    }
    
    // Fossil operations
    fn track_selected(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.selected_fossils.is_empty() {
            self.status_message = Some("Cannot track already tracked files".to_string());
            return Ok(());
        }
        
        if !self.untracked_files.is_empty() {
            let files: Vec<String> = self.untracked_files.iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            fossil::track(files)?;
            self.refresh()?;
            self.status_message = Some(format!("Tracked {} files", self.untracked_files.len()));
        } else {
            self.status_message = Some("No untracked files to track".to_string());
        }
        Ok(())
    }
    
    fn bury_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        fossil::bury(None, None)?;
        self.refresh()?;
        self.status_message = Some("All changes burried".to_string());
        Ok(())
    }
    
    fn bury_with_tag(&mut self, tag: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
        fossil::bury(None, tag.clone())?;
        self.refresh()?;
        let msg = if let Some(t) = tag {
            format!("Changes burried with tag: {}", t)
        } else {
            "Changes burried".to_string()
        };
        self.status_message = Some(msg);
        Ok(())
    }
    
    fn surface(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        fossil::surface()?;
        self.refresh()?;
        self.status_message = Some("Surfaced to latest layer".to_string());
        Ok(())
    }
    
    fn refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reload_config()?;
        self.untracked_files = Self::find_untracked_files(&self.current_dir, &self.config);
        self.selected_fossils.clear();
        self.status_message = Some("Refreshed".to_string());
        Ok(())
    }
    
    fn file_has_changes(&self, tracked_file: &TrackedFile) -> Result<bool, Box<dyn std::error::Error>> {
        let path = PathBuf::from(&tracked_file.original_path);
        if !path.exists() {
            return Ok(true); // Missing file is a change
        }
        
        let content = fs::read(&path)?;
        let current_hash = crate::utils::hash_content(&content);
        Ok(current_hash != tracked_file.last_content_hash)
    }
    
    fn dig_to_layer(&mut self, layer: u32) -> Result<(), Box<dyn std::error::Error>> {
        fossil::dig(layer)?;
        self.reload_config()?;
        self.status_message = Some(format!("Dug to layer {}", layer));
        Ok(())
    }
    
    fn reload_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.config = load_config()?;
        
        self.fossils = self.config.fossils.iter()
            .map(|(hash, file)| (hash.clone(), file.clone()))
            .collect();
        
        let mut all_layers: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        for tracked_file in self.config.fossils.values() {
            for layer_version in &tracked_file.layer_versions {
                all_layers.insert(layer_version.layer);
            }
        }
        self.layers = all_layers.into_iter().rev().collect();
        
        // Reset selection if needed
        if self.fossils.is_empty() {
            self.table_state.select(None);
            self.selected_fossil = None;
        } else if let Some(selected) = self.selected_fossil {
            if selected >= self.fossils.len() {
                self.table_state.select(Some(0));
                self.selected_fossil = Some(0);
            }
        }
        
        Ok(())
    }
}
