use crate::models::storage::{
    DecryptedStorageAccount, StorageBucket, StorageDeleteOut, StorageListOut, StorageObject,
    StorageVerifyOut,
};
use chrono::Utc;
use futures::stream::{self, StreamExt};
use hmac::{Hmac, KeyInit, Mac};
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, HOST};
use reqwest::{Method, Response, StatusCode};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::Duration;
use thiserror::Error;
use url::Url;

type HmacSha256 = Hmac<Sha256>;

pub const MULTIPART_THRESHOLD_BYTES: u64 = 64 * 1024 * 1024;
pub const MULTIPART_PART_BYTES: usize = 16 * 1024 * 1024;
pub const DIRECT_PRESIGN_MAX_BYTES: u64 = 5 * 1024 * 1024 * 1024;

#[derive(Error, Debug)]
pub enum S3Error {
    #[error("Invalid endpoint URL: {0}")]
    InvalidEndpoint(String),
    #[error("Invalid bucket name: {0}")]
    InvalidBucket(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("S3 error ({status}): {message}")]
    Remote {
        status: u16,
        code: String,
        message: String,
    },
    #[error("XML parse error: {0}")]
    Xml(String),
}

#[derive(Debug, Clone)]
pub struct HeadObjectOut {
    pub size: u64,
    pub content_type: String,
    pub etag: String,
}

#[derive(Clone)]
pub struct S3Client {
    http: reqwest::Client,
}

impl Default for S3Client {
    fn default() -> Self {
        Self::new()
    }
}

