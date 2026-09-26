use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use crate::tui::theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::artifact::ArtifactFormat;
use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::fossil::ConfigurationKey;
use crate::fossil::Fossil;
use crate::project::Project;
use crate::record::Record;

use super::analysis_popup::{AnalysisAction, AnalysisPopupState};
use super::bury_popup::{BuryAction, BuryPopupState};
use super::grid::VariantGrid;
use super::{
    AppAction, ListEntry, PreviewPanel, SelectorAction, SelectorPopup,
};

// shared helpers

fn load_fossil_records(fossils: &[Fossil], idx: usize) -> Vec<Record> {
    fossils
        .get(idx)
        .and_then(|f| Fossil::load(&f.path).ok())
        .and_then(|f| f.find_records(None, None).ok())
        .map(|mut recs| {
            recs.reverse();
            recs
        })
        .unwrap_or_default()
}

const SPINNER: &[&str] = &["   ", ".  ", ".. ", "...", " ..", "  ."];

pub fn spinner_frame(start: Instant) -> &'static str {
    let idx = (start.elapsed().as_millis() / 300) as usize % SPINNER.len();
    SPINNER[idx]
}

pub fn render_toast(frame: &mut Frame, area: Rect, text: &str, color: Color) {
    let width = (text.len() as u16 + 4).min(area.width);
    let [popup] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(
            Layout::vertical([Constraint::Length(3)])
                .flex(Flex::Center)
                .areas::<1>(area)[0],
        );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(color)),
        inner,
    );
}

// Focus & Mode

#[derive(PartialEq)]
enum Focus {
    Master,
    Detail,
}

struct BgTask {
    label: String,
    rx: mpsc::Receiver<Result<String, String>>,
    start: Instant,
}

fn poll_result(
    rx: &mpsc::Receiver<Result<String, String>>,
) -> Option<Result<String, String>> {
    match rx.try_recv() {
        Ok(r) => Some(r),
        Err(mpsc::TryRecvError::Disconnected) => {
            Some(Err("background task panicked".into()))
        }
        Err(mpsc::TryRecvError::Empty) => None,
    }
}

enum Mode {
    Browse,
    ProjectSelector(SelectorPopup),
    FossilSelector(SelectorPopup),
    EditSelector(SelectorPopup, Vec<PathBuf>),
    AnalysisPopup(Box<AnalysisPopupState>),
    BuryPopup(BuryPopupState),
    ArtifactSelector(SelectorPopup, Vec<ConfigurationKey>),
    ArtifactRunning(BgTask, PathBuf, ArtifactFormat),
    DeleteConfirm(usize),
}

// MainView

const RECORDS_POLL_INTERVAL: Duration = Duration::from_millis(500);

pub struct MainView {
    projects: Vec<Project>,
    project_idx: usize,
    fossils: Vec<Fossil>,
    fossil_idx: usize,
    records: Vec<Record>,
    grid: VariantGrid,
    selected: BTreeSet<usize>,
    preview: Option<PreviewPanel>,
    preview_index: Option<usize>,
    focus: Focus,
    mode: Mode,
    bg_bury: Option<BgTask>,
    last_records_mtime: Option<SystemTime>,
    last_records_poll: Instant,
}

impl MainView {
    fn new(
        projects: Vec<Project>,
        fossils: Vec<Fossil>,
        records: Vec<Record>,
    ) -> Self {
        let grid = VariantGrid::from_records(&records);
        let initial_idx = grid.current_record_idx();
        let preview = initial_idx
            .and_then(|i| records.get(i))
            .map(PreviewPanel::from_record);
        Self {
            project_idx: 0,
            projects,
            fossil_idx: 0,
            fossils,
            grid,
            selected: BTreeSet::new(),
            preview,
            preview_index: initial_idx,
            records,
            focus: Focus::Master,
            mode: Mode::Browse,
            bg_bury: None,
            last_records_mtime: None,
            last_records_poll: Instant::now(),
        }
    }

