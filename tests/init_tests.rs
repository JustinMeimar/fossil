use fossil::fossil;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[test]
#[serial]
fn test_init_success() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    let result = fossil::init();
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    assert!(temp_dir.path().join(".fossil").exists());
    assert!(temp_dir.path().join(".fossil/db").exists());
    Ok(())
}

#[test]
#[serial]
fn test_init_already_exists() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fs::create_dir(".fossil")?;
    let result = fossil::init();
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Repository already initialized");
    Ok(())
}