impl S3Client {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { http }
    }

    pub fn presign_url(
        &self,
        acc: &DecryptedStorageAccount,
        method: &str,
        bucket: &str,
        key: &str,
        expires_secs: u64,
    ) -> Result<String, S3Error> {
        let expires = expires_secs.clamp(60, 7 * 24 * 3600);
        let (base_url, host_header, canonical_uri) = build_target_url(acc, Some(bucket), Some(key))?;

        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();
        let region = effective_region(&acc.region);
        let scope = format!("{}/{}/s3/aws4_request", date_stamp, region);
        let credential = format!("{}/{}", acc.access_key_id, scope);

        let mut query = BTreeMap::new();
        query.insert("X-Amz-Algorithm".to_string(), "AWS4-HMAC-SHA256".to_string());
        query.insert("X-Amz-Credential".to_string(), credential);
        query.insert("X-Amz-Date".to_string(), amz_date.clone());
        query.insert("X-Amz-Expires".to_string(), expires.to_string());
        query.insert("X-Amz-SignedHeaders".to_string(), "host".to_string());

        let canonical_qs = build_canonical_query(&query);
        let canonical_headers = format!("host:{}\n", host_header);
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\nhost\nUNSIGNED-PAYLOAD",
            method.to_uppercase(),
            canonical_uri,
            canonical_qs,
            canonical_headers
        );

        let signature = compute_sigv4(
            &acc.secret_access_key,
            &date_stamp,
            &region,
            &amz_date,
            &canonical_request,
        );

        Ok(format!(
            "{}?{}&X-Amz-Signature={}",
            base_url, canonical_qs, signature
        ))
    }

    pub async fn verify_account(
        &self,
        acc: &DecryptedStorageAccount,
    ) -> Result<StorageVerifyOut, S3Error> {
        validate_endpoint_url(&acc.endpoint)?;
        match self.list_buckets(acc).await {
            Ok(buckets) => Ok(StorageVerifyOut {
                ok: true,
                message: format!("Terhubung ({} bucket ditemukan)", buckets.len()),
                buckets_count: buckets.len(),
                scoped_bucket_fallback: false,
            }),
            Err(S3Error::Remote { status: 403, .. }) if !acc.default_bucket.trim().is_empty() => {
                let bucket = acc.default_bucket.trim();
                self.list_objects_v2(acc, bucket, "", None, 1).await?;
                Ok(StorageVerifyOut {
                    ok: true,
                    message: format!(
                        "Terhubung ke bucket '{}' (token scoped, ListBuckets tidak diizinkan)",
                        bucket
                    ),
                    buckets_count: 1,
                    scoped_bucket_fallback: true,
                })
            }
            Err(e) => Err(e),
        }
    }

    pub async fn list_buckets(
        &self,
        acc: &DecryptedStorageAccount,
    ) -> Result<Vec<StorageBucket>, S3Error> {
        let resp = self
            .send_signed(acc, Method::GET, None, None, &BTreeMap::new(), &[], Vec::new())
            .await?;
        let text = check_response_text(resp).await?;
        parse_list_buckets_xml(&text)
    }

    pub async fn create_bucket(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
    ) -> Result<(), S3Error> {
        validate_bucket_name(bucket)?;
        let region = effective_region(&acc.region);
        let body = if region != "us-east-1" && region != "auto" {
            format!(
                r#"<CreateBucketConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><LocationConstraint>{}</LocationConstraint></CreateBucketConfiguration>"#,
                xml_escape(&region)
            )
            .into_bytes()
        } else {
            Vec::new()
        };
        let mut extra = Vec::new();
        if !body.is_empty() {
            extra.push(("content-type", "application/xml".to_string()));
        }
        let resp = self
            .send_signed(acc, Method::PUT, Some(bucket), None, &BTreeMap::new(), &extra, body)
            .await?;
        check_response_empty(resp).await
    }

    pub async fn delete_bucket(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
    ) -> Result<(), S3Error> {
        validate_bucket_name(bucket)?;
        let resp = self
            .send_signed(acc, Method::DELETE, Some(bucket), None, &BTreeMap::new(), &[], Vec::new())
            .await?;
        check_response_empty(resp).await
    }

    pub async fn list_objects_v2(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        prefix: &str,
        continuation_token: Option<&str>,
        max_keys: u32,
    ) -> Result<StorageListOut, S3Error> {
        self.list_objects_v2_inner(acc, bucket, prefix, Some("/"), continuation_token, max_keys)
            .await
    }

    pub async fn list_objects_recursive(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        prefix: &str,
    ) -> Result<Vec<StorageObject>, S3Error> {
        let mut all = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let page = self
                .list_objects_v2_inner(acc, bucket, prefix, None, token.as_deref(), 1000)
                .await?;
            for entry in page.entries {
                if !entry.is_dir && !entry.key.ends_with('/') {
                    all.push(entry);
                }
            }
            if page.is_truncated {
                token = page.next_continuation_token;
                if token.is_none() {
                    break;
                }
            } else {
                break;
            }
        }
        Ok(all)
    }

    async fn list_objects_v2_inner(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        prefix: &str,
        delimiter: Option<&str>,
        continuation_token: Option<&str>,
        max_keys: u32,
    ) -> Result<StorageListOut, S3Error> {
        let mut query = BTreeMap::new();
        query.insert("list-type".to_string(), "2".to_string());
        if let Some(d) = delimiter {
            query.insert("delimiter".to_string(), d.to_string());
        }
        if !prefix.is_empty() {
            query.insert("prefix".to_string(), prefix.to_string());
        }
        if let Some(tok) = continuation_token {
            if !tok.is_empty() {
                query.insert("continuation-token".to_string(), tok.to_string());
            }
        }
        query.insert("max-keys".to_string(), max_keys.clamp(1, 1000).to_string());

        let resp = self
            .send_signed(acc, Method::GET, Some(bucket), None, &query, &[], Vec::new())
            .await?;
        let text = check_response_text(resp).await?;
        parse_list_objects_xml(&text, bucket, prefix)
    }

    pub async fn head_object(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        key: &str,
    ) -> Result<HeadObjectOut, S3Error> {
        let resp = self
            .send_signed(acc, Method::HEAD, Some(bucket), Some(key), &BTreeMap::new(), &[], Vec::new())
            .await?;
        if !resp.status().is_success() {
            return Err(S3Error::Remote {
                status: resp.status().as_u16(),
                code: "HeadFailed".into(),
                message: format!("HTTP {}", resp.status().as_u16()),
            });
        }
        let size = resp
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();
        Ok(HeadObjectOut {
            size,
            content_type,
            etag,
        })
    }

    pub async fn put_object(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<(), S3Error> {
        let extra = vec![("content-type", content_type.to_string())];
        let resp = self
            .send_signed(acc, Method::PUT, Some(bucket), Some(key), &BTreeMap::new(), &extra, data)
            .await?;
        check_response_empty(resp).await
    }

    pub async fn initiate_multipart_upload(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        key: &str,
        content_type: &str,
    ) -> Result<String, S3Error> {
        let mut query = BTreeMap::new();
        query.insert("uploads".to_string(), String::new());
        let extra = vec![("content-type", content_type.to_string())];
        let resp = self
            .send_signed(acc, Method::POST, Some(bucket), Some(key), &query, &extra, Vec::new())
            .await?;
        let text = check_response_text(resp).await?;
        parse_tag_value(&text, "UploadId")
            .ok_or_else(|| S3Error::Xml("Missing UploadId in InitiateMultipartUploadResult".into()))
    }

    pub async fn upload_part(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        key: &str,
        upload_id: &str,
        part_number: usize,
        data: Vec<u8>,
    ) -> Result<String, S3Error> {
        let mut query = BTreeMap::new();
        query.insert("partNumber".to_string(), part_number.to_string());
        query.insert("uploadId".to_string(), upload_id.to_string());
        let resp = self
            .send_signed(acc, Method::PUT, Some(bucket), Some(key), &query, &[], data)
            .await?;
        if !resp.status().is_success() {
            return Err(extract_s3_error(resp).await);
        }
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        Ok(etag)
    }

    pub async fn complete_multipart_upload(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: &[(usize, String)],
    ) -> Result<(), S3Error> {
        let mut query = BTreeMap::new();
        query.insert("uploadId".to_string(), upload_id.to_string());
        let mut xml = String::from("<CompleteMultipartUpload>");
        for (num, etag) in parts {
            xml.push_str(&format!(
                "<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>",
                num,
                xml_escape(etag)
            ));
        }
        xml.push_str("</CompleteMultipartUpload>");
        let extra = vec![("content-type", "application/xml".to_string())];
        let resp = self
            .send_signed(
                acc,
                Method::POST,
                Some(bucket),
                Some(key),
                &query,
                &extra,
                xml.into_bytes(),
            )
            .await?;
        check_response_empty(resp).await
    }

    pub async fn abort_multipart_upload(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        key: &str,
        upload_id: &str,
    ) -> Result<(), S3Error> {
        let mut query = BTreeMap::new();
        query.insert("uploadId".to_string(), upload_id.to_string());
        let resp = self
            .send_signed(acc, Method::DELETE, Some(bucket), Some(key), &query, &[], Vec::new())
            .await?;
        check_response_empty(resp).await
    }

    pub async fn get_object_stream(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        key: &str,
    ) -> Result<Response, S3Error> {
        let resp = self
            .send_signed(acc, Method::GET, Some(bucket), Some(key), &BTreeMap::new(), &[], Vec::new())
            .await?;
        if !resp.status().is_success() {
            return Err(extract_s3_error(resp).await);
        }
        Ok(resp)
    }

    pub async fn copy_object_across_buckets(
        &self,
        acc: &DecryptedStorageAccount,
        source_bucket: &str,
        source_key: &str,
        dest_bucket: &str,
        dest_key: &str,
    ) -> Result<(), S3Error> {
        if source_bucket == dest_bucket && source_key == dest_key {
            return Err(S3Error::InvalidKey(
                "Source and destination key cannot be identical on the same bucket".into(),
            ));
        }
        let encoded_source = format!(
            "/{}/{}",
            aws_uri_encode(source_bucket, true),
            aws_uri_encode(source_key.trim_start_matches('/'), false)
        );
        let extra = vec![("x-amz-copy-source", encoded_source)];
        let resp = self
            .send_signed(
                acc,
                Method::PUT,
                Some(dest_bucket),
                Some(dest_key),
                &BTreeMap::new(),
                &extra,
                Vec::new(),
            )
            .await?;
        check_response_empty(resp).await
    }

    pub async fn delete_object(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        key: &str,
    ) -> Result<(), S3Error> {
        let resp = self
            .send_signed(
                acc,
                Method::DELETE,
                Some(bucket),
                Some(key),
                &BTreeMap::new(),
                &[],
                Vec::new(),
            )
            .await?;
        check_response_empty(resp).await
    }

    pub async fn delete_objects_parallel(
        &self,
        acc: &DecryptedStorageAccount,
        bucket: &str,
        keys: Vec<String>,
    ) -> StorageDeleteOut {
        let results: Vec<(String, Result<(), S3Error>)> = stream::iter(keys)
            .map(|k| {
                let this = self.clone();
                let acc = acc.clone();
                let bucket = bucket.to_string();
                async move {
                    let res = this.delete_object(&acc, &bucket, &k).await;
                    (k, res)
                }
            })
            .buffer_unordered(8)
            .collect()
            .await;

        let mut deleted = Vec::new();
        let mut failed = Vec::new();
        for (k, res) in results {
            if res.is_ok() {
                deleted.push(k);
            } else {
                failed.push(k);
            }
        }
        StorageDeleteOut { deleted, failed }
    }

    #[allow(clippy::too_many_arguments)]
    async fn send_signed(
        &self,
        acc: &DecryptedStorageAccount,
        method: Method,
        bucket: Option<&str>,
        key: Option<&str>,
        query: &BTreeMap<String, String>,
        extra_headers: &[(&str, String)],
        body: Vec<u8>,
    ) -> Result<Response, S3Error> {
        let (base_url, host_header, canonical_uri) = build_target_url(acc, bucket, key)?;
        let canonical_qs = build_canonical_query(query);
        let full_url = if canonical_qs.is_empty() {
            base_url
        } else {
            format!("{}?{}", base_url, canonical_qs)
        };

        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();
        let region = effective_region(&acc.region);

        let payload_hash = sha256_hex(&body);

        let mut signed_map = BTreeMap::new();
        signed_map.insert("host".to_string(), host_header.clone());
        signed_map.insert("x-amz-content-sha256".to_string(), payload_hash.clone());
        signed_map.insert("x-amz-date".to_string(), amz_date.clone());
        for (k, v) in extra_headers {
            let lk = k.to_ascii_lowercase();
            if lk.starts_with("x-amz-") || lk == "content-type" {
                signed_map.insert(lk, v.trim().to_string());
            }
        }

        let mut canonical_headers = String::new();
        let mut signed_header_names = Vec::new();
        for (k, v) in &signed_map {
            canonical_headers.push_str(k);
            canonical_headers.push(':');
            canonical_headers.push_str(v);
            canonical_headers.push('\n');
            signed_header_names.push(k.as_str());
        }
        let signed_headers_str = signed_header_names.join(";");

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method.as_str(),
            canonical_uri,
            canonical_qs,
            canonical_headers,
            signed_headers_str,
            payload_hash
        );

        let signature = compute_sigv4(
            &acc.secret_access_key,
            &date_stamp,
            &region,
            &amz_date,
            &canonical_request,
        );

        let auth_header = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}/{}/s3/aws4_request, SignedHeaders={}, Signature={}",
            acc.access_key_id, date_stamp, region, signed_headers_str, signature
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            HOST,
            HeaderValue::from_str(&host_header)
                .map_err(|e| S3Error::InvalidEndpoint(e.to_string()))?,
        );
        headers.insert(
            HeaderName::from_static("x-amz-date"),
            HeaderValue::from_str(&amz_date).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-amz-content-sha256"),
            HeaderValue::from_str(&payload_hash).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_str(&auth_header)
                .map_err(|e| S3Error::Http(e.to_string()))?,
        );
        for (k, v) in extra_headers {
            let name = HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| S3Error::Http(e.to_string()))?;
            let val = HeaderValue::from_str(v).map_err(|e| S3Error::Http(e.to_string()))?;
            if name == CONTENT_TYPE {
                headers.insert(CONTENT_TYPE, val);
            } else {
                headers.insert(name, val);
            }
        }

        self.http
            .request(method, &full_url)
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(|e| S3Error::Http(sanitize_http_error(e)))
    }
}

