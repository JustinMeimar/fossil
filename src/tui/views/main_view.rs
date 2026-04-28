use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Paragraph,
};

use crate::analysis::Record;
use crate::commands;
use crate::entity::DirEntity;
use crate::error::FossilError;
use crate::fossil::Fossil;
use crate::project::Project;

use super::{
    AppAction, ListEntry, PreviewPanel, SelectList,
    SelectorAction, SelectorPopup,
};

fn load_fossil_records(
    fossils: &[Fossil],
    idx: usize,
) -> Vec<Record> {
    fossils
        .get(idx)
        .and_then(|f| Fossil::load(&f.path).ok())
        .and_then(|f| {
            f.find_records(None, None).ok()
        })
        .map(|mut recs| {
            recs.reverse();
            recs
        })
        .unwrap_or_default()
}

const PALETTE: [Color; 6] = [
    Color::Cyan,
    Color::Green,
    Color::Yellow,
    Color::Magenta,
    Color::Blue,
    Color::Red,
];

fn variant_color(name: &str) -> Color {
    let hash =
        name.bytes().fold(0u8, |a, b| a.wrapping_add(b));
    PALETTE[hash as usize % PALETTE.len()]
}

const CARD_H: u16 = 4;
const MASTER_DETAIL_MIN: u16 = 80;

const SPINNER: &[&str] =
    &["   ", ".  ", ".. ", "...", " ..", "  ."];

// ── Focus & Mode ───────────────────────────────────

#[derive(PartialEq)]
enum Focus {
    Master,
    Detail,
}

enum Mode {
    Browse,
    ProjectSelector(SelectorPopup),
    FossilSelector(SelectorPopup),
    AnalysisPopup(AnalysisPopupState),
    BuryPopup(BuryPopupState),
}

// ── AnalysisPopupState ─────────────────────────────

type AnalysisResult = Result<String, String>;

struct LoadingState {
    name: String,
    rx: mpsc::Receiver<AnalysisResult>,
    start: Instant,
}

struct AnalysisPopupState {
    fossil: Fossil,
    project_path: PathBuf,
    names: Vec<String>,
    selector: SelectorPopup,
    loading: Option<LoadingState>,
}

enum AnalysisAction {
    None,
    Dismiss,
    Output(String, String),
    Flash(String),
}

impl AnalysisPopupState {
    fn new(
        fossil: Fossil,
        project_path: PathBuf,
    ) -> Self {
        let names: Vec<String> = fossil
            .config
            .analyze
            .as_ref()
            .map(|spec| {
                spec.names()
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();
        let entries: Vec<ListEntry> = names
            .iter()
            .map(|name| {
                let script = fossil
                    .analyze_script(Some(name))
                    .map(|p| {
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    })
                    .unwrap_or_default();
                ListEntry {
                    name: name.clone(),
                    detail: script,
                    tag: None,
                }
            })
            .collect();
        Self {
            fossil,
            project_path,
            names,
            selector: SelectorPopup::new(
                "analyses", entries,
            ),
            loading: None,
        }
    }

    fn start_analysis(&mut self) -> AnalysisAction {
        let idx = self.selector.list.selected;
        let name = match self.names.get(idx) {
            Some(n) => n.clone(),
            None => return AnalysisAction::None,
        };

        let project_path = self.project_path.clone();
        let fossil_name =
            self.fossil.config.name.clone();
        let analysis_name = name.clone();

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = Project::load(&project_path)
                .and_then(|project| {
                    let specs = vec![fossil_name];
                    commands::analyze(
                        &project,
                        &specs,
                        None,
                        Some(&analysis_name),
                    )
                });
            let _ = tx.send(match result {
                Ok(summary) => Ok(format!("{summary}")),
                Err(e) => Err(e.to_string()),
            });
        });

        self.loading = Some(LoadingState {
            name,
            rx,
            start: Instant::now(),
        });
        AnalysisAction::None
    }

