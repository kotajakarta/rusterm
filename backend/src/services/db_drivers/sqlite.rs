use super::{
    bytes_to_db_value, quote_ident, truncate_text, validate_column_type, validate_default_expr,
    DbDriver, DbDriverError,
};
use crate::models::db_client::{
    ColumnSchema, ConnectionTestResult, CreateTableRequest, DbEngine, DbValue, FetchRowsParams,
    FilterOp, IndexInfo, QueryResult, RowMutationResult, RowsPage, TableInfo, TableSchema,
};
use async_trait::async_trait;
use rusqlite::types::{Value as SqliteValue, ValueRef};
use rusqlite::{Connection, OpenFlags};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

pub struct SqliteDriver {
    file_path: PathBuf,
    read_only: bool,
}

impl SqliteDriver {
    pub fn new(file_path: String, read_only: bool) -> Result<Self, DbDriverError> {
        let trimmed = file_path.trim();
        if trimmed.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Path file SQLite tidak boleh kosong".into(),
            ));
        }
        Ok(Self {
            file_path: PathBuf::from(trimmed),
            read_only,
        })
    }

    fn open_conn(path: &PathBuf, read_only: bool) -> Result<Connection, DbDriverError> {
        if !path.exists() && read_only {
            return Err(DbDriverError::Connection(format!(
                "File SQLite tidak ditemukan: {}",
                path.display()
            )));
        }
        let flags = if read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
        };
        let conn = Connection::open_with_flags(path, flags)
            .map_err(|e| DbDriverError::Connection(e.to_string()))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| DbDriverError::Connection(e.to_string()))?;
        Ok(conn)
    }
}

fn value_ref_to_db_value(v: ValueRef<'_>) -> DbValue {
    match v {
        ValueRef::Null => DbValue::Null,
        ValueRef::Integer(i) => DbValue::Int(i),
        ValueRef::Real(f) => DbValue::Float(f),
        ValueRef::Text(bytes) => {
            let s = String::from_utf8_lossy(bytes).into_owned();
            truncate_text(s)
        }
        ValueRef::Blob(bytes) => bytes_to_db_value(bytes),
    }
}

fn json_to_sqlite_value(v: &serde_json::Value) -> SqliteValue {
    match v {
        serde_json::Value::Null => SqliteValue::Null,
        serde_json::Value::Bool(b) => SqliteValue::Integer(if *b { 1 } else { 0 }),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqliteValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                SqliteValue::Real(f)
            } else {
                SqliteValue::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => SqliteValue::Text(s.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            SqliteValue::Text(v.to_string())
        }
    }
}

