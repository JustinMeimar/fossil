use crate::config::{
    Config, LayerVersion, TrackedFile, find_fossil_config, load_config, save_config,
};
use crate::utils;
use chrono::Utc;
use std::collections::{BTreeSet, HashMap, hash_map::DefaultHasher};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

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

pub fn track(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;

    // Iterate over the files to track.
    for file_pattern in files {
        // A single track may correspond to many files if it's a pattern.
        let paths = utils::expand_pattern(&file_pattern);

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
                    tag: String::new(),
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
                utils::copy_to_store(&path, &path_hash, 0, &content_hash)?;
                println!("Tracked: {} (version 1)", path.display());
            }
        }
    }

    save_config(&config)?;
    Ok(())
}

pub fn untrack(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;

    let untrack_fossil_hahses = files
        .iter()
        .map(|p| utils::expand_pattern(p))
        .flatten()
        .map(|p| utils::hash_path(&p));

    for p in untrack_fossil_hahses {
        match config.fossils.remove(&p) {
            Some(_) => println!("Untracking: {}", p),
            None => eprintln!("Failed to untrack: {}", p),
        }
    }

    save_config(&config)?;
    Ok(())
}

pub fn bury(
    files: Option<Vec<String>>,
    tag: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;
    let mut changes = 0;

    if config.surface_layer != config.current_layer {
        println!("Can only bury files from the surface.");
        return Ok(());
    }
    if config.fossils.is_empty() {
        println!("No fossils to bury. Use 'fossil track <files>' to start tracking files.");
        return Ok(());
    }

    // Increment to new layer
    config.current_layer += 1;
    config.surface_layer += 1;
    let new_layer = config.current_layer;
    let layer_timestamp = Utc::now();

    // Resolve specified files to existing tracked fossils
    let mut target_fossils = std::collections::HashSet::new();
    if let Some(file_patterns) = files {
        for pattern in file_patterns {
            let paths = utils::expand_pattern(&pattern);
            for path in paths {
                let path_hash = utils::hash_path(&path);
                if config.fossils.contains_key(&path_hash) {
                    target_fossils.insert(path_hash);
                }
            }
        }
    }

    for (path_hash, tracked_file) in &mut config.fossils {
        let file_path = PathBuf::from(&tracked_file.original_path);

        // Check if we should process this file. No target fossil -> bury all.
        let should_bury = target_fossils.is_empty() || target_fossils.contains(path_hash);

        // Check the file exists.
        if !file_path.exists() {
            eprintln!("Warning: {} no longer exists", file_path.display());
            continue;
        }

        // Fossils not to be burried or are unchanged, copy the previous layer version.
        if (!should_bury && tracked_file.layer_versions.last().is_some())
            || !utils::file_has_changed(&file_path, tracked_file)?
        {
            let last_layer = tracked_file.layer_versions.last().unwrap();
            let layer_version = LayerVersion {
                layer: new_layer,
                tag: tag.clone().unwrap_or_default(),
                version: last_layer.version,
                content_hash: last_layer.content_hash.clone(),
                timestamp: layer_timestamp,
            };
            tracked_file.layer_versions.push(layer_version);
            continue;
        }

        let content = fs::read(&file_path)?;
        let content_hash = utils::hash_content(&content);

        tracked_file.versions += 1;
        tracked_file.last_tracked = layer_timestamp;
        tracked_file.last_content_hash = content_hash.clone();

        let layer_version = LayerVersion {
            layer: new_layer,
            tag: tag.clone().unwrap_or_default(),
            version: tracked_file.versions - 1,
            content_hash: content_hash.clone(),
            timestamp: layer_timestamp,
        };
        tracked_file.layer_versions.push(layer_version);

        utils::copy_to_store(
            &file_path,
            path_hash,
            tracked_file.versions - 1,
            &content_hash,
        )?;
        changes += 1;
        println!(
            "Burried: {} (layer {}, version {})",
            file_path.display(),
            new_layer,
            tracked_file.versions
        );
    }

    save_config(&config)?;

    if changes > 0 {
        println!("Created layer {} with {} changed files", new_layer, changes);
    } else {
        println!("Created layer {} (no content changes)", new_layer);
    }

    Ok(())
}