    fn tick(&mut self) -> AnalysisAction {
        let loading = match self.loading.as_ref() {
            Some(l) => l,
            None => return AnalysisAction::None,
        };
        match loading.rx.try_recv() {
            Ok(Ok(output)) => {
                let name = loading.name.clone();
                self.loading = None;
                AnalysisAction::Output(name, output)
            }
            Ok(Err(msg)) => {
                self.loading = None;
                AnalysisAction::Flash(msg)
            }
            Err(mpsc::TryRecvError::Empty) => {
                AnalysisAction::None
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.loading = None;
                AnalysisAction::Flash(
                    "analysis thread panicked".into(),
                )
            }
        }
    }

    fn handle_key(
        &mut self,
        key: KeyEvent,
    ) -> AnalysisAction {
        if self.loading.is_some() {
            return AnalysisAction::None;
        }
        match self.selector.handle_key(key) {
            SelectorAction::Select(_) => {
                self.start_analysis()
            }
            SelectorAction::Dismiss => {
                AnalysisAction::Dismiss
            }
            SelectorAction::None => AnalysisAction::None,
        }
    }

    fn render_popup(
        &mut self,
        frame: &mut Frame,
        area: Rect,
    ) {
        if let Some(ref loading) = self.loading {
            let elapsed = loading.start.elapsed();
            let idx =
                (elapsed.as_millis() / 300) as usize
                    % SPINNER.len();
            let spinner = SPINNER[idx];
            let text = format!(
                " running {} {spinner}",
                loading.name,
            );
            let width = (text.len() as u16 + 4)
                .min(area.width);
            let h = 3u16;
            let [popup] = Layout::horizontal([
                Constraint::Length(width),
            ])
            .flex(Flex::Center)
            .areas(
                Layout::vertical([
                    Constraint::Length(h),
                ])
                .flex(Flex::Center)
                .areas::<1>(area)[0],
            );
            frame.render_widget(Clear, popup);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(
                    Style::default().fg(Color::Yellow),
                );
            let inner = block.inner(popup);
            frame.render_widget(block, popup);
            frame.render_widget(
                Paragraph::new(text).style(
                    Style::default().fg(Color::Yellow),
                ),
                inner,
            );
        } else {
            self.selector.render_popup(frame, area);
        }
    }
}

// ── BuryPopupState ────────────────────────────────

struct BuryLoadingState {
    variant: String,
    rx: mpsc::Receiver<Result<String, String>>,
    start: Instant,
}

struct BuryPopupState {
    fossil_path: PathBuf,
    project_path: PathBuf,
    variants: Vec<String>,
    selector: SelectorPopup,
    loading: Option<BuryLoadingState>,
}

enum BuryAction {
    None,
    Dismiss,
    Done(String),
    Flash(String),
}

impl BuryPopupState {
    fn new(
        fossil: &Fossil,
        project_path: PathBuf,
    ) -> Self {
        let variants: Vec<String> = fossil
            .config
            .variants
            .keys()
            .cloned()
            .collect();
        let entries: Vec<ListEntry> = variants
            .iter()
            .map(|name| {
                let cmd = fossil
                    .resolve_variant(name)
                    .map(|v| v.command.join(" "))
                    .unwrap_or_default();
                ListEntry {
                    name: name.clone(),
                    detail: cmd,
                    tag: None,
                }
            })
            .collect();
        Self {
            fossil_path: fossil.path.clone(),
            project_path,
            variants,
            selector: SelectorPopup::new(
                "bury variant", entries,
            ),
            loading: None,
        }
    }

