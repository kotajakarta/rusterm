//! Server-to-server file transfers (Remote Sync and Direct Transfer in the file manager).
//!
//! A job streams the files agent's `pack` tar output from the source server into `unpack` on
//! the destination. russh grows a channel's receive window as soon as data arrives, not when it
//! is consumed, so the source is paced with a credit protocol (see `_CreditWriter` in
//! `files_agent.py`) to keep the relay's memory bounded.

use crate::models::server::DecryptedServerCreds;
use crate::services::ssh_pool::{build_agent_command, SshPool};
use rand::RngCore;
use russh::ChannelMsg;
use serde_json::{json, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// Bytes the source may send per credit byte (must match `CREDIT_BYTES` in the agent).
pub const CREDIT_BYTES: u64 = 1_048_576;
/// Credits granted up front: the most data in flight between the two servers.
pub const INITIAL_CREDITS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransferResult {
    pub files: u64,
    pub dirs: u64,
    pub links: u64,
    pub bytes: u64,
}

/// Mirrors `TransferJob` in `frontend/src/types.ts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferJob {
    pub id: String,
    pub remote_id: Option<i64>,
    pub remote_name: String,
    pub direction: String,
    pub state: String,
    pub source_server_id: i64,
    pub source_root: String,
    pub dest_server_id: i64,
    pub dest_root: String,
    pub rels: Vec<String>,
    pub read_gitignore: bool,
    pub sent_bytes: u64,
    pub total_bytes: u64,
    pub total_files: u64,
    pub result: Option<TransferResult>,
    pub error: Option<String>,
}

/// What to transfer; `rels` are relative to `source_root` and land under `dest_root`.
#[derive(Debug, Clone)]
pub struct TransferSpec {
    pub remote_id: Option<i64>,
    pub remote_name: String,
    pub direction: String,
    pub source_server_id: i64,
    pub source_root: String,
    pub dest_server_id: i64,
    pub dest_root: String,
    pub rels: Vec<String>,
    pub ignore: Vec<String>,
    pub read_gitignore: bool,
    /// Delete the sources after a successful copy (Direct Transfer "move").
    pub move_sources: bool,
}

struct JobEntry {
    job: Mutex<TransferJob>,
    cancelled: AtomicBool,
    notify: Notify,
}

/// The running job's side of the manager: progress updates, outcome, cancellation.
#[derive(Clone)]
pub struct JobHandle {
    entry: Arc<JobEntry>,
}

impl JobHandle {
    pub fn update(&self, f: impl FnOnce(&mut TransferJob)) {
        if let Ok(mut job) = self.entry.job.lock() {
            f(&mut job);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.entry.cancelled.load(Ordering::SeqCst)
    }

    /// Resolves once the job is cancelled (immediately if it already is).
    pub async fn cancelled(&self) {
        loop {
            let notified = self.entry.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }

    pub fn finish_ok(&self, result: TransferResult) {
        self.update(|j| {
            if j.state == "running" {
                j.state = "done".into();
                j.result = Some(result);
            }
        });
    }

    pub fn finish_err(&self, message: String) {
        self.update(|j| {
            if j.state == "running" {
                j.state = "error".into();
                j.error = Some(message);
            }
        });
    }
}

#[derive(Clone, Default)]
pub struct TransferManager {
    jobs: Arc<Mutex<HashMap<String, Arc<JobEntry>>>>,
}

impl TransferManager {
    pub fn create(&self, spec: &TransferSpec) -> (TransferJob, JobHandle) {
        let mut id_bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut id_bytes);
        let id: String = id_bytes.iter().map(|b| format!("{:02x}", b)).collect();
        let job = TransferJob {
            id: id.clone(),
            remote_id: spec.remote_id,
            remote_name: spec.remote_name.clone(),
            direction: spec.direction.clone(),
            state: "running".into(),
            source_server_id: spec.source_server_id,
            source_root: spec.source_root.clone(),
            dest_server_id: spec.dest_server_id,
            dest_root: spec.dest_root.clone(),
            rels: spec.rels.clone(),
            read_gitignore: spec.read_gitignore,
            sent_bytes: 0,
            total_bytes: 0,
            total_files: 0,
            result: None,
            error: None,
        };
        let entry = Arc::new(JobEntry {
            job: Mutex::new(job.clone()),
            cancelled: AtomicBool::new(false),
            notify: Notify::new(),
        });
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.insert(id, entry.clone());
        }
        (job, JobHandle { entry })
    }

    fn entry(&self, id: &str) -> Option<Arc<JobEntry>> {
        self.jobs.lock().ok()?.get(id).cloned()
    }

    pub fn get(&self, id: &str) -> Option<TransferJob> {
        let entry = self.entry(id)?;
        let job = entry.job.lock().ok()?.clone();
        Some(job)
    }

