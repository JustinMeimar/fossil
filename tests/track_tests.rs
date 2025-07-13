use fossil::fossil;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[test]
#[serial]
fn test_track_success() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "test content")?;
    let result = fossil::track(vec!["test.txt".to_string()]);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    assert!(temp_dir.path().join(".fossil/db").exists());
    Ok(())
}

#[test]
#[serial]
fn test_track_nonexistent_file() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let result = fossil::track(vec!["nonexistent.txt".to_string()]);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}

#[test]
#[serial]
fn test_track_already_tracked() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "test content")?;
    fossil::track(vec!["test.txt".to_string()])?;
    let result = fossil::track(vec!["test.txt".to_string()]);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}

#[test]
#[serial]
fn test_untrack_success() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "test content")?;
    fossil::track(vec!["test.txt".to_string()])?;
    let result = fossil::untrack(vec!["test.txt".to_string()]);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}

#[test]
#[serial]
fn test_untrack_not_tracked() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "test content")?;
    let result = fossil::untrack(vec!["test.txt".to_string()]);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}