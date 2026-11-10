use crate::models::server::{DecryptedServerCreds, ServerStats};
use async_trait::async_trait;
use russh::client::{self, Handle, Handler};
use russh_keys::key::PublicKey;
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
}

pub const FILES_AGENT_SRC: &str = include_str!("../files_agent.py");

pub fn build_agent_command() -> String {
    format!("python3 -c '{}'", FILES_AGENT_SRC.replace('\'', "'\\''"))
}

pub struct SimpleClientHandler;

#[async_trait]
impl Handler for SimpleClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

pub struct SshSession {
    pub handle: Mutex<Handle<SimpleClientHandler>>,
}

#[derive(Clone)]
pub struct SshPool {
    pool: Arc<Mutex<HashMap<i64, Arc<SshSession>>>>,
}

impl Default for SshPool {
    fn default() -> Self {
        Self::new()
    }
}

impl SshPool {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_or_connect(
        &self,
        creds: &DecryptedServerCreds,
    ) -> Result<Arc<SshSession>, SshError> {
        let mut map = self.pool.lock().await;

        if let Some(session) = map.get(&creds.id) {
            let handle = session.handle.lock().await;
            if !handle.is_closed() {
                return Ok(session.clone());
            }
        }

        let config = Arc::new(client::Config::default());
        let addr = format!("{}:{}", creds.host, creds.port);
        let mut handle = client::connect(config, addr, SimpleClientHandler).await?;

        if creds.auth_type == "password" {
            let password = creds.password.as_deref().unwrap_or("");
            let auth_res = handle
                .authenticate_password(&creds.username, password)
                .await?;
            if !auth_res {
                return Err(SshError::AuthFailed("Invalid username or password".into()));
            }
        } else if creds.auth_type == "key" {
            let key_str = creds.private_key.as_deref().unwrap_or("");
            let key = russh_keys::decode_secret_key(key_str, creds.passphrase.as_deref())
                .map_err(|e| SshError::KeyParse(e.to_string()))?;
            let auth_res = handle
                .authenticate_publickey(&creds.username, Arc::new(key))
                .await?;
            if !auth_res {
                return Err(SshError::AuthFailed("Key authentication rejected".into()));
            }
        } else {
            return Err(SshError::AuthFailed(format!(
                "Unknown auth type: {}",
                creds.auth_type
            )));
        }

        let session = Arc::new(SshSession {
            handle: Mutex::new(handle),
        });
        map.insert(creds.id, session.clone());
        Ok(session)
    }

    pub async fn disconnect(&self, server_id: i64) {
        let mut map = self.pool.lock().await;
        if let Some(session) = map.remove(&server_id) {
            let handle = session.handle.lock().await;
            let _ = handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
        }
    }

    pub async fn test_connection(&self, creds: &DecryptedServerCreds) -> Result<(), SshError> {
        let config = Arc::new(client::Config::default());
        let addr = format!("{}:{}", creds.host, creds.port);
        let timeout_duration = std::time::Duration::from_secs(10);
        let connect_fut = client::connect(config, addr, SimpleClientHandler);
        let mut handle = tokio::time::timeout(timeout_duration, connect_fut)
            .await
            .map_err(|_| SshError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "Koneksi SSH timed out (10s)")))??;

        if creds.auth_type == "password" {
            let password = creds.password.as_deref().unwrap_or("");
            let auth_res = handle
                .authenticate_password(&creds.username, password)
                .await?;
            if !auth_res {
                return Err(SshError::AuthFailed("Username atau password salah".into()));
            }
        } else if creds.auth_type == "key" {
            let key_str = creds.private_key.as_deref().unwrap_or("");
            let key = russh_keys::decode_secret_key(key_str, creds.passphrase.as_deref())
                .map_err(|e| SshError::KeyParse(format!("Format private key tidak valid: {}", e)))?;
            let auth_res = handle
                .authenticate_publickey(&creds.username, Arc::new(key))
                .await?;
            if !auth_res {
                return Err(SshError::AuthFailed("Autentikasi private key ditolak oleh server".into()));
            }
        } else {
            return Err(SshError::AuthFailed(format!(
                "Tipe autentikasi tidak didukung: {}",
                creds.auth_type
            )));
        }

        let _ = handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
        Ok(())
    }

    pub async fn run_command(
        &self,
        creds: &DecryptedServerCreds,
        cmd: &str,
    ) -> Result<String, SshError> {
        let session = self.get_or_connect(creds).await?;
        let mut channel = {
            let handle = session.handle.lock().await;
            handle.channel_open_session().await?
        };

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
        let session = self.get_or_connect(creds).await?;
        let mut channel = {
            let handle = session.handle.lock().await;
            handle.channel_open_session().await?
        };

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
