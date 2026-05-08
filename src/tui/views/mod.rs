pub mod analysis_popup;
pub mod bury_popup;
pub mod grid;
pub mod help;
pub mod main_view;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, Paragraph,
};

use crate::record::Record;
use crate::tui::theme;

// VimNav

trait VimNav {
    fn pos(&self) -> usize;
    fn set_pos(&mut self, pos: usize);
    fn max_pos(&self) -> usize;

    fn nav(&mut self, key: KeyEvent) -> bool {
        let max = self.max_pos();
        let pos = self.pos();
        let next = match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some((pos + 1).min(max)),
            KeyCode::Char('k') | KeyCode::Up => Some(pos.saturating_sub(1)),
            KeyCode::Char('g') => Some(0),
            KeyCode::Char('G') => Some(max),
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                Some((pos + 12).min(max))
            }
            KeyCode::Char('u')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                Some(pos.saturating_sub(12))
            }
            _ => None,
        };
        if let Some(p) = next {
            self.set_pos(p);
            true
        } else {
            false
        }
    }
}

// AppAction

pub enum AppAction {
    None,
    Quit,
    Flash(String),
    ShowHelp,
    Edit(std::path::PathBuf),
}

// SelectList

pub struct ListEntry {
    pub name: String,
    pub detail: String,
    pub tag: Option<(String, Color)>,
}

pub struct SelectList {
    pub selected: usize,
    pub offset: usize,
    pub entries: Vec<ListEntry>,
}

impl SelectList {
    pub fn new(entries: Vec<ListEntry>) -> Self {
        Self {
            selected: 0,
            offset: 0,
            entries,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn handle_nav(&mut self, key: KeyEvent) -> bool {
        self.nav(key)
    }

    pub fn ensure_visible(&mut self, height: usize) {
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected - height + 1;
        }
    }
}

impl VimNav for SelectList {
    fn pos(&self) -> usize {
        self.selected
    }
    fn set_pos(&mut self, pos: usize) {
        self.selected = pos;
    }
    fn max_pos(&self) -> usize {
        self.entries.len().saturating_sub(1)
    }
}

// ScrollBuffer

pub struct ScrollBuffer {
    pub lines: Vec<String>,
    pub scroll: u16,
    pub h_scroll: u16,
}

impl ScrollBuffer {
    pub fn new(lines: Vec<String>) -> Self {
        Self {
            lines,
            scroll: 0,
            h_scroll: 0,
        }
    }

    pub fn from_text(text: &str) -> Self {
        Self::new(text.lines().map(|l| l.to_string()).collect())
    }

    fn max_h_scroll(&self) -> u16 {
        self.lines
            .iter()
            .map(|l| l.len() as u16)
            .max()
            .unwrap_or(0)
    }

    fn max_scroll(&self) -> u16 {
        (self.lines.len() as u16).saturating_sub(1)
    }

    pub fn handle_nav(&mut self, key: KeyEvent) -> bool {
        if self.nav(key) {
            return true;
        }
        match key.code {
            KeyCode::Char('l') | KeyCode::Right => {
                self.h_scroll = (self.h_scroll + 8).min(self.max_h_scroll());
                true
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.h_scroll = self.h_scroll.saturating_sub(8);
                true
            }
            _ => false,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let text = self.lines.join("\n");
        let paragraph = Paragraph::new(text)
            .style(Style::default().fg(theme::TEXT))
            .scroll((self.scroll, self.h_scroll));
        frame.render_widget(paragraph, area);
    }
}

impl VimNav for ScrollBuffer {
    fn pos(&self) -> usize {
        self.scroll as usize
    }
    fn set_pos(&mut self, pos: usize) {
        self.scroll = pos as u16;
    }
    fn max_pos(&self) -> usize {
        self.max_scroll() as usize
    }
}

// Record metadata (pure)

fn metadata_lines(record: &Record) -> Vec<String> {
    let m = &record.manifest;
    vec![
        format!("fossil:      {}", m.fossil),
        format!("project:     {}", m.project),
        format!("timestamp:   {}", m.timestamp),
        format!(
            "variant:     {}",
            m.variant.as_ref().map(|v| v.as_str()).unwrap_or("-")
        ),
        format!("command:     {}", m.command),
        format!("iterations:  {}", m.iterations),
        format!("git:         {} ({})", m.git.commit, m.git.branch),
        format!(
            "cpu:         core={} gov={} boost={}",
            m.cpu.pinned_core, m.cpu.governor, m.cpu.boost
        ),
        format!("kernel:      {}", m.kernel),
    ]
}

// PreviewPanel

pub struct PreviewPanel {
    pub title: String,
    pub metadata: Vec<String>,
    pub content_title: String,
    pub content: ScrollBuffer,
}

impl PreviewPanel {
    pub fn from_record(record: &Record) -> Self {
        let title = record
            .manifest
            .variant
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| record.id());
        let results_path = record.dir.join("results.json");
        let raw = std::fs::read_to_string(&results_path).ok();
        let lines: Vec<String> = match &raw {
            Some(s) => s.lines().map(String::from).collect(),
            None => {
                vec!["(no results)".to_string()]
            }
        };
        Self {
            title,
            metadata: metadata_lines(record),
            content_title: "results.json".to_string(),
            content: ScrollBuffer::new(lines),
        }
    }

