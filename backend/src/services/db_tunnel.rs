use crate::models::server::DecryptedServerCreds;
use crate::services::db_drivers::DbDriverError;
use crate::services::ssh_pool::SshPool;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Loopback TCP forwarder yang meneruskan koneksi lokal `127.0.0.1:<ephemeral>`
/// melalui SSH channel `direct-tcpip` ke `remote_host:remote_port`.
pub struct ActiveTunnel {
    pub local_addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl ActiveTunnel {
    pub fn local_port(&self) -> u16 {
        self.local_addr.port()
    }

    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }

    pub fn close(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.task.abort();
    }
}

impl Drop for ActiveTunnel {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.task.abort();
    }
}

pub async fn open_ssh_tunnel(
    ssh_pool: &SshPool,
    server_creds: &DecryptedServerCreds,
    remote_host: String,
    remote_port: u16,
) -> Result<ActiveTunnel, DbDriverError> {
    let session = ssh_pool
        .get_or_connect(server_creds)
        .await
        .map_err(|e| DbDriverError::Tunnel(e.to_string()))?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| DbDriverError::Tunnel(format!("Gagal bind loopback tunnel: {}", e)))?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| DbDriverError::Tunnel(e.to_string()))?;

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
    let session_arc = Arc::clone(&session);

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    break;
                }
                accept_res = listener.accept() => {
                    let (mut local_stream, peer_addr) = match accept_res {
                        Ok(pair) => pair,
                        Err(_) => break,
                    };
                    let sess = Arc::clone(&session_arc);
                    let rhost = remote_host.clone();
                    tokio::spawn(async move {
                        let channel_res = {
                            let handle = sess.handle.lock().await;
                            handle
                                .channel_open_direct_tcpip(
                                    rhost,
                                    remote_port as u32,
                                    peer_addr.ip().to_string(),
                                    peer_addr.port() as u32,
                                )
                                .await
                        };
                        if let Ok(channel) = channel_res {
                            let mut ssh_stream = channel.into_stream();
                            let _ = tokio::io::copy_bidirectional(&mut local_stream, &mut ssh_stream).await;
                        }
                    });
                }
            }
        }
    });

    Ok(ActiveTunnel {
        local_addr,
        shutdown_tx: Some(shutdown_tx),
        task,
    })
}
