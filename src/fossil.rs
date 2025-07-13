// use crate::config::{Config, Fossil, FossilVersion, save_config, load_config};
use crate::utils;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use chrono::Utc;

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    // let fossil_dir = PathBuf::from(".fossil");
    // let store_dir = fossil_dir.join("store");
    //
    // if fossil_dir.exists() {
    //     return Err(std::io::Error::new(
    //         std::io::ErrorKind::AlreadyExists,
    //         "Fossil repository already exists",
    //     )
    //     .into());
    // }
    //
    // fs::create_dir_all(&store_dir)?;
    //
    // let empty_config = Config {
    //     fossils: Vec::new(),
    //     created: Utc::now(),
    // };
    // save_config(&empty_config)?;
    Ok(())
}

pub fn track(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    // let mut config = load_config()?;
    // let paths: Vec<PathBuf> = utils::file_globs_to_paths(files)?;
    // 
    // // Iterate over each path and create a new fossil record if it is untracked.
    // for path in paths {
    //
    //     // let content = fs::read(&path)?;        
    //     // let path_hash = utils::hash_path(&path);
    //     // let content_hash = utils::hash_content(&content);
    //     //
    //     // if config.fossils.contains(&path_hash) {
    //     //     println!("Fossil is already tracked...");
    //     //     continue;
    //     // }
    //     //
    //     // // Create the fossil record and add it to the config.
    //     // let fossil = Fossil::new(&path, &content);
    //     // config.add_fossil_record(&fossil)?; 
    //     println!("Tracked: {} (version 1)", path.display());
    // }
    //
    // save_config(&config)?;
    Ok(())
}

pub fn untrack(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    // let mut config = load_config()?;
    // let untrack_paths = utils::file_globs_to_paths(files)?;
    // let untrack_hashes = utils::paths_to_hashes(&untrack_paths)?;
    //
    // for p in untrack_hashes {
    //     match config.fossils.remove(&p) {
    //         Some(_) => println!("Untracking: {}", p),
    //         None => eprintln!("Failed to untrack: {}", p),
    //     }
    // }
    //
    // save_config(&config)?;
    Ok(())
}

pub fn bury_files(files: Vec<String>, tag: String) -> Result<(), Box<dyn Error>> {
    // let mut config = load_config()?; 
    //
    // let bury_paths = if files.is_empty() {
    //     config.get_all_file_paths() 
    // } else {
    //     utils::file_globs_to_paths(files)
    // }?;
    // 
    // for path in bury_paths { 
    //     let hash = utils::hash_path(&path);
    //     if config.fossils.contains_key(&hash) {
    //         let fossil = config.fossils.get_mut(&hash).unwrap(); 
    //         fossil.update(tag.clone());
    //         println!("Burying fossil: {}", fossil.file_path.display());
    //     } else {
    //         eprintln!("Error: Hash of path {} not found in store.", hash);
    //     }
    // }
    //
    // save_config(&config)?;
    Ok(())
}

fn dig_file_version(path: &PathBuf, layer: u32) -> Result<(), Box<dyn std::error::Error>> {
    // let config = load_config()?;
    // let path_hash = utils::hash_path(path);
    // let fossil_file = config.fossils.get(&path_hash).ok_or("File not tracked")?;
    //
    // if let Some(layer_version) = utils::find_layer_version(&fossil_file, layer) {
    //     let store_path = utils::get_store_path(
    //         &path_hash,
    //         layer_version.version,
    //         &layer_version.content_hash,
    //     );
    //
    //     if !store_path.exists() {
    //         eprintln!("Warning: Store file missing for {}", path.display());
    //         return Ok(());
    //     }
    //     utils::create_symlink(&store_path, &path)?;
    //     println!(
    //         "Restored: {} -> {} (layer {})",
    //         path.display(),
    //         store_path.display(),
    //         layer
    //     );
    // } else {
    //     // File didn't exist in target layer, so remove if exists
    //     if path.exists() || path.is_symlink() {
    //         fs::remove_file(&path)?;
    //         println!(
    //             "Removed: {} (didn't exist in layer {})",
    //             path.display(),
    //             layer
    //         );
    //     }
    // }
    Ok(())
}

