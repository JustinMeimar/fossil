
use crate::utils;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::io;
use diffy::{Patch, create_patch};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub fossils: Vec<Fossil>,
    pub created: DateTime<Utc>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Fossil {
    pub hash: String,
    pub base_content: Vec<u8>,
    pub layer_versions: Vec<FossilVersion>,
    pub last_tracked: DateTime<Utc>,
    pub current_version: u32,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct FossilVersion {
    pub patch_text: String,  // Store patch as string
    pub version_no: u32,
    pub tag: String,
    pub timestamp: DateTime<Utc>,
}

impl Fossil {
    pub fn new(path: &PathBuf, content: &Vec<u8>) -> io::Result<Self> {
        let path_hash = utils::hash_path(path);
        let cont_hash = utils::hash_content(content);
        
        Ok(Fossil {
            hash: format!("{}:{}", path_hash, cont_hash),
            base_content: content.clone(),
            layer_versions: Vec::new(),
            last_tracked: Utc::now(),
            current_version: 1
        })
    }
    
    pub fn get_version_content(&self, version: u32)
        -> Result<Vec<u8>, Box<dyn std::error::Error>>
    {
        if version == 0 {
            return Err("Invalid version".into());
        }
        
        if version == 1 {
            return Ok(self.base_content.clone());
        }
        
        if version > self.layer_versions.len() as u32 + 1 {
            return Err("Version too high".into());
        }
        
        let mut content = String::from_utf8(self.base_content.clone())?;
        for layer in &self.layer_versions[0..(version - 1) as usize] {
            content = layer.apply_to(&content)?;
        }
        
        Ok(content.into_bytes())
    }
    
    pub fn add_version(&mut self, new_content: &Vec<u8>, tag: Option<String>)
        -> Result<(), Box<dyn std::error::Error>>
    {
        let prev_content = self.get_version_content(self.current_version)?;    
        if prev_content == *new_content {
            return Ok(());
        }
        
        let new_version = FossilVersion::from_diff(
            &prev_content,
            new_content,
            self.current_version + 1,
            tag
        )?;
        
        self.layer_versions.push(new_version);
        self.current_version += 1;
        self.last_tracked = Utc::now();
        
        Ok(())
    }
}

impl FossilVersion {
    pub fn from_diff(old_content: &Vec<u8>, new_content: &Vec<u8>, 
                     version_no: u32, tag: Option<String>) 
                     -> Result<Self, Box<dyn std::error::Error>> {
        let old_str = std::str::from_utf8(old_content)?;
        let new_str = std::str::from_utf8(new_content)?; 
        let patch = create_patch(old_str, new_str);
        
        Ok(FossilVersion {
            patch_text: patch.to_string(),
            version_no,
            tag: tag.unwrap_or_default(),
            timestamp: Utc::now()
        })
    }
    
    pub fn apply_to(&self, content: &str) -> Result<String, Box<dyn std::error::Error>> {
        let patch = diffy::Patch::from_str(&self.patch_text)?;
        Ok(diffy::apply(content, &patch)?)
    }
    
    pub fn get_diff_display(&self) -> &str {
        &self.patch_text
    }
    
    pub fn has_changes(&self) -> bool {
        !self.patch_text.trim().is_empty()
    }
}

pub fn find_fossil_config() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut current_dir = std::env::current_dir()?;
    loop {
        let fossil_dir = current_dir.join(".fossil");
        if fossil_dir.exists() {
            return Ok(fossil_dir);
        }
        if current_dir.join(".git").exists() {
            break;
        }
        match current_dir.parent() {
            Some(parent) => current_dir = parent.to_path_buf(),
            None => break,
        }
    }
    Err("No .fossil directory found".into())
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let fossil_dir = find_fossil_config()?;
    let config_path = fossil_dir.join("config.toml");
    let content = fs::read_to_string(&config_path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

pub fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = PathBuf::from(".fossil/config.toml");
    let content = toml::to_string_pretty(config)?;
    fs::write(&config_path, content)?;
    Ok(())
}
