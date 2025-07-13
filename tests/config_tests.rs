
use fossil::config::{FossilDb, Fossil, FossilVersion};

use serial_test::serial;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_fossil_version_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test_db");
    let file_path = temp_dir.path().join("test_file.txt");

    let db = FossilDb::new(db_path.to_str().unwrap())?;

    // Create initial file content
    fs::write(&file_path, "initial content")?;

    // Create fossil with base content
    let mut fossil = Fossil {
       path: file_path.clone(),
       versions: Vec::new(),
       base_content: fs::read(&file_path)?,
       cur_version: 0,
    };

    // Store initial fossil
    db.create_fossil(&fossil)?;

    // Update file and create version 1
    fs::write(&file_path, "modified content v1")?;
    fossil.update(None)?;
    db.update_fossil(&fossil)?;

    // Update file and create version 2  
    fs::write(&file_path, "modified content v2")?;
    fossil.update(None)?;
    db.update_fossil(&fossil)?;

    // Retrieve fossil from database
    let key = fossil.hash()?;
    let retrieved_fossil = db.get_fossil(&key)?.unwrap();

    // Test version content retrieval
    let v0_content = retrieved_fossil.get_version_content(0)?;
    assert_eq!(v0_content, b"initial content");

    let v1_content = retrieved_fossil.get_version_content(1)?;
    assert_eq!(v1_content, b"modified content v1");

    let v2_content = retrieved_fossil.get_version_content(2)?;
    assert_eq!(v2_content, b"modified content v2");

    // Test that we have 2 versions (patches)
    assert_eq!(retrieved_fossil.versions.len(), 2);

    Ok(())
}
