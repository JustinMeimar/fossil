#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Command,
}

pub struct App {
    pub fossils: Vec<FossilDisplay>,
    pub selected_index: usize,
    pub mode: AppMode,
    pub command_input: String,
    pub should_quit: bool,
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
        let fossil_data = Self::load_fossils()?;
        Ok(App {
            fossils: fossil_data,
            selected_index: 0,
            mode: AppMode::Normal,
            command_input: String::new(),
            should_quit: false,
        })
    }

    fn load_fossils() -> Result<Vec<FossilDisplay>, Box<dyn std::error::Error>> {
        use crate::config::FossilDb;
        
        let db = FossilDb::open_default()?;
        let fossils = db.get_all_fossils()?;
        
        let mut fossil_displays = Vec::new();
        for fossil in fossils {
            let total_versions = fossil.versions.len();
            let tag_count = fossil.versions.iter().filter(|v| v.tag.is_some()).count();
            let current_content = fossil.get_version_content(fossil.cur_version)?;
            let preview = String::from_utf8_lossy(&current_content);
            let truncated_preview = if preview.len() > 50 {
                format!("{}...", &preview[..50])
            } else {
                preview.to_string()
            };
            
            fossil_displays.push(FossilDisplay {
                path: fossil.path.display().to_string(),
                current_version: fossil.cur_version,
                total_versions,
                tag_count,
                preview: truncated_preview.replace('\n', " "),
            });
        }
        
        Ok(fossil_displays)
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_index < self.fossils.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    pub fn enter_command_mode(&mut self) {
        self.mode = AppMode::Command;
        self.command_input.clear();
    }

    pub fn exit_command_mode(&mut self) {
        self.mode = AppMode::Normal;
        self.command_input.clear();
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
            // For now, just exit command mode
            // TODO: Implement actual command execution
            self.exit_command_mode();
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}