    /// Cancels a running job; finished jobs are returned unchanged.
    pub fn cancel(&self, id: &str) -> Option<TransferJob> {
        let entry = self.entry(id)?;
        let job = {
            let mut job = entry.job.lock().ok()?;
            if job.state == "running" {
                job.state = "cancelled".into();
                entry.cancelled.store(true, Ordering::SeqCst);
                entry.notify.notify_waiters();
            }
            job.clone()
        };
        Some(job)
    }
}

/// Normalizes an absolute path (`.`/`..`/duplicate slashes); `None` for relative paths.
fn normalize_abs(path: &str) -> Option<Vec<&str>> {
    if !path.starts_with('/') || path.contains('\0') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    Some(parts)
}

/// Paths relative to `root` for the agent (`"."` for the root itself). Paths outside the root
/// are rejected; containment is checked per path component, so `/var/www2` is not inside
/// `/var/www`.
pub fn relative_paths(root: &str, paths: &[String]) -> Result<Vec<String>, String> {
    let root_parts =
        normalize_abs(root).ok_or_else(|| format!("Folder sumber tidak valid: {}", root))?;
    paths
        .iter()
        .map(|p| {
            let parts = normalize_abs(p).ok_or_else(|| format!("Path tidak valid: {}", p))?;
            if parts.len() < root_parts.len() || parts[..root_parts.len()] != root_parts[..] {
                return Err(format!("Path di luar folder sumber {}: {}", root, p));
            }
            let rest = &parts[root_parts.len()..];
            Ok(if rest.is_empty() { ".".to_string() } else { rest.join("/") })
        })
        .collect()
}

type Channel = russh::Channel<russh::client::Msg>;

fn join_remote(root: &str, rel: &str) -> String {
    if rel == "." {
        root.to_string()
    } else {
        format!("{}/{}", root.trim_end_matches('/'), rel)
    }
}

/// The `error` of an agent JSON reply, or a readable fallback built from the raw output.
fn agent_error(raw: &[u8], fallback: &str) -> String {
    match serde_json::from_slice::<Value>(raw) {
        Ok(v) => v
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or(fallback)
            .to_string(),
        Err(_) => {
            let text = String::from_utf8_lossy(raw).trim().to_string();
            if text.is_empty() {
                fallback.to_string()
            } else {
                text
            }
        }
    }
}

async fn open_agent(pool: &SshPool, creds: &DecryptedServerCreds, header: &Value) -> Result<Channel, String> {
    let channel = pool
        .open_channel(creds)
        .await
        .map_err(|e| format!("Koneksi ke {} gagal: {}", creds.name, e))?;
    channel
        .exec(true, build_agent_command())
        .await
        .map_err(|e| format!("Agent gagal dijalankan di {}: {}", creds.name, e))?;
    let mut line = header.to_string().into_bytes();
    line.push(b'\n');
    channel
        .data(&line[..])
        .await
        .map_err(|e| format!("Gagal mengirim perintah ke {}: {}", creds.name, e))?;
    Ok(channel)
}

/// Reads a channel until it closes: (stdout, stderr).
async fn drain(channel: &mut Channel) -> (Vec<u8>, Vec<u8>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { ref data } => out.extend_from_slice(data),
            ChannelMsg::ExtendedData { ref data, .. } => err.extend_from_slice(data),
            ChannelMsg::Close => break,
            _ => {}
        }
    }
    (out, err)
}

enum PumpEnd {
    SourceDone { exit: Option<u32>, stderr: Vec<u8> },
    DestinationFailed,
    Cancelled,
}

async fn pump(src: &mut Channel, dst: &Channel, job: &JobHandle) -> Result<PumpEnd, String> {
    let mut forwarded: u64 = 0;
    let mut credited: u64 = 0;
    let mut exit = None;
    let mut stderr = Vec::new();
    loop {
        tokio::select! {
            _ = job.cancelled() => return Ok(PumpEnd::Cancelled),
            msg = src.wait() => match msg {
                Some(ChannelMsg::Data { ref data }) => {
                    // A stalled destination never opens its window again: stay cancellable.
                    let sent = tokio::select! {
                        r = dst.data(&data[..]) => r.is_ok(),
                        _ = job.cancelled() => return Ok(PumpEnd::Cancelled),
                    };
                    if !sent {
                        return Ok(PumpEnd::DestinationFailed);
                    }
                    forwarded += data.len() as u64;
                    job.update(|j| j.sent_bytes = forwarded);
                    while forwarded / CREDIT_BYTES > credited {
                        tokio::select! {
                            r = src.data(&[1u8][..]) => {
                                r.map_err(|e| format!("Koneksi ke server sumber terputus: {}", e))?
                            }
                            _ = job.cancelled() => return Ok(PumpEnd::Cancelled),
                        }
                        credited += 1;
                    }
                }
                Some(ChannelMsg::ExtendedData { ref data, .. }) => stderr.extend_from_slice(data),
                Some(ChannelMsg::ExitStatus { exit_status }) => exit = Some(exit_status),
                Some(ChannelMsg::Close) | None => return Ok(PumpEnd::SourceDone { exit, stderr }),
                _ => {}
            }
        }
    }
}

