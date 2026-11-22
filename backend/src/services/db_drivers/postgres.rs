use super::{
    bytes_to_db_value, quote_ident, truncate_text, validate_column_type, validate_default_expr,
    DbDriver, DbDriverError,
};
use crate::models::db_client::{
    ColumnSchema, ConnectionTestResult, CreateTableRequest, DbEngine, DbValue, FetchRowsParams,
    FilterOp, IndexInfo, QueryResult, RowMutationResult, RowsPage, SslMode, TableInfo, TableSchema,
};
use async_trait::async_trait;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_postgres::types::{ToSql, Type};
use tokio_postgres::{Client, Config, NoTls, Row};
use tokio_postgres_rustls::MakeRustlsConnect;

pub struct PostgresDriver {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    default_database: String,
    ssl_mode: SslMode,
    read_only: bool,
}

impl PostgresDriver {
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
            port: if port == 0 { 5432 } else { port },
            username,
            password,
            default_database: if default_database.trim().is_empty() {
                "postgres".to_string()
            } else {
                default_database
            },
            ssl_mode,
            read_only,
        }
    }

    fn make_tls() -> Result<MakeRustlsConnect, DbDriverError> {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls_config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|e| DbDriverError::Connection(format!("TLS init gagal: {}", e)))?
        .with_root_certificates(root_store)
        .with_no_client_auth();
        Ok(MakeRustlsConnect::new(tls_config))
    }

    async fn connect(&self, database: &str) -> Result<Client, DbDriverError> {
        let db = if database.trim().is_empty() {
            &self.default_database
        } else {
            database.trim()
        };
        let mut cfg = Config::new();
        cfg.host(&self.host)
            .port(self.port)
            .user(&self.username)
            .dbname(db)
            .connect_timeout(Duration::from_secs(10));
        if let Some(ref pw) = self.password {
            cfg.password(pw);
        }

        match self.ssl_mode {
            SslMode::Disable => {
                let (client, connection) = cfg
                    .connect(NoTls)
                    .await
                    .map_err(|e| DbDriverError::Connection(e.to_string()))?;
                tokio::spawn(async move {
                    let _ = connection.await;
                });
                Ok(client)
            }
            SslMode::Require => {
                let tls = Self::make_tls()?;
                let (client, connection) = cfg
                    .connect(tls)
                    .await
                    .map_err(|e| DbDriverError::Connection(e.to_string()))?;
                tokio::spawn(async move {
                    let _ = connection.await;
                });
                Ok(client)
            }
            SslMode::Prefer => {
                if let Ok(tls) = Self::make_tls() {
                    if let Ok((client, connection)) = cfg.connect(tls).await {
                        tokio::spawn(async move {
                            let _ = connection.await;
                        });
                        return Ok(client);
                    }
                }
                let (client, connection) = cfg
                    .connect(NoTls)
                    .await
                    .map_err(|e| DbDriverError::Connection(e.to_string()))?;
                tokio::spawn(async move {
                    let _ = connection.await;
                });
                Ok(client)
            }
        }
    }
}

/// Split `schema.table` jika mengandung titik, default schema `public`.
fn split_pg_table(table: &str) -> (&str, &str) {
    if let Some((schema, name)) = table.split_once('.') {
        (schema, name)
    } else {
        ("public", table)
    }
}

fn quote_pg_table(table: &str) -> Result<String, DbDriverError> {
    let (schema, name) = split_pg_table(table);
    Ok(format!(
        "{}.{}",
        quote_ident(DbEngine::Postgres, schema)?,
        quote_ident(DbEngine::Postgres, name)?
    ))
}

