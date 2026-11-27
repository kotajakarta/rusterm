use crate::models::bookmark::{Bookmark, BookmarkInput};
use crate::models::cloudflare::CFAccount;
use crate::models::db_client::{
    DbConnectionInput, DbConnectionOutput, DbEngine, DecryptedDbConnection, SslMode,
};
use crate::models::tailscale::TailscaleAccount;
use crate::models::files::{FileRemote, FileRemoteInput};
use crate::models::server::{
    BackupBookmark, BackupDbConnection, BackupPayload, BackupServer, BackupSnippet, BackupStats,
    BackupStorageAccount, DecryptedServerCreds, ServerInput, ServerOutput,
};
use crate::models::snippet::{Snippet, SnippetInput};
use crate::models::storage::{
    DecryptedStorageAccount, StorageAccountInput, StorageAccountOutput, StorageAccountUpdateInput,
};
use crate::services::crypto::CryptoService;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Pool error: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("Crypto error: {0}")]
    Crypto(String),
    #[error("Not found")]
    NotFound,
}

#[derive(Clone)]
pub struct DbStore {
    pool: Pool<SqliteConnectionManager>,
}

impl DbStore {
    pub fn new(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::new(manager)?;

        let conn = pool.get()?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS servers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL DEFAULT 22,
                username TEXT NOT NULL,
                auth_type TEXT NOT NULL,
                password TEXT,
                private_key TEXT,
                passphrase TEXT,
                group_name TEXT NOT NULL DEFAULT '',
                color TEXT NOT NULL DEFAULT '',
                host_key TEXT,
                host_key_fp TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                sort_order INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                server_id INTEGER NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                startup_cmd TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS snippets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS file_remotes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                local_server_id INTEGER NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
                local_path TEXT NOT NULL,
                remote_server_id INTEGER NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
                remote_path TEXT NOT NULL,
                ignore_json TEXT NOT NULL DEFAULT '[]',
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS cf_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                token TEXT NOT NULL,
                account_id TEXT NOT NULL,
                account_name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS tailscale_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                api_key TEXT NOT NULL,
                tailnet TEXT NOT NULL DEFAULT '-',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS db_connections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                engine TEXT NOT NULL,
                host TEXT NOT NULL DEFAULT '127.0.0.1',
                port INTEGER NOT NULL DEFAULT 5432,
                username TEXT NOT NULL DEFAULT '',
                password TEXT,
                default_database TEXT NOT NULL DEFAULT '',
                file_path TEXT NOT NULL DEFAULT '',
                ssl_mode TEXT NOT NULL DEFAULT 'prefer',
                ssh_server_id INTEGER REFERENCES servers(id) ON DELETE SET NULL,
                read_only INTEGER NOT NULL DEFAULT 0,
                color TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS storage_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                provider TEXT NOT NULL DEFAULT 's3',
                endpoint TEXT NOT NULL,
                region TEXT NOT NULL DEFAULT 'us-east-1',
                access_key_id TEXT NOT NULL,
                secret_access_key TEXT NOT NULL,
                default_bucket TEXT NOT NULL DEFAULT '',
                path_style INTEGER NOT NULL DEFAULT 1,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;

        Ok(Self { pool })
    }

