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
        let mut items: Vec<Self> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().map(|t| t.is_dir()).unwrap_or(false)
            })
            .filter_map(|e| Self::load(&e.path()).ok())
            .collect();
        items.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
        Ok(items)
    }
}
