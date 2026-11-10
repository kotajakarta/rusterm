use std::path::{Path, PathBuf};
use xterm_backend::services::app_launcher::{build_app_args, check_existing_instance, resolve_data_dir};

#[test]
fn test_build_app_args() {
    let port = 1945;
    let profile = Path::new("C:\\Users\\Test\\AppData\\Local\\TermDeck\\browser");
    let args = build_app_args(port, profile);

    assert!(args.contains(&"--app=http://127.0.0.1:1945".to_string()));
    assert!(args.contains(&"--no-first-run".to_string()));
    assert!(args.contains(&"--no-default-browser-check".to_string()));
    assert!(args.contains(&"--disable-features=msStartupBoost".to_string()));
    assert!(args.iter().any(|a| a.starts_with("--user-data-dir=")));
}

#[tokio::test]
async fn test_check_existing_instance_non_running_port() {
    let is_running = check_existing_instance(59123).await;
    assert!(!is_running);
}

#[test]
fn test_resolve_data_dir_explicit_cli() {
    let custom = PathBuf::from("/custom/data");
    let exe = PathBuf::from("/bin/termdeck.exe");
    let result = resolve_data_dir(Some(custom.clone()), Some(exe));
    assert_eq!(result, custom);
}

#[test]
fn test_resolve_data_dir_from_exe_path() {
    let exe = PathBuf::from("/opt/termdeck/termdeck.exe");
    let result = resolve_data_dir(None, Some(exe));
    assert_eq!(result, PathBuf::from("/opt/termdeck/conf"));
}

#[test]
fn test_resolve_data_dir_fallback() {
    let result = resolve_data_dir(None, None);
    assert!(result.ends_with("TermDeck") || result.ends_with("xterm-rust") || result == PathBuf::from("."));
}
