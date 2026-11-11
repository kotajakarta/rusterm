use crate::models::db::DbStore;
use crate::models::server::{DecryptedServerCreds, ServerStats};
use async_trait::async_trait;
use russh::client::{self, Handle, Handler};
use russh_keys::key::PublicKey;
use russh_keys::PublicKeyBase64;
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use tokio::sync::Mutex;

pub const STATS_SCRIPT: &str = r#"
echo '@@hostname'; hostname 2>/dev/null || cat /etc/hostname 2>/dev/null
echo '@@os'; ( . /etc/os-release 2>/dev/null && echo "$PRETTY_NAME" ) || uname -s
echo '@@kernel'; uname -sr 2>/dev/null
echo '@@uptime'; cat /proc/uptime 2>/dev/null
echo '@@load'; cat /proc/loadavg 2>/dev/null
echo '@@cpu'; nproc 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null
echo '@@mem'; head -5 /proc/meminfo 2>/dev/null
echo '@@disk'; df -Pk / 2>/dev/null | tail -1
"#;

pub fn parse_stats(raw: &str) -> ServerStats {
    let mut sections: HashMap<String, Vec<String>> = HashMap::new();
    let mut current_section: Option<String> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("@@") {
            let section_name = trimmed[2..].trim().to_string();
            current_section = Some(section_name.clone());
            sections.entry(section_name).or_default();
        } else if let Some(ref sec) = current_section {
            if !trimmed.is_empty() {
                sections.entry(sec.clone()).or_default().push(trimmed.to_string());
            }
        }
    }

    let first = |name: &str| -> String {
        sections
            .get(name)
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default()
    };

    let hostname = first("hostname");
    let os = first("os");
    let kernel = first("kernel");

    let uptime_str = first("uptime");
    let uptime_seconds = uptime_str
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok());

    let load_str = first("load");
    let load: Vec<f64> = load_str
        .split_whitespace()
        .take(3)
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    let cpu_str = first("cpu");
    let cpu_count = cpu_str.trim().parse::<u32>().ok();

    let mut mem_total: Option<u64> = None;
    let mut mem_available: Option<u64> = None;
    let mut mem_free: Option<u64> = None;

    if let Some(mem_lines) = sections.get("mem") {
        for line in mem_lines {
            let mut parts = line.splitn(2, ':');
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                let key = k.trim();
                let num_val = v
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(|kb| kb * 1024);
                if key == "MemTotal" {
                    mem_total = num_val;
                } else if key == "MemAvailable" {
                    mem_available = num_val;
                } else if key == "MemFree" {
                    mem_free = num_val;
                }
            }
        }
    }

    let mem_used = match (mem_total, mem_available.or(mem_free)) {
        (Some(total), Some(avail)) if total >= avail => Some(total - avail),
        _ => None,
    };

    let disk_str = first("disk");
    let disk_tokens: Vec<&str> = disk_str.split_whitespace().collect();
    let (disk_total, disk_used) = if disk_tokens.len() >= 3 {
        let total = disk_tokens[1].parse::<u64>().ok().map(|kb| kb * 1024);
        let used = disk_tokens[2].parse::<u64>().ok().map(|kb| kb * 1024);
        (total, used)
    } else {
        (None, None)
    };

    ServerStats {
        hostname,
        os,
        kernel,
        uptime_seconds,
        load,
        cpu_count,
        mem_total,
        mem_used,
        disk_total,
        disk_used,
    }
}

#[derive(Error, Debug)]
pub enum SshError {
    #[error("SSH error: {0}")]
    Russh(#[from] russh::Error),
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Key parse error: {0}")]
    KeyParse(String),
    #[error("Agent error: {0}")]
    AgentError(String),
    #[error("{0}")]
    HostKeyMismatch(String),
}

/// Result of [`SshPool::exec_capture`].
#[derive(Debug, Default)]
pub struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_status: Option<u32>,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.exit_status == Some(0)
    }

    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).trim().to_string()
    }

    /// Human readable reason for a failed command: stderr, else stdout, else the exit code.
    pub fn failure_message(&self) -> String {
        let stderr = String::from_utf8_lossy(&self.stderr).trim().to_string();
        if !stderr.is_empty() {
            return stderr;
        }
        let stdout = self.stdout_text();
        if !stdout.is_empty() {
            return stdout;
        }
        match self.exit_status {
            Some(code) => format!("Perintah gagal (exit code {})", code),
            None => "Perintah berakhir tanpa status".to_string(),
        }
    }
}

pub const FILES_AGENT_SRC: &str = include_str!("../files_agent.py");

pub fn build_agent_command() -> String {
    format!("python3 -c '{}'", FILES_AGENT_SRC.replace('\'', "'\\''"))
}