    pub fn set_content(&mut self, title: &str, text: &str) {
        self.content_title = title.to_string();
        self.content = ScrollBuffer::from_text(text);
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let border_color = if focused { theme::FOCUS } else { theme::MUTED };
        let title_color = if focused { theme::FOCUS } else { theme::TEXT };
        let meta_h = self.metadata.len() as u16 + 2; // +2 for border

        let [meta_area, content_area] =
            Layout::vertical([Constraint::Length(meta_h), Constraint::Min(0)])
                .areas(area);

        // metadata panel
        let meta_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                format!(" {} ", self.title),
                Style::default().fg(title_color),
            ));
        let meta_inner = meta_block.inner(meta_area);
        frame.render_widget(meta_block, meta_area);
        let meta_text = self.metadata.join("\n");
        frame.render_widget(
            Paragraph::new(meta_text).style(Style::default().fg(theme::TEXT)),
            meta_inner,
        );

        // content panel
        let content_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                format!(" {} ", self.content_title),
                Style::default().fg(title_color),
            ));
        let content_inner = content_block.inner(content_area);
        frame.render_widget(content_block, content_area);
        self.content.render(frame, content_inner);
    }

    pub fn handle_nav(&mut self, key: KeyEvent) -> bool {
        self.content.handle_nav(key)
    }
}

// SelectorPopup

pub enum SelectorAction {
    None,
    Select(usize),
    Dismiss,
}

pub struct SelectorPopup {
    pub title: String,
    pub list: SelectList,
}

impl SelectorPopup {
    pub fn new(title: impl Into<String>, entries: Vec<ListEntry>) -> Self {
        Self {
            title: title.into(),
            list: SelectList::new(entries),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SelectorAction {
        if self.list.handle_nav(key) {
            return SelectorAction::None;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char('l') => {
                SelectorAction::Select(self.list.selected)
            }
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => {
                SelectorAction::Dismiss
            }
            _ => SelectorAction::None,
        }
    }

    pub fn render_popup(&mut self, frame: &mut Frame, area: Rect) {
        let width = 50u16.min(area.width.saturating_sub(4));
        let item_count = self.list.len() as u16;
        let height = (item_count + 2)
            .max(5)
            .min(area.height.saturating_sub(4));

        let [popup] = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(
                Layout::vertical([Constraint::Length(height)])
                    .flex(Flex::Center)
                    .areas::<1>(area)[0],
            );

        frame.render_widget(Clear, popup);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme::FOCUS))
            .title(Span::styled(
                format!(" {} ", self.title),
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let height = inner.height as usize;
        self.list.ensure_visible(height);

        let items: Vec<ListItem> = self
            .list
            .entries
            .iter()
            .enumerate()
            .skip(self.list.offset)
            .take(height)
            .map(|(i, entry)| {
                let sel = i == self.list.selected;
                let style = if sel {
                    Style::default()
                        .fg(theme::FOCUS)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme::TEXT)
                };
                let prefix = if sel { ">" } else { " " };
                let mut spans = vec![
                    Span::styled(format!("{prefix} {}", entry.name), style),
                    Span::styled(
                        format!("  {}", entry.detail),
                        Style::default().fg(theme::MUTED),
                    ),
                ];
                if let Some((ref tag, color)) = entry.tag {
                    spans.push(Span::styled(
                        format!("  {tag}"),
                        Style::default().fg(color),
                    ));
                }
                ListItem::new(Line::from(spans))
            })
            .collect();
        frame.render_widget(List::new(items), inner);
    }
}
