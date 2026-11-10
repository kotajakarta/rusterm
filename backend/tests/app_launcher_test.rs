use std::path::Path;
use xterm_backend::services::app_launcher::build_app_args;

#[test]
fn test_build_app_args() {
    let port = 9095;
    let profile = Path::new("C:\\Users\\Test\\AppData\\Local\\TermDeck\\browser");
    let args = build_app_args(port, profile);

    assert!(args.contains(&"--app=http://127.0.0.1:9095".to_string()));
    assert!(args.contains(&"--no-first-run".to_string()));
    assert!(args.contains(&"--no-default-browser-check".to_string()));
    assert!(args.contains(&"--disable-features=msStartupBoost".to_string()));
    assert!(args.iter().any(|a| a.starts_with("--user-data-dir=")));
}
