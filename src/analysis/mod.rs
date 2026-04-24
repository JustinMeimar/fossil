mod parser;
pub mod quantity;
use crate::manifest::Manifest;
pub use parser::Parser;
pub use quantity::{AnalysisResult, Summary};
use std::path::PathBuf;

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
