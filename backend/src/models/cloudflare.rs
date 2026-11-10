use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFAccount {
    pub id: i64,
    pub name: String,
    pub token_masked: String,
    pub account_id: String,
    pub account_name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFAccountIn {
    pub name: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFAccountVerifyIn {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFAccountUpdateIn {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFTunnel {
    pub id: String,
    pub name: String,
    pub status: String,
    pub conns_count: u32,
    pub created_at: String,
    pub remote_config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFZone {
    pub id: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFRoute {
    pub hostname: String,
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFRouteIn {
    pub hostname: String,
    pub service: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFSyncMetadata {
    pub device_name: String,
    pub updated_at: i64,
    pub servers_count: usize,
    pub snippets_count: usize,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFSyncLocalCounts {
    pub servers_count: usize,
    pub snippets_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFSyncStatusOut {
    pub configured: bool,
    pub has_remote: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_metadata: Option<CFSyncMetadata>,
    pub local_counts: CFSyncLocalCounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFSyncNowIn {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<i64>,
    pub pin: String,
    #[serde(default = "default_device_name")]
    pub device_name: String,
}

fn default_device_name() -> String {
    "TermDeck Desktop".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFSyncNowOut {
    pub success: bool,
    pub synced_at: i64,
    pub stats: crate::models::server::BackupStats,
    pub remote_metadata: CFSyncMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFSyncPushIn {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<i64>,
    pub pin: String,
    #[serde(default = "default_device_name")]
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFSyncPushOut {
    pub success: bool,
    pub synced_at: i64,
    pub remote_metadata: CFSyncMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFSyncPullIn {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<i64>,
    pub pin: String,
    #[serde(default)]
    pub replace_local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFSyncPullOut {
    pub success: bool,
    pub stats: crate::models::server::BackupStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_metadata: Option<CFSyncMetadata>,
}
