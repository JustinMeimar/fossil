use std::collections::BTreeSet;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::config::{Config, TrackedFile};
use crate::tui::events::{handle_events, AppEvent};

pub struct ListApp {
    pub config: Config,
    pub fossils: Vec<(String, TrackedFile)>,
    pub layers: Vec<u32>,
    pub table_state: TableState,
    pub selected_fossil: Option<usize>,
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
        
        Self {
            config,
            fossils,
            layers,
            table_state,
            selected_fossil: Some(0),
        }
    }
    
    pub fn run<B: Backend>(&mut self, terminal: &mut tui::Terminal<B>) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|f| self.draw(f))?;
            
            if let Some(event) = handle_events()? {
                match event {
                    AppEvent::Quit => break,
                    AppEvent::Up => self.previous(),
                    AppEvent::Down => self.next(),
                    _ => {}
                }
            }
        }
        Ok(())
    }
    
    fn draw<B: Backend>(&mut self, f: &mut Frame<B>) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(0)].as_ref())
            .split(f.size());
        
        self.draw_sidebar(f, chunks[0]);
        self.draw_main_table(f, chunks[1]);
    }
    
    fn draw_sidebar<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(self.layers.len() as u16 + 2), Constraint::Min(0)].as_ref())
            .split(area);
        
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
        
        let help_text = vec![
            "Controls:",
            "↑/↓ - Navigate",
            "q - Quit",
            "Esc - Back",
        ];
        
        let help = Paragraph::new(help_text.join("\n"))
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .style(Style::default().fg(Color::Gray));
        
        f.render_widget(help, chunks[1]);
    }
    
    fn draw_main_table<B: Backend>(&mut self, f: &mut Frame<B>, area: Rect) {
        let header_cells = ["Hash", "Path", "Versions", "Layers", "Last Tracked"]
            .iter()
            .map(|h| tui::widgets::Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        
        let header = Row::new(header_cells).height(1).bottom_margin(1);
        
        let rows = self.fossils.iter().map(|(hash, tracked_file)| {
            let cells = vec![
                tui::widgets::Cell::from(hash[..8.min(hash.len())].to_string()),
                tui::widgets::Cell::from(tracked_file.original_path.clone()),
                tui::widgets::Cell::from(tracked_file.versions.to_string()),
                tui::widgets::Cell::from(tracked_file.layer_versions.len().to_string()),
                tui::widgets::Cell::from(tracked_file.last_tracked.format("%Y-%m-%d %H:%M:%S").to_string()),
            ];
            Row::new(cells).height(1).bottom_margin(1)
        });
        
        let table = Table::new(rows)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title("Tracked Fossils"))
            .highlight_style(Style::default().bg(Color::Gray).fg(Color::Black))
            .highlight_symbol(">> ")
            .widths(&[
                Constraint::Length(16),
                Constraint::Min(20),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(20),
            ]);
        
        f.render_stateful_widget(table, area, &mut self.table_state);
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
}