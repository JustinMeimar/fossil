use ratatui::widgets::TableState;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::PathBuf;

use crate::config::{Config, FossilRecord, load_config};

pub struct App {
    pub config: Config,
    pub fossils: Vec<(String, FossilRecord)>,
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
    pub should_quit: bool,
}

#[derive(PartialEq, Clone)]
pub enum InputMode {
    Normal,
    TagInput,
    TagDigInput,
}

impl App {
    pub fn new(config: Config) -> Self {
        let fossils: Vec<(String, FossilRecord)> = config
            .fossils
            .iter()
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
            should_quit: false,
        }
    }

    fn find_untracked_files(dir: &PathBuf, config: &Config) -> Vec<PathBuf> {
        let mut untracked = Vec::new();

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let is_tracked = config.fossils.values().any(|f| f.original_path == path);

                    if !is_tracked && !path.starts_with(".fossil") {
                        untracked.push(path);
                    }
                }
            }
        }

        untracked.sort();
        untracked
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn next(&mut self) {
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
        self.status_message = Some(format!("Selected item {}/{}", i + 1, self.fossils.len()));
    }

    pub fn previous(&mut self) {
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
        self.status_message = Some(format!("Selected item {}/{}", i + 1, self.fossils.len()));
    }

    pub fn goto_first(&mut self) {
        if !self.fossils.is_empty() {
            self.table_state.select(Some(0));
            self.selected_fossil = Some(0);
        }
    }

    pub fn goto_last(&mut self) {
        if !self.fossils.is_empty() {
            let last = self.fossils.len() - 1;
            self.table_state.select(Some(last));
            self.selected_fossil = Some(last);
        }
    }

    pub fn page_up(&mut self) {
        if !self.fossils.is_empty() {
            let current = self.table_state.selected().unwrap_or(0);
            let new_pos = if current >= 10 { current - 10 } else { 0 };
            self.table_state.select(Some(new_pos));
            self.selected_fossil = Some(new_pos);
        }
    }

    pub fn page_down(&mut self) {
        if !self.fossils.is_empty() {
            let current = self.table_state.selected().unwrap_or(0);
            let new_pos = std::cmp::min(current + 10, self.fossils.len() - 1);
            self.table_state.select(Some(new_pos));
            self.selected_fossil = Some(new_pos);
        }
    }

    pub fn toggle_selection(&mut self) {
        if let Some(selected) = self.selected_fossil {
            if self.selected_fossils.contains(&selected) {
                self.selected_fossils.remove(&selected);
                self.status_message = Some("Deselected file".to_string());
            } else {
                self.selected_fossils.insert(selected);
                self.status_message = Some("Selected file".to_string());
            }
        }
    }

    pub fn select_all(&mut self) {
        self.selected_fossils = (0..self.fossils.len()).collect();
        self.status_message = Some(format!("Selected all {} files", self.fossils.len()));
    }

    pub fn deselect_all(&mut self) {
        let count = self.selected_fossils.len();
        self.selected_fossils.clear();
        self.status_message = Some(format!("Deselected {} files", count));
    }

    pub fn toggle_preview(&mut self) {
        self.show_preview = !self.show_preview;
        self.status_message = Some(format!("Preview panel {}", if self.show_preview { "shown" } else { "hidden" }));
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        self.status_message = Some(format!("Help {}", if self.show_help { "shown" } else { "hidden" }));
    }

    pub fn handle_char_input(&mut self, c: char) {
        match self.input_mode {
            InputMode::Normal => {
                // Normal mode - characters are handled by events
            }
            InputMode::TagInput | InputMode::TagDigInput => {
                if c.is_alphanumeric() || c == ' ' || c == '_' || c == '-' || c == '.' {
                    self.input_buffer.push(c);
                }
            }
        }
    }

    pub fn handle_backspace(&mut self) {
        if self.input_mode != InputMode::Normal {
            self.input_buffer.pop();
        }
    }

    pub fn handle_enter(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.input_mode {
            InputMode::TagInput => {
                let tag = if self.input_buffer.is_empty() {
                    None
                } else {
                    Some(self.input_buffer.clone())
                };
                self.bury_with_tag(tag)?;
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            }
            InputMode::TagDigInput => {
                if !self.input_buffer.is_empty() {
                    self.dig_by_tag(&self.input_buffer.clone())?;
                } else {
                    self.status_message = Some("No tag specified".to_string());
                }
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            }
            InputMode::Normal => {
                // No special action in normal mode
            }
        }
        Ok(())
    }

    pub fn handle_escape(&mut self) {
        if self.input_mode != InputMode::Normal {
            self.input_mode = InputMode::Normal;
            self.input_buffer.clear();
            self.status_message = None;
        }
    }


    pub fn start_tag_input(&mut self) {
        self.input_mode = InputMode::TagInput;
        self.input_buffer.clear();
    }

    pub fn start_tag_dig_input(&mut self) {
        self.input_mode = InputMode::TagDigInput;
        self.input_buffer.clear();
    }

    pub fn track_selected(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.selected_fossils.is_empty() {
            self.status_message = Some("Track command pressed - cannot track already tracked files".to_string());
            return Ok(());
        }

        if !self.untracked_files.is_empty() {
            self.status_message = Some(format!("Track command pressed - would track {} files", self.untracked_files.len()));
        } else {
            self.status_message = Some("Track command pressed - no untracked files to track".to_string());
        }
        Ok(())
    }

    pub fn bury_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.status_message = Some("Bury All command pressed - would bury all changes".to_string());
        Ok(())
    }

    pub fn bury_with_tag(&mut self, tag: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
        let tag_string = tag.unwrap_or_default();
        let msg = if !tag_string.is_empty() {
            format!("Bury with tag command pressed - would bury changes with tag: {}", tag_string)
        } else {
            "Bury with tag command pressed - would bury changes".to_string()
        };
        self.status_message = Some(msg);
        Ok(())
    }

    pub fn surface(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Find the maximum layer (layers are sorted in descending order)
        let max_layer = self.layers.first().copied().unwrap_or(0);
        
        // Update the surface layer to the maximum layer
        self.config.surface_layer = max_layer;
        
        self.status_message = Some(format!("Surface command pressed - would surface to layer {}", max_layer));
        Ok(())
    }

    pub fn dig_to_layer(&mut self, layer: u32) -> Result<(), Box<dyn std::error::Error>> {
        self.status_message = Some(format!("Dig command pressed - would dig to layer {}", layer));
        Ok(())
    }

    pub fn dig_by_tag(&mut self, tag: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.status_message = Some(format!("Dig by tag command pressed - would dig files with tag '{}'", tag));
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.status_message = Some("Refresh command pressed - would refresh file status".to_string());
        Ok(())
    }

    pub fn file_has_changes(
        &self,
        tracked_file: &FossilRecord,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let path = PathBuf::from(&tracked_file.original_path);
        if !path.exists() {
            return Ok(true); // Missing file is a change
        }

        let content = fs::read(&path)?;
        let current_hash = crate::utils::hash_content(&content);
        Ok(current_hash != tracked_file.last_content_hash)
    }

    fn reload_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.config = load_config()?;

        self.fossils = self
            .config
            .fossils
            .iter()
            .map(|(hash, file)| (hash.clone(), file.clone()))
            .collect();

        let mut all_layers: BTreeSet<u32> = BTreeSet::new();
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
