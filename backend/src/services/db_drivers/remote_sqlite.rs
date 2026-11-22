use super::{
    quote_ident, validate_column_type, validate_default_expr, DbDriver, DbDriverError,
    DEFAULT_QUERY_TIMEOUT_SECS,
};
use crate::models::db_client::{
    ConnectionTestResult, CreateTableRequest, DbEngine, FetchRowsParams, FilterOp, QueryResult,
    RowMutationResult, RowsPage, TableInfo, TableSchema,
};
use crate::models::server::DecryptedServerCreds;
use crate::services::ssh_pool::SshPool;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use std::collections::BTreeMap;

/// Script Python 3 (stdlib `sqlite3`) yang dijalankan di server remote via SSH.
/// Seluruh parameter (file_path, read_only, op, SQL/values/pk) dikirim lewat stdin JSON
/// agar bebas dari shell/command injection dan aman untuk database live (termasuk WAL).
pub const REMOTE_SQLITE_AGENT_PY: &str = r#"
import json
import os
import sqlite3
import sys
import time
import urllib.parse

MAX_CELL_CHARS = 65536
MAX_BYTES_PREVIEW = 64

def qident(name):
    return '"' + str(name).replace('"', '""') + '"'

def py_to_db_value(v):
    if v is None:
        return {"type": "null"}
    if isinstance(v, bool):
        return {"type": "bool", "value": v}
    if isinstance(v, int):
        return {"type": "int", "value": v}
    if isinstance(v, float):
        return {"type": "float", "value": v}
    if isinstance(v, (bytes, bytearray, memoryview)):
        b = bytes(v)
        return {
            "type": "bytes",
            "value": {
                "size": len(b),
                "preview_hex": b[:MAX_BYTES_PREVIEW].hex(),
            },
        }
    s = str(v)
    if len(s) > MAX_CELL_CHARS:
        return {"type": "text", "value": {"value": s[:MAX_CELL_CHARS], "truncated": True}}
    return {"type": "text", "value": {"value": s, "truncated": False}}

def json_to_param(v):
    if v is None:
        return None
    if isinstance(v, bool):
        return 1 if v else 0
    if isinstance(v, (int, float, str)):
        return v
    return json.dumps(v)

def open_conn(path, read_only):
    if read_only:
        if not os.path.exists(path):
            raise RuntimeError(f"File SQLite tidak ditemukan: {path}")
        encoded = urllib.parse.quote(path, safe="/")
        conn = sqlite3.connect(f"file:{encoded}?mode=ro", uri=True, timeout=5.0)
    else:
        conn = sqlite3.connect(path, timeout=5.0)
    return conn

