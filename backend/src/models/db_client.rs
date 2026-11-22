use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DbEngine {
    Postgres,
    Mysql,
    Sqlite,
}

impl DbEngine {
    pub fn as_str(&self) -> &'static str {
        match self {
            DbEngine::Postgres => "postgres",
            DbEngine::Mysql => "mysql",
            DbEngine::Sqlite => "sqlite",
        }
    }

    pub fn from_str_checked(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "postgres" | "postgresql" => Some(DbEngine::Postgres),
            "mysql" | "mariadb" => Some(DbEngine::Mysql),
            "sqlite" | "sqlite3" => Some(DbEngine::Sqlite),
            _ => None,
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            DbEngine::Postgres => 5432,
            DbEngine::Mysql => 3306,
            DbEngine::Sqlite => 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SslMode {
    Disable,
    Prefer,
    Require,
}

impl Default for SslMode {
    fn default() -> Self {
        SslMode::Prefer
    }
}

impl SslMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SslMode::Disable => "disable",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "disable" | "off" | "false" => SslMode::Disable,
            "require" | "on" | "true" => SslMode::Require,
            _ => SslMode::Prefer,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DbConnectionInput {
    pub name: String,
    pub engine: DbEngine,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    /// `None` saat update mempertahankan password tersimpan; `Some("")` menghapus password.
    pub password: Option<String>,
    #[serde(default)]
    pub default_database: String,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub ssl_mode: SslMode,
    pub ssh_server_id: Option<i64>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub color: String,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DbConnectionOutput {
    pub id: i64,
    pub name: String,
    pub engine: DbEngine,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub has_password: bool,
    pub default_database: String,
    pub file_path: String,
    pub ssl_mode: SslMode,
    pub ssh_server_id: Option<i64>,
    pub read_only: bool,
    pub color: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct DecryptedDbConnection {
    pub id: i64,
    pub name: String,
    pub engine: DbEngine,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub default_database: String,
    pub file_path: String,
    pub ssl_mode: SslMode,
    pub ssh_server_id: Option<i64>,
    pub read_only: bool,
}

impl fmt::Debug for DecryptedDbConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecryptedDbConnection")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("engine", &self.engine)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("has_password", &self.password.is_some())
            .field("default_database", &self.default_database)
            .field("file_path", &self.file_path)
            .field("ssl_mode", &self.ssl_mode)
            .field("ssh_server_id", &self.ssh_server_id)
            .field("read_only", &self.read_only)
            .finish()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TestDbConnectionInput {
    /// Jika `id` diisi dan `password` adalah `None`, gunakan password tersimpan dari koneksi tersebut.
    pub id: Option<i64>,
    pub engine: DbEngine,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    pub password: Option<String>,
    #[serde(default)]
    pub default_database: String,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub ssl_mode: SslMode,
    pub ssh_server_id: Option<i64>,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnectionTestResult {
    pub ok: bool,
    pub message: String,
    pub version: Option<String>,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum DbValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text { value: String, truncated: bool },
    Json(serde_json::Value),
    Bytes { size: usize, preview_hex: String },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TableInfo {
    pub name: String,
    pub kind: String, // "table" | "view"
    pub row_estimate: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub is_primary_key: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TableSchema {
    pub database: String,
    pub table: String,
    pub columns: Vec<ColumnSchema>,
    pub primary_keys: Vec<String>,
    pub indexes: Vec<IndexInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RowsPage {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<DbValue>>,
    pub page: u64,
    pub limit: u64,
    pub total_estimate: Option<i64>,
    pub has_more: bool,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<DbValue>>,
    pub affected_rows: Option<u64>,
    pub elapsed_ms: u64,
    pub truncated: bool,
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Neq,
    Contains,
    Gt,
    Gte,
    Lt,
    Lte,
    IsNull,
    IsNotNull,
}

impl FilterOp {
    pub fn from_str_checked(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "eq" | "=" => Some(FilterOp::Eq),
            "neq" | "!=" | "<>" => Some(FilterOp::Neq),
            "contains" | "like" => Some(FilterOp::Contains),
            "gt" | ">" => Some(FilterOp::Gt),
            "gte" | ">=" => Some(FilterOp::Gte),
            "lt" | "<" => Some(FilterOp::Lt),
            "lte" | "<=" => Some(FilterOp::Lte),
            "is_null" => Some(FilterOp::IsNull),
            "is_not_null" => Some(FilterOp::IsNotNull),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RowFilter {
    pub column: String,
    pub op: FilterOp,
    pub value: Option<String>,
}

#[derive(Clone, Debug)]
pub struct FetchRowsParams {
    pub database: String,
    pub table: String,
    pub page: u64,
    pub limit: u64,
    pub sort_col: Option<String>,
    pub sort_desc: bool,
    pub filter: Option<RowFilter>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum MutationOp {
    Insert,
    Update,
    Delete,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RowMutationRequest {
    #[serde(default)]
    pub db: String,
    pub op: MutationOp,
    /// Map primary key column -> nilai (wajib untuk update & delete).
    #[serde(default)]
    pub pk: BTreeMap<String, serde_json::Value>,
    /// Map column -> nilai baru (wajib untuk insert & update).
    #[serde(default)]
    pub values: BTreeMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RowMutationResult {
    pub affected_rows: u64,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateDatabaseRequest {
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateColumnSpec {
    pub name: String,
    pub data_type: String,
    #[serde(default = "default_true")]
    pub nullable: bool,
    #[serde(default)]
    pub primary_key: bool,
    pub default_value: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateTableRequest {
    #[serde(default)]
    pub db: String,
    pub table: String,
    pub columns: Vec<CreateColumnSpec>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecuteQueryRequest {
    #[serde(default)]
    pub db: String,
    pub sql: String,
    pub max_rows: Option<u64>,
}
