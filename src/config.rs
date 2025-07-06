use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Deserialize, Serialize, Clone)]
pub struct TrackedFile {
    pub original_path: String,
    pub versions: u32,
    pub last_tracked: DateTime<Utc>,
    pub last_content_hash: String,
    pub layer_versions: Vec<LayerVersion>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct LayerVersion {
    pub layer: u32,
    pub tag: String,
    pub version: u32,
    pub content_hash: String,
    pub timestamp: DateTime<Utc>,
}


/// The struct representing the .fossil/config.toml file, which remembers
/// the files in the project to track, their place in the store and version.
/// Example:
/// ```toml
/// [fossils]
/// 
/// [fossils."a1b2c3d4e5f6"]
/// original_path = "./build/meta/output"
/// versions = 7
/// last_tracked = "2023-01-01T00:00:00Z"
/// 
/// ```
#[derive(Deserialize, Serialize)]
pub struct Config {
    pub fossils: HashMap<String, TrackedFile>,
    pub current_layer: u32,
    pub surface_layer: u32,
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let fossil_dir = find_fossil_config()?;
    let config_path = fossil_dir.join("config.toml");
    
    if !config_path.exists() {
        return Ok(Config {
            fossils: HashMap::new(),
            current_layer: 0,
            surface_layer: 0,
        });
    }
    
    let content = fs::read_to_string(&config_path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

pub fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = PathBuf::from(".fossil/config.toml"); 
    let content = toml::to_string_pretty(config)?;

    // Write newly tracked files to the config.
    fs::write(&config_path, content)?;
    Ok(())
}

pub fn find_fossil_config() -> Result<PathBuf, Box<dyn std::error::Error>> {
   let mut current_dir = std::env::current_dir()?;
   
   loop {
       let fossil_dir = current_dir.join(".fossil");
       if fossil_dir.exists() {
           return Ok(fossil_dir);
       }
       // Don't recurse past the git root.
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