pub fn validate_endpoint_url(endpoint: &str) -> Result<Url, S3Error> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(S3Error::InvalidEndpoint("Endpoint cannot be empty".into()));
    }
    let parsed = Url::parse(trimmed).map_err(|e| S3Error::InvalidEndpoint(e.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(S3Error::InvalidEndpoint(format!(
                "Unsupported URL scheme '{}'; only http and https are allowed",
                other
            )));
        }
    }
    if parsed.host_str().is_none() {
        return Err(S3Error::InvalidEndpoint("Endpoint must include a host".into()));
    }
    Ok(parsed)
}

pub fn validate_bucket_name(name: &str) -> Result<(), S3Error> {
    let b = name.trim();
    if b.len() < 3 || b.len() > 63 {
        return Err(S3Error::InvalidBucket(
            "Bucket name must be between 3 and 63 characters".into(),
        ));
    }
    if b.contains("..") {
        return Err(S3Error::InvalidBucket(
            "Bucket name cannot contain consecutive periods".into(),
        ));
    }
    let bytes = b.as_bytes();
    if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
        return Err(S3Error::InvalidBucket(
            "Bucket name must start with a lowercase letter or digit".into(),
        ));
    }
    let last = bytes[bytes.len() - 1];
    if !last.is_ascii_lowercase() && !last.is_ascii_digit() {
        return Err(S3Error::InvalidBucket(
            "Bucket name must end with a lowercase letter or digit".into(),
        ));
    }
    if !bytes
        .iter()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-' || *c == b'.')
    {
        return Err(S3Error::InvalidBucket(
            "Bucket name may only contain lowercase letters, numbers, hyphens, and dots".into(),
        ));
    }
    Ok(())
}