pub fn dig_files(files: Vec<String>, tag: String) -> Result<(), Box<dyn std::error::Error>> {
    // let config = load_config()?;
    //
    // if config.fossils.is_empty() {
    //     println!("No fossils to dig. Use 'fossil track <files>' to track files.");
    //     return Ok(());
    // }
    //
    // // Find files by paths and collect them
    // let mut paths_to_dig = Vec::new();
    // for path in files {
    //     let path_buf = PathBuf::from(path);
    //     if config.fossils.contains_key(&utils::hash_path(&path_buf)) {
    //         paths_to_dig.push(path_buf);
    //     }
    // }
    //
    // // Should have specified at least one file
    // if paths_to_dig.is_empty() {
    //     println!("No tracked files found matching the specified paths.");
    //     return Ok(());
    // }
    //
    // // Try to restore each file to the specified layer
    // let mut files_restored = 0;
    // for fossil_path in paths_to_dig {
    //     match dig_file_version(&fossil_path, layer) {
    //         Ok(_) => files_restored += 1,
    //         Err(e) => eprintln!("Failed to dig {} with error: {}", fossil_path.display(), e),
    //     }
    // }
    //
    // println!("Dug {} files", files_restored);
    Ok(())
}

pub fn dig_by_tag(tag: &str) -> Result<(), Box<dyn std::error::Error>> {
    // let mut config = load_config()?;
    //
    // if config.fossils.is_empty() {
    //     println!("No fossils to dig. Use 'fossil track <files>' to start tracking files.");
    //     return Ok(());
    // }
    //
    // let files_with_tag: Vec<(String, Fossil)> = config
    //     .fossils
    //     .iter()
    //     .filter(|(_, tracked_file)| tracked_file.layer_versions.iter().any(|lv| lv.tag == tag))
    //     .map(|(path_hash, tracked_file)| (path_hash.clone(), tracked_file.clone()))
    //     .collect();
    //
    // if files_with_tag.is_empty() {
    //     println!("No tracked files found with tag '{}'.", tag);
    //     return Ok(());
    // }
    //
    // let mut files_restored = 0;
    //
    // for (path_hash, tracked_file) in files_with_tag {
    //     let file_path = PathBuf::from(&tracked_file.file_path);
    //
    //     if let Some(layer_version) = tracked_file.layer_versions.iter().find(|lv| lv.tag == tag) {
    //         match dig_file_version(&file_path, layer_version.layer) {
    //             Ok(_) => {
    //                 utils::update_file_layer(&mut config, &path_hash, layer_version.layer);
    //                 files_restored += 1;
    //             }
    //             Err(e) => eprintln!(
    //                 "Failed to dig {} with error: {}",
    //                 file_path.display(),
    //                 e
    //             ),
    //         }
    //     }
    // }
    //
    // save_config(&config)?;
    // println!("Dug {} files with tag '{}'", files_restored, tag);
    Ok(())
}

pub fn dig_by_layer(layer: u32) -> Result<(), Box<dyn std::error::Error>> {
//     let mut config = load_config()?;
//
//     if config.fossils.is_empty() {
//         println!("No fossils to dig. Use 'fossil track <files>' to start tracking files.");
//         return Ok(());
//     }
//
//     if layer > config.surface_layer {
//         println!("Can not dig to a layer above the surface.");
//         return Ok(());
//     }
//
//     let mut files_restored = 0;
//     let mut files_removed = 0;
//
//     let fossils_data: Vec<(String, Fossil)> = config
//         .fossils
//         .iter()
//         .map(|(k, v)| (k.clone(), v.clone()))
//         .collect();
//
//     for (path_hash, tracked_file) in fossils_data {
//         let file_path = PathBuf::from(&tracked_file.file_path);
//
//         match dig_file_version(&file_path, layer) {
//             Ok(_) => {
//                 utils::update_file_layer(&mut config, &path_hash, layer);
//                 if utils::find_layer_version(&tracked_file, layer).is_some() {
//                     files_restored += 1;
//                 } else {
//                     files_removed += 1;
//                 }
//             }
//             Err(e) => eprintln!(
//                 "Failed to dig {} with error: {}",
//                 file_path.display(),
//                 e
//             ),
//         }
//     }
//
//     config.current_layer = layer;
//     save_config(&config)?;
//
//     println!(
//         "Excavated to layer {} ({} files restored, {} files removed)",
//         layer, files_restored, files_removed
//     );

    Ok(())
}

