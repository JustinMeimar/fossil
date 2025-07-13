use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::error;

pub fn file_globs_to_paths(files: Vec<String>)
    -> Result<Vec<PathBuf>, Box<dyn error::Error>>
{
    let paths: Vec<PathBuf> = files
        .iter()
        .map(|f| expand_pattern(&f))
        .flatten()
        .filter(|p| p.exists())
        .collect();
    Ok(paths)
}

pub fn path_to_content(path: &PathBuf)
    -> Result<Vec<u8>, Box<dyn error::Error>>
{
    let content = fs::read(path)?;
    Ok(content)
}

pub fn paths_to_hashes(paths: &Vec<PathBuf>)
    -> Result<Vec<String>, Box<dyn error::Error>> 
{
    let hashes: Vec<String> = paths
        .iter()
        .map(|p| expand_pattern(&p.to_string_lossy()))
        .flatten()
        .map(|p| hash_path(&p))
        .collect();
    Ok(hashes)
}

pub fn hash_path(path: &PathBuf) -> String {
    let normalized = path.canonicalize().unwrap_or_else(|_| path.clone());

    let mut hasher = DefaultHasher::new();
    normalized.to_string_lossy().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

pub fn hash_content(content: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
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

