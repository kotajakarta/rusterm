use std::path::PathBuf;

pub const DEFAULT_PORT: u16 = 1945;
/// Only this machine can reach the app unless `--host` says otherwise.
pub const DEFAULT_HOST: &str = "127.0.0.1";

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub port: u16,
    /// Address to bind; anything but loopback exposes the app to the network.
    pub host: String,
    pub data_dir: Option<PathBuf>,
    pub dist_dir: Option<PathBuf>,
    pub no_browser: bool,
    /// Fixed session-protection token (dev server); random per start when absent.
    pub token: Option<String>,
}

impl CliArgs {
    pub fn loopback_only(&self) -> bool {
        matches!(self.host.as_str(), "127.0.0.1" | "localhost" | "::1" | "[::1]")
    }
}

/// Parses `std::env::args()`-style arguments (the first item is the program name).
pub fn parse_args(args: &[String]) -> CliArgs {
    let mut cli = CliArgs {
        port: DEFAULT_PORT,
        host: DEFAULT_HOST.to_string(),
        data_dir: None,
        dist_dir: None,
        no_browser: false,
        token: None,
    };
    let mut i = 1;
    while i < args.len() {
        let value = args.get(i + 1);
        let mut consumed = true;
        match (args[i].as_str(), value) {
            ("--port", Some(v)) => cli.port = v.parse().unwrap_or(DEFAULT_PORT),
            ("--host", Some(v)) => cli.host = v.clone(),
            ("--data-dir", Some(v)) => cli.data_dir = Some(PathBuf::from(v)),
            ("--dist-dir", Some(v)) => cli.dist_dir = Some(PathBuf::from(v)),
            ("--token", Some(v)) => cli.token = Some(v.clone()),
            ("--no-browser", _) => {
                cli.no_browser = true;
                consumed = false;
            }
            _ => consumed = false,
        }
        i += if consumed { 2 } else { 1 };
    }
    cli
}
