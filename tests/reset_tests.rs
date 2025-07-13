use fossil::fossil;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[test]
#[serial]
fn test_reset_success() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "initial")?;
    fossil::track(vec!["test.txt".to_string()])?;
    fs::write(&test_file, "modified")?;
    fossil::bury_files(vec!["test.txt".to_string()], None)?;
    fs::write(&test_file, "working")?;
    let result = fossil::reset();
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    assert!(!temp_dir.path().join(".fossil").exists());
    let content = fs::read_to_string(&test_file)?;
    assert_eq!(content, "modified");
    Ok(())
}

#[test]
#[serial]
fn test_reset_empty_repo() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let result = fossil::reset();
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    assert!(!temp_dir.path().join(".fossil").exists());
    Ok(())
}

#[test]
#[serial]
fn test_reset_multiple_files() -> Result<(), Box<dyn std::error::Error>> {
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
    fossil::bury_files(vec!["test1.txt".to_string(), "test2.txt".to_string()], None)?;
    fs::write(&test_file1, "working1")?;
    fs::write(&test_file2, "working2")?;
    let result = fossil::reset();
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    assert!(!temp_dir.path().join(".fossil").exists());
    let content1 = fs::read_to_string(&test_file1)?;
    let content2 = fs::read_to_string(&test_file2)?;
    assert_eq!(content1, "modified1");
    assert_eq!(content2, "modified2");
    Ok(())
}

#[test]
#[serial]
fn test_reset_no_repo() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    let result = fossil::reset();
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_err());
    Ok(())
}