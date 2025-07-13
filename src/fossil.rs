use crate::config::{FossilDb, Fossil, FossilVersion, find_fossil_config};
use crate::utils;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use chrono::Utc;

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let fossil_dir = std::path::Path::new(".fossil");
    if fossil_dir.exists() {
        return Err("Repository already initialized".into());
    }
    fs::create_dir(fossil_dir)?;
    let db_path = fossil_dir.join("db");
    let _db = sled::open(db_path)?;
    Ok(())
}

pub fn track(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let fossil_dir = find_fossil_config()?;
    let db_path = fossil_dir.join("db");
    let db = FossilDb::new(db_path.to_str().unwrap())?;
    let paths = utils::file_globs_to_paths(files)?;
    for path in paths {
        if db.get_fossil_by_path(&path)?.is_some() {
            continue;
        }
        let base_content = fs::read(&path)?;
        let fossil = Fossil {
            path: path.clone(),
            versions: Vec::new(),
            base_content,
            cur_version: 0,
        };
        db.create_fossil(&fossil)?;
    }
    Ok(())
}

pub fn untrack(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let fossil_dir = find_fossil_config()?;
    let db_path = fossil_dir.join("db");
    let db = FossilDb::new(db_path.to_str().unwrap())?;
    let paths = utils::file_globs_to_paths(files)?;
    for path in paths {
        if let Some(fossil) = db.get_fossil_by_path(&path)? {
            let latest_version = fossil.versions.len();
            let latest_content = fossil.get_version_content(latest_version)?;
            fs::write(&path, latest_content)?;
            let key = fossil.hash()?;
            db.delete_fossil(&key)?;
        }
    }
    Ok(())
}

pub fn bury_files(files: Vec<String>, tag: Option<String>) -> Result<(), Box<dyn Error>> {
    let fossil_dir = find_fossil_config()?;
    let db_path = fossil_dir.join("db");
    let db = FossilDb::new(db_path.to_str().unwrap())?;
    let paths = utils::file_globs_to_paths(files)?;
    for path in paths {
        if let Some(mut fossil) = db.get_fossil_by_path(&path)? {
            let latest_version = fossil.versions.len();
            if fossil.cur_version != latest_version {
                return Err(format!("Cannot bury file {} - not at latest version", path.display()).into());
            }
            fossil.update(tag.clone())?;
            fossil.cur_version = fossil.versions.len();
            db.update_fossil(&fossil)?;
        }
    }
    Ok(())
}


pub fn dig_files(files: Vec<String>, tag: Option<String>, version: Option<usize>)
    -> Result<(), Box<dyn std::error::Error>>
{
    /// TODO: Ensure either version or tag is supplied, but not neither and not both.
    
    let dig_paths = utils::file_globs_to_paths(files)?;
    let dig_hashes = utils::paths_to_hashes(&dig_paths)?;
     
    /// TODO:
    /// - for each path use the hash to find the corresponding fossil.
    /// - if the fossil exists, we should find the layer to restore to
    /// - if the tag is supplied, find the latest version which matches that tag
    /// - if the version is supplied, ensure it is a valid version.
    /// - get the version contents by applying patches from the base content to the chosen
    ///   version then write back that content to the file_path
    Ok(())
}

pub fn surface() -> Result<(), Box<dyn std::error::Error>> {
    let fossil_dir = find_fossil_config()?;
    let db_path = fossil_dir.join("db");
    let db = FossilDb::new(db_path.to_str().unwrap())?;
    let fossils = db.get_all_fossils()?;
    for mut fossil in fossils {
        let latest_version = fossil.versions.len();
        let latest_content = fossil.get_version_content(latest_version)?;
        fs::write(&fossil.path, latest_content)?;
        fossil.cur_version = latest_version;
        db.update_fossil(&fossil)?;
    }
    Ok(())
}

pub fn list() -> Result<(), Box<dyn std::error::Error>> {
    let fossil_dir = find_fossil_config()?;
    let db_path = fossil_dir.join("db");
    let db = FossilDb::new(db_path.to_str().unwrap())?;
    let fossils = db.get_all_fossils()?;
    for fossil in fossils {
        let total_versions = fossil.versions.len();
        let tag_count = fossil.versions.iter().filter(|v| v.tag.is_some()).count();
        let current_content = fossil.get_version_content(fossil.cur_version)?;
        let preview = String::from_utf8_lossy(&current_content);
        let truncated_preview = if preview.len() > 50 {
            format!("{}...", &preview[..50])
        } else {
            preview.to_string()
        };
        println!("{} | v{}/{} | {} tags | {}", 
                 fossil.path.display(), 
                 fossil.cur_version, 
                 total_versions, 
                 tag_count, 
                 truncated_preview.replace('\n', " "));
    }
    Ok(())
}

pub fn reset() -> Result<(), Box<dyn std::error::Error>> {
    
    /// TODO:
    /// - call surface to restore all file versions.
    /// - remove the .fossil and DB etc. 
    Ok(())
}
