use fossil::fossil;
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

    assert!(fossil::bury_files(vec![], String::from("")).is_ok());
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
    fossil::bury_files(vec![], String::from("")).unwrap();

    assert!(fossil::dig_by_layer(0).is_ok());
}

#[test]
#[serial]
fn test_complex_tag_workflow() {
    let _temp_dir = setup_test_dir();

    fossil::init().unwrap();

    fs::write("r1.txt", "a").unwrap();
    fs::write("r2.txt", "a").unwrap();

    fossil::track(vec!["r1.txt".to_string()]).unwrap();
    fossil::track(vec!["r2.txt".to_string()]).unwrap();

    fs::write("r1.txt", "ab").unwrap();
    fs::write("r2.txt", "ab").unwrap();

    fossil::bury_files(vec!["r1.txt".to_string()], "foo".to_string()).unwrap();
    fossil::bury_files(vec!["r2.txt".to_string()], "bar".to_string()).unwrap();

    fs::write("r1.txt", "abc").unwrap();
    fs::write("r2.txt", "abc").unwrap();
    fossil::bury_files(vec![], "".to_string()).unwrap();

    assert_eq!(fs::read_to_string("r1.txt").unwrap(), "abc");
    assert_eq!(fs::read_to_string("r2.txt").unwrap(), "abc");

    fossil::dig_by_tag("foo").unwrap();
    assert!(PathBuf::from("r1.txt").exists());

    fossil::dig_by_tag("bar").unwrap();
    assert!(PathBuf::from("r2.txt").exists());
}

#[test]
#[serial]
fn test_tag_persistence_across_layers() {
    let _temp_dir = setup_test_dir();

    fossil::init().unwrap();
    fs::write("test.txt", "version1").unwrap();
    fossil::track(vec!["test.txt".to_string()]).unwrap();

    fs::write("test.txt", "version2").unwrap();
    fossil::bury_files(vec!["test.txt".to_string()], "stable".to_string()).unwrap();

    fs::write("test.txt", "version3").unwrap();
    fossil::bury_files(vec!["test.txt".to_string()], "".to_string()).unwrap();

    fs::write("test.txt", "version4").unwrap();
    fossil::bury_files(vec!["test.txt".to_string()], "latest".to_string()).unwrap();

    fossil::dig_by_tag("stable").unwrap();
    assert!(PathBuf::from("test.txt").exists());
}

#[test]
#[serial]
fn test_bury_all_files_workflow() {
    let _temp_dir = setup_test_dir();

    fossil::init().unwrap();
    fs::write("file1.txt", "content1").unwrap();
    fs::write("file2.txt", "content2").unwrap();
    fs::write("file3.txt", "content3").unwrap();

    fossil::track(vec!["file1.txt".to_string(), "file2.txt".to_string(), "file3.txt".to_string()]).unwrap();

    fs::write("file1.txt", "modified1").unwrap();
    fs::write("file2.txt", "modified2").unwrap();
    fs::write("file3.txt", "modified3").unwrap();

    fossil::bury_files(vec![], "all_files".to_string()).unwrap();

    assert!(PathBuf::from("file1.txt").exists());
    assert!(PathBuf::from("file2.txt").exists());
    assert!(PathBuf::from("file3.txt").exists());
}

#[test]
#[serial]
fn test_dig_by_layer_workflow() {
    let _temp_dir = setup_test_dir();

    fossil::init().unwrap();
    fs::write("test.txt", "layer0").unwrap();
    fossil::track(vec!["test.txt".to_string()]).unwrap();

    fs::write("test.txt", "layer1").unwrap();
    fossil::bury_files(vec!["test.txt".to_string()], "".to_string()).unwrap();

    fs::write("test.txt", "layer2").unwrap();
    fossil::bury_files(vec!["test.txt".to_string()], "".to_string()).unwrap();

    fossil::dig_by_layer(1).unwrap();
    assert!(PathBuf::from("test.txt").exists());

    fossil::dig_by_layer(0).unwrap();
    assert!(PathBuf::from("test.txt").exists());
}

#[test]
#[serial]
fn test_multiple_files_different_tags() {
    let _temp_dir = setup_test_dir();

    fossil::init().unwrap();
    fs::write("alpha.txt", "alpha_v1").unwrap();
    fs::write("beta.txt", "beta_v1").unwrap();
    fs::write("gamma.txt", "gamma_v1").unwrap();

    fossil::track(vec!["alpha.txt".to_string(), "beta.txt".to_string(), "gamma.txt".to_string()]).unwrap();

    fs::write("alpha.txt", "alpha_v2").unwrap();
    fossil::bury_files(vec!["alpha.txt".to_string()], "alpha_tag".to_string()).unwrap();

    fs::write("beta.txt", "beta_v2").unwrap();
    fossil::bury_files(vec!["beta.txt".to_string()], "beta_tag".to_string()).unwrap();

    fs::write("gamma.txt", "gamma_v2").unwrap();
    fossil::bury_files(vec!["gamma.txt".to_string()], "gamma_tag".to_string()).unwrap();

    fs::write("alpha.txt", "alpha_v3").unwrap();
    fs::write("beta.txt", "beta_v3").unwrap();
    fs::write("gamma.txt", "gamma_v3").unwrap();
    fossil::bury_files(vec![], "".to_string()).unwrap();

    fossil::dig_by_tag("alpha_tag").unwrap();
    fossil::dig_by_tag("beta_tag").unwrap();
    fossil::dig_by_tag("gamma_tag").unwrap();

    assert!(PathBuf::from("alpha.txt").exists());
    assert!(PathBuf::from("beta.txt").exists());
    assert!(PathBuf::from("gamma.txt").exists());
}

#[test]
#[serial]
fn test_simple_tag_workflow() {
    let _temp_dir = setup_test_dir();

    fossil::init().unwrap();
    fs::write("test.txt", "initial").unwrap();
    fossil::track(vec!["test.txt".to_string()]).unwrap();

    fs::write("test.txt", "tagged_version").unwrap();
    fossil::bury_files(vec!["test.txt".to_string()], "my_tag".to_string()).unwrap();

    let result = fossil::dig_by_tag("my_tag");
    assert!(result.is_ok());
}
