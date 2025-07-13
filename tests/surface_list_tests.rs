use fossil::fossil;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[test]
#[serial]
fn test_surface_success() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "initial content")?;
    fossil::track(vec!["test.txt".to_string()])?;
    fs::write(&test_file, "modified content")?;
    fossil::bury_files(vec!["test.txt".to_string()], None)?;
    fs::write(&test_file, "current content")?;
    fossil::bury_files(vec!["test.txt".to_string()], None)?;
    fs::write(&test_file, "temporary change")?;
    let result = fossil::surface();
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    let content = fs::read_to_string(&test_file)?;
    assert_eq!(content, "current content");
    Ok(())
}

#[test]
#[serial]
fn test_surface_empty_repo() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let result = fossil::surface();
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}

#[test]
#[serial]
fn test_list_empty_repo() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(temp_dir.path())?;
    fossil::init()?;
    let result = fossil::list();
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}

#[test]
#[serial]
fn test_list_with_files() -> Result<(), Box<dyn std::error::Error>> {
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
    fossil::bury_files(vec!["test1.txt".to_string()], Some("v1".to_string()))?;
    let result = fossil::list();
    std::env::set_current_dir(original_dir)?;
    assert!(result.is_ok());
    Ok(())
}

#[test]
#[serial]
fn test_surface_multiple_versions() -> Result<(), Box<dyn std::error::Error>> {
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
    fs::write(&test_file, "working")?;
    fossil::surface()?;
    std::env::set_current_dir(original_dir)?;
    let content = fs::read_to_string(&test_file)?;
    assert_eq!(content, "v2");
    Ok(())
}