fn effective_region(region: &str) -> String {
    let r = region.trim();
    if r.is_empty() {
        "us-east-1".to_string()
    } else {
        r.to_string()
    }
}

fn build_target_url(
    acc: &DecryptedStorageAccount,
    bucket: Option<&str>,
    key: Option<&str>,
) -> Result<(String, String, String), S3Error> {
    let parsed = validate_endpoint_url(&acc.endpoint)?;
    let scheme = parsed.scheme();
    let host = parsed
        .host_str()
        .ok_or_else(|| S3Error::InvalidEndpoint("Missing host".into()))?;
    let port_suffix = match (scheme, parsed.port()) {
        ("http", Some(80)) | ("https", Some(443)) | (_, None) => String::new(),
        (_, Some(p)) => format!(":{}", p),
    };

    let base_path = parsed.path().trim_end_matches('/');

    if acc.path_style || bucket.is_none() {
        let host_header = format!("{}{}", host, port_suffix);
        let mut canonical_uri = String::new();
        if !base_path.is_empty() {
            canonical_uri.push_str(&aws_uri_encode(base_path, false));
        }
        if let Some(b) = bucket {
            canonical_uri.push('/');
            canonical_uri.push_str(&aws_uri_encode(b, true));
            if let Some(k) = key {
                let k_trimmed = k.trim_start_matches('/');
                if !k_trimmed.is_empty() {
                    canonical_uri.push('/');
                    canonical_uri.push_str(&aws_uri_encode(k_trimmed, false));
                }
            }
        }
        if canonical_uri.is_empty() {
            canonical_uri.push('/');
        }
        let full = format!("{}://{}{}", scheme, host_header, canonical_uri);
        Ok((full, host_header, canonical_uri))
    } else {
        let b = bucket.unwrap();
        let host_header = format!("{}.{}{}", b, host, port_suffix);
        let mut canonical_uri = String::new();
        if !base_path.is_empty() {
            canonical_uri.push_str(&aws_uri_encode(base_path, false));
        }
        if let Some(k) = key {
            let k_trimmed = k.trim_start_matches('/');
            if !k_trimmed.is_empty() {
                canonical_uri.push('/');
                canonical_uri.push_str(&aws_uri_encode(k_trimmed, false));
            }
        }
        if canonical_uri.is_empty() {
            canonical_uri.push('/');
        }
        let full = format!("{}://{}{}", scheme, host_header, canonical_uri);
        Ok((full, host_header, canonical_uri))
    }
}

