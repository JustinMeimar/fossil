use crate::config::{Fossil, FossilDb};
use crate::cli::Commands;
use crate::dispatch_command;
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Command,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandType {
    General,
    Bury,
    Dig,
}


pub struct App {
    pub fossils: Vec<Fossil>,
    pub cursor_idx: usize,
    pub select_fossils: HashSet<usize>,
    pub mode: AppMode,
    pub command_input: String,
    pub command_type: CommandType,
    pub should_quit: bool,
    pub status_message: Option<String>,
    last_refresh: Instant,
}

#[derive(Clone)]
pub struct FossilDisplay {
    pub path: String,
    pub current_version: usize,
    pub total_versions: usize,
    pub tag_count: usize,
    pub preview: String,
}

impl App {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let fossils = Self::load_fossils()?;
        Ok(App {
            fossils,
            cursor_idx: 0,
            select_fossils: HashSet::new(),
            mode: AppMode::Normal,
            command_input: String::new(),
            command_type: CommandType::General,
            should_quit: false,
            status_message: None,
            last_refresh: Instant::now(),
        })
    }

    fn load_fossils() -> Result<Vec<Fossil>, Box<dyn std::error::Error>> {
        
        let db = FossilDb::open_default()?;
        let fossils = db.get_all_fossils()?;
        // 
        // let mut fossil_displays = Vec::new();
        // for fossil in fossils {
        //     let total_versions = fossil.versions.len();
        //     let tag_count = fossil.versions.iter()
        //         .filter(|v| v.tag.is_some()).count();
        //     let current_content = fossil.get_version_content(fossil.cur_version)?;
        //     let preview = String::from_utf8_lossy(&current_content);
        //     let truncated_preview = if preview.len() > 50 {
        //         format!("{}...", &preview[..50])
        //     } else {
        //         preview.to_string()
        //     };
        //     
        //     fossil_displays.push(FossilDisplay {
        //         path: fossil.path.display().to_string(),
        //         current_version: fossil.cur_version,
        //         total_versions,
        //         tag_count,
        //         preview: truncated_preview.replace('\n', " "),
        //     });
        // }
        
        Ok(fossils)
    }

    pub fn refresh_data(&mut self) {
        if let Ok(new_fossils) = Self::load_fossils() {
            self.fossils = new_fossils;
            self.last_refresh = Instant::now();
            
            // Keep cursor in bounds
            if self.cursor_idx >= self.fossils.len() {
                self.cursor_idx = self.fossils.len().saturating_sub(1);
            }
            
            // Clean up invalid selections
            self.select_fossils.retain(|&idx| idx < self.fossils.len());
        }
    }

    pub fn should_auto_refresh(&self) -> bool {
        self.last_refresh.elapsed() >= Duration::from_secs(1)
    }

    pub fn move_up(&mut self) {
        if self.cursor_idx > 0 {
            self.cursor_idx -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_idx < self.fossils.len().saturating_sub(1) {
            self.cursor_idx += 1;
        }
    }
    
    pub fn select_fossil(&mut self) {
        if !self.select_fossils.insert(self.cursor_idx) {
            self.select_fossils.remove(&self.cursor_idx);
        } 
    }

    pub fn enter_command_mode(&mut self) {
        self.mode = AppMode::Command;
        self.command_input.clear();
        self.command_type = CommandType::General;
    }

    pub fn enter_bury_mode(&mut self) {
        self.mode = AppMode::Command;
        self.command_input.clear();
        self.command_type = CommandType::Bury;
    }

    pub fn enter_dig_mode(&mut self) {
        self.mode = AppMode::Command;
        self.command_input.clear();
        self.command_type = CommandType::Dig;
    }

    pub fn exit_command_mode(&mut self) {
        self.mode = AppMode::Normal;
        self.command_input.clear();
        self.command_type = CommandType::General;
    }

    pub fn add_char_to_command(&mut self, c: char) {
        if self.mode == AppMode::Command {
            self.command_input.push(c);
        }
    }

    pub fn remove_char_from_command(&mut self) {
        if self.mode == AppMode::Command {
            self.command_input.pop();
        }
    }

    pub fn execute_command(&mut self) {
        if self.mode == AppMode::Command {
            match self.command_type {
                CommandType::General => {
                    // TODO: Parse general commands
                    self.exit_command_mode();
                }
                CommandType::Bury => {
                    let tag = if self.command_input.trim().is_empty() {
                        None
                    } else {
                        Some(self.command_input.trim().to_string())
                    };
                    self.exit_command_mode();
                    self.execute_bury_with_tag(tag);
                }
                CommandType::Dig => {
                    let input = self.command_input.trim().to_string();
                    self.exit_command_mode();
                    
                    if input.is_empty() {
                        self.execute_dig_with_params(None, None);
                    } else if input.chars().all(|c| c.is_ascii_digit()) {
                        // It's a version number
                        if let Ok(version) = input.parse::<usize>() {
                            self.execute_dig_with_params(None, Some(version));
                        } else {
                            self.status_message = Some("Invalid version number".to_string());
                        }
                    } else {
                        // It's a tag
                        self.execute_dig_with_params(Some(input), None);
                    }
                }
            }
        }
    }

    pub fn execute_cli_command(&mut self, command: Commands) {
        self.status_message = match dispatch_command(Some(command)) {
            Ok(_) => Some("Command executed successfully".to_string()),
            Err(e) => Some(format!("Command failed: {}", e)),
        };
        self.refresh_data();
    }

    pub fn execute_bury_with_tag(&mut self, tag: Option<String>) {
        let selected_files = self.get_selected_file_paths();
        let command = Commands::Bury { 
            tag, 
            files: selected_files 
        };
        self.execute_cli_command(command);
    }

    pub fn execute_dig_with_params(&mut self, tag: Option<String>, version: Option<usize>) {
        let selected_files = self.get_selected_file_paths();
        let command = Commands::Dig { 
            tag, 
            version, 
            files: selected_files 
        };
        self.execute_cli_command(command);
    }

    pub fn execute_surface(&mut self) {
        let command = Commands::Surface;
        self.execute_cli_command(command);
    }

    pub fn execute_track(&mut self) {
        let selected_files = self.get_selected_file_paths();
        if !selected_files.is_empty() {
            let command = Commands::Track { files: selected_files };
            self.execute_cli_command(command);
        } else {
            self.status_message = Some("No files selected to track".to_string());
        }
    }

    pub fn execute_untrack(&mut self) {
        let selected_files = self.get_selected_file_paths();
        if !selected_files.is_empty() {
            let command = Commands::Untrack { files: selected_files };
            self.execute_cli_command(command);
        } else {
            self.status_message = Some("No files selected to untrack".to_string());
        }
    }
    
    fn get_selected_file_paths(&self) -> Vec<String> {
        self.select_fossils
            .iter()
            .filter_map(|&idx| self.fossils.get(idx).map(|f| f.path.to_string_lossy().to_string()))
            .collect()
    } 

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }


    pub fn get_total_fossils(&self) -> usize {
        self.fossils.len()
    }

    pub fn get_total_versions(&self) -> usize {
        self.fossils.iter().map(|f| f.versions.len()).sum()
    }

    pub fn get_tagged_versions_count(&self) -> usize {
        self.fossils.iter()
            .flat_map(|f| &f.versions)
            .filter(|v| v.tag.is_some())
            .count()
    }
}
