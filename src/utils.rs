use crate::config::{TrackedFile, LayerVersion};
use std::path::PathBuf;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::fs;
use std::os::unix::fs as unix_fs;

pub fn hash_path(path: &PathBuf) -> String {
   let normalized = path.canonicalize()
       .unwrap_or_else(|_| path.clone());
   
   let mut hasher = DefaultHasher::new();
   normalized.to_string_lossy().hash(&mut hasher);
   format!("{:x}", hasher.finish())
}

pub fn hash_content(content: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

pub fn file_has_changed(file: &PathBuf, tracked_file: &TrackedFile)
    -> Result<bool, Box<dyn std::error::Error>>
{
    let current_content = fs::read(file)?;
    let current_hash = hash_content(&current_content);
    Ok(current_hash != tracked_file.last_content_hash)
}

pub fn find_layer_version(tracked_file: &TrackedFile,
                      target_layer: u32) -> Option<&LayerVersion> {
    tracked_file.layer_versions
        .iter()
        .rev()
        .find(|lv| lv.layer <= target_layer)
}

pub fn get_store_path(path_hash: &str, version: u32, content_hash: &str) -> PathBuf {
    PathBuf::from(".fossil/store")
        .join(path_hash)
        .join(version.to_string())
        .join(content_hash)
}

pub fn restore_file(target: &PathBuf, source_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if target.exists() {
        fs::remove_file(target)?;
    }
    
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    
    fs::copy(source_path, target)?;
    Ok(())
}


pub fn create_symlink(target: &PathBuf, link_path: &PathBuf)
    -> Result<(), Box<dyn std::error::Error>> 
{
    if link_path.exists() {
        fs::remove_file(link_path)?;
    }
    
    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    unix_fs::symlink(target, link_path)?;
    Ok(())
}

pub fn expand_pattern(pattern: &str) -> Vec<PathBuf> {
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

pub fn copy_to_store(file: &PathBuf, path_hash: &str, version: u32,
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
