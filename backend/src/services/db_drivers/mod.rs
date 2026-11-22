pub mod mysql;
pub mod postgres;
pub mod remote_sqlite;
pub mod sqlite;

use crate::models::db_client::{
    ConnectionTestResult, CreateTableRequest, DbEngine, DbValue, FetchRowsParams, QueryResult,
    RowMutationResult, TableInfo, TableSchema,
};
use async_trait::async_trait;
use std::collections::BTreeMap;
use thiserror::Error;

pub const DEFAULT_PAGE_SIZE: u64 = 100;
pub const MAX_PAGE_SIZE: u64 = 1000;
pub const MAX_CELL_CHARS: usize = 65_536;
pub const MAX_BYTES_PREVIEW: usize = 64;
pub const DEFAULT_QUERY_TIMEOUT_SECS: u64 = 20;

#[derive(Error, Debug)]
pub enum DbDriverError {
    #[error("Koneksi gagal: {0}")]
    Connection(String),
    #[error("Query gagal: {0}")]
    Query(String),
    #[error("Input tidak valid: {0}")]
    InvalidInput(String),
    #[error("Operasi ditolak: koneksi berada dalam mode Read-Only")]
    ReadOnly,
    #[error("Operasi tidak didukung pada engine ini: {0}")]
    Unsupported(String),
    #[error("Timeout: operasi melebihi batas waktu ({0} detik)")]
    Timeout(u64),
    #[error("SSH tunnel gagal: {0}")]
    Tunnel(String),
}

/// Escape dan validasi SQL identifier sesuai dialect.
/// Mencegah null byte dan identifier kosong.
pub fn quote_ident(engine: DbEngine, ident: &str) -> Result<String, DbDriverError> {
    if ident.is_empty() || ident.len() > 256 {
        return Err(DbDriverError::InvalidInput(
            "Nama identifier tidak boleh kosong atau lebih dari 256 karakter".into(),
        ));
    }
    if ident.contains('\0') {
        return Err(DbDriverError::InvalidInput(
            "Nama identifier mengandung karakter tidak valid".into(),
        ));
    }
    match engine {
        DbEngine::Postgres | DbEngine::Sqlite => {
            Ok(format!("\"{}\"", ident.replace('"', "\"\"")))
        }
        DbEngine::Mysql => Ok(format!("`{}`", ident.replace('`', "``"))),
    }
}

/// Validasi tipe kolom SQL sederhana untuk Create Table.
/// Mengizinkan huruf, angka, spasi, koma, kurung, underscore (mis. `VARCHAR(255)`, `DECIMAL(10, 2)`).
pub fn validate_column_type(data_type: &str) -> Result<String, DbDriverError> {
    let trimmed = data_type.trim();
    if trimmed.is_empty() || trimmed.len() > 128 {
        return Err(DbDriverError::InvalidInput(
            "Tipe data kolom tidak boleh kosong atau terlalu panjang".into(),
        ));
    }
    let valid = trimmed.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, ' ' | '(' | ')' | ',' | '_' | '[' | ']')
    });
    if !valid {
        return Err(DbDriverError::InvalidInput(format!(
            "Tipe data kolom '{}' mengandung karakter tidak diizinkan",
            trimmed
        )));
    }
    Ok(trimmed.to_string())
}

/// Validasi nilai default literal sederhana pada Create Table.
/// Menolak statement separator (`;`) dan komentar SQL (`--`, `/*`).
pub fn validate_default_expr(expr: &str) -> Result<String, DbDriverError> {
    let trimmed = expr.trim();
    if trimmed.len() > 256
        || trimmed.contains('\0')
        || trimmed.contains(';')
        || trimmed.contains("--")
        || trimmed.contains("/*")
        || trimmed.contains("*/")
    {
        return Err(DbDriverError::InvalidInput(
            "Nilai default kolom mengandung karakter tidak diizinkan".into(),
        ));
    }
    Ok(trimmed.to_string())
}

pub fn truncate_text(s: String) -> DbValue {
    if s.chars().count() > MAX_CELL_CHARS {
        let clipped: String = s.chars().take(MAX_CELL_CHARS).collect();
        DbValue::Text {
            value: clipped,
            truncated: true,
        }
    } else {
        DbValue::Text {
            value: s,
            truncated: false,
        }
    }
}

pub fn bytes_to_db_value(bytes: &[u8]) -> DbValue {
    let preview_len = bytes.len().min(MAX_BYTES_PREVIEW);
    DbValue::Bytes {
        size: bytes.len(),
        preview_hex: hex::encode(&bytes[..preview_len]),
    }
}

pub fn clamp_limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE)
}

pub fn clamp_page(page: Option<u64>) -> u64 {
    page.unwrap_or(1).max(1)
}

#[async_trait]
pub trait DbDriver: Send + Sync {
    async fn test_connection(&self) -> Result<ConnectionTestResult, DbDriverError>;
    async fn list_databases(&self) -> Result<Vec<String>, DbDriverError>;
    async fn list_schemas(&self, _database: &str) -> Result<Vec<String>, DbDriverError> {
        Ok(Vec::new())
    }
    async fn create_schema(&self, _database: &str, _name: &str) -> Result<(), DbDriverError> {
        Err(DbDriverError::Unsupported(
            "Schema tidak didukung pada engine ini".into(),
        ))
    }
    async fn drop_schema(&self, _database: &str, _name: &str) -> Result<(), DbDriverError> {
        Err(DbDriverError::Unsupported(
            "Schema tidak didukung pada engine ini".into(),
        ))
    }
    async fn list_tables(
        &self,
        database: &str,
        schema: Option<&str>,
    ) -> Result<Vec<TableInfo>, DbDriverError>;
    async fn describe_table(
        &self,
        database: &str,
        table: &str,
    ) -> Result<TableSchema, DbDriverError>;
    async fn fetch_rows(&self, params: &FetchRowsParams) -> Result<crate::models::db_client::RowsPage, DbDriverError>;
    async fn execute_query(
        &self,
        database: &str,
        sql: &str,
        read_only: bool,
        max_rows: u64,
    ) -> Result<QueryResult, DbDriverError>;
    async fn insert_row(
        &self,
        database: &str,
        table: &str,
        values: &BTreeMap<String, serde_json::Value>,
    ) -> Result<RowMutationResult, DbDriverError>;
    async fn update_row(
        &self,
        database: &str,
        table: &str,
        pk: &BTreeMap<String, serde_json::Value>,
        values: &BTreeMap<String, serde_json::Value>,
    ) -> Result<RowMutationResult, DbDriverError>;
    async fn delete_row(
        &self,
        database: &str,
        table: &str,
        pk: &BTreeMap<String, serde_json::Value>,
    ) -> Result<RowMutationResult, DbDriverError>;
    async fn create_database(&self, name: &str) -> Result<(), DbDriverError>;
    async fn drop_database(&self, name: &str) -> Result<(), DbDriverError>;
    async fn create_table(&self, req: &CreateTableRequest) -> Result<(), DbDriverError>;
    async fn drop_table(&self, database: &str, table: &str) -> Result<(), DbDriverError>;
}