pub fn surface() -> Result<(), Box<dyn std::error::Error>> {
//     let mut config = load_config()?;
//
//     for (path_hash, tracked_file) in &config.fossils {
//         if let Some(layer_version) = utils::find_layer_version(tracked_file, config.surface_layer) {
//             // Restore the file back directly, not as a symlink.
//             let store_path = utils::get_store_path(
//                 path_hash,
//                 layer_version.version,
//                 &layer_version.content_hash,
//             );
//             let file_path = PathBuf::from(&tracked_file.file_path);
//
//             // Todo: Handle error propogation better.
//             match utils::restore_file(&file_path, &store_path) {
//                 Ok(_) => println!("Restored fossil: {} to surface", file_path.display()),
//                 Err(e) => eprintln!("Failed to restore fossil to surface.. {}", e),
//             }
//         }
//     }
//
//     config.current_layer = config.surface_layer;
//     save_config(&config)?;
    Ok(())
}

pub fn list() -> Result<(), Box<dyn std::error::Error>> {
    // let config = load_config()?;
    //
    // println!("Current layer: {}", config.current_layer);
    // println!();
    //
    // if config.fossils.is_empty() {
    //     println!("No fossils found. Use 'fossil track <files>' to start tracking files.");
    //     return Ok(());
    // }
    //
    // // Collect all layers and their timestamps
    // let mut all_layers: BTreeSet<u32> = BTreeSet::new();
    // for tracked_file in config.fossils.values() {
    //     for layer_version in &tracked_file.layer_versions {
    //         all_layers.insert(layer_version.layer);
    //     }
    // }
    //
    // if !all_layers.is_empty() {
    //     println!("Available layers:");
    //     for layer in all_layers.iter().rev() {
    //         let current_marker = if *layer == config.current_layer {
    //             " (current)"
    //         } else {
    //             ""
    //         };
    //         println!("  Layer {}{}", layer, current_marker);
    //     }
    //     println!();
    // }
    //
    // // Print out all the fossils we have a record of.
    // println!("Tracked fossils:");
    // println!(
    //     "{:<16} {:<40} {:<8} {:<8} {:<20}",
    //     "Hash", "Path", "Versions", "Layers", "Last Tracked"
    // );
    // println!("{}", "=".repeat(100));
    //
    // for (hash, tracked_file) in &config.fossils {
    //     println!(
    //         "{:<16} {:<40} {:<8} {:<8} {:<20}",
    //         &hash[..8.min(hash.len())],
    //         tracked_file.file_path.display(),
    //         tracked_file.versions,
    //         tracked_file.layer_versions.len(),
    //         tracked_file.last_tracked.format("%Y-%m-%d %H:%M:%S")
    //     );
    // }
    Ok(())
}

pub fn reset() -> Result<(), Box<dyn std::error::Error>> {
    // let fossil_dir = find_fossil_config()?;
    // let store_dir = fossil_dir.join("store");
    //
    // // Restore symlinks with their original files before clearing.
    // surface()?;
    //
    // if store_dir.exists() {
    //     fs::remove_dir_all(&store_dir)?;
    //     fs::create_dir_all(&store_dir)?;
    // }
    //
    // let empty_config = Config {
    //     fossils: HashMap::new(),
    //     current_layer: 0,
    //     surface_layer: 0,
    //     file_current_layers: HashMap::new(),
    // };
    // save_config(&empty_config)?;

    Ok(())
}
