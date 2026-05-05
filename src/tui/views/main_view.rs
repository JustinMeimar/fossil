use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use crate::tui::theme;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Paragraph,
};

use crate::record::Record;
use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::figure::Figure;
use crate::fossil::Fossil;
use crate::project::Project;

use super::{
    AppAction, ListEntry, PreviewPanel,
    SelectorAction, SelectorPopup,
};
use super::analysis_popup::{
    AnalysisAction, AnalysisPopupState,
};
use super::bury_popup::{BuryAction, BuryPopupState};
use super::grid::VariantGrid;

// shared helpers

fn load_fossil_records(
    fossils: &[Fossil],
    idx: usize,
) -> Vec<Record> {
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

const SPINNER: &[&str] =
    &["   ", ".  ", ".. ", "...", " ..", "  ."];

pub fn spinner_frame(start: Instant) -> &'static str {
    let idx = (start.elapsed().as_millis() / 300)
        as usize
        % SPINNER.len();
    SPINNER[idx]
}

pub fn render_toast(
    frame: &mut Frame,
    area: Rect,
    text: &str,
    color: Color,
) {
    let width =
        (text.len() as u16 + 4).min(area.width);
    let [popup] =
        Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(
                Layout::vertical([
                    Constraint::Length(3),
                ])
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
        Paragraph::new(text)
            .style(Style::default().fg(color)),
        inner,
    );
}

// Focus & Mode

#[derive(PartialEq)]
enum Focus {
    Master,
    Detail,
}

struct FigureLoading {
    name: String,
    rx: mpsc::Receiver<Result<String, String>>,
    start: Instant,
}

enum Mode {
    Browse,
    ProjectSelector(SelectorPopup),
    FossilSelector(SelectorPopup),
    EditSelector(SelectorPopup, Vec<PathBuf>),
    AnalysisPopup(Box<AnalysisPopupState>),
    BuryPopup(BuryPopupState),
    FigureSelector(SelectorPopup, Vec<String>),
    FigureRunning(FigureLoading),
    DeleteConfirm(usize),
}

