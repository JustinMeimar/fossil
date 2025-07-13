use fossil::fossil;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[test]
#[serial]
fn test_dig_by_version() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "v0")?;
    fossil::track(vec!["test.txt".to_string()])?;
    fs::write(&test_file, "v1")?;
    fossil::bury_files(vec!["test.txt".to_string()], None)?;
    fs::write(&test_file, "v2")?;
    fossil::bury_files(vec!["test.txt".to_string()], None)?;
    fs::write(&test_file, "current")?;
    let result = fossil::dig_files(vec!["test.txt".to_string()], None, Some(1));
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    let content = fs::read_to_string(&test_file)?;
    assert_eq!(content, "v1");
    Ok(())
}

#[test]
#[serial]
fn test_dig_by_tag() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "v0")?;
    fossil::track(vec!["test.txt".to_string()])?;
    fs::write(&test_file, "v1")?;
    fossil::bury_files(vec!["test.txt".to_string()], Some("stable".to_string()))?;
    fs::write(&test_file, "v2")?;
    fossil::bury_files(vec!["test.txt".to_string()], None)?;
    fs::write(&test_file, "current")?;
    let result = fossil::dig_files(vec!["test.txt".to_string()], Some("stable".to_string()), None);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    let content = fs::read_to_string(&test_file)?;
    assert_eq!(content, "v1");
    Ok(())
}

#[test]
#[serial]
fn test_dig_invalid_version() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "v0")?;
    fossil::track(vec!["test.txt".to_string()])?;
    let result = fossil::dig_files(vec!["test.txt".to_string()], None, Some(5));
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Version 5 does not exist"));
    Ok(())
}

#[test]
#[serial]
fn test_dig_invalid_tag() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "v0")?;
    fossil::track(vec!["test.txt".to_string()])?;
    let result = fossil::dig_files(vec!["test.txt".to_string()], Some("nonexistent".to_string()), None);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Tag 'nonexistent' not found"));
    Ok(())
}

#[test]
#[serial]
fn test_dig_both_tag_and_version() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "v0")?;
    fossil::track(vec!["test.txt".to_string()])?;
    let result = fossil::dig_files(vec!["test.txt".to_string()], Some("tag".to_string()), Some(1));
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Cannot specify both tag and version"));
    Ok(())
}

#[test]
#[serial]
fn test_dig_neither_tag_nor_version() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "v0")?;
    fossil::track(vec!["test.txt".to_string()])?;
    let result = fossil::dig_files(vec!["test.txt".to_string()], None, None);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Must specify either tag or version"));
    Ok(())
}

#[test]
#[serial]
fn test_dig_untracked_file() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "content")?;
    let result = fossil::dig_files(vec!["test.txt".to_string()], None, Some(1));
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}

#[test]
#[serial]
fn test_dig_to_base_version() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "base")?;
    fossil::track(vec!["test.txt".to_string()])?;
    fs::write(&test_file, "modified")?;
    fossil::bury_files(vec!["test.txt".to_string()], None)?;
    let result = fossil::dig_files(vec!["test.txt".to_string()], None, Some(0));
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    let content = fs::read_to_string(&test_file)?;
    assert_eq!(content, "base");
    Ok(())
}