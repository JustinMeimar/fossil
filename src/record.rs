use std::path::PathBuf;

use crate::manifest::Manifest;

/// [Fossil Doc] `Record`
/// -------------------------------------------------------------
/// A Record is a single preserved run, one invocation of `bury`.
/// Contains a manifest (metadata) and results (observations).
/// The fossil record is the collection of all Records for a Fossil.
pub struct Record {
    pub dir: PathBuf,
    pub manifest: Manifest,
}

impl Record {
    pub fn id(&self) -> String {
        self.dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}