def handle(req):
    path = req["file_path"]
    read_only = bool(req.get("read_only", False))
    op = req["op"]
    t0 = time.monotonic()

    conn = open_conn(path, read_only)
    try:
        cur = conn.cursor()

        if op == "test_connection":
            ver = cur.execute("SELECT sqlite_version()").fetchone()[0]
            elapsed = int((time.monotonic() - t0) * 1000)
            return {
                "ok": True,
                "message": "Koneksi Remote SQLite berhasil",
                "version": f"SQLite {ver} (SSH)",
                "elapsed_ms": elapsed,
            }

        if op == "list_tables":
            rows = cur.execute(
                "SELECT name, type FROM sqlite_master "
                "WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' "
                "ORDER BY type ASC, name ASC"
            ).fetchall()
            out = []
            for name, kind in rows:
                out.append({
                    "name": name,
                    "kind": "view" if kind == "view" else "table",
                    "row_estimate": None,
                })
            return out

        if op == "describe_table":
            table = req["table"]
            qt = qident(table)
            col_rows = cur.execute(f"PRAGMA table_info({qt})").fetchall()
            if not col_rows:
                raise RuntimeError(f"Tabel '{table}' tidak ditemukan")
            columns = []
            pk_pairs = []
            for cid, name, dtype, notnull, dflt, pk in col_rows:
                columns.append({
                    "name": name,
                    "data_type": dtype or "ANY",
                    "nullable": not bool(notnull),
                    "default_value": None if dflt is None else str(dflt),
                    "is_primary_key": pk > 0,
                })
                if pk > 0:
                    pk_pairs.append((pk, name))
            pk_pairs.sort(key=lambda x: x[0])
            primary_keys = [name for _, name in pk_pairs]

            indexes = []
            idx_rows = cur.execute(f"PRAGMA index_list({qt})").fetchall()
            for idx in idx_rows:
                idx_name = idx[1]
                unique = bool(idx[2])
                info_rows = cur.execute(f"PRAGMA index_info({qident(idx_name)})").fetchall()
                idx_cols = [r[2] for r in info_rows if r[2] is not None]
                indexes.append({
                    "name": idx_name,
                    "columns": idx_cols,
                    "unique": unique,
                })
            return {
                "database": "main",
                "table": table,
                "columns": columns,
                "primary_keys": primary_keys,
                "indexes": indexes,
            }

        if op == "fetch_rows":
            table = req["table"]
            page = max(1, int(req.get("page", 1)))
            limit = max(1, min(1000, int(req.get("limit", 100))))
            offset = (page - 1) * limit
            qt = qident(table)

            where_sql = ""
            params = []
            flt = req.get("filter")
            if flt:
                col_q = qident(flt["column"])
                fop = flt["op"]
                fval = flt.get("value") or ""
                if fop == "eq":
                    where_sql = f" WHERE {col_q} = ?"
                    params.append(fval)
                elif fop == "neq":
                    where_sql = f" WHERE {col_q} != ?"
                    params.append(fval)
                elif fop == "contains":
                    where_sql = f" WHERE CAST({col_q} AS TEXT) LIKE ?"
                    params.append(f"%{fval}%")
                elif fop == "gt":
                    where_sql = f" WHERE {col_q} > ?"
                    params.append(fval)
                elif fop == "gte":
                    where_sql = f" WHERE {col_q} >= ?"
                    params.append(fval)
                elif fop == "lt":
                    where_sql = f" WHERE {col_q} < ?"
                    params.append(fval)
                elif fop == "lte":
                    where_sql = f" WHERE {col_q} <= ?"
                    params.append(fval)
                elif fop == "is_null":
                    where_sql = f" WHERE {col_q} IS NULL"
                elif fop == "is_not_null":
                    where_sql = f" WHERE {col_q} IS NOT NULL"

            order_sql = ""
            sort_col = req.get("sort_col")
            if sort_col:
                direction = "DESC" if req.get("sort_desc") else "ASC"
                order_sql = f" ORDER BY {qident(sort_col)} {direction}"

            total = cur.execute(f"SELECT COUNT(*) FROM {qt}{where_sql}", params).fetchone()[0]
            data_sql = f"SELECT * FROM {qt}{where_sql}{order_sql} LIMIT ? OFFSET ?"
            cur.execute(data_sql, params + [limit + 1, offset])
            cols = [d[0] for d in (cur.description or [])]
            fetched = cur.fetchall()
            has_more = len(fetched) > limit
            if has_more:
                fetched = fetched[:limit]
            rows = [[py_to_db_value(cell) for cell in r] for r in fetched]
            elapsed = int((time.monotonic() - t0) * 1000)
            return {
                "columns": cols,
                "rows": rows,
                "page": page,
                "limit": limit,
                "total_estimate": total,
                "has_more": has_more,
                "elapsed_ms": elapsed,
            }

        if op == "execute_query":
            sql = req["sql"]
            max_rows = max(1, min(1000, int(req.get("max_rows", 500))))
            cur.execute(sql)
            if cur.description is not None:
                cols = [d[0] for d in cur.description]
                fetched = cur.fetchmany(max_rows + 1)
                truncated = len(fetched) > max_rows
                if truncated:
                    fetched = fetched[:max_rows]
                rows = [[py_to_db_value(cell) for cell in r] for r in fetched]
                elapsed = int((time.monotonic() - t0) * 1000)
                return {
                    "columns": cols,
                    "rows": rows,
                    "affected_rows": None,
                    "elapsed_ms": elapsed,
                    "truncated": truncated,
                    "message": None,
                }
            else:
                conn.commit()
                affected = cur.rowcount if cur.rowcount is not None and cur.rowcount >= 0 else 0
                elapsed = int((time.monotonic() - t0) * 1000)
                return {
                    "columns": [],
                    "rows": [],
                    "affected_rows": affected,
                    "elapsed_ms": elapsed,
                    "truncated": False,
                    "message": f"{affected} baris terpengaruh",
                }

        if op == "insert_row":
            if read_only:
                raise RuntimeError("Operasi ditolak: koneksi berada dalam mode Read-Only")
            table = req["table"]
            values = req["values"]
            cols = list(values.keys())
            col_sql = ", ".join(qident(c) for c in cols)
            ph_sql = ", ".join("?" for _ in cols)
            params = [json_to_param(values[c]) for c in cols]
            cur.execute(f"INSERT INTO {qident(table)} ({col_sql}) VALUES ({ph_sql})", params)
            conn.commit()
            elapsed = int((time.monotonic() - t0) * 1000)
            return {"affected_rows": max(0, cur.rowcount), "elapsed_ms": elapsed}

        if op == "update_row":
            if read_only:
                raise RuntimeError("Operasi ditolak: koneksi berada dalam mode Read-Only")
            table = req["table"]
            pk = req["pk"]
            values = req["values"]
            set_cols = list(values.keys())
            pk_cols = list(pk.keys())
            set_sql = ", ".join(f"{qident(c)} = ?" for c in set_cols)
            where_parts = []
            params = [json_to_param(values[c]) for c in set_cols]
            for c in pk_cols:
                if pk[c] is None:
                    where_parts.append(f"{qident(c)} IS NULL")
                else:
                    where_parts.append(f"{qident(c)} = ?")
                    params.append(json_to_param(pk[c]))
            where_sql = " AND ".join(where_parts)
            cur.execute(f"UPDATE {qident(table)} SET {set_sql} WHERE {where_sql}", params)
            conn.commit()
            elapsed = int((time.monotonic() - t0) * 1000)
            return {"affected_rows": max(0, cur.rowcount), "elapsed_ms": elapsed}

        if op == "delete_row":
            if read_only:
                raise RuntimeError("Operasi ditolak: koneksi berada dalam mode Read-Only")
            table = req["table"]
            pk = req["pk"]
            where_parts = []
            params = []
            for c, v in pk.items():
                if v is None:
                    where_parts.append(f"{qident(c)} IS NULL")
                else:
                    where_parts.append(f"{qident(c)} = ?")
                    params.append(json_to_param(v))
            where_sql = " AND ".join(where_parts)
            cur.execute(f"DELETE FROM {qident(table)} WHERE {where_sql}", params)
            conn.commit()
            elapsed = int((time.monotonic() - t0) * 1000)
            return {"affected_rows": max(0, cur.rowcount), "elapsed_ms": elapsed}

        if op == "create_table":
            if read_only:
                raise RuntimeError("Operasi ditolak: koneksi berada dalam mode Read-Only")
            cur.execute(req["ddl"])
            conn.commit()
            return {"ok": True}

        if op == "drop_table":
            if read_only:
                raise RuntimeError("Operasi ditolak: koneksi berada dalam mode Read-Only")
            table = req["table"]
            cur.execute(f"DROP TABLE {qident(table)}")
            conn.commit()
            return {"ok": True}

        raise RuntimeError(f"Operasi tidak dikenal: {op}")
    finally:
        conn.close()

