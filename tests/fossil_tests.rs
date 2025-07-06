use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use fossil::fossil;
use serial_test::serial;

fn setup_test_dir() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();
    temp_dir
}

#[test]
#[serial]
fn test_init() {
    let _temp_dir = setup_test_dir();
    
    assert!(fossil::init().is_ok());
    assert!(PathBuf::from(".fossil").exists());
    assert!(PathBuf::from(".fossil/store").exists());
}

#[test]
#[serial]
fn test_init_already_exists() {
    let _temp_dir = setup_test_dir();
    
    fossil::init().unwrap();
    let result = fossil::init();
    
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_track_file() {
    let _temp_dir = setup_test_dir();
    
    fossil::init().unwrap();
    
    fs::write("test.txt", "test content").unwrap();
    
    assert!(fossil::track(vec!["test.txt".to_string()]).is_ok());
}

#[test]
#[serial]
fn test_track_nonexistent_file() {
    let _temp_dir = setup_test_dir();
    
    fossil::init().unwrap();
    
    assert!(fossil::track(vec!["nonexistent.txt".to_string()]).is_ok());
}

#[test]
#[serial]
fn test_bury() {
    let _temp_dir = setup_test_dir();
    
    fossil::init().unwrap();
    
    fs::write("test.txt", "test content").unwrap();
    fossil::track(vec!["test.txt".to_string()]).unwrap();
    
    assert!(fossil::burry(None, None).is_ok());
}

#[test]
#[serial]
fn test_list() {
    let _temp_dir = setup_test_dir();
    
    fossil::init().unwrap();
    
    assert!(fossil::list().is_ok());
}

#[test]
#[serial]
fn test_surface() {
    let _temp_dir = setup_test_dir();
    
    fossil::init().unwrap();
    
    fs::write("test.txt", "test content").unwrap();
    fossil::track(vec!["test.txt".to_string()]).unwrap();
    
    // Remove the file to test surface restoration
    fs::remove_file("test.txt").unwrap();
    
    assert!(fossil::surface().is_ok());
}

#[test]
#[serial]
fn test_dig() {
    let _temp_dir = setup_test_dir();
    
    fossil::init().unwrap();
    
    fs::write("test.txt", "test content").unwrap();
    fossil::track(vec!["test.txt".to_string()]).unwrap();
    fossil::bury(None, None).unwrap();
    
    assert!(fossil::dig(0).is_ok());
}