pub fn aws_uri_encode(input: &str, encode_slash: bool) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for &b in input.as_bytes() {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if unreserved || (!encode_slash && b == b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn build_canonical_query(query: &BTreeMap<String, String>) -> String {
    let mut pairs: Vec<(String, String)> = query
        .iter()
        .map(|(k, v)| (aws_uri_encode(k, true), aws_uri_encode(v, true)))
        .collect();
    pairs.sort();
    pairs
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&")
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn compute_sigv4(
    secret_key: &str,
    date_stamp: &str,
    region: &str,
    amz_date: &str,
    canonical_request: &str,
) -> String {
    let scope = format!("{}/{}/s3/aws4_request", date_stamp, region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date,
        scope,
        sha256_hex(canonical_request.as_bytes())
    );
    let k_date = hmac_sha256(format!("AWS4{}", secret_key).as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()))
}

fn sanitize_http_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        return "Request timed out".to_string();
    }
    if e.is_connect() {
        return "Connection failed to storage endpoint".to_string();
    }
    // Avoid printing full URL query strings (could contain presigned tokens)
    e.without_url().to_string()
}

async fn extract_s3_error(resp: Response) -> S3Error {
    let status = resp.status();
    let code_u16 = status.as_u16();
    let body = resp.text().await.unwrap_or_default();
    let code = parse_tag_value(&body, "Code").unwrap_or_else(|| status.canonical_reason().unwrap_or("Error").to_string());
    let message = parse_tag_value(&body, "Message").unwrap_or_else(|| {
        if code_u16 == StatusCode::FORBIDDEN.as_u16() {
            "Access Denied (periksa Access Key, Secret Key, atau izin bucket)".to_string()
        } else if code_u16 == StatusCode::NOT_FOUND.as_u16() {
            "Bucket atau object tidak ditemukan".to_string()
        } else {
            format!("HTTP {} ({})", code_u16, code)
        }
    });
    S3Error::Remote {
        status: code_u16,
        code,
        message,
    }
}