def main():
    try:
        raw = sys.stdin.read()
        req = json.loads(raw)
        data = handle(req)
        sys.stdout.write(json.dumps({"ok": True, "data": data}))
    except Exception as e:
        sys.stdout.write(json.dumps({"ok": False, "error": str(e)}))

if __name__ == "__main__":
    main()
"#;

#[derive(Deserialize)]
struct AgentResponse<T> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

pub struct RemoteSqliteDriver {
    ssh_pool: SshPool,
    server_creds: DecryptedServerCreds,
    file_path: String,
    read_only: bool,
}

impl RemoteSqliteDriver {
    pub fn new(
        ssh_pool: SshPool,
        server_creds: DecryptedServerCreds,
        file_path: String,
        read_only: bool,
    ) -> Result<Self, DbDriverError> {
        let trimmed = file_path.trim();
        if trimmed.is_empty() || trimmed.contains('\0') {
            return Err(DbDriverError::InvalidInput(
                "Path file SQLite tidak boleh kosong atau mengandung karakter tidak valid".into(),
            ));
        }
        Ok(Self {
            ssh_pool,
            server_creds,
            file_path: trimmed.to_string(),
            read_only,
        })
    }

    async fn call_agent<T: DeserializeOwned>(
        &self,
        mut payload: JsonValue,
        override_read_only: Option<bool>,
    ) -> Result<T, DbDriverError> {
        let ro = override_read_only.unwrap_or(self.read_only);
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("file_path".to_string(), json!(self.file_path));
            obj.insert("read_only".to_string(), json!(ro));
        }
        let stdin_bytes = serde_json::to_vec(&payload)
            .map_err(|e| DbDriverError::InvalidInput(e.to_string()))?;

        let cmd = format!(
            "python3 -u -c '{}'",
            REMOTE_SQLITE_AGENT_PY.replace('\'', "'\\''")
        );
        let out = self
            .ssh_pool
            .exec_capture(&self.server_creds, &cmd, &stdin_bytes, DEFAULT_QUERY_TIMEOUT_SECS)
            .await
            .map_err(|e| DbDriverError::Tunnel(e.to_string()))?;