    pub fn load(projects_dir: PathBuf) -> Result<Self, FossilError> {
        let projects = Project::list_all(&projects_dir)?;
        let last = std::fs::read_to_string(projects_dir.join(".last")).ok();
        let project_idx = last
            .and_then(|name| {
                let name = name.trim();
                projects.iter().position(|p| p.config.name == name)
            })
            .unwrap_or(0);
        let (fossils, records) = if let Some(p) = projects.get(project_idx) {
            let fossils = Fossil::list_all(&p.path)?;
            let records = load_fossil_records(&fossils, 0);
            (fossils, records)
        } else {
            (Vec::new(), Vec::new())
        };
        let mut view = Self::new(projects, fossils, records);
        view.project_idx = project_idx;
        view.last_records_mtime = view.current_records_mtime();
        Ok(view)
    }

    pub fn project_name(&self) -> &str {
        self.projects
            .get(self.project_idx)
            .map(|p| p.config.name.as_str())
            .unwrap_or("(no project)")
    }

    pub fn bg_bury_label(&self) -> Option<String> {
        self.bg_bury
            .as_ref()
            .map(|b| format!("burying {} {}", b.label, spinner_frame(b.start)))
    }

    pub fn fossil_name(&self) -> &str {
        self.fossils
            .get(self.fossil_idx)
            .map(|f| f.config.name.as_str())
            .unwrap_or("(no fossil)")
    }

    pub fn hints(&self) -> Vec<(&str, &str)> {
        match &self.mode {
            Mode::ProjectSelector(..)
            | Mode::FossilSelector(..)
            | Mode::EditSelector(..)
            | Mode::ArtifactSelector(..) => {
                vec![("enter", "select"), ("esc", "close")]
            }
            Mode::AnalysisPopup(_)
            | Mode::BuryPopup(_)
            | Mode::ArtifactRunning(..) => {
                vec![("enter", "run"), ("esc", "close")]
            }
            Mode::DeleteConfirm(_) => {
                vec![("y", "confirm delete"), ("n/esc", "cancel")]
            }
            Mode::Browse => match self.focus {
                Focus::Master => {
                    let mut h = vec![
                        ("hjkl", "navigate"),
                        ("space", "select"),
                        ("tab", "preview"),
                        ("e", "edit"),
                        ("a", "analyze"),
                        ("b", "bury"),
                        ("d", "delete"),
                        ("?", "help"),
                    ];
                    if !self.selected.is_empty() {
                        h.insert(2, ("esc", "clear"));
                    }
                    h
                }
                Focus::Detail => vec![
                    ("j/k", "scroll"),
                    ("h/l", "pan"),
                    ("c", "copy"),
                    ("tab", "list"),
                    ("d", "artifacts"),
                ],
            },
        }
    }

