use crate::config::{
    Config, LayerVersion, FossilRecord, find_fossil_config, load_config, save_config,
};
use crate::utils;
use std::collections::{BTreeSet, HashMap, hash_map::DefaultHasher};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::error::Error;

pub struct Fossil {
    pub name: String,
    pub content: Vec<u8>,
    pub path: PathBuf,
}

impl Fossil {
    pub fn new(name: String, content: Vec<u8>, path: PathBuf) -> Self {
        Fossil {
            name,
            content,
            path,
        }
    }

    pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
        let content = std::fs::read(&path)?;
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let name = format!("{:x}", hasher.finish());
        Ok(Self {
            name,
            content,
            path,
        })
    }
}

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let fossil_dir = PathBuf::from(".fossil");
    let store_dir = fossil_dir.join("store");

    if fossil_dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Fossil repository already exists",
        )
        .into());
    }

    fs::create_dir_all(&store_dir)?;

    let empty_config = Config {
        fossils: HashMap::new(),
        current_layer: 0,
        surface_layer: 0,
        file_current_layers: HashMap::new(),
    };
    save_config(&empty_config)?;

    Ok(())
}

fn file_globs_to_paths(files: Vec<String>) -> Result<Vec<PathBuf>,
                                                      Box<dyn std::error::Error>> {
    let paths: Vec<PathBuf> = files.iter()
        .map(|f| utils::expand_pattern(&f))
        .flatten()
        .filter(|p| p.exists())
        .collect();
    Ok(paths)
}

fn paths_to_hashes(paths: &Vec<PathBuf>) -> Result<Vec<String>, Box<dyn Error>> {
    let hashes: Vec<String> = paths
        .iter()
        .map(|p| utils::expand_pattern(&p.to_string_lossy()))
        .flatten()
        .map(|p| utils::hash_path(&p))
        .collect();
    Ok(hashes)
}

pub fn track(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;
    let paths: Vec<PathBuf> = file_globs_to_paths(files)?; 
    for path in paths {
 
        // Read file content for hashing
        let content = fs::read(&path)?;
        let content_hash = utils::hash_content(&content);
        let path_hash = utils::hash_path(&path);
        let path_str = path.to_string_lossy().to_string();

        if config.fossils.contains_key(&path_hash) {
            println!("Fossil is already tracked...");
        } else {
            // New fossils are added both to the store and the config.
            let layer_version = LayerVersion::new(config.current_layer, &content_hash);
            let tracked_file = FossilRecord::new(path_str, &layer_version);
            
            config.fossils.insert(path_hash.clone(), tracked_file);
            utils::copy_to_store(&path, &path_hash, 0, &content_hash)?;
            println!("Tracked: {} (version 1)", path.display());
        }
    }

    save_config(&config)?;
    Ok(())
}

pub fn untrack(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;
    let untrack_paths = file_globs_to_paths(files)?;
    let untrack_hashes = paths_to_hashes(&untrack_paths)?;

    for p in untrack_hashes {
        match config.fossils.remove(&p) {
            Some(_) => println!("Untracking: {}", p),
            None => eprintln!("Failed to untrack: {}", p),
        }
    }

    save_config(&config)?;
    Ok(())
}

/// Burry the contents of the file referenced by `file_path` into
/// a new layer of the FossilRecord.
fn bury_fossil(file_path: &PathBuf, fossil: &FossilRecord,
               copy: bool, tag: String) -> Result<(), Box<dyn Error>>
{
    let content = fs::read(&file_path)?;
    let content_hash = utils::hash_content(&content); 
    let last_layer = fossil.layer_versions
        .iter()
        .last()
        .unwrap();
    
    let _new_layer = if copy {
        LayerVersion::copy_from_previous(&last_layer) 
    } else {
        LayerVersion::new_from_previous(&last_layer, content_hash, tag)
    };

    // TODO: Fix me - fossil is immutable reference, need to get mutable reference from config
    // fossil.push_layer(new_layer);

    Ok(())
}

