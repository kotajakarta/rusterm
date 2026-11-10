use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerOutput {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub auth_type: String,
    pub group_name: String,
    pub color: String,
    pub has_password: bool,
    pub has_private_key: bool,
    pub host_key_fingerprint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerInput {
    pub name: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub auth_type: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub passphrase: Option<String>,
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub color: String,
}

#[derive(Clone, Debug)]
pub struct DecryptedServerCreds {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ServerStats {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub uptime_seconds: Option<f64>,
    pub load: Vec<f64>,
    pub cpu_count: Option<u32>,
    pub mem_total: Option<u64>,
    pub mem_used: Option<u64>,
    pub disk_total: Option<u64>,
    pub disk_used: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnectionCheck {
    pub ok: bool,
    pub message: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TestServerInput {
    pub id: Option<i64>,
    #[serde(default)]
    pub name: String,
    pub host: String,
    #[serde(default = "default_test_port")]
    pub port: i64,
    pub username: String,
    pub auth_type: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub passphrase: Option<String>,
}

fn default_test_port() -> i64 {
    22
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupBookmark {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub startup_cmd: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupServer {
    pub name: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub auth_type: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub passphrase: Option<String>,
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub bookmarks: Vec<BackupBookmark>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupSnippet {
    pub name: String,
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupPayload {
    pub version: u32,
    pub servers: Vec<BackupServer>,
    pub snippets: Vec<BackupSnippet>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupKdf {
    pub algorithm: String,
    pub iterations: u32,
    pub salt_hex: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupEnvelope {
    pub magic: String,
    pub version: u32,
    pub kdf: BackupKdf,
    pub cipher: String,
    pub iv_hex: String,
    pub ciphertext_hex: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BackupStats {
    pub servers_added: i64,
    pub servers_updated: i64,
    pub bookmarks_added: i64,
    pub snippets_added: i64,
}

