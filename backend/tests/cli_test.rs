use std::path::PathBuf;
use xterm_backend::cli::parse_args;

fn args(list: &[&str]) -> Vec<String> {
    std::iter::once("rusterm").chain(list.iter().copied()).map(String::from).collect()
}

#[test]
fn defaults_bind_loopback_only() {
    let cli = parse_args(&args(&[]));
    assert_eq!(cli.port, 1945);
    assert_eq!(cli.host, "127.0.0.1");
    assert!(cli.data_dir.is_none() && cli.dist_dir.is_none() && cli.token.is_none());
    assert!(!cli.no_browser);
    assert!(cli.loopback_only());
}

#[test]
fn all_flags_are_parsed() {
    let cli = parse_args(&args(&[
        "--port", "2000", "--host", "0.0.0.0", "--data-dir", "/d", "--dist-dir", "/w",
        "--no-browser", "--token", "dev",
    ]));
    assert_eq!(cli.port, 2000);
    assert_eq!(cli.host, "0.0.0.0");
    assert_eq!(cli.data_dir, Some(PathBuf::from("/d")));
    assert_eq!(cli.dist_dir, Some(PathBuf::from("/w")));
    assert!(cli.no_browser);
    assert_eq!(cli.token.as_deref(), Some("dev"));
    assert!(!cli.loopback_only());
    assert!(parse_args(&args(&["--host", "localhost"])).loopback_only());
    assert!(parse_args(&args(&["--host", "::1"])).loopback_only());
}

#[test]
fn bad_port_falls_back_to_default() {
    assert_eq!(parse_args(&args(&["--port", "abc"])).port, 1945);
}
