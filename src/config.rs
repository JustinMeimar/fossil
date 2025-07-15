use sled::Db;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use crate::utils;
use diffy::{create_patch_bytes, apply_bytes, Patch};

pub struct FossilDb {
    db: Db,
}

impl FossilDb {
    pub fn new(path: &PathBuf) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        Ok(FossilDb { db })
    }

    pub fn open_default() -> Result<Self, Box<dyn std::error::Error>> {
        let fossil_dir = find_fossil_config()?;
        let db_path = fossil_dir.join("db");
        if !db_path.exists() {
            return Err("Couldn't find fossil database in expected location.".into());
        }
        FossilDb::new(&db_path).map_err(|e| e.into())
    }
    
    pub fn create_fossil(&self, fossil: &Fossil) -> Result<(), Box<dyn std::error::Error>> {
        let key = fossil.hash()?;
        let value = serde_json::to_vec(fossil)?;  
        self.db.insert(key.as_bytes(), value)?;
        Ok(())
    }
    
    pub fn get_fossil_by_path(&self, path: &PathBuf)
        -> Result<Option<Fossil>, Box<dyn std::error::Error>>
    {
        let path_hash = utils::hash_path(path);
        let prefix = format!("{}:", path_hash);
        
        for item in self.db.scan_prefix(prefix.as_bytes()) {
            let (_, value) = item?;
            let fossil: Fossil = serde_json::from_slice(&value)?;
            return Ok(Some(fossil));
        }
        
        Ok(None)
    }

    pub fn get_fossil(&self, key: &str) -> Result<Option<Fossil>, Box<dyn std::error::Error>> {
        if let Some(bytes) = self.db.get(key.as_bytes())? {
            let fossil: Fossil = serde_json::from_slice(&bytes)?;
            Ok(Some(fossil))
        } else {
            Ok(None)
        }
    }
    
    pub fn update_fossil(&self, fossil: &Fossil) -> Result<(), Box<dyn std::error::Error>> {
        self.create_fossil(fossil)
    }
    
    pub fn delete_fossil(&self, key: &str) -> Result<(), sled::Error> {
        self.db.remove(key.as_bytes())?;
        Ok(())
    }
    
    pub fn get_all_fossils(&self) -> Result<Vec<Fossil>, Box<dyn std::error::Error>> {
        let mut fossils = Vec::new();
        for item in self.db.iter() {
            let (_, value) = item?;
            let fossil: Fossil = serde_json::from_slice(&value)?;
            fossils.push(fossil);
        }
        Ok(fossils)
    }
}

/// A fossil tracks a file through several versions.
#[derive(Deserialize, Serialize)]
pub struct Fossil {
    pub path: PathBuf,
    pub versions: Vec<FossilVersion>,
    pub base_content: Vec<u8>, 
    pub cur_version: usize,
}

impl Fossil {
    /// Compute a unique ID for the database to store this fossil.
    pub fn hash(&self) -> Result<String, Box<dyn std::error::Error>> {
        let path_hash = utils::hash_path(&self.path);
        let content_hash = utils::hash_content(&self.base_content);
        let hash = format!("{}:{}", path_hash, content_hash);
        Ok(hash)
    }
    
    /// Retrieve the content of a specific version.
    pub fn get_version_content(&self, version_no: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if version_no > self.versions.len() {
            return Err("Requested version greater than exists for fossil.".into());
        }
        
        let mut content = self.base_content.clone();
        
        // Apply patches sequentially up to the target version
        for version in &self.versions[..version_no] {
            let patch = Patch::<[u8]>::from_bytes(&version.patch_bytes)?;
            content = apply_bytes(&content, &patch)?;
        }
        
        Ok(content)
    }
    
    pub fn update(&mut self, tag: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
        let last_version_no = self.versions.len();
        let last_content = self.get_version_content(last_version_no)?;
        let current_content = fs::read(&self.path)?;
        
        if last_content == current_content {
            return Ok(()); // No changes
        }
        
        // Use create_patch_bytes for binary data
        let patch = create_patch_bytes(&last_content, &current_content);
        
        self.versions.push(FossilVersion {
            version_no: (last_version_no + 1) as u32,
            patch_bytes: patch.to_bytes(),
            tag,
        });
        
        Ok(())
    }
    
    pub fn resolve_version(&self, tag: Option<String>, version: Option<usize>)
        -> Result<usize, Box<dyn std::error::Error>>
    {
        match (tag, version) {
            (Some(_), Some(_)) => Err("Cannot specify both tag and version".into()),
            (None, None) => Err("Must specify either tag or version".into()),
            (None, Some(v)) => {
                if v > self.versions.len() {
                    Err(format!("Version {} does not exist (max: {})", v, self.versions.len()).into())
                } else {
                    Ok(v)
                }
            },
            (Some(tag), None) => {
                for (i, fossil_version) in self.versions.iter().enumerate().rev() {
                    if fossil_version.tag.as_ref() == Some(&tag) {
                        return Ok(i + 1);
                    }
                }
                Err(format!("Tag '{}' not found", tag).into())
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct FossilVersion {
    pub version_no: u32, 
    pub patch_bytes: Vec<u8>,
    pub tag: Option<String>,
}

pub fn find_fossil_config() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut current_dir = std::env::current_dir()?;
    loop {
        let fossil_dir = current_dir.join(".fossil");
        if fossil_dir.exists() {
            return Ok(fossil_dir);
        }
        if current_dir.join(".git").exists() {
            break;
        }
        match current_dir.parent() {
            Some(parent) => current_dir = parent.to_path_buf(),
            None => break,
        }
    }
    Err("No .fossil directory found".into())
}