fn pg_cell_to_db_value(row: &Row, idx: usize) -> DbValue {
    let col = &row.columns()[idx];
    let ty = col.type_();

    match *ty {
        Type::BOOL => match row.try_get::<_, Option<bool>>(idx) {
            Ok(Some(v)) => DbValue::Bool(v),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::INT2 => match row.try_get::<_, Option<i16>>(idx) {
            Ok(Some(v)) => DbValue::Int(v as i64),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::INT4 => match row.try_get::<_, Option<i32>>(idx) {
            Ok(Some(v)) => DbValue::Int(v as i64),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::INT8 => match row.try_get::<_, Option<i64>>(idx) {
            Ok(Some(v)) => DbValue::Int(v),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::FLOAT4 => match row.try_get::<_, Option<f32>>(idx) {
            Ok(Some(v)) => DbValue::Float(v as f64),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::FLOAT8 => match row.try_get::<_, Option<f64>>(idx) {
            Ok(Some(v)) => DbValue::Float(v),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::JSON | Type::JSONB => match row.try_get::<_, Option<serde_json::Value>>(idx) {
            Ok(Some(v)) => DbValue::Json(v),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::BYTEA => match row.try_get::<_, Option<Vec<u8>>>(idx) {
            Ok(Some(v)) => bytes_to_db_value(&v),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::TIMESTAMP => match row.try_get::<_, Option<chrono::NaiveDateTime>>(idx) {
            Ok(Some(v)) => truncate_text(v.format("%Y-%m-%d %H:%M:%S%.f").to_string()),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::TIMESTAMPTZ => match row.try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(idx) {
            Ok(Some(v)) => truncate_text(v.to_rfc3339()),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::DATE => match row.try_get::<_, Option<chrono::NaiveDate>>(idx) {
            Ok(Some(v)) => truncate_text(v.to_string()),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        Type::TIME => match row.try_get::<_, Option<chrono::NaiveTime>>(idx) {
            Ok(Some(v)) => truncate_text(v.to_string()),
            Ok(None) => DbValue::Null,
            Err(_) => DbValue::Null,
        },
        _ => match row.try_get::<_, Option<String>>(idx) {
            Ok(Some(v)) => truncate_text(v),
            Ok(None) => DbValue::Null,
            Err(_) => truncate_text(format!("<{}>", ty.name())),
        },
    }
}

fn json_to_pg_param(v: &serde_json::Value) -> Box<dyn ToSql + Sync + Send> {
    match v {
        serde_json::Value::Null => Box::new(Option::<String>::None),
        serde_json::Value::Bool(b) => Box::new(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Box::new(i)
            } else if let Some(f) = n.as_f64() {
                Box::new(f)
            } else {
                Box::new(n.to_string())
            }
        }
        serde_json::Value::String(s) => Box::new(s.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => Box::new(v.clone()),
    }
}

#[async_trait]
impl DbDriver for PostgresDriver {
    async fn test_connection(&self) -> Result<ConnectionTestResult, DbDriverError> {
        let start = Instant::now();
        let client = self.connect(&self.default_database).await?;
        let row = client
            .query_one("SELECT version()", &[])
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let ver: String = row.get(0);
        let short_ver = ver.split(',').next().unwrap_or(&ver).to_string();
        Ok(ConnectionTestResult {
            ok: true,
            message: format!("Terhubung ke PostgreSQL ({}:{})", self.host, self.port),
            version: Some(short_ver),
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn list_databases(&self) -> Result<Vec<String>, DbDriverError> {
        let client = self.connect(&self.default_database).await?;
        let rows = client
            .query(
                "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname ASC",
                &[],
            )
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.get::<_, String>(0)).collect())
    }

    async fn list_tables(&self, database: &str) -> Result<Vec<TableInfo>, DbDriverError> {
        let client = self.connect(database).await?;
        let rows = client
            .query(
                "SELECT t.table_schema, t.table_name, t.table_type,
                        c.reltuples::bigint
                 FROM information_schema.tables t
                 LEFT JOIN pg_namespace n ON n.nspname = t.table_schema
                 LEFT JOIN pg_class c ON c.relnamespace = n.oid AND c.relname = t.table_name
                 WHERE t.table_schema NOT IN ('pg_catalog', 'information_schema')
                 ORDER BY t.table_schema ASC, t.table_name ASC",
                &[],
            )
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let schema: String = r.get(0);
            let name: String = r.get(1);
            let ttype: String = r.get(2);
            let est: Option<i64> = r.try_get(3).ok().flatten();
            let display = if schema == "public" {
                name
            } else {
                format!("{}.{}", schema, name)
            };
            let kind = if ttype.to_uppercase().contains("VIEW") {
                "view".to_string()
            } else {
                "table".to_string()
            };
            out.push(TableInfo {
                name: display,
                kind,
                row_estimate: est.filter(|v| *v >= 0),
            });
        }
        Ok(out)
    }

    async fn describe_table(
        &self,
        database: &str,
        table: &str,
    ) -> Result<TableSchema, DbDriverError> {
        let client = self.connect(database).await?;
        let (schema, name) = split_pg_table(table);

        let pk_rows = client
            .query(
                "SELECT a.attname
                 FROM pg_index i
                 JOIN pg_class c ON c.oid = i.indrelid
                 JOIN pg_namespace n ON n.oid = c.relnamespace
                 JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
                 WHERE i.indisprimary AND n.nspname = $1 AND c.relname = $2
                 ORDER BY array_position(i.indkey, a.attnum)",
                &[&schema, &name],
            )
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        let primary_keys: Vec<String> = pk_rows.into_iter().map(|r| r.get(0)).collect();
        let pk_set: HashSet<&str> = primary_keys.iter().map(|s| s.as_str()).collect();

        let col_rows = client
            .query(
                "SELECT column_name, data_type, is_nullable, column_default
                 FROM information_schema.columns
                 WHERE table_schema = $1 AND table_name = $2
                 ORDER BY ordinal_position ASC",
                &[&schema, &name],
            )
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;

        let columns: Vec<ColumnSchema> = col_rows
            .into_iter()
            .map(|r| {
                let col_name: String = r.get(0);
                let data_type: String = r.get(1);
                let is_nullable: String = r.get(2);
                let default_value: Option<String> = r.get(3);
                let is_pk = pk_set.contains(col_name.as_str());
                ColumnSchema {
                    name: col_name,
                    data_type,
                    nullable: is_nullable.eq_ignore_ascii_case("YES"),
                    default_value,
                    is_primary_key: is_pk,
                }
            })
            .collect();

        let idx_rows = client
            .query(
                "SELECT i.relname AS index_name, ix.indisunique,
                        array_agg(a.attname ORDER BY array_position(ix.indkey, a.attnum)) AS cols
                 FROM pg_class t
                 JOIN pg_namespace n ON n.oid = t.relnamespace
                 JOIN pg_index ix ON t.oid = ix.indrelid
                 JOIN pg_class i ON i.oid = ix.indexrelid
                 JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
                 WHERE n.nspname = $1 AND t.relname = $2
                 GROUP BY i.relname, ix.indisunique
                 ORDER BY i.relname ASC",
                &[&schema, &name],
            )
            .await
            .unwrap_or_default();

        let indexes: Vec<IndexInfo> = idx_rows
            .into_iter()
            .map(|r| IndexInfo {
                name: r.get(0),
                unique: r.get(1),
                columns: r.try_get::<_, Vec<String>>(2).unwrap_or_default(),
            })
            .collect();

        Ok(TableSchema {
            database: database.to_string(),
            table: table.to_string(),
            columns,
            primary_keys,
            indexes,
        })
    }

    async fn fetch_rows(&self, params: &FetchRowsParams) -> Result<RowsPage, DbDriverError> {
        let start = Instant::now();
        let client = self.connect(&params.database).await?;
        let quoted_table = quote_pg_table(&params.table)?;

        let sort_clause = match &params.sort_col {
            Some(col) if !col.is_empty() => {
                let q = quote_ident(DbEngine::Postgres, col)?;
                let dir = if params.sort_desc { "DESC" } else { "ASC" };
                format!(" ORDER BY {} {}", q, dir)
            }
            _ => String::new(),
        };

        let mut where_sql = String::new();
        let mut filter_str: Option<String> = None;
        if let Some(ref f) = params.filter {
            let qcol = quote_ident(DbEngine::Postgres, &f.column)?;
            match f.op {
                FilterOp::IsNull => where_sql = format!(" WHERE {} IS NULL", qcol),
                FilterOp::IsNotNull => where_sql = format!(" WHERE {} IS NOT NULL", qcol),
                FilterOp::Contains => {
                    where_sql = format!(" WHERE CAST({} AS TEXT) ILIKE $1", qcol);
                    filter_str = Some(format!("%{}%", f.value.clone().unwrap_or_default()));
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
                    where_sql = format!(" WHERE CAST({} AS TEXT) {} $1", qcol, sym);
                    filter_str = Some(f.value.clone().unwrap_or_default());
                }
            }
        }

        let offset = ((params.page.saturating_sub(1)) * params.limit) as i64;
        let fetch_limit = (params.limit + 1) as i64;

        let total_estimate: Option<i64> = if let Some(ref val) = filter_str {
            let sql = format!("SELECT COUNT(*) FROM {}{}", quoted_table, where_sql);
            client.query_one(&sql, &[val]).await.ok().map(|r| r.get(0))
        } else {
            let sql = format!("SELECT COUNT(*) FROM {}{}", quoted_table, where_sql);
            client.query_one(&sql, &[]).await.ok().map(|r| r.get(0))
        };

        let rows = if let Some(ref val) = filter_str {
            let sql = format!(
                "SELECT * FROM {}{}{} LIMIT $2 OFFSET $3",
                quoted_table, where_sql, sort_clause
            );
            client
                .query(&sql, &[val, &fetch_limit, &offset])
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?
        } else {
            let sql = format!(
                "SELECT * FROM {}{}{} LIMIT $1 OFFSET $2",
                quoted_table, where_sql, sort_clause
            );
            client
                .query(&sql, &[&fetch_limit, &offset])
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?
        };

        let columns: Vec<String> = if let Some(first) = rows.first() {
            first.columns().iter().map(|c| c.name().to_string()).collect()
        } else {
            // Ambil nama kolom meski tabel kosong
            let empty_stmt = client
                .prepare(&format!("SELECT * FROM {} LIMIT 0", quoted_table))
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            empty_stmt
                .columns()
                .iter()
                .map(|c| c.name().to_string())
                .collect()
        };

        let has_more = rows.len() as u64 > params.limit;
        let mut out_rows = Vec::with_capacity(rows.len().min(params.limit as usize));
        for row in rows.iter().take(params.limit as usize) {
            let mut vals = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                vals.push(pg_cell_to_db_value(row, i));
            }
            out_rows.push(vals);
        }

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
        let mut client = self.connect(database).await?;
        let ro = self.read_only || read_only;

        if ro {
            let tx = client
                .build_transaction()
                .read_only(true)
                .start()
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            let stmt = tx
                .prepare(trimmed)
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            if stmt.columns().is_empty() {
                let _ = tx.rollback().await;
                return Err(DbDriverError::ReadOnly);
            }
            let columns: Vec<String> = stmt.columns().iter().map(|c| c.name().to_string()).collect();
            let rows = tx
                .query(&stmt, &[])
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            let _ = tx.rollback().await;

            let truncated = rows.len() as u64 > max_rows;
            let mut out_rows = Vec::with_capacity(rows.len().min(max_rows as usize));
            for row in rows.iter().take(max_rows as usize) {
                let mut vals = Vec::with_capacity(columns.len());
                for i in 0..columns.len() {
                    vals.push(pg_cell_to_db_value(row, i));
                }
                out_rows.push(vals);
            }
            return Ok(QueryResult {
                columns,
                rows: out_rows,
                affected_rows: None,
                elapsed_ms: start.elapsed().as_millis() as u64,
                truncated,
                message: None,
            });
        }

        let stmt = client
            .prepare(trimmed)
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        if !stmt.columns().is_empty() {
            let columns: Vec<String> = stmt.columns().iter().map(|c| c.name().to_string()).collect();
            let rows = client
                .query(&stmt, &[])
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            let truncated = rows.len() as u64 > max_rows;
            let mut out_rows = Vec::with_capacity(rows.len().min(max_rows as usize));
            for row in rows.iter().take(max_rows as usize) {
                let mut vals = Vec::with_capacity(columns.len());
                for i in 0..columns.len() {
                    vals.push(pg_cell_to_db_value(row, i));
                }
                out_rows.push(vals);
            }
            Ok(QueryResult {
                columns,
                rows: out_rows,
                affected_rows: None,
                elapsed_ms: start.elapsed().as_millis() as u64,
                truncated,
                message: None,
            })
        } else {
            let affected = client
                .execute(&stmt, &[])
                .await
                .map_err(|e| DbDriverError::Query(e.to_string()))?;
            Ok(QueryResult {
                columns: Vec::new(),
                rows: Vec::new(),
                affected_rows: Some(affected),
                elapsed_ms: start.elapsed().as_millis() as u64,
                truncated: false,
                message: Some(format!("Berhasil ({} baris terpengaruh)", affected)),
            })
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
        let client = self.connect(database).await?;
        let quoted_table = quote_pg_table(table)?;

        let mut cols = Vec::with_capacity(values.len());
        let mut placeholders = Vec::with_capacity(values.len());
        let mut boxed_params: Vec<Box<dyn ToSql + Sync + Send>> = Vec::with_capacity(values.len());
        for (idx, (k, v)) in values.iter().enumerate() {
            cols.push(quote_ident(DbEngine::Postgres, k)?);
            placeholders.push(format!("${}", idx + 1));
            boxed_params.push(json_to_pg_param(v));
        }
        let param_refs: Vec<&(dyn ToSql + Sync)> = boxed_params
            .iter()
            .map(|b| &**b as &(dyn ToSql + Sync))
            .collect();

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quoted_table,
            cols.join(", "),
            placeholders.join(", ")
        );
        let affected = client
            .execute(&sql, &param_refs)
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
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
        let client = self.connect(database).await?;
        let quoted_table = quote_pg_table(table)?;

        let mut sets = Vec::with_capacity(values.len());
        let mut boxed_params: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();
        for (k, v) in values {
            boxed_params.push(json_to_pg_param(v));
            sets.push(format!(
                "{} = ${}",
                quote_ident(DbEngine::Postgres, k)?,
                boxed_params.len()
            ));
        }
        let mut wheres = Vec::with_capacity(pk.len());
        for (k, v) in pk {
            let q = quote_ident(DbEngine::Postgres, k)?;
            if v.is_null() {
                wheres.push(format!("{} IS NULL", q));
            } else {
                boxed_params.push(json_to_pg_param(v));
                wheres.push(format!("{} = ${}", q, boxed_params.len()));
            }
        }
        let param_refs: Vec<&(dyn ToSql + Sync)> = boxed_params
            .iter()
            .map(|b| &**b as &(dyn ToSql + Sync))
            .collect();

        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            quoted_table,
            sets.join(", "),
            wheres.join(" AND ")
        );
        let affected = client
            .execute(&sql, &param_refs)
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
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
        let client = self.connect(database).await?;
        let quoted_table = quote_pg_table(table)?;

        let mut wheres = Vec::with_capacity(pk.len());
        let mut boxed_params: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();
        for (k, v) in pk {
            let q = quote_ident(DbEngine::Postgres, k)?;
            if v.is_null() {
                wheres.push(format!("{} IS NULL", q));
            } else {
                boxed_params.push(json_to_pg_param(v));
                wheres.push(format!("{} = ${}", q, boxed_params.len()));
            }
        }
        let param_refs: Vec<&(dyn ToSql + Sync)> = boxed_params
            .iter()
            .map(|b| &**b as &(dyn ToSql + Sync))
            .collect();

        let sql = format!("DELETE FROM {} WHERE {}", quoted_table, wheres.join(" AND "));
        let affected = client
            .execute(&sql, &param_refs)
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        Ok(RowMutationResult {
            affected_rows: affected,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn create_database(&self, name: &str) -> Result<(), DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        let client = self.connect(&self.default_database).await?;
        let q = quote_ident(DbEngine::Postgres, name)?;
        client
            .batch_execute(&format!("CREATE DATABASE {}", q))
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        Ok(())
    }

    async fn drop_database(&self, name: &str) -> Result<(), DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        let client = self.connect(&self.default_database).await?;
        let q = quote_ident(DbEngine::Postgres, name)?;
        client
            .batch_execute(&format!("DROP DATABASE {}", q))
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
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
        let client = self.connect(&req.db).await?;
        let quoted_table = quote_pg_table(&req.table)?;
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
            let qname = quote_ident(DbEngine::Postgres, &col.name)?;
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
        client
            .batch_execute(&sql)
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        Ok(())
    }

    async fn drop_table(&self, database: &str, table: &str) -> Result<(), DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        let client = self.connect(database).await?;
        let quoted_table = quote_pg_table(table)?;
        client
            .batch_execute(&format!("DROP TABLE {}", quoted_table))
            .await
            .map_err(|e| DbDriverError::Query(e.to_string()))?;
        Ok(())
    }
}