pub fn bury_files(files: Vec<String>, tag: String) -> Result<(), Box<dyn Error>> {
     
    let config = load_config()?;
    let bury_paths = file_globs_to_paths(files)?;
    let bury_hashes = paths_to_hashes(&bury_paths)?;

    for (path, hash) in bury_paths.iter().zip(bury_hashes.iter()) {
        
        let record: &FossilRecord = config.fossils.get(hash).ok_or("File not tracked")?;
        let should_copy = utils::file_has_changed(path, record)?;
        
        // If the file which we're trying to bury hasn't chaged, push a copy.
        bury_fossil(path, record, should_copy, tag.clone())?; 
    }

    save_config(&config)?;
    Ok(())
}

fn dig_file_version(path: &PathBuf, layer: u32) -> Result<(), Box<dyn std::error::Error>>  {
   let config = load_config()?;
   let path_hash = utils::hash_path(path); 
   let fossil_file = config.fossils.get(&path_hash).ok_or("File not tracked")?;
   
   if let Some(layer_version) = utils::find_layer_version(&fossil_file, layer) {
       let store_path = utils::get_store_path(
           &path_hash,
           layer_version.version,
           &layer_version.content_hash,
       );

       if !store_path.exists() {
           eprintln!("Warning: Store file missing for {}", path.display());
           return Ok(());
       }
       utils::create_symlink(&store_path, &path)?;
       println!("Restored: {} -> {} (layer {})", path.display(), store_path.display(), layer); 
   } else {
       // File didn't exist in target layer, so remove if exists
       if path.exists() || path.is_symlink() {
           fs::remove_file(&path)?;
           println!("Removed: {} (didn't exist in layer {})", path.display(), layer);
       }
   }
   Ok(()) 
}

pub fn dig_by_files(layer: u32, files: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;

    if config.fossils.is_empty() {
        println!("No fossils to dig. Use 'fossil track <files>' to track files.");
        return Ok(());
    }

    // Find files by paths and collect them
    let mut paths_to_dig = Vec::new();
    for path in files {
        let path_buf = PathBuf::from(path);
        if config.fossils.contains_key(&utils::hash_path(&path_buf)) {
            paths_to_dig.push(path_buf); 
        } 
    }
    
    // Should have specified at least one file
    if paths_to_dig.is_empty() {
        println!("No tracked files found matching the specified paths.");
        return Ok(());
    }
    
    // Try to restore each file to the specified layer
    let mut files_restored = 0;
    for fossil_path in paths_to_dig { 
        match dig_file_version(&fossil_path, layer) {
            Ok(_) => files_restored += 1,
            Err(e) => eprintln!("Failed to dig {} with error: {}", fossil_path.display(), e),
        } 
    }

    println!("Dug {} files", files_restored);
    Ok(())
}

pub fn dig_by_tag(tag: &str) -> Result<(), Box<dyn std::error::Error>> {
   let mut config = load_config()?;

   if config.fossils.is_empty() {
       println!("No fossils to dig. Use 'fossil track <files>' to start tracking files.");
       return Ok(());
   }

   let files_with_tag: Vec<(String, FossilRecord)> = config
       .fossils
       .iter()
       .filter(|(_, tracked_file)| tracked_file.layer_versions.iter().any(|lv| lv.tag == tag))
       .map(|(path_hash, tracked_file)| (path_hash.clone(), tracked_file.clone()))
       .collect();

   if files_with_tag.is_empty() {
       println!("No tracked files found with tag '{}'.", tag);
       return Ok(());
   }

   let mut files_restored = 0;

   for (path_hash, tracked_file) in files_with_tag {
       let original_path = PathBuf::from(&tracked_file.original_path);

       if let Some(layer_version) = tracked_file.layer_versions.iter().find(|lv| lv.tag == tag) {
           match dig_file_version(&original_path, layer_version.layer) {
               Ok(_) => {
                   utils::update_file_layer(&mut config, &path_hash, layer_version.layer);
                   files_restored += 1;
               },
               Err(e) => eprintln!("Failed to dig {} with error: {}", original_path.display(), e),
           }
       }
   }

   save_config(&config)?;
   println!("Dug {} files with tag '{}'", files_restored, tag);
   Ok(())
}

