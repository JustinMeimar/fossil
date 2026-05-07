use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use crate::tui::theme;
use crate::commands;
use crate::entity::DirEntity;
use crate::fossil::{Fossil, FossilVariantKey};
use crate::project::Project;

use super::{
    ListEntry, SelectorAction, SelectorPopup,
};
use super::main_view::{render_toast, spinner_frame};

struct BuryLoadingState {
    variant: String,
    rx: mpsc::Receiver<Result<String, String>>,
    start: Instant,
}

pub struct BuryPopupState {
    fossil_path: PathBuf,
    project_path: PathBuf,
    variants: Vec<FossilVariantKey>,
    selector: SelectorPopup,
    loading: Option<BuryLoadingState>,
}

pub enum BuryAction {
    None,
    Dismiss,
    Done(String),
    Flash(String),
}

impl BuryPopupState {
    pub fn new(
        fossil: &Fossil,
        project_path: PathBuf,
    ) -> Self {
        let variants: Vec<FossilVariantKey> = fossil
            .config
            .variants
            .keys()
            .cloned()
            .collect();
        let entries: Vec<ListEntry> = variants
            .iter()
            .map(|vn| {
                let cmd = fossil
                    .resolve_variant(
                        vn,
                        &BTreeMap::new(),
                    )
                    .map(|v| v.command)
                    .unwrap_or_default();
                ListEntry {
                    name: vn.to_string(),
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
                            .resolve_variant(&vname, &project.config.constants)?;
                        commands::bury(
                            &fossil,
                            &project,
                            None,
                            Some(v.name),
                            v.command,
                            true,
                        )
                    });
            let _ = tx.send(result.map_err(|e| e.to_string()));
        });

        self.loading = Some(BuryLoadingState {
            variant: variant_name.to_string(),
            rx,
            start: Instant::now(),
        });
        BuryAction::None
    }

    pub fn tick(&mut self) -> BuryAction {
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

    pub fn handle_key(
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

    pub fn render_popup(
        &mut self,
        frame: &mut Frame,
        area: Rect,
    ) {
        if let Some(ref loading) = self.loading {
            let text = format!(
                " burying {} {}",
                loading.variant,
                spinner_frame(loading.start),
            );
            render_toast(frame, area, &text, theme::WARN);
        } else {
            self.selector.render_popup(frame, area);
        }
    }
}
