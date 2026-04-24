use std::path::Path;

use crate::error::FossilError;

pub trait DirEntity: Sized {
    fn load(dir: &Path) -> Result<Self, FossilError>;
    fn sort_key(&self) -> &str;

    fn list_all(parent: &Path) -> Result<Vec<Self>, FossilError> {
        let entries = match std::fs::read_dir(parent) {
            Ok(e) => e,
            Err(_) => return Ok(Vec::new()),
        };
        let mut items = Vec::new();
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            if let Ok(item) = Self::load(&entry.path()) {
                items.push(item);
            }
        }
        items.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
        Ok(items)
    }
}