pub fn dig_by_layer(layer: u32) -> Result<(), Box<dyn std::error::Error>> {
   let mut config = load_config()?;

   if config.fossils.is_empty() {
       println!("No fossils to dig. Use 'fossil track <files>' to start tracking files.");
       return Ok(());
   }

   if layer > config.surface_layer {
       println!("Can not dig to a layer above the surface.");
       return Ok(());
   }

   let mut files_restored = 0;
   let mut files_removed = 0;

   let fossils_data: Vec<(String, FossilRecord)> = config
       .fossils
       .iter()
       .map(|(k, v)| (k.clone(), v.clone()))
       .collect();

   for (path_hash, tracked_file) in fossils_data {
       let original_path = PathBuf::from(&tracked_file.original_path);

       match dig_file_version(&original_path, layer) {
           Ok(_) => {
               utils::update_file_layer(&mut config, &path_hash, layer);
               if utils::find_layer_version(&tracked_file, layer).is_some() {
                   files_restored += 1;
               } else {
                   files_removed += 1;
               }
           },
           Err(e) => eprintln!("Failed to dig {} with error: {}", original_path.display(), e),
       }
   }

   config.current_layer = layer;
   save_config(&config)?;

   println!(
       "Excavated to layer {} ({} files restored, {} files removed)",
       layer, files_restored, files_removed
   );

   Ok(())
}

pub fn surface() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;

    for (path_hash, tracked_file) in &config.fossils {
        if let Some(layer_version) = utils::find_layer_version(tracked_file, config.surface_layer) {
            // Restore the file back directly, not as a symlink.
            let store_path = utils::get_store_path(
                path_hash,
                layer_version.version,
                &layer_version.content_hash,
            );
            let original_path = PathBuf::from(&tracked_file.original_path);

            // Todo: Handle error propogation better.
            match utils::restore_file(&original_path, &store_path) {
                Ok(_) => println!("Restored fossil: {} to surface", original_path.display()),
                Err(e) => eprintln!("Failed to restore fossil to surface.. {}", e),
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
            let current_marker = if *layer == config.current_layer {
                " (current)"
            } else {
                ""
            };
            println!("  Layer {}{}", layer, current_marker);
        }
        println!();
    }

    // Print out all the fossils we have a record of.
    println!("Tracked fossils:");
    println!(
        "{:<16} {:<40} {:<8} {:<8} {:<20}",
        "Hash", "Path", "Versions", "Layers", "Last Tracked"
    );
    println!("{}", "=".repeat(100));

    for (hash, tracked_file) in &config.fossils {
        println!(
            "{:<16} {:<40} {:<8} {:<8} {:<20}",
            &hash[..8.min(hash.len())],
            tracked_file.original_path.display(),
            tracked_file.versions,
            tracked_file.layer_versions.len(),
            tracked_file.last_tracked.format("%Y-%m-%d %H:%M:%S")
        );
    }
    Ok(())
}

pub fn reset() -> Result<(), Box<dyn std::error::Error>> {
    let fossil_dir = find_fossil_config()?;
    let store_dir = fossil_dir.join("store");

    // Restore symlinks with their original files before clearing.
    surface()?;

    if store_dir.exists() {
        fs::remove_dir_all(&store_dir)?;
        fs::create_dir_all(&store_dir)?;
    }

    let empty_config = Config {
        fossils: HashMap::new(),
        current_layer: 0,
        surface_layer: 0,
        file_current_layers: HashMap::new(),
    };
    save_config(&empty_config)?;

    Ok(())
}
