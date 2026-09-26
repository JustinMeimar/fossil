use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use crate::tui::theme;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::analysis::AnalysisResult;
use crate::commands;
use crate::entity::DirEntity;
use crate::fossil::{ConfigurationKey, Fossil};
use crate::project::Project;
use crate::record::Record;

use super::main_view::{render_toast, spinner_frame};
use super::{ListEntry, SelectorAction, SelectorPopup};

struct LoadingState {
    name: String,
    rx: mpsc::Receiver<Result<String, String>>,
    start: Instant,
}

pub struct AnalysisPopupState {
    fossil: Fossil,
    project_path: PathBuf,
    names: Vec<ConfigurationKey>,
    selector: SelectorPopup,
    loading: Option<LoadingState>,
    selected_records: Vec<(String, Record)>,
}

pub enum AnalysisAction {
    None,
    Dismiss,
    Output(String, String),
    Flash(String),
}

impl AnalysisPopupState {
    pub fn new(
        fossil: Fossil,
        project_path: PathBuf,
        selected_records: Vec<(String, Record)>,
    ) -> Self {
        let names = fossil.config.analyses.keys().cloned().collect();
        let entries = fossil
            .config
            .analyses
            .iter()
            .map(|(name, script)| ListEntry {
                name: name.to_string(),
                detail: script.as_str().into(),
                tag: None,
            })
            .collect();
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
                let _ = tx.send(
                    result
                        .and_then(|analysis_result| analysis_result.to_json())
                        .map_err(|e| e.to_string()),
                );
            });
        } else {
            let fossil = self.fossil.clone();
            let project_path = self.project_path.clone();
            let selected = self.selected_records.clone();
            let analysis_name = name.clone();
            std::thread::spawn(move || {
                let project = match Project::load(&project_path) {
                    Ok(project) => project,
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let script = match fossil.resolve_analysis(&analysis_name) {
                    Ok(analysis) => crate::analysis::AnalysisScript::new(
                        &analysis,
                        crate::environment::ExecutionContext::new(
                            &project,
                            &fossil,
                            crate::environment::Operation::Analysis(
                                &analysis.key,
                            ),
                        ),
                    ),
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let mut analysis_result = AnalysisResult::default();
                for (label, record) in &selected {
                    match script.collect(record) {
                        Ok(metric) => {
                            analysis_result
                                .metrics_by_label
                                .insert(label.clone(), metric);
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e.to_string()));
                            return;
                        }
                    }
                }
                let _ = tx
                    .send(analysis_result.to_json().map_err(|e| e.to_string()));
            });
        }

        self.loading = Some(LoadingState {
            name: name.to_string(),
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
            Ok(Ok(output)) => {
                let name = loading.name.clone();
                self.loading = None;
                AnalysisAction::Output(name, output)
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
