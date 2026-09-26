use crate::error::FossilError;
use std::path::Path;

/// [Fossil Doc] A DirEntity is any struct which is backed by a
/// config.toml file, somewhere in `.fossil`. Currently this
/// is just Fossil and Project.
pub trait DirEntity: Sized {
    const CONFIG_FILE: &'static str;
    fn load(dir: &Path) -> Result<Self, FossilError>;
    fn sort_key(&self) -> &str;
    fn list_all(parent: &Path) -> Result<Vec<Self>, FossilError> {
        let entries = match std::fs::read_dir(parent) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(error) => return Err(error.into()),
        };
        let mut items = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path.join(Self::CONFIG_FILE).try_exists()? {
                items.push(Self::load(&path)?);
            }
        }
        items.sort_by(|a, b| a.sort_key().cmp(b.sort_key()));
        Ok(items)
    }
}
