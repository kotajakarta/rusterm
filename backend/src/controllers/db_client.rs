use crate::controllers::servers::{check_auth, AppState};
use crate::models::db::DbError;
use crate::models::db_client::{
    ConnectionTestResult, CreateDatabaseRequest, CreateTableRequest, DbConnectionInput,
    DbConnectionOutput, DecryptedDbConnection, ExecuteQueryRequest, FetchRowsParams, FilterOp,
    MutationOp, QueryResult, RowFilter, RowMutationRequest, RowMutationResult, RowsPage, TableInfo,
    TableSchema, TestDbConnectionInput,
};
use crate::services::db_drivers::{clamp_limit, clamp_page, DbDriverError};
use crate::services::db_pool::DbClientPool;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct DbClientAppState {
    pub app: AppState,
    pub db_pool: DbClientPool,
}

type ApiErr = (StatusCode, Json<Value>);

fn err_json(status: StatusCode, msg: impl Into<String>) -> ApiErr {
    (status, Json(json!({ "error": msg.into() })))
}

fn map_driver_err(err: DbDriverError) -> ApiErr {
    let status = match &err {
        DbDriverError::InvalidInput(_) => StatusCode::BAD_REQUEST,
        DbDriverError::ReadOnly => StatusCode::FORBIDDEN,
        DbDriverError::Unsupported(_) => StatusCode::BAD_REQUEST,
        DbDriverError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
        DbDriverError::Connection(_) | DbDriverError::Tunnel(_) => StatusCode::BAD_GATEWAY,
        DbDriverError::Query(_) => StatusCode::BAD_REQUEST,
    };
    err_json(status, err.to_string())
}

fn map_db_err(err: DbError) -> ApiErr {
    match err {
        DbError::NotFound => err_json(StatusCode::NOT_FOUND, "Koneksi database tidak ditemukan"),
        other => err_json(StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
    }
}

async fn require_auth_and_kek(
    headers: &HeaderMap,
    state: &DbClientAppState,
) -> Result<zeroize::Zeroizing<[u8; 32]>, ApiErr> {
    check_auth(headers, &state.app.vault)
        .await
        .map_err(|s| err_json(s, "Unauthorized"))?;
    state
        .app
        .vault
        .get_kek()
        .await
        .map_err(|_| err_json(StatusCode::UNAUTHORIZED, "Vault terkunci"))
}

pub fn router(state: DbClientAppState) -> Router {
    Router::new()
        .route("/db/connections", get(list_connections).post(create_connection))
        .route("/db/connections/test", post(test_connection))
        .route(
            "/db/connections/:id",
            get(get_connection)
                .put(update_connection)
                .delete(delete_connection),
        )
        .route(
            "/db/:id/databases",
            get(list_databases).post(create_database),
        )
        .route("/db/:id/databases/:db_name", delete(drop_database))
        .route("/db/:id/tables", get(list_tables).post(create_table))
        .route("/db/:id/tables/:table", delete(drop_table))
        .route("/db/:id/tables/:table/schema", get(describe_table))
        .route(
            "/db/:id/tables/:table/rows",
            get(fetch_rows).post(mutate_row),
        )
        .route("/db/:id/query", post(execute_query))
        .with_state(state)
}

async fn list_connections(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DbConnectionOutput>>, ApiErr> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| err_json(s, "Unauthorized"))?;
    state
        .app
        .db
        .list_db_connections()
        .map(Json)
        .map_err(map_db_err)
}

async fn get_connection(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<DbConnectionOutput>, ApiErr> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| err_json(s, "Unauthorized"))?;
    state
        .app
        .db
        .get_db_connection(id)
        .map(Json)
        .map_err(map_db_err)
}

fn validate_connection_input(input: &DbConnectionInput) -> Result<(), ApiErr> {
    if input.name.trim().is_empty() {
        return Err(err_json(StatusCode::BAD_REQUEST, "Nama koneksi wajib diisi"));
    }
    match input.engine {
        crate::models::db_client::DbEngine::Sqlite => {
            if input.file_path.trim().is_empty() {
                return Err(err_json(
                    StatusCode::BAD_REQUEST,
                    "Path file SQLite wajib diisi",
                ));
            }
        }
        crate::models::db_client::DbEngine::Postgres
        | crate::models::db_client::DbEngine::Mysql => {
            if input.host.trim().is_empty() {
                return Err(err_json(StatusCode::BAD_REQUEST, "Host database wajib diisi"));
            }
        }
    }
    Ok(())
}

