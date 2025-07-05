use crate::utils;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::io;
use diffy::{create_patch, apply, PatchFormatter, Patch};

#[derive(Deserialize, Serialize, Clone)]
pub struct FossilRecord {
    pub file_path: PathBuf,
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

impl FossilRecord {
    pub fn new(path: &PathBuf, content: &Vec<u8>) -> Self {
         
        let content_hash = utils::hash_content(&content);
        let first_layer = LayerVersion::new(0, &content_hash);

        FossilRecord {
            file_path: path.clone(),
            versions: 1,
            last_tracked: Utc::now(),
            last_content_hash: first_layer.content_hash.clone(),
            layer_versions: vec![first_layer],
        }
    }
    
    fn get_version_contents(&self, version: u32)
        -> Result<Vec<u8>, Box<dyn std::error::Error>>
    {    
        let layer = self.layer_versions.iter().last().unwrap(); 
        let path_hash = utils::hash_path(&self.file_path);
        
        let version_path = find_fossil_config()?
            .join("store")
            .join(path_hash)
            .join(version.to_string())
            .join(&layer.content_hash);

        let contents = fs::read(&version_path)?;
        Ok(contents)
    }

    pub fn update(&mut self, tag: String) -> Result<(), Box<dyn std::error::Error>> {
        
        // Step 1. Inputs for the patch
        let cur_version = self.versions;
        let last_layer = self.layer_versions.iter().last().unwrap(); 
        let last_fossil_contents = self.get_version_contents(cur_version)?; 
        let cur_fossil_contents = fs::read(&self.file_path)?;
        
        // Step 2. Make the patch
        // let patch = create_patch(&last_fossil_contents, &cur_fossil_contents); 
        // let new_version = if patch.hunks().is_empty()  {
        //     last_layer.version;
        // } else {
        //     last_layer.version + 1;
        // };

        // Step 3. Create new layer from patch.
        let new_layer = LayerVersion {
            layer: last_layer.layer + 1,
            tag: tag,
            version: last_layer.version + 1 , // TODO Use computed version
            content_hash: utils::hash_content(&cur_fossil_contents),
            timestamp: Utc::now(),
            // diff: patch 
        };

        Ok(())
    }

    pub fn push_layer(&mut self, layer: LayerVersion) -> Result<(), Box<dyn std::error::Error>> {
        self.versions += 1;
        self.last_tracked = layer.timestamp;
        self.last_content_hash = layer.content_hash.clone();
        
        // TODO: Prevent dumb copy here!
        utils::copy_to_store(
            &self.file_path,
            &utils::hash_path(&self.file_path),
            layer.version,
            &layer.content_hash,
        )?;

        self.layer_versions.push(layer);

        Ok(())
    }

    // Ensure that there is a store for the last layer.
    pub fn sync(&self) {
        let last_layer = self.layer_versions.iter().last().unwrap();
        let _contents = &last_layer.content_hash;
        let _layer = last_layer.layer;
    }
}

impl LayerVersion {
    pub fn new(layer: u32, content_hash: &String) -> Self {
        LayerVersion {
            layer: layer,
            tag: String::new(),
            version: 0,
            content_hash: content_hash.clone(),
            timestamp: Utc::now(),
        }
    }

    // pub fn copy_from_previous(other: &LayerVersion) -> Self {
    //     
    //     LayerVersion {
    //         layer: other.layer + 1,
    //         tag: other.tag.clone(),
    //         version: other.version,
    //         content_hash: other.content_hash.clone(),
    //         timestamp: Utc::now(),
    //     }
    // }
    //
    // pub fn new_from_previous(other: &LayerVersion, content_hash: String, tag: String) -> Self {
    //     LayerVersion {
    //         layer: other.layer + 1,
    //         tag: tag,
    //         version: other.version + 1,
    //         content_hash: content_hash,
    //         timestamp: Utc::now(),
    //     }
    // }

    pub fn with_tag(mut self, tag: String) -> Self {
        self.tag = tag;
        self
    }

    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    pub fn incr_version(mut self) -> Self {
        self.version += 1;
        self
    }
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
    pub fossils: HashMap<String, FossilRecord>,
    pub file_current_layers: HashMap<String, u32>,
    pub current_layer: u32,
    pub surface_layer: u32,
}

impl Config {

    pub fn add_fossil_record(&mut self, fossil: &FossilRecord)
        -> Result<(), Box<dyn std::error::Error>>
    {
        // We use the hash of the fossils path as it's key in the Config.
        let fossil_key = utils::hash_path(&fossil.file_path);
        let fossil_layer = 0; 
        
        self.fossils.insert(fossil_key.clone(), fossil.clone());
        
        // Copy the file, in it's entirety, into the store.
        utils::copy_to_store(&fossil.file_path,
                             &fossil_key,
                              fossil_layer,
                             &fossil.last_content_hash)?;
        Ok(())
    }
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let fossil_dir = find_fossil_config()?;
    let config_path = fossil_dir.join("config.toml");

    if !config_path.exists() {
        return Ok(Config {
            fossils: HashMap::new(),
            current_layer: 0,
            surface_layer: 0,
            file_current_layers: HashMap::new(),
        });
    }

    let content = fs::read_to_string(&config_path)?;
    let mut config: Config = toml::from_str(&content)?;

    // Ensure file_current_layers is initialized for backward compatibility
    if config.file_current_layers.is_empty() {
        for path_hash in config.fossils.keys() {
            config
                .file_current_layers
                .insert(path_hash.clone(), config.current_layer);
        }
    }

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
