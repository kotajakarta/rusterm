use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleAccount {
    pub id: i64,
    pub name: String,
    pub token_masked: String,
    pub tailnet: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleAccountIn {
    pub name: String,
    pub api_key: String,
    #[serde(default = "default_tailnet")]
    pub tailnet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleAccountUpdateIn {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tailnet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleAccountVerifyIn {
    pub api_key: String,
    #[serde(default = "default_tailnet")]
    pub tailnet: String,
}

fn default_tailnet() -> String {
    "-".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailscaleVerifyOut {
    pub valid: bool,
    pub tailnet: String,
    pub devices_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TailscaleDevice {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub client_version: String,
    #[serde(default)]
    pub update_available: bool,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub last_seen: String,
    #[serde(default)]
    pub key_expiry_disabled: bool,
    #[serde(default)]
    pub expires: String,
    #[serde(default = "default_true")]
    pub authorized: bool,
    #[serde(default)]
    pub is_external: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub blocks_incoming_connections: bool,
    #[serde(default)]
    pub advertised_routes: Vec<String>,
    #[serde(default)]
    pub enabled_routes: Vec<String>,
    #[serde(default)]
    pub online: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TailscaleDeviceRoutes {
    #[serde(default)]
    pub advertised_routes: Vec<String>,
    #[serde(default)]
    pub enabled_routes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetRoutesIn {
    pub routes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAuthorizedIn {
    pub authorized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetKeyExpiryIn {
    pub key_expiry_disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTagsIn {
    pub tags: Vec<String>,
}