async fn check_response_text(resp: Response) -> Result<String, S3Error> {
    if !resp.status().is_success() {
        return Err(extract_s3_error(resp).await);
    }
    resp.text().await.map_err(|e| S3Error::Http(sanitize_http_error(e)))
}

async fn check_response_empty(resp: Response) -> Result<(), S3Error> {
    if !resp.status().is_success() {
        return Err(extract_s3_error(resp).await);
    }
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn parse_tag_value(xml: &str, tag_name: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut inside = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.local_name().as_ref() == tag_name.as_bytes() {
                    inside = true;
                }
            }
            Ok(Event::Text(e)) if inside => {
                if let Ok(t) = e.unescape() {
                    return Some(t.into_owned());
                }
            }
            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == tag_name.as_bytes() {
                    inside = false;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

fn parse_list_buckets_xml(xml: &str) -> Result<Vec<StorageBucket>, S3Error> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut buckets = Vec::new();
    let mut in_bucket = false;
    let mut current_tag = String::new();
    let mut name = String::new();
    let mut creation_date = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if tag == "Bucket" {
                    in_bucket = true;
                    name.clear();
                    creation_date.clear();
                } else if in_bucket {
                    current_tag = tag;
                }
            }
            Ok(Event::Text(e)) if in_bucket => {
                let text = e.unescape().map_err(|err| S3Error::Xml(err.to_string()))?;
                match current_tag.as_str() {
                    "Name" => name.push_str(&text),
                    "CreationDate" => creation_date.push_str(&text),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if tag == "Bucket" {
                    in_bucket = false;
                    if !name.is_empty() {
                        buckets.push(StorageBucket {
                            name: name.clone(),
                            creation_date: creation_date.clone(),
                        });
                    }
                } else {
                    current_tag.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(S3Error::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }
    Ok(buckets)
}

fn parse_list_objects_xml(
    xml: &str,
    bucket: &str,
    prefix: &str,
) -> Result<StorageListOut, S3Error> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut entries = Vec::new();
    let mut is_truncated = false;
    let mut next_token: Option<String> = None;

    let mut section = "";
    let mut current_tag = String::new();
    let mut obj_key = String::new();
    let mut obj_size: u64 = 0;
    let mut obj_modified = String::new();
    let mut obj_etag = String::new();
    let mut common_prefix = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if tag == "Contents" {
                    section = "Contents";
                    obj_key.clear();
                    obj_size = 0;
                    obj_modified.clear();
                    obj_etag.clear();
                } else if tag == "CommonPrefixes" {
                    section = "CommonPrefixes";
                    common_prefix.clear();
                } else {
                    current_tag = tag;
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().map_err(|err| S3Error::Xml(err.to_string()))?;
                match section {
                    "Contents" => match current_tag.as_str() {
                        "Key" => obj_key.push_str(&text),
                        "Size" => {
                            if let Ok(v) = text.trim().parse::<u64>() {
                                obj_size = v;
                            }
                        }
                        "LastModified" => obj_modified.push_str(text.trim()),
                        "ETag" => obj_etag.push_str(text.trim().trim_matches('"')),
                        _ => {}
                    },
                    "CommonPrefixes" => {
                        if current_tag == "Prefix" {
                            common_prefix.push_str(&text);
                        }
                    }
                    _ => match current_tag.as_str() {
                        "IsTruncated" => is_truncated = text.trim().eq_ignore_ascii_case("true"),
                        "NextContinuationToken" => {
                            let t = text.trim();
                            if !t.is_empty() {
                                next_token = Some(t.to_string());
                            }
                        }
                        _ => {}
                    },
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if tag == "Contents" {
                    section = "";
                    if !obj_key.is_empty() && obj_key != prefix {
                        let rel = obj_key.strip_prefix(prefix).unwrap_or(&obj_key);
                        let is_dir = obj_key.ends_with('/');
                        let display = rel.trim_end_matches('/').to_string();
                        entries.push(StorageObject {
                            key: obj_key.clone(),
                            name: if display.is_empty() { obj_key.clone() } else { display },
                            is_dir,
                            size: obj_size,
                            last_modified: obj_modified.clone(),
                            etag: obj_etag.clone(),
                        });
                    }
                } else if tag == "CommonPrefixes" {
                    section = "";
                    if !common_prefix.is_empty() && common_prefix != prefix {
                        let rel = common_prefix.strip_prefix(prefix).unwrap_or(&common_prefix);
                        let display = rel.trim_end_matches('/').to_string();
                        entries.push(StorageObject {
                            key: common_prefix.clone(),
                            name: display,
                            is_dir: true,
                            size: 0,
                            last_modified: String::new(),
                            etag: String::new(),
                        });
                    }
                } else {
                    current_tag.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(S3Error::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    // Sort folders first, then files alphabetically
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));

    Ok(StorageListOut {
        bucket: bucket.to_string(),
        prefix: prefix.to_string(),
        entries,
        is_truncated,
        next_continuation_token: next_token,
    })
}
