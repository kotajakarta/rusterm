use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRemote {
    pub id: i64,
    pub name: String,
    pub local_server_id: i64,
    pub local_path: String,
    pub remote_server_id: i64,
    pub remote_path: String,
    pub ignore: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRemoteInput {
    pub name: String,
    pub local_server_id: i64,
    pub local_path: String,
    pub remote_server_id: i64,
    pub remote_path: String,
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String, // "file" | "directory" | "link" | "other"
    pub size: u64,
    pub mtime: f64,
    pub mode: u32,
    pub perms: String,
    pub owner: String,
    pub group: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListOut {
    pub path: String,
    pub entries: Vec<FileEntry>,
    pub self_entry: FileEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePrincipalsOut {
    pub users: Vec<String>,
    pub groups: Vec<String>,
    pub home: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadOut {
    pub path: String,
    pub content: String,
    pub mtime: f64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileActionBody {
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_mtime: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recursive: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleBookmarkBody {
    pub server_id: i64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncBookmarksBody {
    pub server_id: i64,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleBookmarkOut {
    pub bookmarked: bool,
    pub bookmarks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferBody {
    pub direction: String, // "upload" | "download"
    pub paths: Vec<String>,
}

/// Body of `/files/direct-transfer/scan` and `/start` (`DirectTransferScanIn` /
/// `DirectTransferStartIn` in the frontend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectTransferIn {
    pub source_server_id: i64,
    pub dest_server_id: i64,
    pub source_dir: String,
    pub dest_dir: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub ignore: Option<Vec<String>>,
    #[serde(default)]
    pub use_gitignore: Option<bool>,
}