    pub fn tick(&mut self) -> AppAction {
        if self.last_records_poll.elapsed() >= RECORDS_POLL_INTERVAL {
            self.last_records_poll = Instant::now();
            let current = self.current_records_mtime();
            if current != self.last_records_mtime {
                self.reload_records();
            }
        }
        if let Mode::AnalysisPopup(ref mut popup) = self.mode {
            match popup.tick() {
                AnalysisAction::Output(name, output) => {
                    if let Some(ref mut p) = self.preview {
                        p.set_content(&format!("analysis: {name}"), &output);
                    }
                    self.mode = Mode::Browse;
                    self.focus = Focus::Detail;
                }
                AnalysisAction::Flash(msg) => {
                    self.mode = Mode::Browse;
                    return AppAction::Flash(msg);
                }
                _ => {}
            }
        }
        if let Some(ref bg) = self.bg_bury {
            if let Some(result) = poll_result(&bg.rx) {
                let ok = result.is_ok();
                self.bg_bury = None;
                if ok {
                    self.reload_records();
                }
                return AppAction::Flash(result.unwrap_or_else(|e| e));
            }
        }
        if let Mode::ArtifactRunning(ref task, ref output_path, format) =
            self.mode
        {
            if let Some(result) = poll_result(&task.rx) {
                let path = output_path.clone();
                self.mode = Mode::Browse;
                if let Err(error) = result {
                    return AppAction::Flash(error);
                }
                return match format {
                    ArtifactFormat::Pdf => {
                        crate::io::open(&path);
                        AppAction::Flash(format!("wrote {}", path.display()))
                    }
                    ArtifactFormat::Json => AppAction::Edit(path),
                };
            }
        }
        AppAction::None
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> AppAction {
        enum Resolved {
            None,
            Dismiss,
            SelectProject(usize),
            SelectFossil(usize),
            EditFile(PathBuf),
            AnalysisOutput(String, String),
            RunArtifact(usize),
            Flash(String),
            Browse,
        }

        let resolved = match &mut self.mode {
            Mode::ProjectSelector(sel) => match sel.handle_key(key) {
                SelectorAction::Select(i) => Resolved::SelectProject(i),
                SelectorAction::Dismiss => Resolved::Dismiss,
                SelectorAction::None => Resolved::None,
            },
            Mode::FossilSelector(sel) => match sel.handle_key(key) {
                SelectorAction::Select(i) => Resolved::SelectFossil(i),
                SelectorAction::Dismiss => Resolved::Dismiss,
                SelectorAction::None => Resolved::None,
            },
            Mode::EditSelector(sel, paths) => match sel.handle_key(key) {
                SelectorAction::Select(i) => {
                    let path = paths[i].clone();
                    Resolved::EditFile(path)
                }
                SelectorAction::Dismiss => Resolved::Dismiss,
                SelectorAction::None => Resolved::None,
            },
            Mode::AnalysisPopup(popup) => match popup.handle_key(key) {
                AnalysisAction::Dismiss => Resolved::Dismiss,
                AnalysisAction::Output(name, output) => {
                    Resolved::AnalysisOutput(name, output)
                }
                AnalysisAction::Flash(msg) => Resolved::Flash(msg),
                AnalysisAction::None => Resolved::None,
            },
            Mode::ArtifactSelector(sel, _names) => match sel.handle_key(key) {
                SelectorAction::Select(i) => Resolved::RunArtifact(i),
                SelectorAction::Dismiss => Resolved::Dismiss,
                SelectorAction::None => Resolved::None,
            },
            Mode::ArtifactRunning(..) => Resolved::None,
            Mode::BuryPopup(popup) => match popup.handle_key(key) {
                BuryAction::Dismiss => Resolved::Dismiss,
                BuryAction::Started(variant, rx) => {
                    self.bg_bury = Some(BgTask {
                        label: variant,
                        rx,
                        start: Instant::now(),
                    });
                    Resolved::Dismiss
                }
                BuryAction::None => Resolved::None,
            },
            Mode::DeleteConfirm(idx) => {
                let idx = *idx;
                match key.code {
                    KeyCode::Char('y') => {
                        let msg = self.execute_delete(idx);
                        self.mode = Mode::Browse;
                        return match msg {
                            Ok(m) => AppAction::Flash(m),
                            Err(e) => AppAction::Flash(e.to_string()),
                        };
                    }
                    _ => Resolved::Dismiss,
                }
            }
            Mode::Browse => Resolved::Browse,
        };

        match resolved {
            Resolved::None => return AppAction::None,
            Resolved::Dismiss => {
                self.mode = Mode::Browse;
                return AppAction::None;
            }
            Resolved::SelectProject(i) => {
                self.apply_project_selection(i);
                self.mode = Mode::Browse;
                return AppAction::None;
            }
            Resolved::SelectFossil(i) => {
                self.apply_fossil_selection(i);
                self.mode = Mode::Browse;
                return AppAction::None;
            }
            Resolved::EditFile(path) => {
                self.mode = Mode::Browse;
                return AppAction::Edit(path);
            }
            Resolved::AnalysisOutput(name, output) => {
                if let Some(ref mut p) = self.preview {
                    p.set_content(&format!("analysis: {name}"), &output);
                }
                self.mode = Mode::Browse;
                self.focus = Focus::Detail;
                return AppAction::None;
            }
            Resolved::RunArtifact(i) => {
                return match self.start_artifact(i) {
                    Some(message) => AppAction::Flash(message),
                    None => AppAction::None,
                };
            }
            Resolved::Flash(msg) => {
                self.mode = Mode::Browse;
                return AppAction::Flash(msg);
            }
            Resolved::Browse => {}
        }

        self.handle_browse_key(key)
    }

    fn handle_browse_key(&mut self, key: KeyEvent) -> AppAction {
        match self.focus {
            Focus::Detail => {
                match key.code {
                    KeyCode::Tab | KeyCode::Esc => self.focus = Focus::Master,
                    KeyCode::Char('d') => match self.open_artifact_selector() {
                        Some(msg) => return AppAction::Flash(msg),
                        None => {}
                    },
                    KeyCode::Char('c') => {
                        if let Some(ref panel) = self.preview {
                            let text = panel.content.lines.join("\n");
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_text(text);
                            }
                            return AppAction::Flash("copied".into());
                        }
                    }
                    _ => {
                        if let Some(ref mut panel) = self.preview {
                            panel.handle_nav(key);
                        }
                    }
                }
                AppAction::None
            }
            Focus::Master => {
                let prev_col = self.grid.col;
                let prev_row = self.grid.row;
                if self.grid.handle_key(key) {
                    if self.grid.col != prev_col || self.grid.row != prev_row {
                        self.sync_preview();
                    }
                    return AppAction::None;
                }
                match key.code {
                    KeyCode::Char(' ') => {
                        if let Some(idx) = self.grid.current_record_idx()
                            && !self.selected.remove(&idx)
                        {
                            self.selected.insert(idx);
                        }
                        AppAction::None
                    }
                    KeyCode::Esc => {
                        self.selected.clear();
                        AppAction::None
                    }
                    KeyCode::Tab => {
                        self.focus = Focus::Detail;
                        AppAction::None
                    }
                    KeyCode::Char('p') => {
                        self.open_project_selector();
                        AppAction::None
                    }
                    KeyCode::Char('f') => {
                        self.open_fossil_selector();
                        AppAction::None
                    }
                    KeyCode::Char('a') | KeyCode::Char('s') => {
                        self.open_analysis_popup();
                        AppAction::None
                    }
                    KeyCode::Char('b') => match self.open_bury_popup() {
                        Some(msg) => AppAction::Flash(msg),
                        None => AppAction::None,
                    },
                    KeyCode::Char('e') => {
                        self.open_edit_selector();
                        AppAction::None
                    }
                    KeyCode::Char('d') => {
                        if let Some(idx) = self.grid.current_record_idx() {
                            self.mode = Mode::DeleteConfirm(idx);
                        }
                        AppAction::None
                    }
                    KeyCode::Char('?') => AppAction::ShowHelp,
                    KeyCode::Char('q') => AppAction::Quit,
                    _ => AppAction::None,
                }
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let master_focused = self.focus == Focus::Master;

        if self.records.is_empty() {
            let msg = if self.projects.is_empty() {
                "no projects found"
            } else if self.fossils.is_empty() {
                "no fossils in this project"
            } else {
                "no records found"
            };
            frame.render_widget(
                Paragraph::new(format!(" {msg}  (p:project  f:fossil)"))
                    .style(Style::default().fg(theme::MUTED)),
                area,
            );
        } else {
            let [master, detail] = Layout::horizontal([
                Constraint::Percentage(65),
                Constraint::Percentage(35),
            ])
            .areas(area);

            let master_border = if master_focused {
                theme::FOCUS
            } else {
                theme::MUTED
            };
            let title_color = if master_focused {
                theme::FOCUS
            } else {
                theme::TEXT
            };
            let sel_count = self.selected.len();
            let title_line = if sel_count > 0 {
                Line::from(vec![
                    Span::styled(" records ", Style::default().fg(title_color)),
                    Span::styled(
                        format!("│ {sel_count} selected "),
                        Style::default().fg(theme::MUTED),
                    ),
                ])
            } else {
                Line::from(Span::styled(
                    " records ",
                    Style::default().fg(title_color),
                ))
            };
            let master_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(master_border))
                .title(title_line);
            let master_inner = master_block.inner(master);
            frame.render_widget(master_block, master);
            self.render_grid(frame, master_inner);

            if let Some(ref panel) = self.preview {
                panel.render(frame, detail, !master_focused);
            }
        }

        match &mut self.mode {
            Mode::ProjectSelector(sel)
            | Mode::FossilSelector(sel)
            | Mode::EditSelector(sel, _)
            | Mode::ArtifactSelector(sel, _) => sel.render_popup(frame, area),
            Mode::AnalysisPopup(popup) => popup.render_popup(frame, area),
            Mode::BuryPopup(popup) => popup.render_popup(frame, area),
            Mode::ArtifactRunning(loading, _, _) => {
                let text = format!(
                    " emitting {} {}",
                    loading.label,
                    spinner_frame(loading.start),
                );
                render_toast(frame, area, &text, theme::WARN);
            }
            Mode::DeleteConfirm(idx) => {
                let idx = *idx;
                let label = self
                    .records
                    .get(idx)
                    .map(|r| {
                        let v = r.manifest.variant.as_str();
                        format!("{v} {}", r.manifest.timestamp)
                    })
                    .unwrap_or_default();
                render_toast(
                    frame,
                    area,
                    &format!(" delete {label}? (y/n) "),
                    theme::DANGER,
                );
            }
            Mode::Browse => {}
        }
    }

    // private

    fn sync_preview(&mut self) {
        let idx = match self.grid.current_record_idx() {
            Some(i) => i,
            None => return,
        };
        if self.preview_index == Some(idx) {
            return;
        }
        if let Some(record) = self.records.get(idx) {
            self.preview = Some(PreviewPanel::from_record(record));
            self.preview_index = Some(idx);
        }
    }

    fn set_records(&mut self, records: Vec<Record>) {
        self.grid = VariantGrid::from_records(&records);
        self.selected.clear();
        let initial_idx = self.grid.current_record_idx();
        self.preview = initial_idx
            .and_then(|i| records.get(i))
            .map(PreviewPanel::from_record);
        self.preview_index = initial_idx;
        self.records = records;
        self.focus = Focus::Master;
    }

    fn reload_records(&mut self) {
        let records = load_fossil_records(&self.fossils, self.fossil_idx);
        self.set_records(records);
        self.last_records_mtime = self.current_records_mtime();
    }

    fn current_records_mtime(&self) -> Option<SystemTime> {
        self.fossils
            .get(self.fossil_idx)
            .and_then(|f| std::fs::metadata(f.records_dir()).ok())
            .and_then(|m| m.modified().ok())
    }

    fn open_project_selector(&mut self) {
        let entries: Vec<ListEntry> = self
            .projects
            .iter()
            .map(|p| ListEntry {
                name: p.config.name.clone(),
                detail: p.config.description.clone().unwrap_or_default(),
                tag: None,
            })
            .collect();
        let mut sel = SelectorPopup::new("projects", entries);
        sel.list.selected = self.project_idx;
        self.mode = Mode::ProjectSelector(sel);
    }

    fn apply_project_selection(&mut self, idx: usize) {
        if let Some(p) = self.projects.get(idx) {
            let path = p.path.clone();
            self.project_idx = idx;
            if let Some(parent) = path.parent() {
                let _ = std::fs::write(parent.join(".last"), &p.config.name);
            }
            self.fossils = Fossil::list_all(&path).unwrap_or_default();
            self.fossil_idx = 0;
            self.reload_records();
        }
    }

    fn open_fossil_selector(&mut self) {
        let entries: Vec<ListEntry> = self
            .fossils
            .iter()
            .map(|f| {
                let nv = f.config.variants.len();
                let tag = if nv > 0 {
                    Some((format!("[{nv} variants]"), theme::WARN))
                } else {
                    None
                };
                ListEntry {
                    name: f.config.name.clone(),
                    detail: f.config.description.clone().unwrap_or_default(),
                    tag,
                }
            })
            .collect();
        let mut sel = SelectorPopup::new("fossils", entries);
        sel.list.selected = self.fossil_idx;
        self.mode = Mode::FossilSelector(sel);
    }

    fn apply_fossil_selection(&mut self, idx: usize) {
        if self.fossils.get(idx).is_some() {
            self.fossil_idx = idx;
            self.reload_records();
        }
    }

    fn execute_delete(&mut self, idx: usize) -> Result<String, FossilError> {
        let record = self.records.get(idx).ok_or_else(|| {
            FossilError::NotFound("no record selected".into())
        })?;
        let project = self.projects.get(self.project_idx).ok_or_else(|| {
            FossilError::NotFound("no project selected".into())
        })?;
        let id = record.id();
        project.delete_record(record)?;
        self.reload_records();
        Ok(format!("deleted {id}"))
    }

    fn current_fossil(&self) -> Option<Fossil> {
        self.fossils
            .get(self.fossil_idx)
            .and_then(|f| Fossil::load(&f.path).ok())
    }

    fn current_project_path(&self) -> PathBuf {
        self.projects
            .get(self.project_idx)
            .map(|p| p.path.clone())
            .unwrap_or_default()
    }

    fn open_analysis_popup(&mut self) {
        let fossil = match self.current_fossil() {
            Some(f) => f,
            None => return,
        };

        let selected_records = if self.selected.is_empty() {
            Vec::new()
        } else {
            let records: Vec<&Record> = self
                .selected
                .iter()
                .filter_map(|&i| self.records.get(i))
                .collect();

            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for r in &records {
                let v = r.manifest.variant.as_str();
                *counts.entry(v).or_default() += 1;
            }
            let has_dups = counts.values().any(|&c| c > 1);

            records
                .iter()
                .map(|r| {
                    let v = r.manifest.variant.as_str();
                    let label = if has_dups {
                        let ts = r.manifest.short_timestamp();
                        format!("{v} ({ts})")
                    } else {
                        v.to_string()
                    };
                    (label, (*r).clone())
                })
                .collect()
        };

        self.mode = Mode::AnalysisPopup(Box::new(AnalysisPopupState::new(
            fossil,
            self.current_project_path(),
            selected_records,
        )));
    }

    fn open_bury_popup(&mut self) -> Option<String> {
        if self.bg_bury.is_some() {
            return Some("bury already running".into());
        }
        let fossil = self.current_fossil()?;
        if fossil.config.variants.is_empty() {
            return Some("no variants configured".into());
        }
        self.mode = Mode::BuryPopup(BuryPopupState::new(
            &fossil,
            self.current_project_path(),
        ));
        None
    }

    fn open_artifact_selector(&mut self) -> Option<String> {
        let fossil = self.current_fossil()?;
        if fossil.config.artifacts.is_empty() {
            return Some("no artifacts configured".into());
        }
        let (names, entries) = fossil
            .config
            .artifacts
            .iter()
            .map(|(name, entry)| {
                (
                    name.clone(),
                    ListEntry {
                        name: name.to_string(),
                        detail: entry.script.as_str().into(),
                        tag: Some((
                            entry.format.extension().into(),
                            theme::MUTED,
                        )),
                    },
                )
            })
            .unzip();
        self.mode = Mode::ArtifactSelector(
            SelectorPopup::new("artifacts", entries),
            names,
        );
        None
    }

    fn start_artifact(&mut self, idx: usize) -> Option<String> {
        let name = match &self.mode {
            Mode::ArtifactSelector(_, names) => names.get(idx)?.clone(),
            _ => return Some("artifact selector is not active".into()),
        };
        let fossil = self.current_fossil()?;
        let project = self.projects.get(self.project_idx)?.clone();
        let artifact = match fossil.resolve_artifact(&name) {
            Ok(artifact) => artifact,
            Err(error) => return Some(error.to_string()),
        };
        let format = artifact.format();
        let destination = match artifact.output_path(&fossil, &project) {
            Ok(path) => path,
            Err(error) => return Some(error.to_string()),
        };
        let (tx, rx) = mpsc::channel();
        let artifact_name = name.clone();
        std::thread::spawn(move || {
            let result = crate::commands::emit_artifact(
                &fossil,
                &project,
                Some(&artifact_name),
                None,
                None,
                false,
            )
            .map(|path| format!("wrote {}", path.display()))
            .map_err(|error| error.to_string());
            let _ = tx.send(result);
        });
        self.mode = Mode::ArtifactRunning(
            BgTask {
                label: name.to_string(),
                rx,
                start: Instant::now(),
            },
            destination,
            format,
        );
        None
    }

    fn open_edit_selector(&mut self) {
        let fossil = match self.current_fossil() {
            Some(f) => f,
            None => return,
        };
        let mut entries = Vec::new();
        let mut paths: Vec<PathBuf> = Vec::new();

        entries.push(ListEntry {
            name: "fossil.toml".into(),
            detail: "config".into(),
            tag: None,
        });
        paths.push(fossil.path.join("fossil.toml"));

        for script in fossil.config.analyses.values() {
            entries.push(ListEntry {
                name: script.as_str().into(),
                detail: "analysis".into(),
                tag: None,
            });
            paths.push(fossil.path.join(script.as_str()));
        }

        for (name, entry) in &fossil.config.artifacts {
            let script = entry.script.as_str();
            entries.push(ListEntry {
                name: script.into(),
                detail: format!("artifact: {name}"),
                tag: None,
            });
            paths.push(fossil.path.join(script));
        }

        let project_toml = self.current_project_path().join("project.toml");
        if project_toml.exists() {
            entries.push(ListEntry {
                name: "project.toml".into(),
                detail: "project config".into(),
                tag: None,
            });
            paths.push(project_toml);
        }

        self.mode =
            Mode::EditSelector(SelectorPopup::new("edit", entries), paths);
    }

    pub fn reload(&mut self) {
        if let Some(p) = self.projects.get(self.project_idx) {
            self.fossils = Fossil::list_all(&p.path).unwrap_or_default();
            self.fossil_idx = self
                .fossil_idx
                .min(self.fossils.len().saturating_sub(1));
        }
        self.reload_records();
    }

    fn render_grid(&mut self, frame: &mut Frame, area: Rect) {
        if self.grid.columns.is_empty() {
            return;
        }

        let n_cols = self.grid.columns.len();
        let gap = 1u16;
        let col_w = theme::COL_W;
        let full_visible = ((area.width + gap) / (col_w + gap)).max(1) as usize;
        let full_visible = full_visible.min(n_cols);

        self.grid.ensure_col_visible(full_visible);

        let footer_h = 2u16;
        let body_h = area.height.saturating_sub(footer_h);
        let cards_per_col = (body_h / theme::CARD_H).max(1) as usize;

        self.grid.ensure_visible(cards_per_col);

        let render_cols = if full_visible < n_cols {
            full_visible + 1
        } else {
            full_visible
        };

        let footer_y = area.y + body_h;

        for vi in 0..render_cols {
            let ci = self.grid.col_offset + vi;
            if ci >= n_cols {
                break;
            }
            let col = &self.grid.columns[ci];
            let is_current = ci == self.grid.col;
            let x = area.x + vi as u16 * (col_w + gap);
            let remaining = (area.x + area.width).saturating_sub(x);
            if remaining == 0 {
                break;
            }
            let w = col_w.min(remaining);

            let header_fg = if is_current {
                theme::TEXT
            } else {
                theme::MUTED
            };
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "─".repeat(w as usize),
                    Style::default().fg(theme::MUTED),
                )),
                Rect::new(x, footer_y, w, 1),
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!(" {}", col.name),
                        Style::default()
                            .fg(header_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" {}", col.record_indices.len()),
                        Style::default().fg(theme::MUTED),
                    ),
                ])),
                Rect::new(x, footer_y + 1, w, 1),
            );

            let scroll_off =
                self.grid.scroll_offsets.get(ci).copied().unwrap_or(0);

            for si in 0..cards_per_col {
                let ri = scroll_off + si;
                if ri >= col.record_indices.len() {
                    break;
                }
                let record_idx = col.record_indices[ri];
                let record = &self.records[record_idx];
                let is_focused = is_current && ri == self.grid.row;
                let is_selected = self.selected.contains(&record_idx);

                let card_y = area.y + si as u16 * theme::CARD_H;
                if card_y + theme::CARD_H > footer_y {
                    break;
                }
                let card_area = Rect::new(x, card_y, w, theme::CARD_H);

                let border_color = if is_focused {
                    theme::TEXT
                } else {
                    theme::MUTED
                };
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(border_color));
                let inner = block.inner(card_area);
                frame.render_widget(block, card_area);

                let short_ts = record.manifest.short_timestamp();
                let commit = &record.manifest.git.commit;
                let short_commit = if commit.len() > 7 {
                    &commit[..7]
                } else {
                    commit
                };

                let sel_marker = if is_selected { "● " } else { "  " };
                let text_color = if is_focused {
                    theme::TEXT
                } else {
                    theme::MUTED
                };

                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(vec![
                            Span::styled(
                                sel_marker,
                                Style::default().fg(if is_selected {
                                    theme::SELECT
                                } else {
                                    text_color
                                }),
                            ),
                            Span::styled(
                                short_ts,
                                Style::default().fg(text_color),
                            ),
                        ]),
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(
                                short_commit.to_string(),
                                Style::default().fg(theme::MUTED),
                            ),
                            Span::styled(
                                format!("  n={}", record.manifest.iterations),
                                Style::default().fg(theme::MUTED),
                            ),
                        ]),
                    ]),
                    inner,
                );
            }

            let total = col.record_indices.len();
            let shown = cards_per_col.min(total.saturating_sub(scroll_off));
            if scroll_off + shown < total {
                let more = total - scroll_off - shown;
                let ind_y = area.y + shown as u16 * theme::CARD_H;
                if ind_y < footer_y {
                    frame.render_widget(
                        Paragraph::new(Span::styled(
                            format!("  ↓ {more} more"),
                            Style::default().fg(theme::MUTED),
                        )),
                        Rect::new(x, ind_y, w, 1),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_completion_edits_json_and_reports_errors() {
        let mut view = MainView::new(vec![], vec![], vec![]);
        let destination = PathBuf::from("summary.json");
        for (format, result) in [
            (ArtifactFormat::Json, Ok("done".into())),
            (ArtifactFormat::Pdf, Err("script failed".into())),
        ] {
            let (tx, rx) = mpsc::channel();
            let task = BgTask {
                label: "summary".into(),
                rx,
                start: Instant::now(),
            };
            view.mode =
                Mode::ArtifactRunning(task, destination.clone(), format);
            tx.send(result).unwrap();
            match view.tick() {
                AppAction::Edit(path) => assert_eq!(path, destination),
                AppAction::Flash(message) => {
                    assert_eq!(message, "script failed")
                }
                _ => panic!("unexpected completion action"),
            }
            assert!(matches!(view.mode, Mode::Browse));
        }
    }
}