    fn start_bury(&mut self) -> BuryAction {
        let idx = self.selector.list.selected;
        let variant_name = match self.variants.get(idx)
        {
            Some(n) => n.clone(),
            None => return BuryAction::None,
        };

        let project_path = self.project_path.clone();
        let fossil_path = self.fossil_path.clone();
        let vname = variant_name.clone();

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result =
                Project::load(&project_path)
                    .and_then(|project| {
                        let fossil =
                            Fossil::load(&fossil_path)?;
                        let v = fossil
                            .resolve_variant(&vname)?;
                        commands::bury(
                            &fossil,
                            &project,
                            None,
                            Some(v.name),
                            v.command,
                            true,
                        )
                    });
            let _ = tx.send(match result {
                Ok(s) => Ok(s),
                Err(e) => Err(e.to_string()),
            });
        });

        self.loading = Some(BuryLoadingState {
            variant: variant_name,
            rx,
            start: Instant::now(),
        });
        BuryAction::None
    }

    fn tick(&mut self) -> BuryAction {
        let loading = match self.loading.as_ref() {
            Some(l) => l,
            None => return BuryAction::None,
        };
        match loading.rx.try_recv() {
            Ok(Ok(summary)) => {
                self.loading = None;
                BuryAction::Done(summary)
            }
            Ok(Err(msg)) => {
                self.loading = None;
                BuryAction::Flash(msg)
            }
            Err(mpsc::TryRecvError::Empty) => {
                BuryAction::None
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.loading = None;
                BuryAction::Flash(
                    "bury thread panicked".into(),
                )
            }
        }
    }

    fn handle_key(
        &mut self,
        key: KeyEvent,
    ) -> BuryAction {
        if self.loading.is_some() {
            return BuryAction::None;
        }
        match self.selector.handle_key(key) {
            SelectorAction::Select(_) => {
                self.start_bury()
            }
            SelectorAction::Dismiss => {
                BuryAction::Dismiss
            }
            SelectorAction::None => BuryAction::None,
        }
    }

    fn render_popup(
        &mut self,
        frame: &mut Frame,
        area: Rect,
    ) {
        if let Some(ref loading) = self.loading {
            let elapsed = loading.start.elapsed();
            let idx =
                (elapsed.as_millis() / 300) as usize
                    % SPINNER.len();
            let spinner = SPINNER[idx];
            let text = format!(
                " burying {} {spinner}",
                loading.variant,
            );
            let width = (text.len() as u16 + 4)
                .min(area.width);
            let h = 3u16;
            let [popup] = Layout::horizontal([
                Constraint::Length(width),
            ])
            .flex(Flex::Center)
            .areas(
                Layout::vertical([
                    Constraint::Length(h),
                ])
                .flex(Flex::Center)
                .areas::<1>(area)[0],
            );
            frame.render_widget(Clear, popup);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(
                    Style::default().fg(Color::Yellow),
                );
            let inner = block.inner(popup);
            frame.render_widget(block, popup);
            frame.render_widget(
                Paragraph::new(text).style(
                    Style::default().fg(Color::Yellow),
                ),
                inner,
            );
        } else {
            self.selector.render_popup(frame, area);
        }
    }
}

// ── MainView ───────────────────────────────────────

pub struct MainView {
    projects: Vec<Project>,
    project_idx: usize,
    fossils: Vec<Fossil>,
    fossil_idx: usize,
    records: Vec<Record>,
    list: SelectList,
    preview: Option<PreviewPanel>,
    preview_index: Option<usize>,
    focus: Focus,
    mode: Mode,
}

