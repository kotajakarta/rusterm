use crate::models::server::DecryptedServerCreds;
use crate::models::storage::DecryptedStorageAccount;
use crate::services::s3::{
    S3Client, DIRECT_PRESIGN_MAX_BYTES, MULTIPART_PART_BYTES, MULTIPART_THRESHOLD_BYTES,
};
use crate::services::ssh_pool::{build_agent_command, SshPool};
use chrono::Utc;
use futures::StreamExt;
use rand::RngCore;
use russh::ChannelMsg;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageTransferSpec {
    pub direction: String,
    pub mode: String,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageTransferJob {
    pub id: String,
    pub direction: String,
    pub state: String,
    pub requested_mode: String,
    pub effective_mode: String,
    pub source_label: String,
    pub dest_label: String,
    pub total_files: u64,
    pub transferred_files: u64,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub current_file: String,
    pub error: Option<String>,
    pub created_at: String,
}

struct JobEntry {
    job: Mutex<StorageTransferJob>,
    cancelled: AtomicBool,
    notify: Notify,
}

#[derive(Clone, Default)]
pub struct StorageTransferManager {
    jobs: Arc<Mutex<HashMap<String, Arc<JobEntry>>>>,
}

pub fn sanitize_relative_key(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return Err("Empty relative key".to_string());
    }
    let mut parts = Vec::new();
    for seg in trimmed.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." || seg.contains('\0') || seg.contains('\\') {
            return Err(format!("Unsafe path segment in key: '{}'", raw));
        }
        parts.push(seg);
    }
    if parts.is_empty() {
        return Err("Empty relative key after normalization".to_string());
    }
    Ok(parts.join("/"))
}

fn new_job_id() -> String {
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn join_s3_key(prefix: &str, rel: &str) -> String {
    let p = prefix.trim().trim_start_matches('/');
    let r = rel.trim().trim_start_matches('/');
    if p.is_empty() {
        r.to_string()
    } else if p.ends_with('/') {
        format!("{}{}", p, r)
    } else {
        format!("{}/{}", p, r)
    }
}

fn join_ssh_path(base: &str, rel: &str) -> String {
    let b = base.trim_end_matches('/');
    let r = rel.trim_start_matches('/');
    if b.is_empty() {
        format!("/{}", r)
    } else {
        format!("{}/{}", b, r)
    }
}

fn guess_mime(path: &str) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string()
}

