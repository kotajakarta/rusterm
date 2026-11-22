use super::{
    bytes_to_db_value, quote_ident, truncate_text, validate_column_type, validate_default_expr,
    DbDriver, DbDriverError,
};
use crate::models::db_client::{
    ColumnSchema, ConnectionTestResult, CreateTableRequest, DbEngine, DbValue, FetchRowsParams,
    FilterOp, IndexInfo, QueryResult, RowMutationResult, RowsPage, SslMode, TableInfo, TableSchema,
};
use async_trait::async_trait;
use mysql_async::prelude::*;
use mysql_async::{Conn, OptsBuilder, Params, Row, SslOpts, Value};
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

pub struct MysqlDriver {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    default_database: String,
    ssl_mode: SslMode,
    read_only: bool,
}

impl MysqlDriver {
    pub fn new(
        host: String,
        port: u16,
        username: String,
        password: Option<String>,
        default_database: String,
        ssl_mode: SslMode,
        read_only: bool,
    ) -> Self {
        Self {
            host,
            port: if port == 0 { 3306 } else { port },
            username,
            password,
            default_database,
            ssl_mode,
            read_only,
        }
    }

    async fn connect(&self, database: &str) -> Result<Conn, DbDriverError> {
        let db = if database.trim().is_empty() {
            self.default_database.trim()
        } else {
            database.trim()
        };

        let build_opts = |use_ssl: bool| {
            let mut opts = OptsBuilder::default()
                .ip_or_hostname(self.host.clone())
                .tcp_port(self.port)
                .user(if self.username.is_empty() {
                    None
                } else {
                    Some(self.username.clone())
                })
                .pass(self.password.clone())
                .db_name(if db.is_empty() {
                    None
                } else {
                    Some(db.to_string())
                });
            if use_ssl {
                opts = opts.ssl_opts(SslOpts::default());
            }
            opts
        };

        match self.ssl_mode {
            SslMode::Disable => Conn::new(build_opts(false))
                .await
                .map_err(|e| DbDriverError::Connection(e.to_string())),
            SslMode::Require => Conn::new(build_opts(true))
                .await
                .map_err(|e| DbDriverError::Connection(e.to_string())),
            SslMode::Prefer => match Conn::new(build_opts(true)).await {
                Ok(c) => Ok(c),
                Err(_) => Conn::new(build_opts(false))
                    .await
                    .map_err(|e| DbDriverError::Connection(e.to_string())),
            },
        }
    }
}

fn mysql_value_to_db_value(v: Value) -> DbValue {
    match v {
        Value::NULL => DbValue::Null,
        Value::Int(i) => DbValue::Int(i),
        Value::UInt(u) => {
            if u <= i64::MAX as u64 {
                DbValue::Int(u as i64)
            } else {
                truncate_text(u.to_string())
            }
        }
        Value::Float(f) => DbValue::Float(f as f64),
        Value::Double(d) => DbValue::Float(d),
        Value::Bytes(bytes) => match String::from_utf8(bytes) {
            Ok(s) => truncate_text(s),
            Err(err) => bytes_to_db_value(err.as_bytes()),
        },
        Value::Date(y, m, d, h, min, s, us) => {
            if h == 0 && min == 0 && s == 0 && us == 0 {
                truncate_text(format!("{:04}-{:02}-{:02}", y, m, d))
            } else {
                truncate_text(format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                    y, m, d, h, min, s
                ))
            }
        }
        Value::Time(neg, d, h, m, s, _us) => {
            let sign = if neg { "-" } else { "" };
            let total_h = (d as u64) * 24 + (h as u64);
            truncate_text(format!("{}{:02}:{:02}:{:02}", sign, total_h, m, s))
        }
    }
}

fn json_to_mysql_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::NULL,
        serde_json::Value::Bool(b) => Value::Int(if *b { 1 } else { 0 }),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(u) = n.as_u64() {
                Value::UInt(u)
            } else if let Some(f) = n.as_f64() {
                Value::Double(f)
            } else {
                Value::Bytes(n.to_string().into_bytes())
            }
        }
        serde_json::Value::String(s) => Value::Bytes(s.as_bytes().to_vec()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Value::Bytes(v.to_string().into_bytes())
        }
    }
}

fn mysql_row_to_values(row: Row) -> Vec<DbValue> {
    row.unwrap().into_iter().map(mysql_value_to_db_value).collect()
}

