use std::path::PathBuf;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};


pub struct Fossil {
    pub name: String,
    pub content: Vec<u8>,
    pub path: PathBuf
}

impl Fossil {

    pub fn new(name: String, content: Vec<u8>, path: PathBuf) -> Self {
        Fossil {
            name,
            content,
            path
        }
    }

    pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
        let content = std::fs::read(&path)?;
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let name = format!("{:x}", hasher.finish());
        Ok(Self { name, content, path })
    }
} 

