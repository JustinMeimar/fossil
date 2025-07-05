use std::path::PathBuf;
use std::collections::{HashMap, BTreeSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::fs;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::utils;

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
    pub last_content_hash: String,
    pub layer_versions: Vec<LayerVersion>,
}

#[derive(Deserialize, Serialize)]
pub struct LayerVersion {
    pub layer: u32,
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
        current_layer: 0,
        surface_layer: 0,
    };
    save_config(&empty_config)?;
    
    Ok(())
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
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

fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = PathBuf::from(".fossil/config.toml"); 
    let content = toml::to_string_pretty(config)?;

    // Write newly tracked files to the config.
    fs::write(&config_path, content)?;
    Ok(())
}

fn expand_pattern(pattern: &str) -> Vec<PathBuf> {
    if pattern.contains('*') || pattern.contains('?') {
        // Expand the paths and collect those which are not errors. 
        match glob::glob(pattern) {
            Ok(paths) => paths.filter_map(Result::ok).collect(),
            Err(_) => vec![],
        }
    } else {
        // Treat non-regex paths regularly.
        vec![PathBuf::from(pattern)]
    }
}

fn copy_to_store(file: &PathBuf, path_hash: &str, version: u32,
                 content_hash: &str) -> Result<(), Box<dyn std::error::Error>>
{
    let version_dir = PathBuf::from(".fossil/store")
        .join(path_hash)
        .join(version.to_string());
    fs::create_dir_all(&version_dir)?;
    
    let content_path = version_dir.join(content_hash);
    fs::copy(file, &content_path)?;
    
    Ok(())
}

pub fn track(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;
    
    // Iterate over the files to track.
    for file_pattern in files {

        // A single track may correspond to many files if it's a pattern.
        let paths = expand_pattern(&file_pattern);

        for path in paths {
            if !path.exists() {
                eprintln!("Warning: File {} does not exist", path.display());
                continue;
            }
            
            // Read file content for hashing
            let content = fs::read(&path)?;
            let content_hash = utils::hash_content(&content);
            let path_hash = utils::hash_path(&path);
            let path_str = path.to_string_lossy().to_string();
            
            if config.fossils.contains_key(&path_hash) {
                println!("Fossil is already tracked..."); 
            } else {
                // New fossils are added both to the store and the config.
                let layer_version = LayerVersion {
                    layer: config.current_layer,
                    version: 0,
                    content_hash: content_hash.clone(),
                    timestamp: Utc::now(),
                };
                let tracked_file = TrackedFile {
                    original_path: path_str,
                    versions: 1,
                    last_tracked: Utc::now(),
                    last_content_hash: content_hash.clone(),
                    layer_versions: vec![layer_version],
                };
                config.fossils.insert(path_hash.clone(), tracked_file);
                copy_to_store(&path, &path_hash, 0, &content_hash)?;
                println!("Tracked: {} (version 1)", path.display());
            } 
        }
    }
    
    save_config(&config)?;
    Ok(())
}

// TODO: Make burry take in a Vec<String> of files to burry.
pub fn burry() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;
    let mut changes = 0;
    
    if config.surface_layer != config.current_layer {
        println!("Can only burry files from the surface.");
        return Ok(());
    }
    if config.fossils.is_empty() {
        println!("No fossils to burry. Use 'fossil track <files>' to start tracking files.");
        return Ok(());
    }
    
    // Increment to new layer
    config.current_layer += 1;
    config.surface_layer += 1;
    let new_layer = config.current_layer;
    let layer_timestamp = Utc::now();
    
    for (path_hash, tracked_file) in &mut config.fossils {
        let file_path = PathBuf::from(&tracked_file.original_path);
        
        if !file_path.exists() {
            eprintln!("Warning: {} no longer exists", file_path.display());
            continue;
        }
        
        if utils::file_has_changed(&file_path, tracked_file)? {
            let content = fs::read(&file_path)?;
            let content_hash = utils::hash_content(&content);
            
            tracked_file.versions += 1;
            tracked_file.last_tracked = layer_timestamp;
            tracked_file.last_content_hash = content_hash.clone();
            
            let layer_version = LayerVersion {
                layer: new_layer,
                version: tracked_file.versions - 1,
                content_hash: content_hash.clone(),
                timestamp: layer_timestamp,
            };
            tracked_file.layer_versions.push(layer_version);
            
            copy_to_store(&file_path, path_hash, tracked_file.versions - 1, &content_hash)?;
            changes += 1;
            println!("Burried: {} (layer {}, version {})", file_path.display(), new_layer,
                                                           tracked_file.versions);
        } else {
            // File hasn't changed, but we still add it to this layer with existing content
            if let Some(last_layer_version) = tracked_file.layer_versions.last() {
                let layer_version = LayerVersion {
                    layer: new_layer,
                    version: last_layer_version.version,
                    content_hash: last_layer_version.content_hash.clone(),
                    timestamp: layer_timestamp,
                };
                tracked_file.layer_versions.push(layer_version);
            }
        }
    }
    
    save_config(&config)?;
    
    if changes > 0 {
        println!("Created layer {} with {} changed files", new_layer, changes);
    } else {
        println!("Created layer {} (no content changes)", new_layer);
    }
    
    Ok(())
}