async fn create_connection(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Json(input): Json<DbConnectionInput>,
) -> Result<Json<DbConnectionOutput>, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    validate_connection_input(&input)?;
    let id = state
        .app
        .db
        .create_db_connection(&input, &kek)
        .map_err(map_db_err)?;
    state
        .app
        .db
        .get_db_connection(id)
        .map(Json)
        .map_err(map_db_err)
}

async fn update_connection(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<DbConnectionInput>,
) -> Result<Json<DbConnectionOutput>, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    validate_connection_input(&input)?;
    state
        .app
        .db
        .update_db_connection(id, &input, &kek)
        .map_err(map_db_err)?;
    state
        .app
        .db
        .get_db_connection(id)
        .map(Json)
        .map_err(map_db_err)
}

async fn delete_connection(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiErr> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| err_json(s, "Unauthorized"))?;
    state
        .app
        .db
        .delete_db_connection(id)
        .map_err(map_db_err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn test_connection(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Json(input): Json<TestDbConnectionInput>,
) -> Result<Json<ConnectionTestResult>, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;

    let password = match (&input.password, input.id) {
        (Some(pw), _) if !pw.is_empty() => Some(pw.clone()),
        (Some(_), _) => None,
        (None, Some(existing_id)) => state
            .app
            .db
            .get_db_connection_decrypted(existing_id, &kek)
            .ok()
            .and_then(|c| c.password),
        (None, None) => None,
    };

    let dec = DecryptedDbConnection {
        id: input.id.unwrap_or(0),
        name: "test".to_string(),
        engine: input.engine,
        host: input.host,
        port: if input.port == 0 {
            input.engine.default_port()
        } else {
            input.port
        },
        username: input.username,
        password,
        default_database: input.default_database,
        file_path: input.file_path,
        ssl_mode: input.ssl_mode,
        ssh_server_id: input.ssh_server_id,
        read_only: input.read_only,
    };

    let driver = state
        .db_pool
        .build_driver(&dec, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;

    let res = DbClientPool::with_timeout(driver.test_connection())
        .await
        .map_err(map_driver_err)?;
    Ok(Json(res))
}

#[derive(Deserialize, Default)]
struct DbQueryParam {
    #[serde(default)]
    db: String,
}

#[derive(Deserialize, Default)]
struct RowsQueryParams {
    #[serde(default)]
    db: String,
    page: Option<u64>,
    limit: Option<u64>,
    sort: Option<String>,
    dir: Option<String>,
    filter_col: Option<String>,
    filter_op: Option<String>,
    filter_val: Option<String>,
}

async fn list_databases(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Vec<String>>, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    let conn = state
        .app
        .db
        .get_db_connection_decrypted(id, &kek)
        .map_err(map_db_err)?;
    let driver = state
        .db_pool
        .build_driver(&conn, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;
    let dbs = DbClientPool::with_timeout(driver.list_databases())
        .await
        .map_err(map_driver_err)?;
    Ok(Json(dbs))
}

async fn create_database(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(req): Json<CreateDatabaseRequest>,
) -> Result<StatusCode, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    let conn = state
        .app
        .db
        .get_db_connection_decrypted(id, &kek)
        .map_err(map_db_err)?;
    if conn.read_only {
        return Err(map_driver_err(DbDriverError::ReadOnly));
    }
    let driver = state
        .db_pool
        .build_driver(&conn, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;
    DbClientPool::with_timeout(driver.create_database(&req.name))
        .await
        .map_err(map_driver_err)?;
    Ok(StatusCode::CREATED)
}

async fn drop_database(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path((id, db_name)): Path<(i64, String)>,
) -> Result<StatusCode, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    let conn = state
        .app
        .db
        .get_db_connection_decrypted(id, &kek)
        .map_err(map_db_err)?;
    if conn.read_only {
        return Err(map_driver_err(DbDriverError::ReadOnly));
    }
    let driver = state
        .db_pool
        .build_driver(&conn, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;
    DbClientPool::with_timeout(driver.drop_database(&db_name))
        .await
        .map_err(map_driver_err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_tables(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<DbQueryParam>,
) -> Result<Json<Vec<TableInfo>>, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    let conn = state
        .app
        .db
        .get_db_connection_decrypted(id, &kek)
        .map_err(map_db_err)?;
    let driver = state
        .db_pool
        .build_driver(&conn, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;
    let tables = DbClientPool::with_timeout(driver.list_tables(&q.db))
        .await
        .map_err(map_driver_err)?;
    Ok(Json(tables))
}

async fn create_table(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(req): Json<CreateTableRequest>,
) -> Result<StatusCode, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    let conn = state
        .app
        .db
        .get_db_connection_decrypted(id, &kek)
        .map_err(map_db_err)?;
    if conn.read_only {
        return Err(map_driver_err(DbDriverError::ReadOnly));
    }
    let driver = state
        .db_pool
        .build_driver(&conn, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;
    DbClientPool::with_timeout(driver.create_table(&req))
        .await
        .map_err(map_driver_err)?;
    Ok(StatusCode::CREATED)
}

async fn drop_table(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path((id, table)): Path<(i64, String)>,
    Query(q): Query<DbQueryParam>,
) -> Result<StatusCode, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    let conn = state
        .app
        .db
        .get_db_connection_decrypted(id, &kek)
        .map_err(map_db_err)?;
    if conn.read_only {
        return Err(map_driver_err(DbDriverError::ReadOnly));
    }
    let driver = state
        .db_pool
        .build_driver(&conn, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;
    DbClientPool::with_timeout(driver.drop_table(&q.db, &table))
        .await
        .map_err(map_driver_err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn describe_table(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path((id, table)): Path<(i64, String)>,
    Query(q): Query<DbQueryParam>,
) -> Result<Json<TableSchema>, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    let conn = state
        .app
        .db
        .get_db_connection_decrypted(id, &kek)
        .map_err(map_db_err)?;
    let driver = state
        .db_pool
        .build_driver(&conn, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;
    let schema = DbClientPool::with_timeout(driver.describe_table(&q.db, &table))
        .await
        .map_err(map_driver_err)?;
    Ok(Json(schema))
}

async fn fetch_rows(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path((id, table)): Path<(i64, String)>,
    Query(q): Query<RowsQueryParams>,
) -> Result<Json<RowsPage>, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    let conn = state
        .app
        .db
        .get_db_connection_decrypted(id, &kek)
        .map_err(map_db_err)?;
    let driver = state
        .db_pool
        .build_driver(&conn, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;

    let filter = match (q.filter_col.as_deref(), q.filter_op.as_deref()) {
        (Some(col), Some(op_str)) if !col.trim().is_empty() => {
            let op = FilterOp::from_str_checked(op_str).ok_or_else(|| {
                err_json(StatusCode::BAD_REQUEST, format!("Operator filter '{}' tidak dikenal", op_str))
            })?;
            Some(RowFilter {
                column: col.to_string(),
                op,
                value: q.filter_val,
            })
        }
        _ => None,
    };

    let params = FetchRowsParams {
        database: q.db,
        table,
        page: clamp_page(q.page),
        limit: clamp_limit(q.limit),
        sort_col: q.sort.filter(|s| !s.trim().is_empty()),
        sort_desc: q
            .dir
            .as_deref()
            .map(|d| d.eq_ignore_ascii_case("desc"))
            .unwrap_or(false),
        filter,
    };

    let page = DbClientPool::with_timeout(driver.fetch_rows(&params))
        .await
        .map_err(map_driver_err)?;
    Ok(Json(page))
}

async fn mutate_row(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path((id, table)): Path<(i64, String)>,
    Json(req): Json<RowMutationRequest>,
) -> Result<Json<RowMutationResult>, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    let conn = state
        .app
        .db
        .get_db_connection_decrypted(id, &kek)
        .map_err(map_db_err)?;
    if conn.read_only {
        return Err(map_driver_err(DbDriverError::ReadOnly));
    }
    let driver = state
        .db_pool
        .build_driver(&conn, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;

    let result = match req.op {
        MutationOp::Insert => {
            DbClientPool::with_timeout(driver.insert_row(&req.db, &table, &req.values)).await
        }
        MutationOp::Update => {
            DbClientPool::with_timeout(driver.update_row(&req.db, &table, &req.pk, &req.values))
                .await
        }
        MutationOp::Delete => {
            DbClientPool::with_timeout(driver.delete_row(&req.db, &table, &req.pk)).await
        }
    }
    .map_err(map_driver_err)?;

    Ok(Json(result))
}

async fn execute_query(
    State(state): State<DbClientAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(req): Json<ExecuteQueryRequest>,
) -> Result<Json<QueryResult>, ApiErr> {
    let kek = require_auth_and_kek(&headers, &state).await?;
    let conn = state
        .app
        .db
        .get_db_connection_decrypted(id, &kek)
        .map_err(map_db_err)?;
    let driver = state
        .db_pool
        .build_driver(&conn, &state.app.db, &state.app.pool, &kek)
        .await
        .map_err(map_driver_err)?;
    let max_rows = clamp_limit(req.max_rows);
    let result = DbClientPool::with_timeout(driver.execute_query(
        &req.db,
        &req.sql,
        conn.read_only,
        max_rows,
    ))
    .await
    .map_err(map_driver_err)?;
    Ok(Json(result))
}