async fn transfer(
    pool: &SshPool,
    src: &DecryptedServerCreds,
    dst: &DecryptedServerCreds,
    spec: &TransferSpec,
    job: &JobHandle,
) -> Result<Option<TransferResult>, String> {
    let scan = pool
        .run_agent(
            src,
            &json!({
                "op": "scan",
                "root": spec.source_root,
                "rels": spec.rels,
                "ignore": spec.ignore,
                "read_gitignore": spec.read_gitignore,
            }),
        )
        .await
        .map_err(|e| e.to_string())?;
    job.update(|j| {
        j.total_bytes = scan.get("tar_bytes").and_then(Value::as_u64).unwrap_or(0);
        j.total_files = scan.get("files").and_then(Value::as_u64).unwrap_or(0);
    });

    let mut dst_ch = open_agent(pool, dst, &json!({ "op": "unpack", "root": spec.dest_root })).await?;
    let mut src_ch = match open_agent(
        pool,
        src,
        &json!({
            "op": "pack",
            "root": spec.source_root,
            "rels": spec.rels,
            "ignore": spec.ignore,
            "read_gitignore": spec.read_gitignore,
            "credit": true,
        }),
    )
    .await
    {
        Ok(ch) => ch,
        Err(e) => {
            let _ = dst_ch.close().await;
            return Err(e);
        }
    };
    if let Err(e) = src_ch.data(&[1u8; INITIAL_CREDITS][..]).await {
        let _ = src_ch.close().await;
        let _ = dst_ch.close().await;
        return Err(format!("Gagal memulai transfer: {}", e));
    }

    let end = pump(&mut src_ch, &dst_ch, job).await;
    let _ = src_ch.close().await;
    let end = match end {
        Ok(end) => end,
        Err(e) => {
            let _ = dst_ch.close().await;
            return Err(e);
        }
    };

    let source_error = match end {
        PumpEnd::Cancelled => {
            // Closing stdin mid-archive makes unpack fail and remove its temp file.
            let _ = dst_ch.close().await;
            return Ok(None);
        }
        PumpEnd::SourceDone { exit: Some(0), .. } | PumpEnd::DestinationFailed => None,
        PumpEnd::SourceDone { stderr, .. } => Some(agent_error(&stderr, "Server sumber gagal membuat arsip")),
    };

    let _ = dst_ch.eof().await;
    let drained = tokio::time::timeout(std::time::Duration::from_secs(300), drain(&mut dst_ch)).await;
    let _ = dst_ch.close().await;
    if let Some(err) = source_error {
        return Err(err);
    }
    let (out, err) = drained.map_err(|_| "Server tujuan tidak merespons setelah transfer".to_string())?;
    let reply: Value = serde_json::from_slice(&out)
        .map_err(|_| agent_error(&err, "Respons server tujuan tidak valid (pastikan python3 terpasang)"))?;
    if reply.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(agent_error(&out, "Server tujuan gagal menulis file"));
    }
    let r = &reply["result"];
    let count = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0);
    Ok(Some(TransferResult {
        files: count("files"),
        dirs: count("dirs"),
        links: count("links"),
        bytes: count("bytes"),
    }))
}

/// Runs a job to completion and records its outcome; spawn it with `tokio::spawn`.
pub async fn run_transfer(
    pool: SshPool,
    src: DecryptedServerCreds,
    dst: DecryptedServerCreds,
    spec: TransferSpec,
    job: JobHandle,
) {
    match transfer(&pool, &src, &dst, &spec, &job).await {
        Ok(None) => {}
        Ok(Some(result)) => {
            if spec.move_sources {
                let paths: Vec<String> = spec
                    .rels
                    .iter()
                    .filter(|r| r.as_str() != ".")
                    .map(|r| join_remote(&spec.source_root, r))
                    .collect();
                if !paths.is_empty() {
                    if let Err(e) = pool.run_agent(&src, &json!({ "op": "delete", "paths": paths })).await {
                        job.finish_err(format!("File sudah tersalin, tetapi sumber gagal dihapus: {}", e));
                        return;
                    }
                }
            }
            job.finish_ok(result);
        }
        Err(e) => job.finish_err(e),
    }
}
