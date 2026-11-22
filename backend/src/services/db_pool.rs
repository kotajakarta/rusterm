use crate::models::db::DbStore;
use crate::models::db_client::{DbEngine, DecryptedDbConnection};
use crate::services::db_drivers::mysql::MysqlDriver;
use crate::services::db_drivers::postgres::PostgresDriver;
use crate::services::db_drivers::sqlite::SqliteDriver;
use crate::services::db_drivers::{DbDriver, DbDriverError, DEFAULT_QUERY_TIMEOUT_SECS};
use crate::services::db_tunnel::{open_ssh_tunnel, ActiveTunnel};
use crate::services::ssh_pool::SshPool;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const TUNNEL_IDLE_SECS: u64 = 300;

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
struct TunnelKey {
    ssh_server_id: i64,
    remote_host: String,
    remote_port: u16,
}

struct CachedTunnel {
    tunnel: ActiveTunnel,
    last_used: Instant,
}

#[derive(Clone, Default)]
pub struct DbClientPool {
    tunnels: Arc<Mutex<HashMap<TunnelKey, CachedTunnel>>>,
}

impl DbClientPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bersihkan semua tunnel aktif (mis. saat lock vault / shutdown).
    pub async fn close_all(&self) {
        let mut map = self.tunnels.lock().await;
        for (_, entry) in map.drain() {
            entry.tunnel.close();
        }
    }

    /// Hapus tunnel yang idle melebihi `TUNNEL_IDLE_SECS` atau sudah mati.
    pub async fn evict_idle(&self) {
        let mut map = self.tunnels.lock().await;
        let now = Instant::now();
        let expired: Vec<TunnelKey> = map
            .iter()
            .filter(|(_, v)| {
                v.tunnel.is_finished()
                    || now.duration_since(v.last_used) > Duration::from_secs(TUNNEL_IDLE_SECS)
            })
            .map(|(k, _)| k.clone())
            .collect();
        for key in expired {
            if let Some(entry) = map.remove(&key) {
                entry.tunnel.close();
            }
        }
    }

    async fn resolve_endpoint(
        &self,
        conn: &DecryptedDbConnection,
        db_store: &DbStore,
        ssh_pool: &SshPool,
        kek: &[u8; 32],
    ) -> Result<(String, u16), DbDriverError> {
        let target_port = if conn.port == 0 {
            conn.engine.default_port()
        } else {
            conn.port
        };

        let Some(ssh_server_id) = conn.ssh_server_id else {
            return Ok((conn.host.clone(), target_port));
        };

        self.evict_idle().await;
        let key = TunnelKey {
            ssh_server_id,
            remote_host: conn.host.clone(),
            remote_port: target_port,
        };

        {
            let mut map = self.tunnels.lock().await;
            if let Some(cached) = map.get_mut(&key) {
                if !cached.tunnel.is_finished() {
                    cached.last_used = Instant::now();
                    return Ok(("127.0.0.1".to_string(), cached.tunnel.local_port()));
                } else {
                    map.remove(&key);
                }
            }
        }

        let server_creds = db_store
            .get_server_credentials(ssh_server_id, kek)
            .map_err(|e| {
                DbDriverError::Tunnel(format!("Server SSH (id={}) tidak ditemukan: {}", ssh_server_id, e))
            })?;

        let tunnel = open_ssh_tunnel(ssh_pool, &server_creds, conn.host.clone(), target_port).await?;
        let local_port = tunnel.local_port();

        let mut map = self.tunnels.lock().await;
        map.insert(
            key,
            CachedTunnel {
                tunnel,
                last_used: Instant::now(),
            },
        );
        Ok(("127.0.0.1".to_string(), local_port))
    }

    pub async fn build_driver(
        &self,
        conn: &DecryptedDbConnection,
        db_store: &DbStore,
        ssh_pool: &SshPool,
        kek: &[u8; 32],
    ) -> Result<Box<dyn DbDriver>, DbDriverError> {
        match conn.engine {
            DbEngine::Sqlite => Ok(Box::new(SqliteDriver::new(
                conn.file_path.clone(),
                conn.read_only,
            )?)),
            DbEngine::Postgres => {
                let (host, port) = self.resolve_endpoint(conn, db_store, ssh_pool, kek).await?;
                Ok(Box::new(PostgresDriver::new(
                    host,
                    port,
                    conn.username.clone(),
                    conn.password.clone(),
                    conn.default_database.clone(),
                    conn.ssl_mode.clone(),
                    conn.read_only,
                )))
            }
            DbEngine::Mysql => {
                let (host, port) = self.resolve_endpoint(conn, db_store, ssh_pool, kek).await?;
                Ok(Box::new(MysqlDriver::new(
                    host,
                    port,
                    conn.username.clone(),
                    conn.password.clone(),
                    conn.default_database.clone(),
                    conn.ssl_mode.clone(),
                    conn.read_only,
                )))
            }
        }
    }

    pub async fn with_timeout<F, T>(fut: F) -> Result<T, DbDriverError>
    where
        F: Future<Output = Result<T, DbDriverError>>,
    {
        match tokio::time::timeout(Duration::from_secs(DEFAULT_QUERY_TIMEOUT_SECS), fut).await {
            Ok(res) => res,
            Err(_) => Err(DbDriverError::Timeout(DEFAULT_QUERY_TIMEOUT_SECS)),
        }
    }
}