/// `"<algorithm> <base64>"`, the form stored in `servers.host_key`.
pub fn host_key_string(key: &PublicKey) -> String {
    format!("{} {}", key.name(), key.public_key_base64())
}

/// OpenSSH-style fingerprint, e.g. `SHA256:abc…`.
pub fn host_key_fingerprint(key: &PublicKey) -> String {
    format!("SHA256:{}", key.fingerprint())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyCheck {
    /// Matches the stored key.
    Trusted,
    /// No key stored yet: trust on first use.
    FirstUse,
    Mismatch {
        expected_fp: String,
        presented_fp: String,
    },
}

/// Compares the key a server presents with the stored one. Only the key blob is compared:
/// the algorithm label of an RSA key varies with the negotiated signature hash.
pub fn check_host_key(expected: Option<&str>, presented: &PublicKey) -> HostKeyCheck {
    let Some(stored) = expected else {
        return HostKeyCheck::FirstUse;
    };
    let stored_blob = stored.split_whitespace().last().unwrap_or("");
    if stored_blob == presented.public_key_base64() {
        return HostKeyCheck::Trusted;
    }
    let expected_fp = russh_keys::parse_public_key_base64(stored_blob)
        .map(|k| host_key_fingerprint(&k))
        .unwrap_or_else(|_| "(tidak dikenal)".to_string());
    HostKeyCheck::Mismatch {
        expected_fp,
        presented_fp: host_key_fingerprint(presented),
    }
}

/// Verifies the server's host key against `expected` and records what it saw, so the pool
/// can store a first-use key or report a mismatch after the handshake.
pub struct SimpleClientHandler {
    expected: Option<String>,
    seen: Arc<std::sync::Mutex<Option<(HostKeyCheck, String, String)>>>,
}

#[async_trait]
impl Handler for SimpleClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        let check = check_host_key(self.expected.as_deref(), server_public_key);
        let accept = !matches!(check, HostKeyCheck::Mismatch { .. });
        if let Ok(mut seen) = self.seen.lock() {
            *seen = Some((
                check,
                host_key_string(server_public_key),
                host_key_fingerprint(server_public_key),
            ));
        }
        Ok(accept)
    }
}

pub struct SshSession {
    pub handle: Mutex<Handle<SimpleClientHandler>>,
}

#[derive(Clone)]
pub struct SshPool {
    pool: Arc<Mutex<HashMap<i64, Arc<SshSession>>>>,
    /// Where trusted host keys live; `None` accepts every key (tests only).
    host_keys: Option<DbStore>,
}

impl Default for SshPool {
    fn default() -> Self {
        Self::new()
    }
}

