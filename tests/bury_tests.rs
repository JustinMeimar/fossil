use fossil::fossil;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[test]
#[serial]
fn test_bury_success() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "initial content")?;
    fossil::track(vec!["test.txt".to_string()])?;
    fs::write(&test_file, "modified content")?;
    let result = fossil::bury_files(vec!["test.txt".to_string()], None);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}

#[test]
#[serial]
fn test_bury_with_tag() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "initial content")?;
    fossil::track(vec!["test.txt".to_string()])?;
    fs::write(&test_file, "modified content")?;
    let result = fossil::bury_files(vec!["test.txt".to_string()], Some("v1.0".to_string()));
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}

#[test]
#[serial]
fn test_bury_untracked_file() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "content")?;
    let result = fossil::bury_files(vec!["test.txt".to_string()], None);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}

#[test]
#[serial]
fn test_bury_multiple_files() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file1 = temp_dir.path().join("test1.txt");
    let test_file2 = temp_dir.path().join("test2.txt");
    fs::write(&test_file1, "content1")?;
    fs::write(&test_file2, "content2")?;
    fossil::track(vec!["test1.txt".to_string(), "test2.txt".to_string()])?;
    fs::write(&test_file1, "modified1")?;
    fs::write(&test_file2, "modified2")?;
    let result = fossil::bury_files(vec!["test1.txt".to_string(), "test2.txt".to_string()], None);
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}