pub fn dig_by_files(files: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;

    if config.fossils.is_empty() {
        println!("No fossils to dig. Use 'fossil track <files>' to track files.");
        return Ok(());
    }

    // Find files by paths and collect them
    let mut files_to_dig = Vec::new();

    for path in files {
        let path_buf = PathBuf::from(path);
        let path_hash = utils::hash_path(&path_buf);

        if let Some(tracked_file) = config.fossils.get(&path_hash) {
            files_to_dig.push((path_hash, tracked_file.clone()));
        }
    }

    if files_to_dig.is_empty() {
        println!("No tracked files found matching the specified paths.");
        return Ok(());
    }

    let mut files_restored = 0;

    for (path_hash, tracked_file) in files_to_dig {
        let original_path = PathBuf::from(&tracked_file.original_path);
        let current_layer = *config
            .file_current_layers
            .get(&path_hash)
            .unwrap_or(&config.current_layer);

        if let Some(layer_version) = utils::find_layer_version(&tracked_file, current_layer) {
            let store_path = utils::get_store_path(
                &path_hash,
                layer_version.version,
                &layer_version.content_hash,
            );

            if store_path.exists() {
                utils::create_symlink(&store_path, &original_path)?;
                files_restored += 1;
                println!(
                    "Restored: {} -> {} (layer {})",
                    original_path.display(),
                    store_path.display(),
                    current_layer
                );
            } else {
                eprintln!(
                    "Warning: Store file missing for {}",
                    original_path.display()
                );
            }
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

    // Find files with the tag first and collect them
    let files_with_tag: Vec<(String, TrackedFile)> = config
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

        // Find the layer version with this tag
        if let Some(layer_version) = tracked_file.layer_versions.iter().find(|lv| lv.tag == tag) {
            let store_path = utils::get_store_path(
                &path_hash,
                layer_version.version,
                &layer_version.content_hash,
            );

            if store_path.exists() {
                utils::create_symlink(&store_path, &original_path)?;
                utils::update_file_layer(&mut config, &path_hash, layer_version.layer);
                files_restored += 1;
                println!(
                    "Restored: {} -> {} (tag: '{}', layer {})",
                    original_path.display(),
                    store_path.display(),
                    tag,
                    layer_version.layer
                );
            } else {
                eprintln!(
                    "Warning: Store file missing for {}",
                    original_path.display()
                );
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

    // Clone the data to avoid borrowing issues
    let fossils_data: Vec<(String, TrackedFile)> = config
        .fossils
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    for (path_hash, tracked_file) in fossils_data {
        let original_path = PathBuf::from(&tracked_file.original_path);

        if let Some(layer_version) = utils::find_layer_version(&tracked_file, layer) {
            let store_path = utils::get_store_path(
                &path_hash,
                layer_version.version,
                &layer_version.content_hash,
            );

            if store_path.exists() {
                utils::create_symlink(&store_path, &original_path)?;
                utils::update_file_layer(&mut config, &path_hash, layer);
                files_restored += 1;
                println!(
                    "Restored: {} -> {}",
                    original_path.display(),
                    store_path.display()
                );
            } else {
                eprintln!(
                    "Warning: Store file missing for {}",
                    original_path.display()
                );
            }
        } else {
            // File didn't exist in target layer, so remove if exists
            if original_path.exists() || original_path.is_symlink() {
                fs::remove_file(&original_path)?;
                utils::update_file_layer(&mut config, &path_hash, layer);
                files_removed += 1;
                println!(
                    "Removed: {} (didn't exist in layer {})",
                    original_path.display(),
                    layer
                );
            }
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
            tracked_file.original_path,
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
    surface();

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