impl SshPool {
    /// A pool that accepts any host key. Production code uses [`SshPool::with_host_keys`].
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(HashMap::new())),
            host_keys: None,
        }
    }

    /// A pool that verifies host keys against the servers table (trust on first use).
    pub fn with_host_keys(db: DbStore) -> Self {
        Self {
            pool: Arc::new(Mutex::new(HashMap::new())),
            host_keys: Some(db),
        }
    }

    pub async fn get_or_connect(
        &self,
        creds: &DecryptedServerCreds,
    ) -> Result<Arc<SshSession>, SshError> {
        {
            let map = self.pool.lock().await;
            if let Some(session) = map.get(&creds.id) {
                let handle = session.handle.lock().await;
                if !handle.is_closed() {
                    return Ok(session.clone());
                }
            }
        }

        let session = self.connect_new(creds).await?;
        let mut map = self.pool.lock().await;
        map.insert(creds.id, session.clone());
        Ok(session)
    }

    pub async fn connect_new(
        &self,
        creds: &DecryptedServerCreds,
    ) -> Result<Arc<SshSession>, SshError> {
        let mut config = client::Config::default();
        config.keepalive_interval = Some(std::time::Duration::from_secs(15));
        config.keepalive_max = 3;
        config.inactivity_timeout = Some(std::time::Duration::from_secs(60));
        let config = Arc::new(config);

        let addr = format!("{}:{}", creds.host, creds.port);

        // 1. TCP connect with 10s timeout and TCP_NODELAY
        let tcp_stream = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::net::TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| {
            SshError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("Koneksi TCP ke {} timed out (10s)", addr),
            ))
        })?
        .map_err(|e| {
            SshError::Io(std::io::Error::new(
                e.kind(),
                format!("Gagal menghubungkan ke {}: {}", addr, e),
            ))
        })?;

        let _ = tcp_stream.set_nodelay(true);

        // 2. SSH protocol handshake with 10s timeout, verifying the host key
        let store = self.host_keys.as_ref().filter(|_| creds.id > 0);
        let expected = store.and_then(|db| db.get_host_key(creds.id).ok().flatten());
        let seen = Arc::new(std::sync::Mutex::new(None));
        let handler = SimpleClientHandler {
            expected,
            seen: seen.clone(),
        };
        let connected = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client::connect_stream(config, tcp_stream, handler),
        )
        .await
        .map_err(|_| {
            SshError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "SSH protocol handshake timed out (10s)",
            ))
        })?;
        let seen_key = seen.lock().ok().and_then(|s| s.clone());
        let mut handle = match connected {
            Ok(h) => h,
            Err(e) => {
                if let Some((HostKeyCheck::Mismatch { expected_fp, presented_fp }, _, _)) = seen_key {
                    return Err(SshError::HostKeyMismatch(format!(
                        "Host key server berubah! Tersimpan {}, diterima {}. Jika server memang di-install ulang, buka Edit server lalu klik \"Reset host key\".",
                        expected_fp, presented_fp
                    )));
                }
                return Err(e.into());
            }
        };

        // 3. Authentication with 10s timeout
        if creds.auth_type == "password" {
            let password = creds.password.as_deref().unwrap_or("");
            let auth_res = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                handle.authenticate_password(&creds.username, password),
            )
            .await
            .map_err(|_| {
                SshError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Autentikasi password timed out (10s)",
                ))
            })??;

            if !auth_res {
                return Err(SshError::AuthFailed("Username atau password salah".into()));
            }
        } else if creds.auth_type == "key" {
            let key_str = creds.private_key.as_deref().unwrap_or("");
            let key = russh_keys::decode_secret_key(key_str, creds.passphrase.as_deref())
                .map_err(|e| SshError::KeyParse(format!("Format private key tidak valid: {}", e)))?;
            let auth_res = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                handle.authenticate_publickey(&creds.username, Arc::new(key)),
            )
            .await
            .map_err(|_| {
                SshError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Autentikasi private key timed out (10s)",
                ))
            })??;

            if !auth_res {
                return Err(SshError::AuthFailed("Autentikasi key ditolak oleh server".into()));
            }
        } else {
            return Err(SshError::AuthFailed(format!(
                "Tipe autentikasi tidak didukung: {}",
                creds.auth_type
            )));
        }

        // 4. Trust on first use: remember the key once authentication succeeded.
        if let (Some(db), Some((HostKeyCheck::FirstUse, key, fp))) = (store, seen_key) {
            if let Err(e) = db.set_host_key(creds.id, &key, &fp) {
                tracing::warn!("Gagal menyimpan host key server {}: {}", creds.id, e);
            }
        }

        Ok(Arc::new(SshSession {
            handle: Mutex::new(handle),
        }))
    }

    pub async fn open_channel(
        &self,
        creds: &DecryptedServerCreds,
    ) -> Result<russh::Channel<russh::client::Msg>, SshError> {
        // 1. Try existing session in pool first
        let existing_session = {
            let map = self.pool.lock().await;
            map.get(&creds.id).cloned()
        };

        if let Some(session) = existing_session {
            let channel_opt = {
                let handle = session.handle.lock().await;
                if !handle.is_closed() {
                    tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        handle.channel_open_session(),
                    )
                    .await
                    .ok()
                    .and_then(|r| r.ok())
                } else {
                    None
                }
            };

            if let Some(channel) = channel_opt {
                return Ok(channel);
            }

            // Stale / disconnected session in pool: evict it
            self.disconnect(creds.id).await;
        }

        // 2. Connect fresh session (or reconnect after evicting stale session)
        let new_session = self.connect_new(creds).await?;
        let channel = {
            let handle = new_session.handle.lock().await;
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                handle.channel_open_session(),
            )
            .await
            .map_err(|_| {
                SshError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Channel open timed out (10s)",
                ))
            })??
        };

        // Store connected session in pool
        {
            let mut map = self.pool.lock().await;
            map.insert(creds.id, new_session);
        }

        Ok(channel)
    }

    pub async fn is_session_alive(&self, server_id: i64) -> bool {
        let map = self.pool.lock().await;
        if let Some(session) = map.get(&server_id) {
            let handle = session.handle.lock().await;
            !handle.is_closed()
        } else {
            false
        }
    }

    pub async fn disconnect(&self, server_id: i64) {
        let session_opt = {
            let mut map = self.pool.lock().await;
            map.remove(&server_id)
        };
        if let Some(session) = session_opt {
            let handle = session.handle.lock().await;
            let _ = handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
        }
    }

    pub async fn disconnect_all(&self) {
        let sessions: Vec<Arc<SshSession>> = {
            let mut map = self.pool.lock().await;
            map.drain().map(|(_, s)| s).collect()
        };
        for session in sessions {
            let handle = session.handle.lock().await;
            let _ = handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
        }
    }

    pub async fn test_connection(&self, creds: &DecryptedServerCreds) -> Result<(), SshError> {
        let session = self.connect_new(creds).await?;
        let handle = session.handle.lock().await;
        let _ = handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
        Ok(())
    }

    pub async fn run_command(
        &self,
        creds: &DecryptedServerCreds,
        cmd: &str,
    ) -> Result<String, SshError> {
        let mut channel = self.open_channel(creds).await?;

        channel.exec(true, cmd).await?;
        let mut output = Vec::new();

        let timeout_duration = std::time::Duration::from_secs(15);
        let read_fut = async {
            while let Some(msg) = channel.wait().await {
                match msg {
                    russh::ChannelMsg::Data { ref data } => {
                        output.extend_from_slice(data);
                    }
                    russh::ChannelMsg::ExtendedData { ref data, .. } => {
                        output.extend_from_slice(data);
                    }
                    russh::ChannelMsg::ExitStatus { .. } | russh::ChannelMsg::Close => break,
                    _ => {}
                }
            }
        };

        tokio::time::timeout(timeout_duration, read_fut)
            .await
            .map_err(|_| SshError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "Command timed out")))?;

        Ok(String::from_utf8_lossy(&output).to_string())
    }

    pub async fn run_command_with_stdin(
        &self,
        creds: &DecryptedServerCreds,
        cmd: &str,
        stdin_data: &[u8],
        timeout_secs: u64,
    ) -> Result<Vec<u8>, SshError> {
        let mut channel = self.open_channel(creds).await?;

        channel.exec(true, cmd).await?;
        if !stdin_data.is_empty() {
            channel.data(stdin_data).await?;
        }
        channel.eof().await?;

        let mut output = Vec::new();
        let timeout_duration = std::time::Duration::from_secs(timeout_secs);
        let read_fut = async {
            while let Some(msg) = channel.wait().await {
                match msg {
                    russh::ChannelMsg::Data { ref data } => {
                        output.extend_from_slice(data);
                    }
                    russh::ChannelMsg::ExtendedData { ref data, .. } => {
                        output.extend_from_slice(data);
                    }
                    russh::ChannelMsg::ExitStatus { .. } | russh::ChannelMsg::Close => break,
                    _ => {}
                }
            }
        };

        tokio::time::timeout(timeout_duration, read_fut)
            .await
            .map_err(|_| SshError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "Command timed out")))?;

        Ok(output)
    }

    /// Runs `cmd` and keeps stdout, stderr and the exit status apart, so callers can tell a
    /// failed command from its output (the other helpers merge both streams and ignore the
    /// exit status).
    pub async fn exec_capture(
        &self,
        creds: &DecryptedServerCreds,
        cmd: &str,
        stdin_data: &[u8],
        timeout_secs: u64,
    ) -> Result<CommandOutput, SshError> {
        let mut channel = self.open_channel(creds).await?;

        channel.exec(true, cmd).await?;
        if !stdin_data.is_empty() {
            channel.data(stdin_data).await?;
        }
        channel.eof().await?;

        let mut out = CommandOutput::default();
        let read_fut = async {
            while let Some(msg) = channel.wait().await {
                match msg {
                    russh::ChannelMsg::Data { ref data } => out.stdout.extend_from_slice(data),
                    russh::ChannelMsg::ExtendedData { ref data, .. } => {
                        out.stderr.extend_from_slice(data)
                    }
                    russh::ChannelMsg::ExitStatus { exit_status } => {
                        out.exit_status = Some(exit_status)
                    }
                    russh::ChannelMsg::Eof | russh::ChannelMsg::Close => {
                        if out.exit_status.is_some() || matches!(msg, russh::ChannelMsg::Close) {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        };

        let timed_out = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), read_fut)
            .await
            .is_err();
        let _ = channel.close().await;
        if timed_out {
            return Err(SshError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("Perintah melewati batas waktu ({} detik)", timeout_secs),
            )));
        }
        Ok(out)
    }

    pub async fn run_agent(
        &self,
        creds: &DecryptedServerCreds,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value, SshError> {
        let cmd = build_agent_command();
        let stdin_bytes = serde_json::to_vec(payload)
            .map_err(|e| SshError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;
        let out_bytes = self.run_command_with_stdin(creds, &cmd, &stdin_bytes, 60).await?;
        let out_str = String::from_utf8_lossy(&out_bytes);
        let parsed: serde_json::Value = serde_json::from_str(out_str.trim()).map_err(|e| {
            SshError::AgentError(format!("Failed to parse agent output: {}. Raw: {}", e, out_str))
        })?;

        if parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(parsed.get("result").cloned().unwrap_or(serde_json::Value::Null))
        } else {
            let err_msg = parsed
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("File agent operation failed");
            Err(SshError::AgentError(err_msg.to_string()))
        }
    }
}