#[async_trait]
impl DbDriver for SqliteDriver {
    async fn test_connection(&self) -> Result<ConnectionTestResult, DbDriverError> {
        let path = self.file_path.clone();
        let ro = self.read_only;
        tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            let conn = Self::open_conn(&path, ro)?;
            let version: String = conn
                .query_row("SELECT sqlite_version()", [], |r| r.get(0))
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            Ok(ConnectionTestResult {
                ok: true,
                message: format!("Terhubung ke SQLite ({})", path.display()),
                version: Some(format!("SQLite {}", version)),
                elapsed_ms: start.elapsed().as_millis() as u64,
            })
        })
        .await
        .map_err(|e| DbDriverError::Connection(e.to_string()))?
    }

    async fn list_databases(&self) -> Result<Vec<String>, DbDriverError> {
        Ok(vec!["main".to_string()])
    }

    async fn list_tables(
        &self,
        _database: &str,
        _schema: Option<&str>,
    ) -> Result<Vec<TableInfo>, DbDriverError> {
        let path = self.file_path.clone();
        let ro = self.read_only;
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_conn(&path, ro)?;
            let mut stmt = conn
                .prepare(
                    "SELECT name, type FROM sqlite_master
                     WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
                     ORDER BY type ASC, name ASC",
                )
                .map_err(|e| DbDriverError::Query(e.to_string()))?;

            let rows = stmt
                .query_map([], |row| {
                    Ok(TableInfo {
                        name: row.get(0)?,
                        kind: row.get(1)?,
                        row_estimate: None,
                    })
                })
                .map_err(|e| DbDriverError::Query(e.to_string()))?;

            let mut out = Vec::new();
            for item in rows {
                out.push(item.map_err(|e| DbDriverError::Query(e.to_string()))?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| DbDriverError::Query(e.to_string()))?
    }

    async fn describe_table(
        &self,
        database: &str,
        table: &str,
    ) -> Result<TableSchema, DbDriverError> {
        let path = self.file_path.clone();
        let ro = self.read_only;
        let db_name = if database.is_empty() {
            "main".to_string()
        } else {
            database.to_string()
        };
        let table_name = table.to_string();
        let quoted_table = quote_ident(DbEngine::Sqlite, table)?;

        tokio::task::spawn_blocking(move || {
            let conn = Self::open_conn(&path, ro)?;
            let mut stmt = conn
                .prepare(&format!("PRAGMA table_info({})", quoted_table))
                .map_err(|e| DbDriverError::Query(e.to_string()))?;

            let col_rows = stmt
                .query_map([], |row| {
                    let pk_idx: i64 = row.get(5)?;
                    let notnull: i64 = row.get(3)?;
                    Ok(ColumnSchema {
                        name: row.get(1)?,
                        data_type: row.get::<_, String>(2)?,
                        nullable: notnull == 0,
                        default_value: row.get(4)?,
                        is_primary_key: pk_idx > 0,
                    })
                })
                .map_err(|e| DbDriverError::Query(e.to_string()))?;

            let mut columns = Vec::new();
            let mut primary_keys = Vec::new();
            for col in col_rows {
                let c = col.map_err(|e| DbDriverError::Query(e.to_string()))?;
                if c.is_primary_key {
                    primary_keys.push(c.name.clone());
                }
                columns.push(c);
            }

            let mut idx_stmt = conn
                .prepare(&format!("PRAGMA index_list({})", quoted_table))
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            let idx_list: Vec<(String, bool)> = idx_stmt
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    let unique: i64 = row.get(2)?;
                    Ok((name, unique != 0))
                })
                .map_err(|e| DbDriverError::Query(e.to_string()))?
                .filter_map(|r| r.ok())
                .collect();

            let mut indexes = Vec::new();
            for (idx_name, unique) in idx_list {
                let quoted_idx = quote_ident(DbEngine::Sqlite, &idx_name)?;
                let mut info_stmt = conn
                    .prepare(&format!("PRAGMA index_info({})", quoted_idx))
                    .map_err(|e| DbDriverError::Query(e.to_string()))?;
                let cols: Vec<String> = info_stmt
                    .query_map([], |row| row.get::<_, Option<String>>(2))
                    .map_err(|e| DbDriverError::Query(e.to_string()))?
                    .filter_map(|r| r.ok().flatten())
                    .collect();
                indexes.push(IndexInfo {
                    name: idx_name,
                    columns: cols,
                    unique,
                });
            }

            Ok(TableSchema {
                database: db_name,
                table: table_name,
                columns,
                primary_keys,
                indexes,
            })
        })
        .await
        .map_err(|e| DbDriverError::Query(e.to_string()))?
    }

    async fn fetch_rows(&self, params: &FetchRowsParams) -> Result<RowsPage, DbDriverError> {
        let path = self.file_path.clone();
        let ro = self.read_only;
        let quoted_table = quote_ident(DbEngine::Sqlite, &params.table)?;
        let sort_clause = match &params.sort_col {
            Some(col) if !col.is_empty() => {
                let q = quote_ident(DbEngine::Sqlite, col)?;
                let dir = if params.sort_desc { "DESC" } else { "ASC" };
                format!(" ORDER BY {} {}", q, dir)
            }
            _ => String::new(),
        };

        let mut where_sql = String::new();
        let mut filter_bind: Option<SqliteValue> = None;
        if let Some(ref f) = params.filter {
            let qcol = quote_ident(DbEngine::Sqlite, &f.column)?;
            match f.op {
                FilterOp::IsNull => where_sql = format!(" WHERE {} IS NULL", qcol),
                FilterOp::IsNotNull => where_sql = format!(" WHERE {} IS NOT NULL", qcol),
                FilterOp::Contains => {
                    where_sql = format!(" WHERE CAST({} AS TEXT) LIKE ?", qcol);
                    filter_bind = Some(SqliteValue::Text(format!(
                        "%{}%",
                        f.value.clone().unwrap_or_default()
                    )));
                }
                op => {
                    let sym = match op {
                        FilterOp::Eq => "=",
                        FilterOp::Neq => "!=",
                        FilterOp::Gt => ">",
                        FilterOp::Gte => ">=",
                        FilterOp::Lt => "<",
                        FilterOp::Lte => "<=",
                        _ => "=",
                    };
                    where_sql = format!(" WHERE {} {} ?", qcol, sym);
                    filter_bind = Some(SqliteValue::Text(f.value.clone().unwrap_or_default()));
                }
            }
        }

        let page = params.page;
        let limit = params.limit;
        let offset = (page.saturating_sub(1)) * limit;

        tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            let conn = Self::open_conn(&path, ro)?;

            let count_sql = format!("SELECT COUNT(*) FROM {}{}", quoted_table, where_sql);
            let total_estimate: i64 = if let Some(ref b) = filter_bind {
                conn.query_row(&count_sql, rusqlite::params![b], |r| r.get(0))
                    .map_err(|e| DbDriverError::Query(e.to_string()))?
            } else {
                conn.query_row(&count_sql, [], |r| r.get(0))
                    .map_err(|e| DbDriverError::Query(e.to_string()))?
            };

            let fetch_limit = limit + 1;
            let data_sql = format!(
                "SELECT * FROM {}{}{} LIMIT ? OFFSET ?",
                quoted_table, where_sql, sort_clause
            );
            let mut stmt = conn
                .prepare(&data_sql)
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            let columns: Vec<String> = stmt
                .column_names()
                .iter()
                .map(|s| s.to_string())
                .collect();

            let mut bind_params: Vec<SqliteValue> = Vec::new();
            if let Some(b) = filter_bind {
                bind_params.push(b);
            }
            bind_params.push(SqliteValue::Integer(fetch_limit as i64));
            bind_params.push(SqliteValue::Integer(offset as i64));

            let mut rows_out = Vec::new();
            let mut query_rows = stmt
                .query(rusqlite::params_from_iter(bind_params.iter()))
                .map_err(|e| DbDriverError::Query(e.to_string()))?;

            while let Some(row) = query_rows
                .next()
                .map_err(|e| DbDriverError::Query(e.to_string()))?
            {
                let mut vals = Vec::with_capacity(columns.len());
                for i in 0..columns.len() {
                    let v = row
                        .get_ref(i)
                        .map_err(|e| DbDriverError::Query(e.to_string()))?;
                    vals.push(value_ref_to_db_value(v));
                }
                rows_out.push(vals);
            }

            let has_more = rows_out.len() as u64 > limit;
            if has_more {
                rows_out.truncate(limit as usize);
            }

            Ok(RowsPage {
                columns,
                rows: rows_out,
                page,
                limit,
                total_estimate: Some(total_estimate),
                has_more,
                elapsed_ms: start.elapsed().as_millis() as u64,
            })
        })
        .await
        .map_err(|e| DbDriverError::Query(e.to_string()))?
    }

    async fn execute_query(
        &self,
        _database: &str,
        sql: &str,
        read_only: bool,
        max_rows: u64,
    ) -> Result<QueryResult, DbDriverError> {
        let path = self.file_path.clone();
        let ro = self.read_only || read_only;
        let sql_owned = sql.trim().to_string();
        if sql_owned.is_empty() {
            return Err(DbDriverError::InvalidInput("SQL tidak boleh kosong".into()));
        }

        tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            let conn = Self::open_conn(&path, ro)?;
            let mut stmt = conn
                .prepare(&sql_owned)
                .map_err(|e| DbDriverError::Query(e.to_string()))?;

            if stmt.column_count() > 0 {
                let columns: Vec<String> = stmt
                    .column_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                let mut rows_out = Vec::new();
                let mut truncated = false;
                let mut query_rows = stmt
                    .query([])
                    .map_err(|e| DbDriverError::Query(e.to_string()))?;

                while let Some(row) = query_rows
                    .next()
                    .map_err(|e| DbDriverError::Query(e.to_string()))?
                {
                    if rows_out.len() as u64 >= max_rows {
                        truncated = true;
                        break;
                    }
                    let mut vals = Vec::with_capacity(columns.len());
                    for i in 0..columns.len() {
                        let v = row
                            .get_ref(i)
                            .map_err(|e| DbDriverError::Query(e.to_string()))?;
                        vals.push(value_ref_to_db_value(v));
                    }
                    rows_out.push(vals);
                }

                Ok(QueryResult {
                    columns,
                    rows: rows_out,
                    affected_rows: None,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                    truncated,
                    message: None,
                })
            } else {
                if ro {
                    return Err(DbDriverError::ReadOnly);
                }
                let affected = stmt
                    .execute([])
                    .map_err(|e| DbDriverError::Query(e.to_string()))?
                    as u64;
                Ok(QueryResult {
                    columns: Vec::new(),
                    rows: Vec::new(),
                    affected_rows: Some(affected),
                    elapsed_ms: start.elapsed().as_millis() as u64,
                    truncated: false,
                    message: Some(format!("Berhasil ({} baris terpengaruh)", affected)),
                })
            }
        })
        .await
        .map_err(|e| DbDriverError::Query(e.to_string()))?
    }

    async fn insert_row(
        &self,
        _database: &str,
        table: &str,
        values: &BTreeMap<String, serde_json::Value>,
    ) -> Result<RowMutationResult, DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if values.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Minimal satu kolom harus diisi untuk insert".into(),
            ));
        }
        let path = self.file_path.clone();
        let quoted_table = quote_ident(DbEngine::Sqlite, table)?;
        let mut cols = Vec::with_capacity(values.len());
        let mut placeholders = Vec::with_capacity(values.len());
        let mut binds = Vec::with_capacity(values.len());
        for (k, v) in values {
            cols.push(quote_ident(DbEngine::Sqlite, k)?);
            placeholders.push("?");
            binds.push(json_to_sqlite_value(v));
        }
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quoted_table,
            cols.join(", "),
            placeholders.join(", ")
        );

        tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            let conn = Self::open_conn(&path, false)?;
            let affected = conn
                .execute(&sql, rusqlite::params_from_iter(binds.iter()))
                .map_err(|e| DbDriverError::Query(e.to_string()))? as u64;
            Ok(RowMutationResult {
                affected_rows: affected,
                elapsed_ms: start.elapsed().as_millis() as u64,
            })
        })
        .await
        .map_err(|e| DbDriverError::Query(e.to_string()))?
    }

    async fn update_row(
        &self,
        _database: &str,
        table: &str,
        pk: &BTreeMap<String, serde_json::Value>,
        values: &BTreeMap<String, serde_json::Value>,
    ) -> Result<RowMutationResult, DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if pk.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Update membutuhkan Primary Key".into(),
            ));
        }
        if values.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Tidak ada kolom yang diubah".into(),
            ));
        }
        let path = self.file_path.clone();
        let quoted_table = quote_ident(DbEngine::Sqlite, table)?;
        let mut sets = Vec::with_capacity(values.len());
        let mut binds = Vec::with_capacity(values.len() + pk.len());
        for (k, v) in values {
            sets.push(format!("{} = ?", quote_ident(DbEngine::Sqlite, k)?));
            binds.push(json_to_sqlite_value(v));
        }
        let mut wheres = Vec::with_capacity(pk.len());
        for (k, v) in pk {
            let q = quote_ident(DbEngine::Sqlite, k)?;
            if v.is_null() {
                wheres.push(format!("{} IS NULL", q));
            } else {
                wheres.push(format!("{} = ?", q));
                binds.push(json_to_sqlite_value(v));
            }
        }
        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            quoted_table,
            sets.join(", "),
            wheres.join(" AND ")
        );

        tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            let conn = Self::open_conn(&path, false)?;
            let affected = conn
                .execute(&sql, rusqlite::params_from_iter(binds.iter()))
                .map_err(|e| DbDriverError::Query(e.to_string()))? as u64;
            Ok(RowMutationResult {
                affected_rows: affected,
                elapsed_ms: start.elapsed().as_millis() as u64,
            })
        })
        .await
        .map_err(|e| DbDriverError::Query(e.to_string()))?
    }

    async fn delete_row(
        &self,
        _database: &str,
        table: &str,
        pk: &BTreeMap<String, serde_json::Value>,
    ) -> Result<RowMutationResult, DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if pk.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Delete membutuhkan Primary Key".into(),
            ));
        }
        let path = self.file_path.clone();
        let quoted_table = quote_ident(DbEngine::Sqlite, table)?;
        let mut wheres = Vec::with_capacity(pk.len());
        let mut binds = Vec::with_capacity(pk.len());
        for (k, v) in pk {
            let q = quote_ident(DbEngine::Sqlite, k)?;
            if v.is_null() {
                wheres.push(format!("{} IS NULL", q));
            } else {
                wheres.push(format!("{} = ?", q));
                binds.push(json_to_sqlite_value(v));
            }
        }
        let sql = format!("DELETE FROM {} WHERE {}", quoted_table, wheres.join(" AND "));

        tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            let conn = Self::open_conn(&path, false)?;
            let affected = conn
                .execute(&sql, rusqlite::params_from_iter(binds.iter()))
                .map_err(|e| DbDriverError::Query(e.to_string()))? as u64;
            Ok(RowMutationResult {
                affected_rows: affected,
                elapsed_ms: start.elapsed().as_millis() as u64,
            })
        })
        .await
        .map_err(|e| DbDriverError::Query(e.to_string()))?
    }

    async fn create_database(&self, _name: &str) -> Result<(), DbDriverError> {
        Err(DbDriverError::Unsupported(
            "SQLite berbasis file. Tambahkan koneksi SQLite baru dengan path file tujuan.".into(),
        ))
    }

    async fn drop_database(&self, _name: &str) -> Result<(), DbDriverError> {
        Err(DbDriverError::Unsupported(
            "Drop database tidak didukung pada SQLite.".into(),
        ))
    }

    async fn create_table(&self, req: &CreateTableRequest) -> Result<(), DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if req.columns.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Tabel minimal harus memiliki satu kolom".into(),
            ));
        }
        let path = self.file_path.clone();
        let quoted_table = quote_ident(DbEngine::Sqlite, &req.table)?;
        let mut col_defs = Vec::with_capacity(req.columns.len());
        let mut pk_cols = Vec::new();
        let mut seen = HashSet::new();

        for col in &req.columns {
            if !seen.insert(col.name.to_lowercase()) {
                return Err(DbDriverError::InvalidInput(format!(
                    "Nama kolom duplikat: {}",
                    col.name
                )));
            }
            let qname = quote_ident(DbEngine::Sqlite, &col.name)?;
            let dtype = validate_column_type(&col.data_type)?;
            let mut def = format!("{} {}", qname, dtype);
            if !col.nullable {
                def.push_str(" NOT NULL");
            }
            if let Some(ref d) = col.default_value {
                if !d.trim().is_empty() {
                    let safe_def = validate_default_expr(d)?;
                    def.push_str(&format!(" DEFAULT {}", safe_def));
                }
            }
            if col.primary_key {
                pk_cols.push(qname);
            }
            col_defs.push(def);
        }
        if !pk_cols.is_empty() {
            col_defs.push(format!("PRIMARY KEY ({})", pk_cols.join(", ")));
        }

        let sql = format!("CREATE TABLE {} ({})", quoted_table, col_defs.join(", "));
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_conn(&path, false)?;
            conn.execute(&sql, [])
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| DbDriverError::Query(e.to_string()))?
    }

    async fn drop_table(&self, _database: &str, table: &str) -> Result<(), DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        let path = self.file_path.clone();
        let quoted_table = quote_ident(DbEngine::Sqlite, table)?;
        let sql = format!("DROP TABLE {}", quoted_table);
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_conn(&path, false)?;
            conn.execute(&sql, [])
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| DbDriverError::Query(e.to_string()))?
    }
}