        if !out.success() {
            let msg = out.failure_message();
            if msg.contains("python3") && msg.contains("not found") {
                return Err(DbDriverError::Connection(
                    "python3 tidak ditemukan di server remote (dibutuhkan untuk Remote SQLite via SSH)"
                        .into(),
                ));
            }
            return Err(DbDriverError::Connection(msg));
        }

        let stdout = out.stdout_text();
        let parsed: AgentResponse<T> = serde_json::from_str(stdout.trim()).map_err(|e| {
            DbDriverError::Query(format!("Respons agent Remote SQLite tidak valid: {}", e))
        })?;

        if parsed.ok {
            parsed.data.ok_or_else(|| {
                DbDriverError::Query("Respons agent Remote SQLite tidak memiliki data".into())
            })
        } else {
            let err_msg = parsed.error.unwrap_or_else(|| "Remote SQLite error".into());
            if err_msg.to_lowercase().contains("readonly") || err_msg.contains("Read-Only") {
                Err(DbDriverError::Query(err_msg))
            } else {
                Err(DbDriverError::Query(err_msg))
            }
        }
    }
}

fn filter_op_str(op: FilterOp) -> &'static str {
    match op {
        FilterOp::Eq => "eq",
        FilterOp::Neq => "neq",
        FilterOp::Contains => "contains",
        FilterOp::Gt => "gt",
        FilterOp::Gte => "gte",
        FilterOp::Lt => "lt",
        FilterOp::Lte => "lte",
        FilterOp::IsNull => "is_null",
        FilterOp::IsNotNull => "is_not_null",
    }
}

#[async_trait]
impl DbDriver for RemoteSqliteDriver {
    async fn test_connection(&self) -> Result<ConnectionTestResult, DbDriverError> {
        self.call_agent(json!({ "op": "test_connection" }), None)
            .await
    }

    async fn list_databases(&self) -> Result<Vec<String>, DbDriverError> {
        Ok(vec!["main".to_string()])
    }

    async fn list_tables(
        &self,
        _database: &str,
        _schema: Option<&str>,
    ) -> Result<Vec<TableInfo>, DbDriverError> {
        self.call_agent(json!({ "op": "list_tables" }), None).await
    }

    async fn describe_table(
        &self,
        _database: &str,
        table: &str,
    ) -> Result<TableSchema, DbDriverError> {
        let _ = quote_ident(DbEngine::Sqlite, table)?;
        self.call_agent(json!({ "op": "describe_table", "table": table }), None)
            .await
    }

    async fn fetch_rows(&self, params: &FetchRowsParams) -> Result<RowsPage, DbDriverError> {
        let _ = quote_ident(DbEngine::Sqlite, &params.table)?;
        if let Some(ref sc) = params.sort_col {
            if !sc.trim().is_empty() {
                let _ = quote_ident(DbEngine::Sqlite, sc.trim())?;
            }
        }
        let filter_json = if let Some(ref f) = params.filter {
            let _ = quote_ident(DbEngine::Sqlite, &f.column)?;
            json!({
                "column": f.column,
                "op": filter_op_str(f.op),
                "value": f.value,
            })
        } else {
            JsonValue::Null
        };

        self.call_agent(
            json!({
                "op": "fetch_rows",
                "table": params.table,
                "page": params.page,
                "limit": params.limit,
                "sort_col": params.sort_col.as_deref().filter(|s| !s.trim().is_empty()),
                "sort_desc": params.sort_desc,
                "filter": filter_json,
            }),
            None,
        )
        .await
    }

    async fn execute_query(
        &self,
        _database: &str,
        sql: &str,
        read_only: bool,
        max_rows: u64,
    ) -> Result<QueryResult, DbDriverError> {
        let trimmed = sql.trim();
        if trimmed.is_empty() {
            return Err(DbDriverError::InvalidInput("Query SQL tidak boleh kosong".into()));
        }
        let effective_ro = self.read_only || read_only;
        self.call_agent(
            json!({
                "op": "execute_query",
                "sql": trimmed,
                "max_rows": max_rows,
            }),
            Some(effective_ro),
        )
        .await
    }

