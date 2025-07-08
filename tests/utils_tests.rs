use fossil::utils;
use serial_test::serial;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_test_dir() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();
    temp_dir
}

#[test]
fn test_hash_content() {
    let content = b"test content";
    let hash = utils::hash_content(content);

    assert!(!hash.is_empty());
    assert_eq!(hash.len(), 16);
}

#[test]
fn test_hash_path() {
    let path = PathBuf::from("test.txt");
    let hash = utils::hash_path(&path);

    assert!(!hash.is_empty());
}

#[test]
#[serial]
fn test_expand_pattern() {
    let _temp_dir = setup_test_dir();

    fs::write("test1.txt", "content1").unwrap();
    fs::write("test2.txt", "content2").unwrap();

    let paths = utils::expand_pattern("*.txt");

    assert!(paths.len() >= 2);
}

#[test]
fn test_get_store_path() {
    let path_hash = "abcd1234";
    let version = 1;
    let content_hash = "efgh5678";

    let store_path = utils::get_store_path(path_hash, version, content_hash);

    assert!(store_path.to_string_lossy().contains("abcd1234"));
    assert!(store_path.to_string_lossy().contains("efgh5678"));
}
