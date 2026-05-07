use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use crate::tui::theme;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::commands;
use crate::entity::DirEntity;
use crate::fossil::Fossil;
use crate::project::Project;

use super::main_view::{render_toast, spinner_frame};
use super::{ListEntry, SelectorAction, SelectorPopup};

type AnalysisColumns = Vec<(String, crate::analysis::Metric)>;
type AnalysisResult = Result<(String, AnalysisColumns), String>;

struct LoadingState {
    name: String,
    rx: mpsc::Receiver<AnalysisResult>,
    start: Instant,
}

fn format_metrics(cols: &[(String, crate::analysis::Metric)]) -> String {
    let map: BTreeMap<&str, &crate::analysis::Metric> = cols
        .iter()
        .map(|(n, m)| (n.as_str(), m))
        .collect();
    serde_json::to_string_pretty(&map).unwrap_or_default()
}

pub struct AnalysisPopupState {
    fossil: Fossil,
    project_path: PathBuf,
    names: Vec<String>,
    selector: SelectorPopup,
    loading: Option<LoadingState>,
    selected_records: Vec<(String, PathBuf)>,
}

pub enum AnalysisAction {
    None,
    Dismiss,
    Output(String, String, AnalysisColumns),
    Flash(String),
}

impl AnalysisPopupState {
    pub fn new(
        fossil: Fossil,
        project_path: PathBuf,
        selected_records: Vec<(String, PathBuf)>,
    ) -> Self {
        let names: Vec<String> = fossil
            .config
            .analyze
            .as_ref()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default();
        let entries: Vec<ListEntry> = fossil
            .config
            .analyze
            .as_ref()
            .map(|map| {
                map.iter()
                    .map(|(name, script)| ListEntry {
                        name: name.clone(),
                        detail: script.clone(),
                        tag: None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            fossil,
            project_path,
            names,
            selector: SelectorPopup::new("analyses", entries),
            loading: None,
            selected_records,
        }
    }

    fn start_analysis(&mut self) -> AnalysisAction {
        let idx = self.selector.list.selected;
        let name = match self.names.get(idx) {
            Some(n) => n.clone(),
            None => return AnalysisAction::None,
        };

        let (tx, rx) = mpsc::channel();

        if self.selected_records.is_empty() {
            let project_path = self.project_path.clone();
            let fossil_name = self.fossil.config.name.clone();
            let analysis_name = name.clone();
            std::thread::spawn(move || {
                let result = Project::load(&project_path).and_then(|project| {
                    commands::analyze(
                        &project,
                        &[fossil_name],
                        None,
                        Some(&analysis_name),
                    )
                });
                let _ = tx.send(match result {
                    Ok(cols) => {
                        let s = format_metrics(&cols);
                        Ok((s, cols))
                    }
                    Err(e) => Err(e.to_string()),
                });
            });
        } else {
            let fossil = self.fossil.clone();
            let selected = self.selected_records.clone();
            let analysis_name = name.clone();
            std::thread::spawn(move || {
                let script = match fossil.resolve_analysis(Some(&analysis_name))
                {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let mut cols = Vec::new();
                for (label, dir) in &selected {
                    match script.collect(dir) {
                        Ok(m) => cols.push((label.clone(), m)),
                        Err(e) => {
                            let _ = tx.send(Err(e.to_string()));
                            return;
                        }
                    }
                }
                let _ = tx.send(Ok((format_metrics(&cols), cols)));
            });
        }

        self.loading = Some(LoadingState {
            name,
            rx,
            start: Instant::now(),
        });
        AnalysisAction::None
    }

    pub fn tick(&mut self) -> AnalysisAction {
        let loading = match self.loading.as_ref() {
            Some(l) => l,
            None => return AnalysisAction::None,
        };
        match loading.rx.try_recv() {
            Ok(Ok((output, cols))) => {
                let name = loading.name.clone();
                self.loading = None;
                AnalysisAction::Output(name, output, cols)
            }
            Ok(Err(msg)) => {
                self.loading = None;
                AnalysisAction::Flash(msg)
            }
            Err(mpsc::TryRecvError::Empty) => AnalysisAction::None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.loading = None;
                AnalysisAction::Flash("analysis thread panicked".into())
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> AnalysisAction {
        if self.loading.is_some() {
            return AnalysisAction::None;
        }
        match self.selector.handle_key(key) {
            SelectorAction::Select(_) => self.start_analysis(),
            SelectorAction::Dismiss => AnalysisAction::Dismiss,
            SelectorAction::None => AnalysisAction::None,
        }
    }

    pub fn render_popup(&mut self, frame: &mut Frame, area: Rect) {
        if let Some(ref loading) = self.loading {
            let text = format!(
                " running {} {}",
                loading.name,
                spinner_frame(loading.start),
            );
            render_toast(frame, area, &text, theme::WARN);
        } else {
            self.selector.render_popup(frame, area);
        }
    }
}
