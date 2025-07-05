use std::path::PathBuf;
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::fs;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

pub struct Fossil {
    pub name: String,
    pub content: Vec<u8>,
    pub path: PathBuf
}

#[derive(Deserialize, Serialize)]
pub struct TrackedFile {
    pub original_path: String,
    pub versions: u32,
    pub last_tracked: DateTime<Utc>,
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
/// [fossils."f6e5d4c3b2a1"]
/// original_path = "./build/log/big-test.log"
/// versions = 3
/// last_tracked = "2023-01-01T00:00:00Z"
/// ```
#[derive(Deserialize, Serialize)]
pub struct Config {
    pub fossils: HashMap<String, TrackedFile>,
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

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let fossil_dir = PathBuf::from(".fossil");
    let store_dir = fossil_dir.join("store");

    if fossil_dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Fossil repository already exists"
        ).into());
    }
    
    fs::create_dir_all(&store_dir)?;
    
    let empty_config = Config {
        fossils: HashMap::new(),
    };
    save_config(&empty_config)?;
    
    Ok(())
}

fn hash_path(path: &PathBuf) -> String {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let config_path = PathBuf::from(".fossil/config.toml");
    if !config_path.exists() {
        return Ok(Config {
            fossils: HashMap::new(),
        });
    }
    
    let content = fs::read_to_string(&config_path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = PathBuf::from(".fossil/config.toml");
    let content = toml::to_string_pretty(config)?;
    fs::write(&config_path, content)?;
    Ok(())
}

fn expand_pattern(pattern: &str) -> Vec<PathBuf> {
    if pattern.contains('*') || pattern.contains('?') {
        match glob::glob(pattern) {
            Ok(paths) => paths.filter_map(Result::ok).collect(),
            Err(_) => vec![],
        }
    } else {
        vec![PathBuf::from(pattern)]
    }
}

fn copy_to_store(file: &PathBuf, hash: &str,
                 version: u32) -> Result<(), Box<dyn std::error::Error>>
{
    let store_dir = PathBuf::from(".fossil/store").join(hash);
    fs::create_dir_all(&store_dir)?;
    
    let version_path = store_dir.join(version.to_string());
    fs::copy(file, &version_path)?;
    
    Ok(())
}

pub fn track(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;
    
    for file_pattern in files {
        let paths = expand_pattern(&file_pattern);
        
        for path in paths {
            if !path.exists() {
                eprintln!("Warning: File {} does not exist", path.display());
                continue;
            }
            
            let path_hash = hash_path(&path);
            let path_str = path.to_string_lossy().to_string();
            
            if let Some(tracked_file) = config.fossils.get_mut(&path_hash) {
                tracked_file.versions += 1;
                tracked_file.last_tracked = Utc::now();
                copy_to_store(&path, &path_hash, tracked_file.versions - 1)?;
            } else {
                let tracked_file = TrackedFile {
                    original_path: path_str,
                    versions: 1,
                    last_tracked: Utc::now(),
                };
                config.fossils.insert(path_hash.clone(), tracked_file);
                copy_to_store(&path, &path_hash, 0)?;
            }
            
            println!("Tracked: {}", path.display());
        }
    }
    
    save_config(&config)?;
    Ok(())
} 