impl StorageTransferManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list_jobs(&self) -> Vec<StorageTransferJob> {
        let g = self.jobs.lock().unwrap();
        let mut out: Vec<StorageTransferJob> = g
            .values()
            .map(|e| e.job.lock().unwrap().clone())
            .collect();
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        out
    }

    pub fn get_job(&self, id: &str) -> Option<StorageTransferJob> {
        let g = self.jobs.lock().unwrap();
        g.get(id).map(|e| e.job.lock().unwrap().clone())
    }

    pub fn cancel_job(&self, id: &str) -> bool {
        let entry = {
            let g = self.jobs.lock().unwrap();
            g.get(id).cloned()
        };
        if let Some(e) = entry {
            e.cancelled.store(true, Ordering::SeqCst);
            let mut j = e.job.lock().unwrap();
            if j.state == "queued" || j.state == "running" {
                j.state = "cancelled".to_string();
            }
            e.notify.notify_waiters();
            true
        } else {
            false
        }
    }

    pub async fn wait_finished(&self, id: &str) -> Option<StorageTransferJob> {
        let entry = {
            let g = self.jobs.lock().unwrap();
            g.get(id).cloned()
        }?;
        loop {
            let notified = entry.notify.notified();
            {
                let j = entry.job.lock().unwrap();
                if matches!(j.state.as_str(), "done" | "failed" | "cancelled") {
                    return Some(j.clone());
                }
            }
            notified.await;
        }
    }

    fn insert_job(&self, job: StorageTransferJob) -> Arc<JobEntry> {
        let entry = Arc::new(JobEntry {
            job: Mutex::new(job.clone()),
            cancelled: AtomicBool::new(false),
            notify: Notify::new(),
        });
        self.jobs.lock().unwrap().insert(job.id, entry.clone());
        entry
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_s3_to_s3(
        &self,
        s3: S3Client,
        spec: StorageTransferSpec,
        src_acc: DecryptedStorageAccount,
        src_bucket: String,
        src_prefix: String,
        dst_acc: DecryptedStorageAccount,
        dst_bucket: String,
        dst_prefix: String,
    ) -> StorageTransferJob {
        let same_account = src_acc.id == dst_acc.id && src_acc.endpoint == dst_acc.endpoint;
        let initial_mode = if same_account { "server_copy" } else { "relay" };
        let job = StorageTransferJob {
            id: new_job_id(),
            direction: "s3_to_s3".to_string(),
            state: "queued".to_string(),
            requested_mode: spec.mode.clone(),
            effective_mode: initial_mode.to_string(),
            source_label: format!("{}:{}/{}", src_acc.name, src_bucket, src_prefix),
            dest_label: format!("{}:{}/{}", dst_acc.name, dst_bucket, dst_prefix),
            total_files: 0,
            transferred_files: 0,
            total_bytes: 0,
            transferred_bytes: 0,
            current_file: String::new(),
            error: None,
            created_at: Utc::now().to_rfc3339(),
        };
        let entry = self.insert_job(job.clone());

        tokio::spawn(async move {
            let res = run_s3_to_s3(
                entry.clone(),
                s3,
                src_acc,
                src_bucket,
                src_prefix,
                dst_acc,
                dst_bucket,
                dst_prefix,
                spec.overwrite,
            )
            .await;
            finish_job(&entry, res);
        });

        job
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_ssh_to_s3(
        &self,
        s3: S3Client,
        pool: SshPool,
        spec: StorageTransferSpec,
        creds: DecryptedServerCreds,
        ssh_path: String,
        acc: DecryptedStorageAccount,
        bucket: String,
        s3_prefix: String,
    ) -> StorageTransferJob {
        let initial_mode = if spec.mode == "relay" { "relay" } else { "direct" };
        let job = StorageTransferJob {
            id: new_job_id(),
            direction: "ssh_to_s3".to_string(),
            state: "queued".to_string(),
            requested_mode: spec.mode.clone(),
            effective_mode: initial_mode.to_string(),
            source_label: format!("{}:{}", creds.name, ssh_path),
            dest_label: format!("{}:{}/{}", acc.name, bucket, s3_prefix),
            total_files: 0,
            transferred_files: 0,
            total_bytes: 0,
            transferred_bytes: 0,
            current_file: String::new(),
            error: None,
            created_at: Utc::now().to_rfc3339(),
        };
        let entry = self.insert_job(job.clone());

        tokio::spawn(async move {
            let res = run_ssh_to_s3(
                entry.clone(),
                s3,
                pool,
                creds,
                ssh_path,
                acc,
                bucket,
                s3_prefix,
                spec.mode,
            )
            .await;
            finish_job(&entry, res);
        });

        job
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_s3_to_ssh(
        &self,
        s3: S3Client,
        pool: SshPool,
        spec: StorageTransferSpec,
        acc: DecryptedStorageAccount,
        bucket: String,
        s3_prefix: String,
        creds: DecryptedServerCreds,
        ssh_path: String,
    ) -> StorageTransferJob {
        let initial_mode = if spec.mode == "relay" { "relay" } else { "direct" };
        let job = StorageTransferJob {
            id: new_job_id(),
            direction: "s3_to_ssh".to_string(),
            state: "queued".to_string(),
            requested_mode: spec.mode.clone(),
            effective_mode: initial_mode.to_string(),
            source_label: format!("{}:{}/{}", acc.name, bucket, s3_prefix),
            dest_label: format!("{}:{}", creds.name, ssh_path),
            total_files: 0,
            transferred_files: 0,
            total_bytes: 0,
            transferred_bytes: 0,
            current_file: String::new(),
            error: None,
            created_at: Utc::now().to_rfc3339(),
        };
        let entry = self.insert_job(job.clone());

        tokio::spawn(async move {
            let res = run_s3_to_ssh(
                entry.clone(),
                s3,
                pool,
                acc,
                bucket,
                s3_prefix,
                creds,
                ssh_path,
                spec.mode,
                spec.overwrite,
            )
            .await;
            finish_job(&entry, res);
        });

        job
    }
}

fn finish_job(entry: &Arc<JobEntry>, res: Result<(), String>) {
    let mut j = entry.job.lock().unwrap();
    if entry.cancelled.load(Ordering::SeqCst) {
        j.state = "cancelled".to_string();
    } else {
        match res {
            Ok(()) => j.state = "done".to_string(),
            Err(e) => {
                j.state = "failed".to_string();
                j.error = Some(e);
            }
        }
    }
    entry.notify.notify_waiters();
}

#[allow(clippy::too_many_arguments)]
async fn run_s3_to_s3(
    entry: Arc<JobEntry>,
    s3: S3Client,
    src_acc: DecryptedStorageAccount,
    src_bucket: String,
    src_prefix: String,
    dst_acc: DecryptedStorageAccount,
    dst_bucket: String,
    dst_prefix: String,
    overwrite: bool,
) -> Result<(), String> {
    {
        let mut j = entry.job.lock().unwrap();
        j.state = "running".to_string();
    }

    let objects = list_s3_source_objects(&s3, &src_acc, &src_bucket, &src_prefix).await?;
    let total_bytes: u64 = objects.iter().map(|o| o.size).sum();
    {
        let mut j = entry.job.lock().unwrap();
        j.total_files = objects.len() as u64;
        j.total_bytes = total_bytes;
    }

    let same_account = src_acc.id == dst_acc.id && src_acc.endpoint == dst_acc.endpoint;
    let strip_base = if src_prefix.ends_with('/') {
        src_prefix.as_str()
    } else if objects.len() == 1 && objects[0].key == src_prefix {
        src_prefix.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
    } else {
        src_prefix.as_str()
    };

    for obj in objects {
        if entry.cancelled.load(Ordering::SeqCst) {
            return Ok(());
        }
        let rel = obj
            .key
            .strip_prefix(strip_base)
            .unwrap_or(&obj.key)
            .trim_start_matches('/');
        let rel_clean = sanitize_relative_key(rel)?;
        let dst_key = join_s3_key(&dst_prefix, &rel_clean);

        {
            let mut j = entry.job.lock().unwrap();
            j.current_file = obj.key.clone();
        }

        if !overwrite && s3.head_object(&dst_acc, &dst_bucket, &dst_key).await.is_ok() {
            let mut j = entry.job.lock().unwrap();
            j.transferred_files += 1;
            j.transferred_bytes += obj.size;
            continue;
        }

        if same_account && obj.size <= DIRECT_PRESIGN_MAX_BYTES {
            s3.copy_object_across_buckets(&src_acc, &src_bucket, &obj.key, &dst_bucket, &dst_key)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            {
                let mut j = entry.job.lock().unwrap();
                j.effective_mode = "relay".to_string();
            }
            let resp = s3
                .get_object_stream(&src_acc, &src_bucket, &obj.key)
                .await
                .map_err(|e| e.to_string())?;
            let ct = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();

            if obj.size < MULTIPART_THRESHOLD_BYTES {
                let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
                s3.put_object(&dst_acc, &dst_bucket, &dst_key, bytes.to_vec(), &ct)
                    .await
                    .map_err(|e| e.to_string())?;
            } else {
                stream_response_multipart(
                    &s3,
                    &dst_acc,
                    &dst_bucket,
                    &dst_key,
                    &ct,
                    resp,
                    &entry.cancelled,
                )
                .await?;
            }
        }

        let mut j = entry.job.lock().unwrap();
        j.transferred_files += 1;
        j.transferred_bytes += obj.size;
    }

    Ok(())
}

async fn list_s3_source_objects(
    s3: &S3Client,
    acc: &DecryptedStorageAccount,
    bucket: &str,
    prefix: &str,
) -> Result<Vec<crate::models::storage::StorageObject>, String> {
    let mut objects = s3
        .list_objects_recursive(acc, bucket, prefix)
        .await
        .map_err(|e| e.to_string())?;
    if objects.is_empty() && !prefix.is_empty() && !prefix.ends_with('/') {
        if let Ok(head) = s3.head_object(acc, bucket, prefix).await {
            objects.push(crate::models::storage::StorageObject {
                key: prefix.to_string(),
                name: prefix.rsplit('/').next().unwrap_or(prefix).to_string(),
                is_dir: false,
                size: head.size,
                last_modified: String::new(),
                etag: head.etag,
            });
        }
    }
    Ok(objects)
}

async fn stream_response_multipart(
    s3: &S3Client,
    dst_acc: &DecryptedStorageAccount,
    dst_bucket: &str,
    dst_key: &str,
    content_type: &str,
    resp: reqwest::Response,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let upload_id = s3
        .initiate_multipart_upload(dst_acc, dst_bucket, dst_key, content_type)
        .await
        .map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(MULTIPART_PART_BYTES);
    let mut parts: Vec<(usize, String)> = Vec::new();
    let mut part_num: usize = 1;

    while let Some(chunk_res) = stream.next().await {
        if cancelled.load(Ordering::SeqCst) {
            let _ = s3
                .abort_multipart_upload(dst_acc, dst_bucket, dst_key, &upload_id)
                .await;
            return Ok(());
        }
        let chunk = match chunk_res {
            Ok(c) => c,
            Err(e) => {
                let _ = s3
                    .abort_multipart_upload(dst_acc, dst_bucket, dst_key, &upload_id)
                    .await;
                return Err(e.to_string());
            }
        };
        buf.extend_from_slice(&chunk);
        if buf.len() >= MULTIPART_PART_BYTES {
            let data = std::mem::take(&mut buf);
            match s3
                .upload_part(dst_acc, dst_bucket, dst_key, &upload_id, part_num, data)
                .await
            {
                Ok(etag) => {
                    parts.push((part_num, etag));
                    part_num += 1;
                }
                Err(e) => {
                    let _ = s3
                        .abort_multipart_upload(dst_acc, dst_bucket, dst_key, &upload_id)
                        .await;
                    return Err(e.to_string());
                }
            }
        }
    }

    if !buf.is_empty() || parts.is_empty() {
        match s3
            .upload_part(dst_acc, dst_bucket, dst_key, &upload_id, part_num, buf)
            .await
        {
            Ok(etag) => parts.push((part_num, etag)),
            Err(e) => {
                let _ = s3
                    .abort_multipart_upload(dst_acc, dst_bucket, dst_key, &upload_id)
                    .await;
                return Err(e.to_string());
            }
        }
    }

    s3.complete_multipart_upload(dst_acc, dst_bucket, dst_key, &upload_id, &parts)
        .await
        .map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
async fn run_ssh_to_s3(
    entry: Arc<JobEntry>,
    s3: S3Client,
    pool: SshPool,
    creds: DecryptedServerCreds,
    ssh_path: String,
    acc: DecryptedStorageAccount,
    bucket: String,
    s3_prefix: String,
    mode: String,
) -> Result<(), String> {
    {
        let mut j = entry.job.lock().unwrap();
        j.state = "running".to_string();
    }

    let walk = pool
        .run_agent(&creds, &json!({ "op": "s3_walk", "path": ssh_path }))
        .await
        .map_err(|e| e.to_string())?;
    let files = walk
        .get("files")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let total_bytes: u64 = files
        .iter()
        .filter_map(|f| f.get("size").and_then(|v| v.as_u64()))
        .sum();
    {
        let mut j = entry.job.lock().unwrap();
        j.total_files = files.len() as u64;
        j.total_bytes = total_bytes;
    }

    let mut use_relay = mode == "relay";

    for f in files {
        if entry.cancelled.load(Ordering::SeqCst) {
            return Ok(());
        }
        let rel = f.get("rel").and_then(|v| v.as_str()).unwrap_or("");
        let full_path = f.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let size = f.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        let rel_clean = sanitize_relative_key(rel)?;
        let dst_key = join_s3_key(&s3_prefix, &rel_clean);
        let ct = guess_mime(&rel_clean);

        {
            let mut j = entry.job.lock().unwrap();
            j.current_file = full_path.to_string();
        }

        let try_direct = !use_relay && size <= DIRECT_PRESIGN_MAX_BYTES;
        if try_direct {
            let url = s3
                .presign_url(&acc, "PUT", &bucket, &dst_key, 1800)
                .map_err(|e| e.to_string())?;
            let req = json!({
                "op": "http_put_file",
                "path": full_path,
                "url": url,
                "content_type": ct,
            });
            match run_agent_long(&pool, &creds, &req, 600).await {
                Ok(_) => {
                    let mut j = entry.job.lock().unwrap();
                    j.transferred_files += 1;
                    j.transferred_bytes += size;
                    continue;
                }
                Err(err) if mode == "auto" => {
                    tracing::warn!(
                        "Direct presigned upload failed on SSH server ({}); falling back to relay",
                        err
                    );
                    use_relay = true;
                    let mut j = entry.job.lock().unwrap();
                    j.effective_mode = "relay".to_string();
                }
                Err(err) => return Err(err),
            }
        }

        // Relay mode: read file via SSH channel and upload to S3
        let data = ssh_read_file_bytes(&pool, &creds, full_path).await?;
        s3.put_object(&acc, &bucket, &dst_key, data, &ct)
            .await
            .map_err(|e| e.to_string())?;

        let mut j = entry.job.lock().unwrap();
        j.transferred_files += 1;
        j.transferred_bytes += size;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_s3_to_ssh(
    entry: Arc<JobEntry>,
    s3: S3Client,
    pool: SshPool,
    acc: DecryptedStorageAccount,
    bucket: String,
    s3_prefix: String,
    creds: DecryptedServerCreds,
    ssh_path: String,
    mode: String,
    overwrite: bool,
) -> Result<(), String> {
    {
        let mut j = entry.job.lock().unwrap();
        j.state = "running".to_string();
    }

    let objects = list_s3_source_objects(&s3, &acc, &bucket, &s3_prefix).await?;
    let total_bytes: u64 = objects.iter().map(|o| o.size).sum();
    {
        let mut j = entry.job.lock().unwrap();
        j.total_files = objects.len() as u64;
        j.total_bytes = total_bytes;
    }

    let strip_base = if s3_prefix.ends_with('/') {
        s3_prefix.as_str()
    } else if objects.len() == 1 && objects[0].key == s3_prefix {
        s3_prefix.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
    } else {
        s3_prefix.as_str()
    };

    let mut use_relay = mode == "relay";

    for obj in objects {
        if entry.cancelled.load(Ordering::SeqCst) {
            return Ok(());
        }
        let rel = obj
            .key
            .strip_prefix(strip_base)
            .unwrap_or(&obj.key)
            .trim_start_matches('/');
        let rel_clean = sanitize_relative_key(rel)?;
        let dst_path = join_ssh_path(&ssh_path, &rel_clean);

        {
            let mut j = entry.job.lock().unwrap();
            j.current_file = obj.key.clone();
        }

        if !use_relay {
            let url = s3
                .presign_url(&acc, "GET", &bucket, &obj.key, 1800)
                .map_err(|e| e.to_string())?;
            let req = json!({
                "op": "http_get_file",
                "path": dst_path,
                "url": url,
                "overwrite": overwrite,
            });
            match run_agent_long(&pool, &creds, &req, 600).await {
                Ok(_) => {
                    let mut j = entry.job.lock().unwrap();
                    j.transferred_files += 1;
                    j.transferred_bytes += obj.size;
                    continue;
                }
                Err(err) if mode == "auto" => {
                    tracing::warn!(
                        "Direct presigned download failed on SSH server ({}); falling back to relay",
                        err
                    );
                    use_relay = true;
                    let mut j = entry.job.lock().unwrap();
                    j.effective_mode = "relay".to_string();
                }
                Err(err) => return Err(err),
            }
        }

        // Relay mode: download from S3 and write via SSH channel
        let resp = s3
            .get_object_stream(&acc, &bucket, &obj.key)
            .await
            .map_err(|e| e.to_string())?;
        let data = resp.bytes().await.map_err(|e| e.to_string())?;
        ssh_write_file_bytes(&pool, &creds, &dst_path, overwrite, &data).await?;

        let mut j = entry.job.lock().unwrap();
        j.transferred_files += 1;
        j.transferred_bytes += obj.size;
    }

    Ok(())
}

async fn run_agent_long(
    pool: &SshPool,
    creds: &DecryptedServerCreds,
    payload: &serde_json::Value,
    timeout_secs: u64,
) -> Result<serde_json::Value, String> {
    let cmd = build_agent_command();
    let mut stdin_bytes = serde_json::to_vec(payload).map_err(|e| e.to_string())?;
    stdin_bytes.push(b'\n');
    let out_bytes = pool
        .run_command_with_stdin(creds, &cmd, &stdin_bytes, timeout_secs)
        .await
        .map_err(|e| e.to_string())?;
    let out_str = String::from_utf8_lossy(&out_bytes);
    let parsed: serde_json::Value =
        serde_json::from_str(out_str.trim()).map_err(|e| format!("Invalid agent output: {}", e))?;
    if parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        Ok(parsed.get("result").cloned().unwrap_or(serde_json::Value::Null))
    } else {
        Err(parsed
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("SSH agent operation failed")
            .to_string())
    }
}

async fn ssh_read_file_bytes(
    pool: &SshPool,
    creds: &DecryptedServerCreds,
    path: &str,
) -> Result<Vec<u8>, String> {
    let cmd = build_agent_command();
    let mut header = serde_json::to_vec(&json!({ "op": "s3_read_raw", "path": path }))
        .map_err(|e| e.to_string())?;
    header.push(b'\n');
    let mut ch = pool.open_channel(creds).await.map_err(|e| e.to_string())?;
    ch.exec(true, cmd.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    ch.data(&header[..]).await.map_err(|e| e.to_string())?;
    ch.eof().await.map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    let mut err_buf = Vec::new();
    let mut exit_code: Option<u32> = None;
    while let Some(msg) = ch.wait().await {
        match msg {
            ChannelMsg::Data { data } => out.extend_from_slice(&data),
            ChannelMsg::ExtendedData { data, .. } => err_buf.extend_from_slice(&data),
            ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
            ChannelMsg::Close => break,
            _ => {}
        }
    }
    if exit_code.unwrap_or(0) != 0 {
        return Err(format!(
            "SSH read failed: {}",
            String::from_utf8_lossy(&err_buf).trim()
        ));
    }
    Ok(out)
}

async fn ssh_write_file_bytes(
    pool: &SshPool,
    creds: &DecryptedServerCreds,
    path: &str,
    overwrite: bool,
    data: &[u8],
) -> Result<(), String> {
    let cmd = build_agent_command();
    let mut header = serde_json::to_vec(&json!({
        "op": "s3_write_raw",
        "path": path,
        "overwrite": overwrite,
    }))
    .map_err(|e| e.to_string())?;
    header.push(b'\n');
    let mut ch = pool.open_channel(creds).await.map_err(|e| e.to_string())?;
    ch.exec(true, cmd.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    ch.data(&header[..]).await.map_err(|e| e.to_string())?;
    for chunk in data.chunks(64 * 1024) {
        ch.data(chunk).await.map_err(|e| e.to_string())?;
    }
    ch.eof().await.map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    while let Some(msg) = ch.wait().await {
        match msg {
            ChannelMsg::Data { data } => out.extend_from_slice(&data),
            ChannelMsg::Close => break,
            _ => {}
        }
    }
    let parsed: serde_json::Value = serde_json::from_slice(&out)
        .map_err(|e| format!("Invalid SSH write response: {}", e))?;
    if parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        Ok(())
    } else {
        Err(parsed
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("SSH write failed")
            .to_string())
    }
}
