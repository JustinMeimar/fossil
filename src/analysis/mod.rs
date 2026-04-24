mod parser;
pub mod quantity;
use std::path::PathBuf;
use crate::manifest::Manifest;
pub use parser::Parser;
pub use quantity::{MetricSet, Summary};

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
