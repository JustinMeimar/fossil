use crate::error::FossilError;
use std::path::Path;

/// [Fossil Doc] A DirEntity is any struct which is backed by a
/// config.toml file, somewhere in `.fossil`. Currently this
/// is just Fossil and Project.
///
/// NOTE(Justin): Should Analysis scripts and Figure scripts
/// implement this trait as well?
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
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|e| Self::load(&e.path()).ok())
            .collect();
        items.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
        Ok(items)
    }
}
