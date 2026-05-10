use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc;

use crate::commands;
use crate::entity::DirEntity;
use crate::fossil::{Fossil, FossilVariantKey};
use crate::project::Project;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use super::{ListEntry, SelectorAction, SelectorPopup};

pub struct BuryPopupState {
    fossil_path: PathBuf,
    project_path: PathBuf,
    variants: Vec<FossilVariantKey>,
    selector: SelectorPopup,
}

pub enum BuryAction {
    None,
    Dismiss,
    Started(String, mpsc::Receiver<Result<String, String>>),
}

impl BuryPopupState {
    pub fn new(fossil: &Fossil, project_path: PathBuf) -> Self {
        let variants: Vec<FossilVariantKey> =
            fossil.config.variants.keys().cloned().collect();
        let entries: Vec<ListEntry> = variants
            .iter()
            .map(|vn| {
                let cmd = fossil
                    .resolve_variant(vn, &BTreeMap::new())
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
            selector: SelectorPopup::new("bury variant", entries),
        }
    }

    fn start_bury(&mut self) -> BuryAction {
        let idx = self.selector.list.selected;
        let variant_name = match self.variants.get(idx) {
            Some(n) => n.clone(),
            None => return BuryAction::None,
        };

        let project_path = self.project_path.clone();
        let fossil_path = self.fossil_path.clone();
        let vname = variant_name.clone();

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = Project::load(&project_path).and_then(|project| {
                let fossil = Fossil::load(&fossil_path)?;
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

        BuryAction::Started(variant_name.to_string(), rx)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> BuryAction {
        match self.selector.handle_key(key) {
            SelectorAction::Select(_) => self.start_bury(),
            SelectorAction::Dismiss => BuryAction::Dismiss,
            SelectorAction::None => BuryAction::None,
        }
    }

    pub fn render_popup(&mut self, frame: &mut Frame, area: Rect) {
        self.selector.render_popup(frame, area);
    }
}