    async fn insert_row(
        &self,
        _database: &str,
        table: &str,
        values: &BTreeMap<String, JsonValue>,
    ) -> Result<RowMutationResult, DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if values.is_empty() {
            return Err(DbDriverError::InvalidInput("Nilai kolom tidak boleh kosong".into()));
        }
        let _ = quote_ident(DbEngine::Sqlite, table)?;
        for col in values.keys() {
            let _ = quote_ident(DbEngine::Sqlite, col)?;
        }
        self.call_agent(
            json!({
                "op": "insert_row",
                "table": table,
                "values": values,
            }),
            Some(false),
        )
        .await
    }

    async fn update_row(
        &self,
        _database: &str,
        table: &str,
        pk: &BTreeMap<String, JsonValue>,
        values: &BTreeMap<String, JsonValue>,
    ) -> Result<RowMutationResult, DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if pk.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Primary key wajib diisi untuk mengubah baris".into(),
            ));
        }
        if values.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Tidak ada kolom yang diubah".into(),
            ));
        }
        let _ = quote_ident(DbEngine::Sqlite, table)?;
        for col in pk.keys().chain(values.keys()) {
            let _ = quote_ident(DbEngine::Sqlite, col)?;
        }
        self.call_agent(
            json!({
                "op": "update_row",
                "table": table,
                "pk": pk,
                "values": values,
            }),
            Some(false),
        )
        .await
    }

    async fn delete_row(
        &self,
        _database: &str,
        table: &str,
        pk: &BTreeMap<String, JsonValue>,
    ) -> Result<RowMutationResult, DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if pk.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Primary key wajib diisi untuk menghapus baris".into(),
            ));
        }
        let _ = quote_ident(DbEngine::Sqlite, table)?;
        for col in pk.keys() {
            let _ = quote_ident(DbEngine::Sqlite, col)?;
        }
        self.call_agent(
            json!({
                "op": "delete_row",
                "table": table,
                "pk": pk,
            }),
            Some(false),
        )
        .await
    }

    async fn create_database(&self, _name: &str) -> Result<(), DbDriverError> {
        Err(DbDriverError::Unsupported(
            "SQLite berbasis file; buat koneksi baru untuk file database lain".into(),
        ))
    }

    async fn drop_database(&self, _name: &str) -> Result<(), DbDriverError> {
        Err(DbDriverError::Unsupported(
            "SQLite berbasis file; hapus file langsung dari File Manager".into(),
        ))
    }

    async fn create_table(&self, req: &CreateTableRequest) -> Result<(), DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        if req.columns.is_empty() {
            return Err(DbDriverError::InvalidInput(
                "Minimal 1 kolom diperlukan untuk membuat tabel".into(),
            ));
        }
        let qtable = quote_ident(DbEngine::Sqlite, &req.table)?;
        let mut col_defs = Vec::with_capacity(req.columns.len());
        let pk_cols: Vec<&str> = req
            .columns
            .iter()
            .filter(|c| c.primary_key)
            .map(|c| c.name.as_str())
            .collect();
        let single_pk = pk_cols.len() == 1;

        for c in &req.columns {
            let qc = quote_ident(DbEngine::Sqlite, &c.name)?;
            let dt = validate_column_type(&c.data_type)?;
            let mut def = format!("{} {}", qc, dt);
            if c.primary_key && single_pk {
                def.push_str(" PRIMARY KEY");
            }
            if !c.nullable && !c.primary_key {
                def.push_str(" NOT NULL");
            }
            if let Some(ref dflt) = c.default_value {
                if !dflt.trim().is_empty() {
                    let valid_dflt = validate_default_expr(dflt)?;
                    def.push_str(&format!(" DEFAULT {}", valid_dflt));
                }
            }
            col_defs.push(def);
        }
        if pk_cols.len() > 1 {
            let quoted_pks: Result<Vec<String>, DbDriverError> = pk_cols
                .iter()
                .map(|c| quote_ident(DbEngine::Sqlite, c))
                .collect();
            col_defs.push(format!("PRIMARY KEY ({})", quoted_pks?.join(", ")));
        }

        let ddl = format!("CREATE TABLE {} ({})", qtable, col_defs.join(", "));
        let _: JsonValue = self
            .call_agent(json!({ "op": "create_table", "ddl": ddl }), Some(false))
            .await?;
        Ok(())
    }

    async fn drop_table(&self, _database: &str, table: &str) -> Result<(), DbDriverError> {
        if self.read_only {
            return Err(DbDriverError::ReadOnly);
        }
        let _ = quote_ident(DbEngine::Sqlite, table)?;
        let _: JsonValue = self
            .call_agent(json!({ "op": "drop_table", "table": table }), Some(false))
            .await?;
        Ok(())
    }
}