impl MainView {
    fn new(
        projects: Vec<Project>,
        fossils: Vec<Fossil>,
        records: Vec<Record>,
    ) -> Self {
        let entries = Self::record_entries(&records);
        let preview = records
            .first()
            .map(PreviewPanel::from_record);
        Self {
            project_idx: 0,
            projects,
            fossil_idx: 0,
            fossils,
            list: SelectList::new(entries),
            preview,
            preview_index: if records.is_empty() {
                None
            } else {
                Some(0)
            },
            records,
            focus: Focus::Master,
            mode: Mode::Browse,
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

    pub fn hints(&self) -> &[(&str, &str)] {
        match &self.mode {
            Mode::ProjectSelector(..)
            | Mode::FossilSelector(..) => &[
                ("enter", "select"),
                ("esc", "close"),
            ],
            Mode::AnalysisPopup(_)
            | Mode::BuryPopup(_) => &[
                ("enter", "run"),
                ("esc", "close"),
            ],
            Mode::Browse => match self.focus {
                Focus::Master => &[
                    ("j/k", "navigate"),
                    ("tab", "preview"),
                    ("p", "project"),
                    ("f", "fossil"),
                    ("a", "analyze"),
                    ("b", "bury"),
                    ("?", "help"),
                ],
                Focus::Detail => &[
                    ("j/k", "scroll"),
                    ("h/l", "pan"),
                    ("tab", "list"),
                ],
            },
        }
    }

    pub fn tick(&mut self) -> AppAction {
        if let Mode::AnalysisPopup(ref mut popup) =
            self.mode
        {
            match popup.tick() {
                AnalysisAction::Output(name, output) => {
                    self.preview = Some(
                        PreviewPanel::from_analysis(
                            &name, &output,
                        ),
                    );
                    self.preview_index = None;
                    self.mode = Mode::Browse;
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
            AnalysisOutput(String, String),
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
            Mode::AnalysisPopup(popup) => {
                match popup.handle_key(key) {
                    AnalysisAction::Dismiss => {
                        Resolved::Dismiss
                    }
                    AnalysisAction::Output(n, o) => {
                        Resolved::AnalysisOutput(n, o)
                    }
                    AnalysisAction::Flash(msg) => {
                        Resolved::Flash(msg)
                    }
                    AnalysisAction::None => {
                        Resolved::None
                    }
                }
            }
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
            Resolved::AnalysisOutput(name, output) => {
                self.preview = Some(
                    PreviewPanel::from_analysis(
                        &name, &output,
                    ),
                );
                self.preview_index = None;
                self.mode = Mode::Browse;
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
                if let Some(ref mut panel) = self.preview
                {
                    panel.handle_nav(key);
                }
                AppAction::None
            }
            Focus::Master => {
                let prev = self.list.selected;
                if self.list.handle_nav(key) {
                    if self.list.selected != prev {
                        self.sync_preview();
                    }
                    return AppAction::None;
                }
                match key.code {
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
                        .fg(Color::DarkGray),
                ),
                area,
            );
        } else if area.width >= MASTER_DETAIL_MIN {
            let [master, detail] =
                Layout::horizontal([
                    Constraint::Percentage(45),
                    Constraint::Percentage(55),
                ])
                .areas(area);

            let master_border_color = if master_focused
            {
                Color::Cyan
            } else {
                Color::DarkGray
            };
            let master_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(
                    Style::default()
                        .fg(master_border_color),
                )
                .title(Span::styled(
                    " records ",
                    Style::default().fg(
                        if master_focused {
                            Color::Cyan
                        } else {
                            Color::White
                        },
                    ),
                ));
            let master_inner =
                master_block.inner(master);
            frame.render_widget(
                master_block,
                master,
            );
            self.render_cards(frame, master_inner);

            if let Some(ref panel) = self.preview {
                panel.render(
                    frame,
                    detail,
                    !master_focused,
                );
            }
        } else {
            let border_color = if master_focused {
                Color::Cyan
            } else {
                Color::DarkGray
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(
                    Style::default().fg(border_color),
                )
                .title(Span::styled(
                    " records ",
                    Style::default().fg(Color::White),
                ));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            self.render_cards(frame, inner);
        }

        match &mut self.mode {
            Mode::ProjectSelector(sel) => {
                sel.render_popup(frame, area);
            }
            Mode::FossilSelector(sel) => {
                sel.render_popup(frame, area);
            }
            Mode::AnalysisPopup(popup) => {
                popup.render_popup(frame, area);
            }
            Mode::BuryPopup(popup) => {
                popup.render_popup(frame, area);
            }
            Mode::Browse => {}
        }
    }

    // ── private ────────────────────────────────────

    fn record_entries(
        records: &[Record],
    ) -> Vec<ListEntry> {
        records
            .iter()
            .map(|r| ListEntry {
                name: r
                    .manifest
                    .variant
                    .clone()
                    .unwrap_or_else(|| {
                        "untagged".into()
                    }),
                detail: r.manifest.timestamp.clone(),
                tag: None,
            })
            .collect()
    }

    fn sync_preview(&mut self) {
        let idx = self.list.selected;
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
        let entries = Self::record_entries(&records);
        self.preview = records
            .first()
            .map(PreviewPanel::from_record);
        self.preview_index = if records.is_empty() {
            None
        } else {
            Some(0)
        };
        self.list = SelectList::new(entries);
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
                        Color::Yellow,
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

    fn open_analysis_popup(&mut self) {
        let fossil =
            match self.fossils.get(self.fossil_idx) {
                Some(f) => {
                    match Fossil::load(&f.path) {
                        Ok(f) => f,
                        Err(_) => return,
                    }
                }
                None => return,
            };
        let project_path = self
            .projects
            .get(self.project_idx)
            .map(|p| p.path.clone())
            .unwrap_or_default();
        self.mode = Mode::AnalysisPopup(
            AnalysisPopupState::new(
                fossil,
                project_path,
            ),
        );
    }

    fn open_bury_popup(&mut self) -> Option<String> {
        let fossil =
            match self.fossils.get(self.fossil_idx) {
                Some(f) => {
                    match Fossil::load(&f.path) {
                        Ok(f) => f,
                        Err(_) => return None,
                    }
                }
                None => return None,
            };
        if fossil.config.variants.is_empty() {
            return Some(
                "no variants configured".into(),
            );
        }
        let project_path = self
            .projects
            .get(self.project_idx)
            .map(|p| p.path.clone())
            .unwrap_or_default();
        self.mode = Mode::BuryPopup(
            BuryPopupState::new(
                &fossil,
                project_path,
            ),
        );
        None
    }

    fn render_cards(
        &mut self,
        frame: &mut Frame,
        area: Rect,
    ) {
        let visible =
            (area.height / CARD_H).max(1) as usize;
        self.list.ensure_visible(visible);

        let constraints: Vec<Constraint> = (0..visible)
            .map(|_| Constraint::Length(CARD_H))
            .chain(std::iter::once(Constraint::Min(0)))
            .collect();
        let slots =
            Layout::vertical(constraints).split(area);

        for (slot_idx, slot) in
            slots.iter().enumerate().take(visible)
        {
            let idx = self.list.offset + slot_idx;
            if idx >= self.records.len() {
                break;
            }
            let r = &self.records[idx];
            let sel = idx == self.list.selected;

            let variant = r
                .manifest
                .variant
                .as_deref()
                .unwrap_or("untagged");
            let color = variant_color(variant);

            let border_style = if sel {
                Style::default().fg(color)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style)
                .title(Span::styled(
                    format!(" {variant} "),
                    Style::default()
                        .fg(Color::Black)
                        .bg(color)
                        .add_modifier(Modifier::BOLD),
                ));

            let inner = block.inner(*slot);
            frame.render_widget(block, *slot);

            let ts = &r.manifest.timestamp;
            let commit = &r.manifest.git.commit;
            let branch = &r.manifest.git.branch;
            let iters = r.manifest.iterations;
            let cmd = &r.manifest.command;
            let max_cmd =
                inner.width.saturating_sub(2) as usize;
            let cmd_trunc = if cmd.len() > max_cmd {
                format!(
                    "{}...",
                    &cmd[..max_cmd.saturating_sub(3)]
                )
            } else {
                cmd.clone()
            };

            let lines = vec![
                Line::from(vec![
                    Span::styled(
                        format!("{ts}  "),
                        Style::default()
                            .fg(Color::White),
                    ),
                    Span::styled(
                        commit.to_string(),
                        Style::default()
                            .fg(Color::Yellow),
                    ),
                    Span::styled(
                        format!(" ({branch})"),
                        Style::default()
                            .fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("  n={iters}"),
                        Style::default()
                            .fg(Color::DarkGray),
                    ),
                ]),
                Line::from(Span::styled(
                    cmd_trunc,
                    Style::default()
                        .fg(Color::DarkGray),
                )),
            ];
            frame.render_widget(
                Paragraph::new(lines),
                inner,
            );
        }
    }
}
