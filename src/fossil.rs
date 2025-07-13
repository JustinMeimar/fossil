// use crate::config::{Config, Fossil, FossilVersion, save_config, load_config};
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
   
    let paths = utils::file_globs_to_paths(files)?;
    let hashes = utils::paths_to_hashes(&paths)?;

    /// TODO:
    /// - check if the path exists in the DB 
    /// - if it doesn't exist, then we should create a fossil and add that fossil.
    /// - the fossil has an inital version of just 1 and base content set to fs::read of path
    /// - then we should flush the DB so it is synced
    Ok(())
}

pub fn untrack(files: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    
    let untrack_paths = utils::file_globs_to_paths(files)?;
    let untrack_hashes = utils::paths_to_hashes(&untrack_paths)?;
    
    /// TODO:
    /// - for each path hash, check if there is an entry in the DB
    /// - if there is an entry, we should restore that file to it's latest verison.
    ///   for example, it may have 4 versions and we are checkedout to v2, we should
    ///   write back the latest version and untrack the file.
    /// - remove the corresponding fossils from the DB and flush.
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