    pub fn list_servers(&self) -> Result<Vec<ServerOutput>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, host, port, username, auth_type, group_name, color,
                    password IS NOT NULL AND password != '',
                    private_key IS NOT NULL AND private_key != '',
                    host_key_fp
             FROM servers
             ORDER BY sort_order ASC, id ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ServerOutput {
                id: row.get(0)?,
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                auth_type: row.get(5)?,
                group_name: row.get(6)?,
                color: row.get(7)?,
                has_password: row.get(8)?,
                has_private_key: row.get(9)?,
                host_key_fingerprint: row.get(10)?,
            })
        })?;

        let mut servers = Vec::new();
        for s in rows {
            servers.push(s?);
        }
        Ok(servers)
    }

    pub fn create_server(&self, input: &ServerInput, kek: &[u8; 32]) -> Result<i64, DbError> {
        let conn = self.pool.get()?;
        let enc_password = match &input.password {
            Some(p) if !p.is_empty() => {
                Some(CryptoService::encrypt(p.as_bytes(), kek).map_err(|e| DbError::Crypto(e.to_string()))?)
            }
            _ => None,
        };
        let enc_key = match &input.private_key {
            Some(k) if !k.is_empty() => {
                Some(CryptoService::encrypt(k.as_bytes(), kek).map_err(|e| DbError::Crypto(e.to_string()))?)
            }
            _ => None,
        };
        let enc_passphrase = match &input.passphrase {
            Some(p) if !p.is_empty() => {
                Some(CryptoService::encrypt(p.as_bytes(), kek).map_err(|e| DbError::Crypto(e.to_string()))?)
            }
            _ => None,
        };

        conn.execute(
            r#"INSERT INTO servers (name, host, port, username, auth_type, password, private_key, passphrase, group_name, color)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
            params![
                input.name,
                input.host,
                input.port,
                input.username,
                input.auth_type,
                enc_password,
                enc_key,
                enc_passphrase,
                input.group_name,
                input.color,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn update_server(&self, id: i64, input: &ServerInput, kek: &[u8; 32]) -> Result<(), DbError> {
        let conn = self.pool.get()?;

        // If password/key/passphrase is None, preserve existing from DB
        #[allow(clippy::type_complexity)]
        let (existing_pw, existing_key, existing_passphrase, old_host, old_port): (
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            i64,
        ) = conn
            .query_row(
                "SELECT password, private_key, passphrase, host, port FROM servers WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .map_err(|_| DbError::NotFound)?;

        let enc_password = match &input.password {
            Some(p) if !p.is_empty() => {
                Some(CryptoService::encrypt(p.as_bytes(), kek).map_err(|e| DbError::Crypto(e.to_string()))?)
            }
            Some(_) => None, // empty string clears
            None => existing_pw,
        };

        let enc_key = match &input.private_key {
            Some(k) if !k.is_empty() => {
                Some(CryptoService::encrypt(k.as_bytes(), kek).map_err(|e| DbError::Crypto(e.to_string()))?)
            }
            Some(_) => None,
            None => existing_key,
        };

        let enc_passphrase = match &input.passphrase {
            Some(p) if !p.is_empty() => {
                Some(CryptoService::encrypt(p.as_bytes(), kek).map_err(|e| DbError::Crypto(e.to_string()))?)
            }
            Some(_) => None,
            None => existing_passphrase,
        };

        conn.execute(
            r#"UPDATE servers
               SET name = ?1, host = ?2, port = ?3, username = ?4, auth_type = ?5,
                   password = ?6, private_key = ?7, passphrase = ?8, group_name = ?9, color = ?10
               WHERE id = ?11"#,
            params![
                input.name,
                input.host,
                input.port,
                input.username,
                input.auth_type,
                enc_password,
                enc_key,
                enc_passphrase,
                input.group_name,
                input.color,
                id,
            ],
        )?;

        // A trusted host key belongs to the machine at host:port; another address is another
        // machine, so it must be trusted afresh.
        if old_host != input.host || old_port != input.port {
            conn.execute(
                "UPDATE servers SET host_key = NULL, host_key_fp = NULL WHERE id = ?1",
                params![id],
            )?;
        }

        Ok(())
    }

    /// The trusted SSH host key (`"<algorithm> <base64>"`), if one has been stored.
    pub fn get_host_key(&self, id: i64) -> Result<Option<String>, DbError> {
        let conn = self.pool.get()?;
        conn.query_row(
            "SELECT host_key FROM servers WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|_| DbError::NotFound)
    }

    pub fn set_host_key(&self, id: i64, key: &str, fingerprint: &str) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        conn.execute(
            "UPDATE servers SET host_key = ?1, host_key_fp = ?2 WHERE id = ?3",
            params![key, fingerprint, id],
        )?;
        Ok(())
    }

    pub fn delete_server(&self, id: i64) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        conn.execute("DELETE FROM bookmarks WHERE server_id = ?1", params![id])?;
        conn.execute("DELETE FROM servers WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn reorder_servers(&self, ids: &[i64]) -> Result<(), DbError> {
        let mut conn = self.pool.get()?;
        let tx = conn.transaction()?;
        for (index, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE servers SET sort_order = ?1 WHERE id = ?2",
                params![index as i64, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_server_credentials(&self, id: i64, kek: &[u8; 32]) -> Result<DecryptedServerCreds, DbError> {
        let conn = self.pool.get()?;
        let row = conn.query_row(
            "SELECT id, name, host, port, username, auth_type, password, private_key, passphrase FROM servers WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            },
        ).map_err(|_| DbError::NotFound)?;

        let dec_password = match row.6 {
            Some(enc) => Some(
                String::from_utf8(
                    CryptoService::decrypt(&enc, kek).map_err(|e| DbError::Crypto(e.to_string()))?,
                )
                .map_err(|e| DbError::Crypto(e.to_string()))?,
            ),
            None => None,
        };

        let dec_key = match row.7 {
            Some(enc) => Some(
                String::from_utf8(
                    CryptoService::decrypt(&enc, kek).map_err(|e| DbError::Crypto(e.to_string()))?,
                )
                .map_err(|e| DbError::Crypto(e.to_string()))?,
            ),
            None => None,
        };

        let dec_passphrase = match row.8 {
            Some(enc) => Some(
                String::from_utf8(
                    CryptoService::decrypt(&enc, kek).map_err(|e| DbError::Crypto(e.to_string()))?,
                )
                .map_err(|e| DbError::Crypto(e.to_string()))?,
            ),
            None => None,
        };

        Ok(DecryptedServerCreds {
            id: row.0,
            name: row.1,
            host: row.2,
            port: row.3 as u16,
            username: row.4,
            auth_type: row.5,
            password: dec_password,
            private_key: dec_key,
            passphrase: dec_passphrase,
        })
    }

    // Bookmarks CRUD
    pub fn list_bookmarks(&self) -> Result<Vec<Bookmark>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT id, server_id, name, path, startup_cmd FROM bookmarks ORDER BY id ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                server_id: row.get(1)?,
                name: row.get(2)?,
                path: row.get(3)?,
                startup_cmd: row.get(4)?,
            })
        })?;

        let mut list = Vec::new();
        for b in rows {
            list.push(b?);
        }
        Ok(list)
    }

    pub fn create_bookmark(&self, input: &BookmarkInput) -> Result<i64, DbError> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO bookmarks (server_id, name, path, startup_cmd) VALUES (?1, ?2, ?3, ?4)",
            params![input.server_id, input.name, input.path, input.startup_cmd],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_bookmark(&self, id: i64, input: &BookmarkInput) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        conn.execute(
            "UPDATE bookmarks SET server_id = ?1, name = ?2, path = ?3, startup_cmd = ?4 WHERE id = ?5",
            params![input.server_id, input.name, input.path, input.startup_cmd, id],
        )?;
        Ok(())
    }

    pub fn delete_bookmark(&self, id: i64) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        conn.execute("DELETE FROM bookmarks WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_bookmark(&self, id: i64) -> Result<Bookmark, DbError> {
        let conn = self.pool.get()?;
        conn.query_row(
            "SELECT id, server_id, name, path, startup_cmd FROM bookmarks WHERE id = ?1",
            params![id],
            |row| {
                Ok(Bookmark {
                    id: row.get(0)?,
                    server_id: row.get(1)?,
                    name: row.get(2)?,
                    path: row.get(3)?,
                    startup_cmd: row.get(4)?,
                })
            },
        ).map_err(|_| DbError::NotFound)
    }

    // Snippets CRUD
    pub fn list_snippets(&self) -> Result<Vec<Snippet>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT id, name, command FROM snippets ORDER BY id ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Snippet {
                id: row.get(0)?,
                name: row.get(1)?,
                command: row.get(2)?,
            })
        })?;

        let mut list = Vec::new();
        for s in rows {
            list.push(s?);
        }
        Ok(list)
    }

    pub fn create_snippet(&self, input: &SnippetInput) -> Result<i64, DbError> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO snippets (name, command) VALUES (?1, ?2)",
            params![input.name, input.command],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_snippet(&self, id: i64, input: &SnippetInput) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        conn.execute(
            "UPDATE snippets SET name = ?1, command = ?2 WHERE id = ?3",
            params![input.name, input.command, id],
        )?;
        Ok(())
    }

    pub fn delete_snippet(&self, id: i64) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        conn.execute("DELETE FROM snippets WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn reset_host_key(&self, id: i64) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        conn.execute(
            "UPDATE servers SET host_key = NULL, host_key_fp = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn rotate_kek(&self, old_kek: &[u8; 32], new_kek: &[u8; 32]) -> Result<(), DbError> {
        let mut conn = self.pool.get()?;
        let tx = conn.transaction()?;

        let mut stmt = tx.prepare(
            "SELECT id, password, private_key, passphrase FROM servers",
        )?;

        let rows: Vec<(i64, Option<String>, Option<String>, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        for (id, pw, pk, pass) in rows {
            let new_pw = match pw {
                Some(ref enc) if !enc.is_empty() => {
                    let decrypted = CryptoService::decrypt(enc, old_kek)
                        .map_err(|e| DbError::Crypto(e.to_string()))?;
                    Some(CryptoService::encrypt(&decrypted, new_kek)
                        .map_err(|e| DbError::Crypto(e.to_string()))?)
                }
                _ => pw,
            };

            let new_pk = match pk {
                Some(ref enc) if !enc.is_empty() => {
                    let decrypted = CryptoService::decrypt(enc, old_kek)
                        .map_err(|e| DbError::Crypto(e.to_string()))?;
                    Some(CryptoService::encrypt(&decrypted, new_kek)
                        .map_err(|e| DbError::Crypto(e.to_string()))?)
                }
                _ => pk,
            };

            let new_pass = match pass {
                Some(ref enc) if !enc.is_empty() => {
                    let decrypted = CryptoService::decrypt(enc, old_kek)
                        .map_err(|e| DbError::Crypto(e.to_string()))?;
                    Some(CryptoService::encrypt(&decrypted, new_kek)
                        .map_err(|e| DbError::Crypto(e.to_string()))?)
                }
                _ => pass,
            };

            tx.execute(
                "UPDATE servers SET password = ?1, private_key = ?2, passphrase = ?3 WHERE id = ?4",
                params![new_pw, new_pk, new_pass, id],
            )?;
        }

        let mut db_stmt = tx.prepare("SELECT id, password FROM db_connections")?;
        let db_rows: Vec<(i64, Option<String>)> = db_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        drop(db_stmt);

        for (id, pw) in db_rows {
            let new_pw = match pw {
                Some(ref enc) if !enc.is_empty() => {
                    let decrypted = CryptoService::decrypt(enc, old_kek)
                        .map_err(|e| DbError::Crypto(e.to_string()))?;
                    Some(
                        CryptoService::encrypt(&decrypted, new_kek)
                            .map_err(|e| DbError::Crypto(e.to_string()))?,
                    )
                }
                _ => pw,
            };
            tx.execute(
                "UPDATE db_connections SET password = ?1 WHERE id = ?2",
                params![new_pw, id],
            )?;
        }

        let mut st_stmt = tx.prepare("SELECT id, secret_access_key FROM storage_accounts")?;
        let st_rows: Vec<(i64, String)> = st_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        drop(st_stmt);

        for (id, enc_secret) in st_rows {
            if !enc_secret.is_empty() {
                let decrypted = CryptoService::decrypt(&enc_secret, old_kek)
                    .map_err(|e| DbError::Crypto(e.to_string()))?;
                let new_enc = CryptoService::encrypt(&decrypted, new_kek)
                    .map_err(|e| DbError::Crypto(e.to_string()))?;
                tx.execute(
                    "UPDATE storage_accounts SET secret_access_key = ?1 WHERE id = ?2",
                    params![new_enc, id],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn export_backup_data(&self, kek: &[u8; 32]) -> Result<BackupPayload, DbError> {
        let servers = self.list_servers()?;
        let mut backup_servers = Vec::new();

        for s in servers {
            let creds = self.get_server_credentials(s.id, kek)?;
            let bookmarks = self
                .list_bookmarks()?
                .into_iter()
                .filter(|b| b.server_id == s.id)
                .map(|b| BackupBookmark {
                    name: b.name,
                    path: b.path,
                    startup_cmd: b.startup_cmd,
                })
                .collect();

            backup_servers.push(BackupServer {
                name: s.name,
                host: s.host,
                port: s.port,
                username: s.username,
                auth_type: s.auth_type,
                password: creds.password,
                private_key: creds.private_key,
                passphrase: creds.passphrase,
                group_name: s.group_name,
                color: s.color,
                bookmarks,
            });
        }

        let snippets = self
            .list_snippets()?
            .into_iter()
            .map(|sn| BackupSnippet {
                name: sn.name,
                command: sn.command,
            })
            .collect();

        let server_ref_by_id: std::collections::HashMap<i64, (String, String)> = self
            .list_servers()?
            .into_iter()
            .map(|s| (s.id, (s.name, s.host)))
            .collect();

        let mut backup_db_conns = Vec::new();
        for c in self.list_db_connections()? {
            let dec = self.get_db_connection_decrypted(c.id, kek)?;
            let (ssh_server_name, ssh_server_host) = match dec.ssh_server_id {
                Some(sid) => match server_ref_by_id.get(&sid) {
                    Some((sname, shost)) => (Some(sname.clone()), Some(shost.clone())),
                    None => (None, None),
                },
                None => (None, None),
            };
            backup_db_conns.push(BackupDbConnection {
                name: dec.name,
                engine: dec.engine.as_str().to_string(),
                host: dec.host,
                port: dec.port,
                username: dec.username,
                password: dec.password,
                default_database: dec.default_database,
                file_path: dec.file_path,
                ssl_mode: dec.ssl_mode.as_str().to_string(),
                ssh_server_name,
                ssh_server_host,
                read_only: dec.read_only,
                color: c.color,
            });
        }

        let mut backup_storage_accounts = Vec::new();
        for sa in self.list_storage_accounts()? {
            let dec = self.get_storage_account_decrypted(sa.id, kek)?;
            backup_storage_accounts.push(BackupStorageAccount {
                name: dec.name,
                provider: dec.provider,
                endpoint: dec.endpoint,
                region: dec.region,
                access_key_id: dec.access_key_id,
                secret_access_key: dec.secret_access_key,
                default_bucket: dec.default_bucket,
                path_style: dec.path_style,
            });
        }

        Ok(BackupPayload {
            version: 1,
            servers: backup_servers,
            snippets,
            db_connections: backup_db_conns,
            storage_accounts: backup_storage_accounts,
        })
    }

    pub fn import_backup_data(
        &self,
        payload: &BackupPayload,
        kek: &[u8; 32],
    ) -> Result<BackupStats, DbError> {
        let mut stats = BackupStats::default();
        let existing_servers = self.list_servers()?;

        for srv in &payload.servers {
            let existing = existing_servers
                .iter()
                .find(|s| s.name == srv.name && s.host == srv.host);
            let srv_input = ServerInput {
                name: srv.name.clone(),
                host: srv.host.clone(),
                port: srv.port,
                username: srv.username.clone(),
                auth_type: srv.auth_type.clone(),
                password: srv.password.clone(),
                private_key: srv.private_key.clone(),
                passphrase: srv.passphrase.clone(),
                group_name: srv.group_name.clone(),
                color: srv.color.clone(),
            };

            let server_id = match existing {
                Some(ex) => {
                    self.update_server(ex.id, &srv_input, kek)?;
                    stats.servers_updated += 1;
                    ex.id
                }
                None => {
                    let id = self.create_server(&srv_input, kek)?;
                    stats.servers_added += 1;
                    id
                }
            };

            for bm in &srv.bookmarks {
                self.create_bookmark(&BookmarkInput {
                    server_id,
                    name: bm.name.clone(),
                    path: bm.path.clone(),
                    startup_cmd: bm.startup_cmd.clone(),
                })?;
                stats.bookmarks_added += 1;
            }
        }

        let existing_snippets = self.list_snippets()?;
        for sn in &payload.snippets {
            if !existing_snippets.iter().any(|s| s.name == sn.name) {
                self.create_snippet(&SnippetInput {
                    name: sn.name.clone(),
                    command: sn.command.clone(),
                })?;
                stats.snippets_added += 1;
            }
        }

        if !payload.db_connections.is_empty() {
            let server_id_by_ref: std::collections::HashMap<(String, String), i64> = self
                .list_servers()?
                .into_iter()
                .map(|s| ((s.name, s.host), s.id))
                .collect();
            let existing_db_conns = self.list_db_connections()?;

            for dbc in &payload.db_connections {
                let Some(engine) = DbEngine::from_str_checked(&dbc.engine) else {
                    continue;
                };
                let ssh_server_id = match (&dbc.ssh_server_name, &dbc.ssh_server_host) {
                    (Some(sname), Some(shost)) => {
                        server_id_by_ref.get(&(sname.clone(), shost.clone())).copied()
                    }
                    _ => None,
                };
                let input = DbConnectionInput {
                    name: dbc.name.clone(),
                    engine,
                    host: dbc.host.clone(),
                    port: dbc.port,
                    username: dbc.username.clone(),
                    password: dbc.password.clone(),
                    default_database: dbc.default_database.clone(),
                    file_path: dbc.file_path.clone(),
                    ssl_mode: SslMode::from_str_lossy(&dbc.ssl_mode),
                    ssh_server_id,
                    read_only: dbc.read_only,
                    color: dbc.color.clone(),
                };
                if let Some(ex) = existing_db_conns
                    .iter()
                    .find(|c| c.name == dbc.name && c.engine == engine)
                {
                    self.update_db_connection(ex.id, &input, kek)?;
                    stats.db_connections_updated += 1;
                } else {
                    self.create_db_connection(&input, kek)?;
                    stats.db_connections_added += 1;
                }
            }
        }

        if !payload.storage_accounts.is_empty() {
            let existing_storage = self.list_storage_accounts()?;
            for sa in &payload.storage_accounts {
                if let Some(ex) = existing_storage
                    .iter()
                    .find(|s| s.name == sa.name && s.endpoint == sa.endpoint)
                {
                    let upd = StorageAccountUpdateInput {
                        name: sa.name.clone(),
                        provider: sa.provider.clone(),
                        endpoint: sa.endpoint.clone(),
                        region: sa.region.clone(),
                        access_key_id: sa.access_key_id.clone(),
                        secret_access_key: Some(sa.secret_access_key.clone()),
                        default_bucket: sa.default_bucket.clone(),
                        path_style: sa.path_style,
                    };
                    self.update_storage_account(ex.id, &upd, kek)?;
                    stats.storage_accounts_updated += 1;
                } else {
                    let inp = StorageAccountInput {
                        name: sa.name.clone(),
                        provider: sa.provider.clone(),
                        endpoint: sa.endpoint.clone(),
                        region: sa.region.clone(),
                        access_key_id: sa.access_key_id.clone(),
                        secret_access_key: sa.secret_access_key.clone(),
                        default_bucket: sa.default_bucket.clone(),
                        path_style: sa.path_style,
                    };
                    self.create_storage_account(&inp, kek)?;
                    stats.storage_accounts_added += 1;
                }
            }
        }

        Ok(stats)
    }

    pub fn list_file_remotes(&self) -> Result<Vec<FileRemote>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, local_server_id, local_path, remote_server_id, remote_path, ignore_json, updated_at
             FROM file_remotes
             ORDER BY name ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let ignore_json: String = row.get(6)?;
            let ignore: Vec<String> = serde_json::from_str(&ignore_json).unwrap_or_default();
            Ok(FileRemote {
                id: row.get(0)?,
                name: row.get(1)?,
                local_server_id: row.get(2)?,
                local_path: row.get(3)?,
                remote_server_id: row.get(4)?,
                remote_path: row.get(5)?,
                ignore,
                updated_at: row.get(7)?,
            })
        })?;

        let mut remotes = Vec::new();
        for r in rows {
            remotes.push(r?);
        }
        Ok(remotes)
    }

    pub fn get_file_remote(&self, id: i64) -> Result<Option<FileRemote>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, local_server_id, local_path, remote_server_id, remote_path, ignore_json, updated_at
             FROM file_remotes
             WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map([id], |row| {
            let ignore_json: String = row.get(6)?;
            let ignore: Vec<String> = serde_json::from_str(&ignore_json).unwrap_or_default();
            Ok(FileRemote {
                id: row.get(0)?,
                name: row.get(1)?,
                local_server_id: row.get(2)?,
                local_path: row.get(3)?,
                remote_server_id: row.get(4)?,
                remote_path: row.get(5)?,
                ignore,
                updated_at: row.get(7)?,
            })
        })?;

        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    pub fn create_file_remote(&self, input: &FileRemoteInput) -> Result<FileRemote, DbError> {
        let conn = self.pool.get()?;
        let ignore_json = serde_json::to_string(&input.ignore).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO file_remotes (name, local_server_id, local_path, remote_server_id, remote_path, ignore_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                input.name,
                input.local_server_id,
                input.local_path,
                input.remote_server_id,
                input.remote_path,
                ignore_json,
            ],
        )?;
        let id = conn.last_insert_rowid();
        self.get_file_remote(id)?.ok_or(DbError::NotFound)
    }

    pub fn update_file_remote(&self, id: i64, input: &FileRemoteInput) -> Result<FileRemote, DbError> {
        let conn = self.pool.get()?;
        let ignore_json = serde_json::to_string(&input.ignore).unwrap_or_else(|_| "[]".to_string());
        let count = conn.execute(
            "UPDATE file_remotes
             SET name = ?1, local_server_id = ?2, local_path = ?3, remote_server_id = ?4, remote_path = ?5, ignore_json = ?6, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?7",
            params![
                input.name,
                input.local_server_id,
                input.local_path,
                input.remote_server_id,
                input.remote_path,
                ignore_json,
                id,
            ],
        )?;
        if count == 0 {
            return Err(DbError::NotFound);
        }
        self.get_file_remote(id)?.ok_or(DbError::NotFound)
    }

    pub fn delete_file_remote(&self, id: i64) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        let count = conn.execute("DELETE FROM file_remotes WHERE id = ?1", params![id])?;
        if count == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    pub fn list_file_bookmark_paths(&self, server_id: i64) -> Result<Vec<String>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT path FROM bookmarks WHERE server_id = ?1 ORDER BY path ASC",
        )?;
        let rows = stmt.query_map([server_id], |row| row.get::<_, String>(0))?;
        let mut paths = Vec::new();
        for r in rows {
            paths.push(r?);
        }
        Ok(paths)
    }

    pub fn toggle_file_bookmark(&self, server_id: i64, path: &str) -> Result<bool, DbError> {
        let conn = self.pool.get()?;
        let clean_path = if path.len() > 1 && path.ends_with('/') {
            path.trim_end_matches('/').to_string()
        } else {
            path.to_string()
        };

        let existing_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM bookmarks WHERE server_id = ?1 AND path = ?2",
                params![server_id, clean_path],
                |r| r.get(0),
            )
            .rusqlite_to_option()?;

        if let Some(id) = existing_id {
            conn.execute("DELETE FROM bookmarks WHERE id = ?1", params![id])?;
            Ok(false) // removed
        } else {
            let name = std::path::Path::new(&clean_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(if clean_path == "/" { "/" } else { "folder" });
            conn.execute(
                "INSERT INTO bookmarks (server_id, name, path, startup_cmd) VALUES (?1, ?2, ?3, '')",
                params![server_id, name, clean_path],
            )?;
            Ok(true) // added
        }
    }

    pub fn sync_file_bookmarks(&self, server_id: i64, paths: &[String]) -> Result<Vec<String>, DbError> {
        let conn = self.pool.get()?;
        let existing = self.list_file_bookmark_paths(server_id)?;
        let existing_set: std::collections::HashSet<String> = existing.into_iter().collect();

        for p in paths {
            let clean = if p.len() > 1 && p.ends_with('/') {
                p.trim_end_matches('/').to_string()
            } else {
                p.to_string()
            };
            if !existing_set.contains(&clean) {
                let name = std::path::Path::new(&clean)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(if clean == "/" { "/" } else { "folder" });
                conn.execute(
                    "INSERT INTO bookmarks (server_id, name, path, startup_cmd) VALUES (?1, ?2, ?3, '')",
                    params![server_id, name, clean],
                )?;
            }
        }
        self.list_file_bookmark_paths(server_id)
    }

    pub fn list_cf_accounts(&self) -> Result<Vec<CFAccount>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, token, account_id, account_name, created_at
             FROM cf_accounts
             ORDER BY name ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let token: String = row.get(2)?;
            let token_masked = if token.len() <= 8 {
                "••••••••".to_string()
            } else {
                format!("{}••••{}", &token[..4], &token[token.len() - 4..])
            };
            Ok(CFAccount {
                id: row.get(0)?,
                name: row.get(1)?,
                token_masked,
                account_id: row.get(3)?,
                account_name: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_cf_account(&self, id: i64) -> Result<Option<(CFAccount, String)>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, token, account_id, account_name, created_at
             FROM cf_accounts
             WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map([id], |row| {
            let token: String = row.get(2)?;
            let token_masked = if token.len() <= 8 {
                "••••••••".to_string()
            } else {
                format!("{}••••{}", &token[..4], &token[token.len() - 4..])
            };
            let acc = CFAccount {
                id: row.get(0)?,
                name: row.get(1)?,
                token_masked,
                account_id: row.get(3)?,
                account_name: row.get(4)?,
                created_at: row.get(5)?,
            };
            Ok((acc, token))
        })?;

        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn create_cf_account(
        &self,
        name: &str,
        token: &str,
        account_id: &str,
        account_name: &str,
    ) -> Result<CFAccount, DbError> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO cf_accounts (name, token, account_id, account_name)
             VALUES (?1, ?2, ?3, ?4)",
            params![name, token, account_id, account_name],
        )?;
        let id = conn.last_insert_rowid();
        let (acc, _) = self.get_cf_account(id)?.ok_or(DbError::NotFound)?;
        Ok(acc)
    }

    pub fn update_cf_account(
        &self,
        id: i64,
        name: &str,
        token: Option<&str>,
    ) -> Result<CFAccount, DbError> {
        let conn = self.pool.get()?;
        if let Some(tok) = token {
            conn.execute(
                "UPDATE cf_accounts SET name = ?1, token = ?2 WHERE id = ?3",
                params![name, tok, id],
            )?;
        } else {
            conn.execute(
                "UPDATE cf_accounts SET name = ?1 WHERE id = ?2",
                params![name, id],
            )?;
        }
        let (acc, _) = self.get_cf_account(id)?.ok_or(DbError::NotFound)?;
        Ok(acc)
    }

    pub fn delete_cf_account(&self, id: i64) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        let count = conn.execute("DELETE FROM cf_accounts WHERE id = ?1", params![id])?;
        if count == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    pub fn list_tailscale_accounts(&self) -> Result<Vec<TailscaleAccount>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, api_key, tailnet, created_at
             FROM tailscale_accounts
             ORDER BY name ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let token: String = row.get(2)?;
            let token_masked = if token.len() <= 8 {
                "••••••••".to_string()
            } else {
                let show_len = 4.min(token.len() / 2);
                format!("{}••••{}", &token[..show_len], &token[token.len() - show_len..])
            };
            Ok(TailscaleAccount {
                id: row.get(0)?,
                name: row.get(1)?,
                token_masked,
                tailnet: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_tailscale_account(&self, id: i64) -> Result<Option<(TailscaleAccount, String)>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, api_key, tailnet, created_at
             FROM tailscale_accounts
             WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map([id], |row| {
            let token: String = row.get(2)?;
            let token_masked = if token.len() <= 8 {
                "••••••••".to_string()
            } else {
                let show_len = 4.min(token.len() / 2);
                format!("{}••••{}", &token[..show_len], &token[token.len() - show_len..])
            };
            Ok((
                TailscaleAccount {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    token_masked,
                    tailnet: row.get(3)?,
                    created_at: row.get(4)?,
                },
                token,
            ))
        })?;

        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn create_tailscale_account(
        &self,
        name: &str,
        api_key: &str,
        tailnet: &str,
    ) -> Result<TailscaleAccount, DbError> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO tailscale_accounts (name, api_key, tailnet)
             VALUES (?1, ?2, ?3)",
            params![name, api_key, tailnet],
        )?;
        let id = conn.last_insert_rowid();
        let (acc, _) = self.get_tailscale_account(id)?.ok_or(DbError::NotFound)?;
        Ok(acc)
    }

    pub fn update_tailscale_account(
        &self,
        id: i64,
        name: &str,
        api_key: Option<&str>,
        tailnet: Option<&str>,
    ) -> Result<TailscaleAccount, DbError> {
        let conn = self.pool.get()?;
        match (api_key, tailnet) {
            (Some(tok), Some(tn)) => {
                conn.execute(
                    "UPDATE tailscale_accounts SET name = ?1, api_key = ?2, tailnet = ?3 WHERE id = ?4",
                    params![name, tok, tn, id],
                )?;
            }
            (Some(tok), None) => {
                conn.execute(
                    "UPDATE tailscale_accounts SET name = ?1, api_key = ?2 WHERE id = ?3",
                    params![name, tok, id],
                )?;
            }
            (None, Some(tn)) => {
                conn.execute(
                    "UPDATE tailscale_accounts SET name = ?1, tailnet = ?2 WHERE id = ?3",
                    params![name, tn, id],
                )?;
            }
            (None, None) => {
                conn.execute(
                    "UPDATE tailscale_accounts SET name = ?1 WHERE id = ?2",
                    params![name, id],
                )?;
            }
        }
        let (acc, _) = self.get_tailscale_account(id)?.ok_or(DbError::NotFound)?;
        Ok(acc)
    }

    pub fn delete_tailscale_account(&self, id: i64) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        let count = conn.execute("DELETE FROM tailscale_accounts WHERE id = ?1", params![id])?;
        if count == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    pub fn list_db_connections(&self) -> Result<Vec<DbConnectionOutput>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, engine, host, port, username,
                    password IS NOT NULL AND password != '',
                    default_database, file_path, ssl_mode, ssh_server_id,
                    read_only, color, created_at
             FROM db_connections
             ORDER BY sort_order ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let engine_str: String = row.get(2)?;
            let ssl_str: String = row.get(9)?;
            let read_only_int: i64 = row.get(11)?;
            let port_i64: i64 = row.get(4)?;
            Ok(DbConnectionOutput {
                id: row.get(0)?,
                name: row.get(1)?,
                engine: DbEngine::from_str_checked(&engine_str).unwrap_or(DbEngine::Postgres),
                host: row.get(3)?,
                port: u16::try_from(port_i64).unwrap_or(0),
                username: row.get(5)?,
                has_password: row.get(6)?,
                default_database: row.get(7)?,
                file_path: row.get(8)?,
                ssl_mode: SslMode::from_str_lossy(&ssl_str),
                ssh_server_id: row.get(10)?,
                read_only: read_only_int != 0,
                color: row.get(12)?,
                created_at: row.get(13)?,
            })
        })?;
        let mut out = Vec::new();
        for item in rows {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn get_db_connection(&self, id: i64) -> Result<DbConnectionOutput, DbError> {
        let conn = self.pool.get()?;
        conn.query_row(
            "SELECT id, name, engine, host, port, username,
                    password IS NOT NULL AND password != '',
                    default_database, file_path, ssl_mode, ssh_server_id,
                    read_only, color, created_at
             FROM db_connections
             WHERE id = ?1",
            params![id],
            |row| {
                let engine_str: String = row.get(2)?;
                let ssl_str: String = row.get(9)?;
                let read_only_int: i64 = row.get(11)?;
                let port_i64: i64 = row.get(4)?;
                Ok(DbConnectionOutput {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    engine: DbEngine::from_str_checked(&engine_str).unwrap_or(DbEngine::Postgres),
                    host: row.get(3)?,
                    port: u16::try_from(port_i64).unwrap_or(0),
                    username: row.get(5)?,
                    has_password: row.get(6)?,
                    default_database: row.get(7)?,
                    file_path: row.get(8)?,
                    ssl_mode: SslMode::from_str_lossy(&ssl_str),
                    ssh_server_id: row.get(10)?,
                    read_only: read_only_int != 0,
                    color: row.get(12)?,
                    created_at: row.get(13)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound,
            other => DbError::Sqlite(other),
        })
    }

    pub fn get_db_connection_decrypted(
        &self,
        id: i64,
        kek: &[u8; 32],
    ) -> Result<DecryptedDbConnection, DbError> {
        let conn = self.pool.get()?;
        #[allow(clippy::type_complexity)]
        let row: (
            i64,
            String,
            String,
            String,
            i64,
            String,
            Option<String>,
            String,
            String,
            String,
            Option<i64>,
            i64,
        ) = conn
            .query_row(
                "SELECT id, name, engine, host, port, username, password,
                        default_database, file_path, ssl_mode, ssh_server_id, read_only
                 FROM db_connections
                 WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                        r.get(8)?,
                        r.get(9)?,
                        r.get(10)?,
                        r.get(11)?,
                    ))
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound,
                other => DbError::Sqlite(other),
            })?;

        let password = match row.6 {
            Some(ref enc) if !enc.is_empty() => {
                let plain = CryptoService::decrypt(enc, kek)
                    .map_err(|e| DbError::Crypto(e.to_string()))?;
                Some(String::from_utf8_lossy(&plain).into_owned())
            }
            _ => None,
        };

        Ok(DecryptedDbConnection {
            id: row.0,
            name: row.1,
            engine: DbEngine::from_str_checked(&row.2).unwrap_or(DbEngine::Postgres),
            host: row.3,
            port: u16::try_from(row.4).unwrap_or(0),
            username: row.5,
            password,
            default_database: row.7,
            file_path: row.8,
            ssl_mode: SslMode::from_str_lossy(&row.9),
            ssh_server_id: row.10,
            read_only: row.11 != 0,
        })
    }

    pub fn create_db_connection(
        &self,
        input: &DbConnectionInput,
        kek: &[u8; 32],
    ) -> Result<i64, DbError> {
        let conn = self.pool.get()?;
        let enc_password = match &input.password {
            Some(p) if !p.is_empty() => Some(
                CryptoService::encrypt(p.as_bytes(), kek)
                    .map_err(|e| DbError::Crypto(e.to_string()))?,
            ),
            _ => None,
        };
        let port = if input.port == 0 {
            input.engine.default_port()
        } else {
            input.port
        };
        conn.execute(
            r#"INSERT INTO db_connections
               (name, engine, host, port, username, password, default_database, file_path, ssl_mode, ssh_server_id, read_only, color)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
            params![
                input.name.trim(),
                input.engine.as_str(),
                input.host.trim(),
                port as i64,
                input.username.trim(),
                enc_password,
                input.default_database.trim(),
                input.file_path.trim(),
                input.ssl_mode.as_str(),
                input.ssh_server_id,
                if input.read_only { 1i64 } else { 0i64 },
                input.color.trim(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_db_connection(
        &self,
        id: i64,
        input: &DbConnectionInput,
        kek: &[u8; 32],
    ) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        let existing_pw: Option<String> = conn
            .query_row(
                "SELECT password FROM db_connections WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound,
                other => DbError::Sqlite(other),
            })?;

        let enc_password = match &input.password {
            Some(p) if !p.is_empty() => Some(
                CryptoService::encrypt(p.as_bytes(), kek)
                    .map_err(|e| DbError::Crypto(e.to_string()))?,
            ),
            Some(_) => None,
            None => existing_pw,
        };
        let port = if input.port == 0 {
            input.engine.default_port()
        } else {
            input.port
        };

        conn.execute(
            r#"UPDATE db_connections
               SET name = ?1, engine = ?2, host = ?3, port = ?4, username = ?5,
                   password = ?6, default_database = ?7, file_path = ?8, ssl_mode = ?9,
                   ssh_server_id = ?10, read_only = ?11, color = ?12
               WHERE id = ?13"#,
            params![
                input.name.trim(),
                input.engine.as_str(),
                input.host.trim(),
                port as i64,
                input.username.trim(),
                enc_password,
                input.default_database.trim(),
                input.file_path.trim(),
                input.ssl_mode.as_str(),
                input.ssh_server_id,
                if input.read_only { 1i64 } else { 0i64 },
                input.color.trim(),
                id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_db_connection(&self, id: i64) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        let count = conn.execute("DELETE FROM db_connections WHERE id = ?1", params![id])?;
        if count == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    pub fn list_storage_accounts(&self) -> Result<Vec<StorageAccountOutput>, DbError> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, provider, endpoint, region, access_key_id,
                    secret_access_key != '', default_bucket, path_style, created_at
             FROM storage_accounts
             ORDER BY sort_order ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let path_style_int: i64 = row.get(8)?;
            Ok(StorageAccountOutput {
                id: row.get(0)?,
                name: row.get(1)?,
                provider: row.get(2)?,
                endpoint: row.get(3)?,
                region: row.get(4)?,
                access_key_id: row.get(5)?,
                has_secret: row.get(6)?,
                default_bucket: row.get(7)?,
                path_style: path_style_int != 0,
                created_at: row.get(9)?,
            })
        })?;
        let mut out = Vec::new();
        for item in rows {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn get_storage_account(&self, id: i64) -> Result<StorageAccountOutput, DbError> {
        let conn = self.pool.get()?;
        conn.query_row(
            "SELECT id, name, provider, endpoint, region, access_key_id,
                    secret_access_key != '', default_bucket, path_style, created_at
             FROM storage_accounts
             WHERE id = ?1",
            params![id],
            |row| {
                let path_style_int: i64 = row.get(8)?;
                Ok(StorageAccountOutput {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider: row.get(2)?,
                    endpoint: row.get(3)?,
                    region: row.get(4)?,
                    access_key_id: row.get(5)?,
                    has_secret: row.get(6)?,
                    default_bucket: row.get(7)?,
                    path_style: path_style_int != 0,
                    created_at: row.get(9)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound,
            other => DbError::Sqlite(other),
        })
    }

    pub fn get_storage_account_decrypted(
        &self,
        id: i64,
        kek: &[u8; 32],
    ) -> Result<DecryptedStorageAccount, DbError> {
        let conn = self.pool.get()?;
        #[allow(clippy::type_complexity)]
        let row: (i64, String, String, String, String, String, String, String, i64) = conn
            .query_row(
                "SELECT id, name, provider, endpoint, region, access_key_id,
                        secret_access_key, default_bucket, path_style
                 FROM storage_accounts
                 WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                        r.get(8)?,
                    ))
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound,
                other => DbError::Sqlite(other),
            })?;

        let plain_secret = if row.6.is_empty() {
            String::new()
        } else {
            let bytes = CryptoService::decrypt(&row.6, kek)
                .map_err(|e| DbError::Crypto(e.to_string()))?;
            String::from_utf8_lossy(&bytes).into_owned()
        };

        Ok(DecryptedStorageAccount {
            id: row.0,
            name: row.1,
            provider: row.2,
            endpoint: row.3,
            region: row.4,
            access_key_id: row.5,
            secret_access_key: plain_secret,
            default_bucket: row.7,
            path_style: row.8 != 0,
        })
    }

    pub fn create_storage_account(
        &self,
        input: &StorageAccountInput,
        kek: &[u8; 32],
    ) -> Result<i64, DbError> {
        let conn = self.pool.get()?;
        let enc_secret = CryptoService::encrypt(input.secret_access_key.as_bytes(), kek)
            .map_err(|e| DbError::Crypto(e.to_string()))?;
        conn.execute(
            r#"INSERT INTO storage_accounts
               (name, provider, endpoint, region, access_key_id, secret_access_key, default_bucket, path_style)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                input.name.trim(),
                input.provider.trim(),
                input.endpoint.trim().trim_end_matches('/'),
                input.region.trim(),
                input.access_key_id.trim(),
                enc_secret,
                input.default_bucket.trim(),
                if input.path_style { 1i64 } else { 0i64 },
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_storage_account(
        &self,
        id: i64,
        input: &StorageAccountUpdateInput,
        kek: &[u8; 32],
    ) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        let existing_secret: String = conn
            .query_row(
                "SELECT secret_access_key FROM storage_accounts WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound,
                other => DbError::Sqlite(other),
            })?;

        let enc_secret = match &input.secret_access_key {
            Some(s) if !s.is_empty() => CryptoService::encrypt(s.as_bytes(), kek)
                .map_err(|e| DbError::Crypto(e.to_string()))?,
            _ => existing_secret,
        };

        conn.execute(
            r#"UPDATE storage_accounts
               SET name = ?1, provider = ?2, endpoint = ?3, region = ?4,
                   access_key_id = ?5, secret_access_key = ?6,
                   default_bucket = ?7, path_style = ?8
               WHERE id = ?9"#,
            params![
                input.name.trim(),
                input.provider.trim(),
                input.endpoint.trim().trim_end_matches('/'),
                input.region.trim(),
                input.access_key_id.trim(),
                enc_secret,
                input.default_bucket.trim(),
                if input.path_style { 1i64 } else { 0i64 },
                id,
            ],
        )?;
        Ok(())
    }

    pub fn delete_storage_account(&self, id: i64) -> Result<(), DbError> {
        let conn = self.pool.get()?;
        let count = conn.execute("DELETE FROM storage_accounts WHERE id = ?1", params![id])?;
        if count == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }
}

trait RusqliteOptionExt<T> {
    fn rusqlite_to_option(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> RusqliteOptionExt<T> for Result<T, rusqlite::Error> {
    fn rusqlite_to_option(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