#[async_trait]
impl DbDriver for MysqlDriver {
    async fn test_connection(&self) -> Result<ConnectionTestResult, DbDriverError> {
        let start = Instant::now();
        let mut conn = self.connect(&self.default_database).await?;
        let ver: Option<String> = conn
            .query_first("SELECT VERSION()")
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let _ = conn.disconnect().await;
        Ok(ConnectionTestResult {
            ok: true,
            message: format!("Terhubung ke MySQL/MariaDB ({}:{})", self.host, self.port),
            version: ver.map(|v| format!("MySQL {}", v)),
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn list_databases(&self) -> Result<Vec<String>, DbDriverError> {
        let mut conn = self.connect("").await?;
        let dbs: Vec<String> = conn
            .query("SHOW DATABASES")
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let _ = conn.disconnect().await;
        Ok(dbs)
    }

    async fn list_tables(&self, database: &str) -> Result<Vec<TableInfo>, DbDriverError> {
        let db = if database.trim().is_empty() {
            self.default_database.trim()
        } else {
            database.trim()
        };
        if db.is_empty() {
            return Ok(Vec::new());
        }
        let mut conn = self.connect(db).await?;
        let rows: Vec<(String, String, Option<u64>)> = conn
            .exec(
                "SELECT TABLE_NAME, TABLE_TYPE, TABLE_ROWS
                 FROM information_schema.TABLES
                 WHERE TABLE_SCHEMA = ?
                 ORDER BY TABLE_TYPE ASC, TABLE_NAME ASC",
                (db,),
            )
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let _ = conn.disconnect().await;

        Ok(rows
            .into_iter()
            .map(|(name, ttype, est)| TableInfo {
                name,
                kind: if ttype.to_uppercase().contains("VIEW") {
                    "view".to_string()
                } else {
                    "table".to_string()
                },
                row_estimate: est.and_then(|v| i64::try_from(v).ok()),
            })
            .collect())
    }

    async fn describe_table(
        &self,
        database: &str,
        table: &str,
    ) -> Result<TableSchema, DbDriverError> {
        let db = if database.trim().is_empty() {
            self.default_database.trim()
        } else {
            database.trim()
        };
        let mut conn = self.connect(db).await?;
        let col_rows: Vec<(String, String, String, Option<String>, String)> = conn
            .exec(
                "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_DEFAULT, COLUMN_KEY
                 FROM information_schema.COLUMNS
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
                 ORDER BY ORDINAL_POSITION ASC",
                (db, table),
            )
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;

        let mut columns = Vec::with_capacity(col_rows.len());
        let mut primary_keys = Vec::new();
        for (name, data_type, is_nullable, default_value, key) in col_rows {
            let is_pk = key.eq_ignore_ascii_case("PRI");
            if is_pk {
                primary_keys.push(name.clone());
            }
            columns.push(ColumnSchema {
                name,
                data_type,
                nullable: is_nullable.eq_ignore_ascii_case("YES"),
                default_value,
                is_primary_key: is_pk,
            });
        }

        let idx_rows: Vec<(String, i64, String)> = conn
            .exec(
                "SELECT INDEX_NAME, NON_UNIQUE, COLUMN_NAME
                 FROM information_schema.STATISTICS
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?
                 ORDER BY INDEX_NAME ASC, SEQ_IN_INDEX ASC",
                (db, table),
            )
            .await
            .unwrap_or_default();
        let _ = conn.disconnect().await;

        let mut idx_map: BTreeMap<String, (bool, Vec<String>)> = BTreeMap::new();
        for (idx_name, non_unique, col_name) in idx_rows {
            let entry = idx_map
                .entry(idx_name)
                .or_insert_with(|| (non_unique == 0, Vec::new()));
            entry.1.push(col_name);
        }
        let indexes = idx_map
            .into_iter()
            .map(|(name, (unique, columns))| IndexInfo {
                name,
                columns,
                unique,
            })
            .collect();

        Ok(TableSchema {
            database: db.to_string(),
            table: table.to_string(),
            columns,
            primary_keys,
            indexes,
        })
    }

    async fn fetch_rows(&self, params: &FetchRowsParams) -> Result<RowsPage, DbDriverError> {
        let start = Instant::now();
        let mut conn = self.connect(&params.database).await?;
        let quoted_table = quote_ident(DbEngine::Mysql, &params.table)?;

        let sort_clause = match &params.sort_col {
            Some(col) if !col.is_empty() => {
                let q = quote_ident(DbEngine::Mysql, col)?;
                let dir = if params.sort_desc { "DESC" } else { "ASC" };
                format!(" ORDER BY {} {}", q, dir)
            }
            _ => String::new(),
        };

        let mut where_sql = String::new();
        let mut filter_bind: Option<Value> = None;
        if let Some(ref f) = params.filter {
            let qcol = quote_ident(DbEngine::Mysql, &f.column)?;
            match f.op {
                FilterOp::IsNull => where_sql = format!(" WHERE {} IS NULL", qcol),
                FilterOp::IsNotNull => where_sql = format!(" WHERE {} IS NOT NULL", qcol),
                FilterOp::Contains => {
                    where_sql = format!(" WHERE CAST({} AS CHAR) LIKE ?", qcol);
                    filter_bind = Some(Value::Bytes(
                        format!("%{}%", f.value.clone().unwrap_or_default()).into_bytes(),
                    ));
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
                    filter_bind = Some(Value::Bytes(
                        f.value.clone().unwrap_or_default().into_bytes(),
                    ));
                }
            }
        }

        let count_sql = format!("SELECT COUNT(*) FROM {}{}", quoted_table, where_sql);
        let total_estimate: Option<i64> = if let Some(ref b) = filter_bind {
            conn.exec_first(&count_sql, Params::Positional(vec![b.clone()]))
                .await
                .ok()
                .flatten()
        } else {
            conn.query_first(&count_sql).await.ok().flatten()
        };

        let offset = (params.page.saturating_sub(1)) * params.limit;
        let fetch_limit = params.limit + 1;
        let data_sql = format!(
            "SELECT * FROM {}{}{} LIMIT ? OFFSET ?",
            quoted_table, where_sql, sort_clause
        );

        let mut bind_vals = Vec::new();
        if let Some(b) = filter_bind {
            bind_vals.push(b);
        }
        bind_vals.push(Value::UInt(fetch_limit));
        bind_vals.push(Value::UInt(offset));

        let rows: Vec<Row> = conn
            .exec(&data_sql, Params::Positional(bind_vals))
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;

        let columns: Vec<String> = if let Some(first) = rows.first() {
            first
                .columns_ref()
                .iter()
                .map(|c| c.name_str().into_owned())
                .collect()
        } else {
            let stmt = conn
                .prep(format!("SELECT * FROM {} LIMIT 0", quoted_table))
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            stmt.columns()
                .iter()
                .map(|c| c.name_str().into_owned())
                .collect()
        };
        let _ = conn.disconnect().await;

        let has_more = rows.len() as u64 > params.limit;
        let out_rows: Vec<Vec<DbValue>> = rows
            .into_iter()
            .take(params.limit as usize)
            .map(mysql_row_to_values)
            .collect();

        Ok(RowsPage {
            columns,
            rows: out_rows,
            page: params.page,
            limit: params.limit,
            total_estimate,
            has_more,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn execute_query(
        &self,
        database: &str,
        sql: &str,
        read_only: bool,
        max_rows: u64,
    ) -> Result<QueryResult, DbDriverError> {
        let trimmed = sql.trim();
        if trimmed.is_empty() {
            return Err(DbDriverError::InvalidInput("SQL tidak boleh kosong".into()));
        }
        let start = Instant::now();
        let mut conn = self.connect(database).await?;
        let ro = self.read_only || read_only;

        if ro {
            conn.query_drop("SET TRANSACTION READ ONLY")
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            conn.query_drop("START TRANSACTION READ ONLY")
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
        }

        let res = conn.query_iter(trimmed).await;
        match res {
            Ok(mut result_set) => {
                let columns: Vec<String> = result_set
                    .columns_ref()
                    .iter()
                    .map(|c| c.name_str().into_owned())
                    .collect();
                let affected = result_set.affected_rows();
                let mut out_rows = Vec::new();
                let mut truncated = false;

                if !columns.is_empty() {
                    let all_rows: Vec<Row> = result_set
                        .collect()
                        .await
                        .map_err(|e| DbDriverError::Query(e.to_string()))?;
                    truncated = all_rows.len() as u64 > max_rows;
                    out_rows = all_rows
                        .into_iter()
                        .take(max_rows as usize)
                        .map(mysql_row_to_values)
                        .collect();
                } else {
                    result_set
                        .drop_result()
                        .await
                        .map_err(|e| DbDriverError::Query(e.to_string()))?;
                }

                if ro {
                    let _ = conn.query_drop("ROLLBACK").await;
                }
                let _ = conn.disconnect().await;

                Ok(QueryResult {
                    columns: columns.clone(),
                    rows: out_rows,
                    affected_rows: if columns.is_empty() {
                        Some(affected)
                    } else {
                        None
                    },
                    elapsed_ms: start.elapsed().as_millis() as u64,
                    truncated,
                    message: if columns.is_empty() {
                        Some(format!("Berhasil ({} baris terpengaruh)", affected))
                    } else {
                        None
                    },
                })
            }
            Err(e) => {
                if ro {
                    let _ = conn.query_drop("ROLLBACK").await;
                }
                let _ = conn.disconnect().await;
                Err(DbDriverError::Query(e.to_string()))
            }
        }
    }

    async fn insert_row(
        &self,
        database: &str,
        table: &str,
        values: &BTreeMap<String, serde_json::Value>,
    ) -> Result<RowMutationResult, DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if values.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Minimal satu kolom harus diisi".into(),
            ));
        }
        let start = Instant::now();
        let mut conn = self.connect(database).await?;
        let quoted_table = quote_ident(DbEngine::Mysql, table)?;
        let mut cols = Vec::with_capacity(values.len());
        let mut placeholders = Vec::with_capacity(values.len());
        let mut binds = Vec::with_capacity(values.len());
        for (k, v) in values {
            cols.push(quote_ident(DbEngine::Mysql, k)?);
            placeholders.push("?");
            binds.push(json_to_mysql_value(v));
        }
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quoted_table,
            cols.join(", "),
            placeholders.join(", ")
        );
        conn.exec_drop(&sql, Params::Positional(binds))
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let affected = conn.affected_rows();
        let _ = conn.disconnect().await;
        Ok(RowMutationResult {
            affected_rows: affected,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn update_row(
        &self,
        database: &str,
        table: &str,
        pk: &BTreeMap<String, serde_json::Value>,
        values: &BTreeMap<String, serde_json::Value>,
    ) -> Result<RowMutationResult, DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if pk.is_empty() || values.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Primary key dan nilai perubahan wajib diisi".into(),
            ));
        }
        let start = Instant::now();
        let mut conn = self.connect(database).await?;
        let quoted_table = quote_ident(DbEngine::Mysql, table)?;
        let mut sets = Vec::with_capacity(values.len());
        let mut binds = Vec::new();
        for (k, v) in values {
            sets.push(format!("{} = ?", quote_ident(DbEngine::Mysql, k)?));
            binds.push(json_to_mysql_value(v));
        }
        let mut wheres = Vec::with_capacity(pk.len());
        for (k, v) in pk {
            let q = quote_ident(DbEngine::Mysql, k)?;
            if v.is_null() {
                wheres.push(format!("{} IS NULL", q));
            } else {
                wheres.push(format!("{} = ?", q));
                binds.push(json_to_mysql_value(v));
            }
        }
        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            quoted_table,
            sets.join(", "),
            wheres.join(" AND ")
        );
        conn.exec_drop(&sql, Params::Positional(binds))
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let affected = conn.affected_rows();
        let _ = conn.disconnect().await;
        Ok(RowMutationResult {
            affected_rows: affected,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn delete_row(
        &self,
        database: &str,
        table: &str,
        pk: &BTreeMap<String, serde_json::Value>,
    ) -> Result<RowMutationResult, DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if pk.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Primary key wajib diisi untuk delete".into(),
            ));
        }
        let start = Instant::now();
        let mut conn = self.connect(database).await?;
        let quoted_table = quote_ident(DbEngine::Mysql, table)?;
        let mut wheres = Vec::with_capacity(pk.len());
        let mut binds = Vec::new();
        for (k, v) in pk {
            let q = quote_ident(DbEngine::Mysql, k)?;
            if v.is_null() {
                wheres.push(format!("{} IS NULL", q));
            } else {
                wheres.push(format!("{} = ?", q));
                binds.push(json_to_mysql_value(v));
            }
        }
        let sql = format!("DELETE FROM {} WHERE {}", quoted_table, wheres.join(" AND "));
        conn.exec_drop(&sql, Params::Positional(binds))
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let affected = conn.affected_rows();
        let _ = conn.disconnect().await;
        Ok(RowMutationResult {
            affected_rows: affected,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn create_database(&self, name: &str) -> Result<(), DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        let mut conn = self.connect("").await?;
        let q = quote_ident(DbEngine::Mysql, name)?;
        conn.query_drop(format!("CREATE DATABASE {}", q))
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let _ = conn.disconnect().await;
        Ok(())
    }

    async fn drop_database(&self, name: &str) -> Result<(), DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        let mut conn = self.connect("").await?;
        let q = quote_ident(DbEngine::Mysql, name)?;
        conn.query_drop(format!("DROP DATABASE {}", q))
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let _ = conn.disconnect().await;
        Ok(())
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
        let mut conn = self.connect(&req.db).await?;
        let quoted_table = quote_ident(DbEngine::Mysql, &req.table)?;
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
            let qname = quote_ident(DbEngine::Mysql, &col.name)?;
            let dtype = validate_column_type(&col.data_type)?;
            let mut def = format!("{} {}", qname, dtype);
            if !col.nullable {
                def.push_str(" NOT NULL");
            }
            if let Some(ref d) = col.default_value {
                if !d.trim().is_empty() {
                    def.push_str(&format!(" DEFAULT {}", validate_default_expr(d)?));
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
        conn.query_drop(&sql)
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let _ = conn.disconnect().await;
        Ok(())
    }

    async fn drop_table(&self, database: &str, table: &str) -> Result<(), DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        let mut conn = self.connect(database).await?;
        let quoted_table = quote_ident(DbEngine::Mysql, table)?;
        conn.query_drop(format!("DROP TABLE {}", quoted_table))
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let _ = conn.disconnect().await;
        Ok(())
    }
}