pub fn dig(depth: u32) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;
    
    if config.fossils.is_empty() {
        println!("No fossils to dig. Use 'fossil track <files>' to start tracking files.");
        return Ok(());
    }
    
    let target_layer = config.current_layer.saturating_sub(depth);
    
    if target_layer == config.current_layer && depth > 0 {
        return Err("Cannot dig deeper than available layers".into());
    }
    
    let mut files_restored = 0;
    let mut files_removed = 0;
    
    for (path_hash, tracked_file) in &config.fossils {
        let original_path = PathBuf::from(&tracked_file.original_path);
        
        if let Some(layer_version) = utils::find_layer_version(tracked_file, target_layer) {
            let store_path = utils::get_store_path(path_hash, layer_version.version, &layer_version.content_hash);
            
            if store_path.exists() {
                utils::create_symlink(&store_path, &original_path)?;
                files_restored += 1;
                println!("Restored: {} -> {}", original_path.display(), store_path.display());
            } else {
                eprintln!("Warning: Store file missing for {}", original_path.display());
            }
        } else {
            // File didn't exist in target layer, so remove if exists
            if original_path.exists() || original_path.is_symlink() {
                fs::remove_file(&original_path)?;
                files_removed += 1;
                println!("Removed: {} (didn't exist in layer {})", original_path.display(), target_layer);
            }
        }
    }
    
    config.current_layer = target_layer;
    save_config(&config)?;
    
    println!("Excavated to layer {} ({} files restored, {} files removed)", 
             target_layer, files_restored, files_removed);
    
    Ok(())
}

pub fn surface() -> Result<(), Box<dyn std::error::Error>> {
         
    let mut config = load_config()?;

    for (path_hash, tracked_file) in &config.fossils {

        if let Some(layer_version) = utils::find_layer_version(tracked_file, 
                                                        config.surface_layer) {
            // Restore the file back directly, not as a symlink. 
            let store_path = utils::get_store_path(path_hash, layer_version.version,
                                                       &layer_version.content_hash);
            let original_path =  PathBuf::from(&tracked_file.original_path);

            // Todo: Handle error propogation better. 
            match utils::restore_file(&original_path, &store_path) {
                Ok(_) => println!("Restored fossil: {} to surface", original_path.display()),
                Err(e) => eprintln!("Failed to restore fossil to surface.. {}", e)
            }
        }
    }

    config.current_layer = config.surface_layer;
    save_config(&config)?;
    Ok(())
} 

pub fn list() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?; 
    
    println!("Current layer: {}", config.current_layer);
    println!();
    
    if config.fossils.is_empty() {
        println!("No fossils found. Use 'fossil track <files>' to start tracking files.");
        return Ok(());
    }
    
    // Collect all layers and their timestamps
    let mut all_layers: BTreeSet<u32> = BTreeSet::new();
    for tracked_file in config.fossils.values() {
        for layer_version in &tracked_file.layer_versions {
            all_layers.insert(layer_version.layer);
        }
    }
    
    if !all_layers.is_empty() {
        println!("Available layers:");
        for layer in all_layers.iter().rev() {
            let current_marker = if *layer == config.current_layer { " (current)" } else { "" };
            println!("  Layer {}{}", layer, current_marker);
        }
        println!();
    }
    
    // Print out all the fossils we have a record of.
    println!("Tracked fossils:");
    println!("{:<16} {:<40} {:<8} {:<8} {:<20}", "Hash", "Path", "Versions", "Layers", "Last Tracked");
    println!("{}", "=".repeat(100));
    
    for (hash, tracked_file) in &config.fossils {
        println!("{:<16} {:<40} {:<8} {:<8} {:<20}", 
            &hash[..8.min(hash.len())],
            tracked_file.original_path,
            tracked_file.versions,
            tracked_file.layer_versions.len(),
            tracked_file.last_tracked.format("%Y-%m-%d %H:%M:%S")
        );
    } 
    Ok(())
} 