// MainView

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
    last_analysis: Option<Vec<(String, crate::analysis::Metric)>>,
    focus: Focus,
    mode: Mode,
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
            last_analysis: None,
        }
    }

    pub fn load(
        projects_dir: PathBuf,
    ) -> Result<Self, FossilError> {
        let projects =
            Project::list_all(&projects_dir)?;
        let (fossils, records) =
            if let Some(p) = projects.first() {
                let fossils =
                    Fossil::list_all(&p.path)?;
                let records =
                    load_fossil_records(&fossils, 0);
                (fossils, records)
            } else {
                (Vec::new(), Vec::new())
            };
        Ok(Self::new(projects, fossils, records))
    }

    pub fn project_name(&self) -> &str {
        self.projects
            .get(self.project_idx)
            .map(|p| p.config.name.as_str())
            .unwrap_or("(no project)")
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
            | Mode::EditSelector(..) => vec![
                ("enter", "select"),
                ("esc", "close"),
            ],
            Mode::AnalysisPopup(_)
            | Mode::BuryPopup(_)
            | Mode::FigureRunning(_) => vec![
                ("enter", "run"),
                ("esc", "close"),
            ],
            Mode::FigureSelector(..) => vec![
                ("enter", "select"),
                ("esc", "close"),
            ],
            Mode::DeleteConfirm(_) => vec![
                ("y", "confirm delete"),
                ("n/esc", "cancel"),
            ],
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
                Focus::Detail => {
                    let mut h = vec![
                        ("j/k", "scroll"),
                        ("h/l", "pan"),
                        ("tab", "list"),
                    ];
                    if self.last_analysis.is_some() {
                        h.push(("f", "figure"));
                    }
                    h
                }
            },
        }
    }

    pub fn tick(&mut self) -> AppAction {
        if let Mode::AnalysisPopup(ref mut popup) =
            self.mode
        {
            match popup.tick() {
                AnalysisAction::Output(name, output, cols) => {
                    if let Some(ref mut p) = self.preview
                    {
                        p.set_content(
                            &format!("analysis: {name}"),
                            &output,
                        );
                    }
                    self.last_analysis = Some(cols);
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
        if let Mode::BuryPopup(ref mut popup) =
            self.mode
        {
            match popup.tick() {
                BuryAction::Done(summary) => {
                    self.reload_records();
                    self.mode = Mode::Browse;
                    return AppAction::Flash(summary);
                }
                BuryAction::Flash(msg) => {
                    self.mode = Mode::Browse;
                    return AppAction::Flash(msg);
                }
                _ => {}
            }
        }
        if let Mode::FigureRunning(ref loading) =
            self.mode
        {
            match loading.rx.try_recv() {
                Ok(Ok(msg)) => {
                    self.mode = Mode::Browse;
                    return AppAction::Flash(msg);
                }
                Ok(Err(msg)) => {
                    self.mode = Mode::Browse;
                    return AppAction::Flash(msg);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.mode = Mode::Browse;
                    return AppAction::Flash(
                        "figure thread panicked".into(),
                    );
                }
                _ => {}
            }
        }
        AppAction::None
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
    ) -> AppAction {
        enum Resolved {
            None,
            Dismiss,
            SelectProject(usize),
            SelectFossil(usize),
            EditFile(PathBuf),
            AnalysisOutput(String, String, Vec<(String, crate::analysis::Metric)>),
            RunFigure(usize),
            Flash(String),
            Browse,
        }

        let resolved = match &mut self.mode {
            Mode::ProjectSelector(sel) => {
                match sel.handle_key(key) {
                    SelectorAction::Select(i) => {
                        Resolved::SelectProject(i)
                    }
                    SelectorAction::Dismiss => {
                        Resolved::Dismiss
                    }
                    SelectorAction::None => {
                        Resolved::None
                    }
                }
            }
            Mode::FossilSelector(sel) => {
                match sel.handle_key(key) {
                    SelectorAction::Select(i) => {
                        Resolved::SelectFossil(i)
                    }
                    SelectorAction::Dismiss => {
                        Resolved::Dismiss
                    }
                    SelectorAction::None => {
                        Resolved::None
                    }
                }
            }
            Mode::EditSelector(sel, paths) => {
                match sel.handle_key(key) {
                    SelectorAction::Select(i) => {
                        let path = paths[i].clone();
                        Resolved::EditFile(path)
                    }
                    SelectorAction::Dismiss => {
                        Resolved::Dismiss
                    }
                    SelectorAction::None => {
                        Resolved::None
                    }
                }
            }
            Mode::AnalysisPopup(popup) => {
                match popup.handle_key(key) {
                    AnalysisAction::Dismiss => {
                        Resolved::Dismiss
                    }
                    AnalysisAction::Output(n, o, c) => {
                        Resolved::AnalysisOutput(n, o, c)
                    }
                    AnalysisAction::Flash(msg) => {
                        Resolved::Flash(msg)
                    }
                    AnalysisAction::None => {
                        Resolved::None
                    }
                }
            }
            Mode::FigureSelector(sel, _names) => {
                match sel.handle_key(key) {
                    SelectorAction::Select(i) => {
                        Resolved::RunFigure(i)
                    }
                    SelectorAction::Dismiss => {
                        Resolved::Dismiss
                    }
                    SelectorAction::None => {
                        Resolved::None
                    }
                }
            }
            Mode::FigureRunning(_) => Resolved::None,
            Mode::BuryPopup(popup) => {
                match popup.handle_key(key) {
                    BuryAction::Dismiss => {
                        Resolved::Dismiss
                    }
                    BuryAction::Flash(msg) => {
                        Resolved::Flash(msg)
                    }
                    _ => Resolved::None,
                }
            }
            Mode::DeleteConfirm(idx) => {
                let idx = *idx;
                match key.code {
                    KeyCode::Char('y') => {
                        let msg =
                            self.execute_delete(idx);
                        self.mode = Mode::Browse;
                        return match msg {
                            Ok(m) => AppAction::Flash(m),
                            Err(e) => AppAction::Flash(
                                e.to_string(),
                            ),
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
            Resolved::AnalysisOutput(name, output, cols) => {
                if let Some(ref mut p) = self.preview {
                    p.set_content(
                        &format!("analysis: {name}"),
                        &output,
                    );
                }
                self.last_analysis = Some(cols);
                self.mode = Mode::Browse;
                self.focus = Focus::Detail;
                return AppAction::None;
            }
            Resolved::RunFigure(i) => {
                self.start_figure(i);
                return AppAction::None;
            }
            Resolved::Flash(msg) => {
                self.mode = Mode::Browse;
                return AppAction::Flash(msg);
            }
            Resolved::Browse => {}
        }

        match self.focus {
            Focus::Detail => {
                if matches!(
                    key.code,
                    KeyCode::Tab | KeyCode::Esc
                ) {
                    self.focus = Focus::Master;
                    return AppAction::None;
                }
                if key.code == KeyCode::Char('f')
                    && self.last_analysis.is_some()
                {
                    self.open_figure_selector();
                    return AppAction::None;
                }
                if let Some(ref mut panel) = self.preview
                {
                    panel.handle_nav(key);
                }
                AppAction::None
            }
            Focus::Master => {
                let prev_col = self.grid.col;
                let prev_row = self.grid.row;
                if self.grid.handle_key(key) {
                    if self.grid.col != prev_col
                        || self.grid.row != prev_row
                    {
                        self.sync_preview();
                    }
                    return AppAction::None;
                }
                match key.code {
                    KeyCode::Char(' ') => {
                        if let Some(idx) =
                            self.grid.current_record_idx()
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
                    KeyCode::Char('a')
                    | KeyCode::Char('s') => {
                        self.open_analysis_popup();
                        AppAction::None
                    }
                    KeyCode::Char('b') => {
                        match self.open_bury_popup() {
                            Some(msg) => {
                                AppAction::Flash(msg)
                            }
                            None => AppAction::None,
                        }
                    }
                    KeyCode::Char('e') => {
                        self.open_edit_selector();
                        AppAction::None
                    }
                    KeyCode::Char('d') => {
                        if let Some(idx) =
                            self.grid.current_record_idx()
                        {
                            self.mode =
                                Mode::DeleteConfirm(idx);
                        }
                        AppAction::None
                    }
                    KeyCode::Char('?') => {
                        AppAction::ShowHelp
                    }
                    KeyCode::Char('q') => {
                        AppAction::Quit
                    }
                    _ => AppAction::None,
                }
            }
        }
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
    ) {
        let master_focused =
            self.focus == Focus::Master;

        if self.records.is_empty() {
            let msg = if self.projects.is_empty() {
                "no projects found"
            } else if self.fossils.is_empty() {
                "no fossils in this project"
            } else {
                "no records found"
            };
            frame.render_widget(
                Paragraph::new(format!(
                    " {msg}  (p:project  f:fossil)"
                ))
                .style(
                    Style::default()
                        .fg(theme::MUTED),
                ),
                area,
            );
        } else {
            let [master, detail] =
                Layout::horizontal([
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
                    Span::styled(
                        " records ",
                        Style::default()
                            .fg(title_color),
                    ),
                    Span::styled(
                        format!(
                            "│ {sel_count} selected "
                        ),
                        Style::default()
                            .fg(theme::MUTED),
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
                .border_style(
                    Style::default().fg(master_border),
                )
                .title(title_line);
            let master_inner =
                master_block.inner(master);
            frame.render_widget(
                master_block,
                master,
            );
            self.render_grid(frame, master_inner);

            if let Some(ref panel) = self.preview {
                panel.render(
                    frame,
                    detail,
                    !master_focused,
                );
            }
        }

        match &mut self.mode {
            Mode::ProjectSelector(sel) => {
                sel.render_popup(frame, area);
            }
            Mode::FossilSelector(sel) => {
                sel.render_popup(frame, area);
            }
            Mode::EditSelector(sel, _) => {
                sel.render_popup(frame, area);
            }
            Mode::AnalysisPopup(popup) => {
                popup.render_popup(frame, area);
            }
            Mode::BuryPopup(popup) => {
                popup.render_popup(frame, area);
            }
            Mode::FigureSelector(sel, _) => {
                sel.render_popup(frame, area);
            }
            Mode::FigureRunning(loading) => {
                let text = format!(
                    " rendering {} {}",
                    loading.name,
                    spinner_frame(loading.start),
                );
                render_toast(
                    frame, area, &text, theme::WARN,
                );
            }
            Mode::DeleteConfirm(idx) => {
                let idx = *idx;
                let label = self
                    .records
                    .get(idx)
                    .map(|r| {
                        let v = r
                            .manifest
                            .variant
                            .as_deref()
                            .unwrap_or("untagged");
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
        let idx = match self.grid.current_record_idx()
        {
            Some(i) => i,
            None => return,
        };
        if self.preview_index == Some(idx) {
            return;
        }
        if let Some(record) = self.records.get(idx) {
            self.preview =
                Some(PreviewPanel::from_record(record));
            self.preview_index = Some(idx);
        }
    }

    fn set_records(&mut self, records: Vec<Record>) {
        self.grid =
            VariantGrid::from_records(&records);
        self.selected.clear();
        let initial_idx =
            self.grid.current_record_idx();
        self.preview = initial_idx
            .and_then(|i| records.get(i))
            .map(PreviewPanel::from_record);
        self.preview_index = initial_idx;
        self.records = records;
        self.focus = Focus::Master;
    }

    fn reload_records(&mut self) {
        let records = load_fossil_records(
            &self.fossils,
            self.fossil_idx,
        );
        self.set_records(records);
    }

    fn open_project_selector(&mut self) {
        let entries: Vec<ListEntry> = self
            .projects
            .iter()
            .map(|p| ListEntry {
                name: p.config.name.clone(),
                detail: p
                    .config
                    .description
                    .clone()
                    .unwrap_or_default(),
                tag: None,
            })
            .collect();
        let mut sel =
            SelectorPopup::new("projects", entries);
        sel.list.selected = self.project_idx;
        self.mode = Mode::ProjectSelector(sel);
    }

    fn apply_project_selection(&mut self, idx: usize) {
        if let Some(p) = self.projects.get(idx) {
            let path = p.path.clone();
            self.project_idx = idx;
            self.fossils = Fossil::list_all(&path)
                .unwrap_or_default();
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
                    Some((
                        format!("[{nv} variants]"),
                        theme::WARN,
                    ))
                } else {
                    None
                };
                ListEntry {
                    name: f.config.name.clone(),
                    detail: f
                        .config
                        .description
                        .clone()
                        .unwrap_or_default(),
                    tag,
                }
            })
            .collect();
        let mut sel =
            SelectorPopup::new("fossils", entries);
        sel.list.selected = self.fossil_idx;
        self.mode = Mode::FossilSelector(sel);
    }

    fn apply_fossil_selection(&mut self, idx: usize) {
        if self.fossils.get(idx).is_some() {
            self.fossil_idx = idx;
            self.reload_records();
        }
    }

    fn execute_delete(
        &mut self,
        idx: usize,
    ) -> Result<String, FossilError> {
        let record = self
            .records
            .get(idx)
            .ok_or_else(|| {
                FossilError::NotFound(
                    "no record selected".into(),
                )
            })?;
        let project = self
            .projects
            .get(self.project_idx)
            .ok_or_else(|| {
                FossilError::NotFound(
                    "no project selected".into(),
                )
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

        let selected_records =
            if self.selected.is_empty() {
                Vec::new()
            } else {
                let records: Vec<&Record> = self
                    .selected
                    .iter()
                    .filter_map(|&i| {
                        self.records.get(i)
                    })
                    .collect();

                let mut counts: BTreeMap<&str, usize> =
                    BTreeMap::new();
                for r in &records {
                    let v = r
                        .manifest
                        .variant
                        .as_deref()
                        .unwrap_or("untagged");
                    *counts.entry(v).or_default() += 1;
                }
                let has_dups =
                    counts.values().any(|&c| c > 1);

                records
                    .iter()
                    .map(|r| {
                        let v = r
                            .manifest
                            .variant
                            .as_deref()
                            .unwrap_or("untagged");
                        let label = if has_dups {
                            let ts = r
                                .manifest
                                .timestamp
                                .get(5..16)
                                .unwrap_or(
                                    &r.manifest
                                        .timestamp,
                                )
                                .replace('T', " ");
                            format!("{v} ({ts})")
                        } else {
                            v.to_string()
                        };
                        (label, r.dir.clone())
                    })
                    .collect()
            };

        self.mode = Mode::AnalysisPopup(Box::new(
            AnalysisPopupState::new(
                fossil,
                self.current_project_path(),
                selected_records,
            ),
        ));
    }

    fn open_bury_popup(&mut self) -> Option<String> {
        let fossil = self.current_fossil()?;
        if fossil.config.variants.is_empty() {
            return Some(
                "no variants configured".into(),
            );
        }
        self.mode = Mode::BuryPopup(
            BuryPopupState::new(
                &fossil,
                self.current_project_path(),
            ),
        );
        None
    }

    fn open_figure_selector(&mut self) {
        let fossil = match self.current_fossil() {
            Some(f) => f,
            None => return,
        };
        let fig_map = match fossil.config.figures.as_ref() {
            Some(m) if !m.is_empty() => m,
            _ => return,
        };
        let names: Vec<String> =
            fig_map.keys().cloned().collect();
        let entries: Vec<ListEntry> = fig_map
            .iter()
            .map(|(name, entry)| ListEntry {
                name: name.clone(),
                detail: entry.script.as_str().to_string(),
                tag: None,
            })
            .collect();
        self.mode = Mode::FigureSelector(
            SelectorPopup::new("figures", entries),
            names,
        );
    }

    fn start_figure(&mut self, idx: usize) {
        let names = match &self.mode {
            Mode::FigureSelector(_, names) => names.clone(),
            _ => return,
        };
        let name = match names.get(idx) {
            Some(n) => n.clone(),
            None => return,
        };
        let fossil = match self.current_fossil() {
            Some(f) => f,
            None => return,
        };
        let columns = match self.last_analysis.clone() {
            Some(c) => c,
            None => return,
        };

        let (tx, rx) = mpsc::channel();
        let fig_name = name.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let fig = Figure::resolve(&fossil, Some(&fig_name))
                    .map_err(|e| e.to_string())?;
                let path = fig.output_path(&fossil);
                fig.run(&fossil, &columns)
                    .map_err(|e| e.to_string())?;
                Ok(format!("wrote {}", path.display()))
            })();
            let _ = tx.send(result);
        });

        self.mode = Mode::FigureRunning(FigureLoading {
            name,
            rx,
            start: Instant::now(),
        });
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

        if let Some(ref spec) = fossil.config.analyze {
            for script in spec.scripts() {
                entries.push(ListEntry {
                    name: script.to_string(),
                    detail: "analysis".into(),
                    tag: None,
                });
                paths.push(fossil.path.join(script));
            }
        }

        let project_toml =
            self.current_project_path()
                .join("project.toml");
        if project_toml.exists() {
            entries.push(ListEntry {
                name: "project.toml".into(),
                detail: "project config".into(),
                tag: None,
            });
            paths.push(project_toml);
        }

        self.mode = Mode::EditSelector(
            SelectorPopup::new("edit", entries),
            paths,
        );
    }

    pub fn reload(&mut self) {
        if let Some(p) =
            self.projects.get(self.project_idx)
        {
            self.fossils = Fossil::list_all(&p.path)
                .unwrap_or_default();
            self.fossil_idx = self
                .fossil_idx
                .min(
                    self.fossils
                        .len()
                        .saturating_sub(1),
                );
        }
        self.reload_records();
    }

    fn render_grid(
        &mut self,
        frame: &mut Frame,
        area: Rect,
    ) {
        if self.grid.columns.is_empty() {
            return;
        }

        let n_cols = self.grid.columns.len();
        let gap = 1u16;
        let col_w = theme::COL_W;
        let full_visible = ((area.width + gap)
            / (col_w + gap))
            .max(1) as usize;
        let full_visible = full_visible.min(n_cols);

        self.grid.ensure_col_visible(full_visible);

        let footer_h = 2u16;
        let body_h =
            area.height.saturating_sub(footer_h);
        let cards_per_col =
            (body_h / theme::CARD_H).max(1) as usize;

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
            let x =
                area.x + vi as u16 * (col_w + gap);
            let remaining =
                (area.x + area.width).saturating_sub(x);
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
                        format!(
                            " {}",
                            col.record_indices.len()
                        ),
                        Style::default()
                            .fg(theme::MUTED),
                    ),
                ])),
                Rect::new(x, footer_y + 1, w, 1),
            );

            let scroll_off = self
                .grid
                .scroll_offsets
                .get(ci)
                .copied()
                .unwrap_or(0);

            for si in 0..cards_per_col {
                let ri = scroll_off + si;
                if ri >= col.record_indices.len() {
                    break;
                }
                let record_idx =
                    col.record_indices[ri];
                let record = &self.records[record_idx];
                let is_focused =
                    is_current && ri == self.grid.row;
                let is_selected =
                    self.selected.contains(&record_idx);

                let card_y =
                    area.y + si as u16 * theme::CARD_H;
                if card_y + theme::CARD_H > footer_y {
                    break;
                }
                let card_area = Rect::new(
                    x, card_y, w, theme::CARD_H,
                );

                let border_color = if is_focused {
                    theme::TEXT
                } else {
                    theme::MUTED
                };
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(
                        Style::default()
                            .fg(border_color),
                    );
                let inner = block.inner(card_area);
                frame.render_widget(block, card_area);

                let ts = &record.manifest.timestamp;
                let short_ts = ts
                    .get(5..16)
                    .unwrap_or(ts)
                    .replace('T', " ");
                let commit =
                    &record.manifest.git.commit;
                let short_commit =
                    if commit.len() > 7 {
                        &commit[..7]
                    } else {
                        commit
                    };

                let sel_marker = if is_selected {
                    "● "
                } else {
                    "  "
                };
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
                                Style::default().fg(
                                    if is_selected {
                                        theme::SELECT
                                    } else {
                                        text_color
                                    },
                                ),
                            ),
                            Span::styled(
                                short_ts,
                                Style::default()
                                    .fg(text_color),
                            ),
                        ]),
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(
                                short_commit
                                    .to_string(),
                                Style::default()
                                    .fg(theme::MUTED),
                            ),
                            Span::styled(
                                format!(
                                    "  n={}",
                                    record
                                        .manifest
                                        .iterations
                                ),
                                Style::default()
                                    .fg(theme::MUTED),
                            ),
                        ]),
                    ]),
                    inner,
                );
            }

            let total = col.record_indices.len();
            let shown = cards_per_col
                .min(total.saturating_sub(scroll_off));
            if scroll_off + shown < total {
                let more = total - scroll_off - shown;
                let ind_y =
                    area.y + shown as u16 * theme::CARD_H;
                if ind_y < footer_y {
                    frame.render_widget(
                        Paragraph::new(Span::styled(
                            format!("  ↓ {more} more"),
                            Style::default()
                                .fg(theme::MUTED),
                        )),
                        Rect::new(x, ind_y, w, 1),
                    );
                }
            }
        }
    }
}
