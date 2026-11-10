use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Bookmark {
    pub id: i64,
    pub server_id: i64,
    pub name: String,
    pub path: String,
    pub startup_cmd: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BookmarkInput {
    pub server_id: i64,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub startup_cmd: String,
}
