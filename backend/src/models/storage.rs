use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAccountOutput {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub endpoint: String,
    pub region: String,
    pub access_key_id: String,
    pub has_secret: bool,
    pub default_bucket: String,
    pub path_style: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAccountInput {
    pub name: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    pub endpoint: String,
    #[serde(default = "default_region")]
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    #[serde(default)]
    pub default_bucket: String,
    #[serde(default = "default_true")]
    pub path_style: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAccountUpdateInput {
    pub name: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    pub endpoint: String,
    #[serde(default = "default_region")]
    pub region: String,
    pub access_key_id: String,
    #[serde(default)]
    pub secret_access_key: Option<String>,
    #[serde(default)]
    pub default_bucket: String,
    #[serde(default = "default_true")]
    pub path_style: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAccountVerifyInput {
    #[serde(default)]
    pub account_id: Option<i64>,
    #[serde(default = "default_provider")]
    pub provider: String,
    pub endpoint: String,
    #[serde(default = "default_region")]
    pub region: String,
    pub access_key_id: String,
    #[serde(default)]
    pub secret_access_key: Option<String>,
    #[serde(default)]
    pub default_bucket: String,
    #[serde(default = "default_true")]
    pub path_style: bool,
}

#[derive(Debug, Clone)]
pub struct DecryptedStorageAccount {
    pub id: i64,
    pub name: String,
    pub provider: String,
    pub endpoint: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub default_bucket: String,
    pub path_style: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageVerifyOut {
    pub ok: bool,
    pub message: String,
    pub buckets_count: usize,
    pub scoped_bucket_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageBucket {
    pub name: String,
    pub creation_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageObject {
    pub key: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub last_modified: String,
    pub etag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageListOut {
    pub bucket: String,
    pub prefix: String,
    pub entries: Vec<StorageObject>,
    pub is_truncated: bool,
    pub next_continuation_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCreateBucketIn {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCreateFolderIn {
    pub prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCopyIn {
    pub source_key: String,
    pub dest_key: String,
    #[serde(default)]
    pub delete_source: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageDeleteIn {
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageDeleteOut {
    pub deleted: Vec<String>,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePresignIn {
    pub key: String,
    #[serde(default = "default_presign_secs")]
    pub expires_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePresignOut {
    pub url: String,
    pub expires_secs: u64,
}

fn default_provider() -> String {
    "s3".to_string()
}

fn default_region() -> String {
    "us-east-1".to_string()
}

fn default_true() -> bool {
    true
}

fn default_presign_secs() -> u64 {
    3600
}
