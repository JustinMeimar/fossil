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
/// ```
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
    let config_path = PathBuf::from(".fossil/config.toml");
    
    // Create the config if it does not exist.
    if !config_path.exists() {
        return Ok(Config {
            fossils: HashMap::new(),
            current_layer: 0,
            surface_layer: 0,
        });
    }
    
    // Load the config if it does exist.
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

