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
    
    let bury_paths = utils::file_globs_to_paths(files)?;
    let bury_hashes = utils::paths_to_hashes(&bury_paths)?;
    
    /// TODO:
    /// - for each path to bury, use hash to find an existing fossil.
    ///- if a fossil exists, we should call update on it and update the DB with 
    ///  the updated fossil
    /// - then we should sync the DB
    /// - we expect the tag to be applied to the FossilVersion for retrieval later.
    /// - A file can only be burried if it's current version is the max version.
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
    
    /// TODO:
    /// - for all the fossils in the db, restore each to their latest version.
    /// - this means apply the patches from the current version to the latest then
    ///   writing the content to the file at the path.
    Ok(())
}

pub fn list() -> Result<(), Box<dyn std::error::Error>> {
    
    /// TODO:
    /// - For each fossil in the DB, list out the path, the current version, the total
    ///   versions, the number of tags, and a inline preview of the content, trunccated.
    Ok(())
}

pub fn reset() -> Result<(), Box<dyn std::error::Error>> {
    
    /// TODO:
    /// - call surface to restore all file versions.
    /// - remove the .fossil and DB etc. 
    Ok(())
}
