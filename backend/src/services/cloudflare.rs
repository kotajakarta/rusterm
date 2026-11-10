use crate::models::cloudflare::{CFRoute, CFRouteIn, CFSyncMetadata, CFTunnel, CFZone};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use reqwest::{Client, Method, StatusCode};
use serde_json::json;

pub const CF_BASE: &str = "https://api.cloudflare.com/client/v4";

#[derive(Clone)]
pub struct CloudflareClient {
    client: Client,
}

impl Default for CloudflareClient {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudflareClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, String> {
        let url = format!("{}{}", CF_BASE, path);
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))
                .map_err(|e| format!("Invalid auth token format: {}", e))?,
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("TermDeck-Cloudflare/1.0"));

        let mut req = self.client.request(method, &url).headers(headers);
        if let Some(b) = body {
            req = req
                .header(CONTENT_TYPE, "application/json")
                .json(&b);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        if !status.is_success() {
            if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(err_arr) = err_json.get("errors").and_then(|v| v.as_array()) {
                    if let Some(first_msg) = err_arr.first().and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
                        return Err(first_msg.to_string());
                    }
                }
            }
            return Err(format!("Cloudflare API returned error {}: {}", status, text));
        }

        serde_json::from_str::<T>(&text)
            .map_err(|e| format!("Failed to parse Cloudflare JSON: {}. Body: {}", e, text))
    }

    pub async fn verify_token(&self, token: &str) -> Result<(String, String, bool), String> {
        let res: serde_json::Value = self
            .request(Method::GET, "/accounts", token, None)
            .await?;

        let accounts = res
            .get("result")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Token tidak valid atau tidak memiliki akses akun Cloudflare.".to_string())?;

        if accounts.is_empty() {
            return Err("Token tidak memiliki akses ke akun Cloudflare mana pun.".into());
        }

        let account_id = accounts[0]
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let account_name = accounts[0]
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed Account")
            .to_string();

        let mut has_dns_access = false;
        let zones_res: Result<serde_json::Value, _> = self
            .request(
                Method::GET,
                &format!("/zones?status=active&account.id={}&per_page=1", account_id),
                token,
                None,
            )
            .await;

        if let Ok(z) = zones_res {
            if let Some(arr) = z.get("result").and_then(|v| v.as_array()) {
                if let Some(first_zone) = arr.first().and_then(|v| v.get("id")).and_then(|v| v.as_str()) {
                    let dns_check: Result<serde_json::Value, _> = self
                        .request(
                            Method::GET,
                            &format!("/zones/{}/dns_records?per_page=1", first_zone),
                            token,
                            None,
                        )
                        .await;
                    if dns_check.is_ok() {
                        has_dns_access = true;
                    }
                }
            }
        }

        Ok((account_id, account_name, has_dns_access))
    }

    pub async fn list_tunnels(&self, account_id: &str, token: &str) -> Result<Vec<CFTunnel>, String> {
        let path = format!("/accounts/{}/cfd_tunnel?is_deleted=false", account_id);
        let res: serde_json::Value = self.request(Method::GET, &path, token, None).await?;

        let arr = res
            .get("result")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tunnels = Vec::new();
        for t in arr {
            let id = t.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let status = t
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("inactive")
                .to_string();
            let conns_count = t
                .get("connections")
                .and_then(|v| v.as_array())
                .map(|a| a.len() as u32)
                .unwrap_or(0);
            let created_at = t
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let remote_config = t
                .get("remote_config")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            tunnels.push(CFTunnel {
                id,
                name,
                status,
                conns_count,
                created_at,
                remote_config,
            });
        }

        Ok(tunnels)
    }

    pub async fn get_tunnel_token(
        &self,
        account_id: &str,
        tunnel_id: &str,
        token: &str,
    ) -> Result<String, String> {
        let path = format!("/accounts/{}/cfd_tunnel/{}/token", account_id, tunnel_id);
        let res: serde_json::Value = self.request(Method::GET, &path, token, None).await?;
        let tok = res
            .get("result")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Token tunnel tidak ditemukan dalam response Cloudflare".to_string())?;
        Ok(tok.to_string())
    }

    pub async fn list_zones(&self, account_id: &str, token: &str) -> Result<Vec<CFZone>, String> {
        let path = format!("/zones?status=active&account.id={}", account_id);
        let res: serde_json::Value = self.request(Method::GET, &path, token, None).await?;

        let arr = res
            .get("result")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut zones = Vec::new();
        for z in arr {
            let id = z.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = z.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let status = z
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("active")
                .to_string();

            zones.push(CFZone { id, name, status });
        }

        Ok(zones)
    }

    pub async fn list_routes(
        &self,
        account_id: &str,
        tunnel_id: &str,
        token: &str,
    ) -> Result<Vec<CFRoute>, String> {
        let path = format!("/accounts/{}/cfd_tunnel/{}/configurations", account_id, tunnel_id);
        let res: serde_json::Value = self.request(Method::GET, &path, token, None).await?;

        let ingress = res
            .get("result")
            .and_then(|v| v.get("config"))
            .and_then(|v| v.get("ingress"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut routes = Vec::new();
        for rule in ingress {
            if let (Some(h), Some(s)) = (
                rule.get("hostname").and_then(|v| v.as_str()),
                rule.get("service").and_then(|v| v.as_str()),
            ) {
                routes.push(CFRoute {
                    hostname: h.to_string(),
                    service: s.to_string(),
                });
            }
        }

        Ok(routes)
    }

    pub async fn add_route(
        &self,
        account_id: &str,
        tunnel_id: &str,
        token: &str,
        route: &CFRouteIn,
    ) -> Result<serde_json::Value, String> {
        let path = format!("/accounts/{}/cfd_tunnel/{}/configurations", account_id, tunnel_id);
        let res: serde_json::Value = self.request(Method::GET, &path, token, None).await?;

        let current_ingress = res
            .get("result")
            .and_then(|v| v.get("config"))
            .and_then(|v| v.get("ingress"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let target_hostname = route.hostname.trim().to_lowercase();
        let mut new_ingress = Vec::new();
        let mut catch_all: Option<serde_json::Value> = None;

        for rule in current_ingress {
            if !rule.get("hostname").is_some()
                && rule.get("service").and_then(|v| v.as_str()) == Some("http_status:404")
            {
                catch_all = Some(rule);
            } else if rule
                .get("hostname")
                .and_then(|v| v.as_str())
                .map(|h| h.to_lowercase())
                == Some(target_hostname.clone())
            {
                continue; // replace existing
            } else {
                new_ingress.push(rule);
            }
        }

        new_ingress.push(json!({
            "hostname": target_hostname,
            "service": route.service.trim(),
        }));

        if let Some(ca) = catch_all {
            new_ingress.push(ca);
        } else {
            new_ingress.push(json!({ "service": "http_status:404" }));
        }

        let put_body = json!({
            "config": {
                "ingress": new_ingress
            }
        });

        self.request::<serde_json::Value>(Method::PUT, &path, token, Some(put_body))
            .await?;

        // Optional DNS CNAME sync if zone_id is provided
        let mut dns_synced = false;
        let mut dns_warning = None;
        let cname_target = format!("{}.cfargotunnel.com", tunnel_id);

        if let Some(ref zid) = route.zone_id {
            let dns_path = format!("/zones/{}/dns_records", zid);
            let check_path = format!("{}?name={}&type=CNAME", dns_path, target_hostname);
            let check_res: Result<serde_json::Value, _> =
                self.request(Method::GET, &check_path, token, None).await;

            match check_res {
                Ok(existing_dns) => {
                    let records = existing_dns
                        .get("result")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();

                    if let Some(first_rec) = records.first() {
                        let rec_id = first_rec.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let update_dns_path = format!("{}/{}", dns_path, rec_id);
                        let update_body = json!({
                            "type": "CNAME",
                            "name": target_hostname,
                            "content": cname_target,
                            "proxied": true,
                            "ttl": 1
                        });
                        if self
                            .request::<serde_json::Value>(
                                Method::PUT,
                                &update_dns_path,
                                token,
                                Some(update_body),
                            )
                            .await
                            .is_ok()
                        {
                            dns_synced = true;
                        }
                    } else {
                        let create_body = json!({
                            "type": "CNAME",
                            "name": target_hostname,
                            "content": cname_target,
                            "proxied": true,
                            "ttl": 1
                        });
                        if self
                            .request::<serde_json::Value>(
                                Method::POST,
                                &dns_path,
                                token,
                                Some(create_body),
                            )
                            .await
                            .is_ok()
                        {
                            dns_synced = true;
                        }
                    }
                }
                Err(e) => {
                    dns_warning = Some(format!("Gagal sinkronisasi DNS: {}", e));
                }
            }
        }

        Ok(json!({
            "ok": true,
            "hostname": target_hostname,
            "service": route.service.trim(),
            "dns_synced": dns_synced,
            "dns_warning": dns_warning,
            "cname_target": cname_target,
        }))
    }

    pub async fn delete_route(
        &self,
        account_id: &str,
        tunnel_id: &str,
        token: &str,
        hostname: &str,
        zone_id: Option<&str>,
    ) -> Result<(), String> {
        let path = format!("/accounts/{}/cfd_tunnel/{}/configurations", account_id, tunnel_id);
        let res: serde_json::Value = self.request(Method::GET, &path, token, None).await?;

        let current_ingress = res
            .get("result")
            .and_then(|v| v.get("config"))
            .and_then(|v| v.get("ingress"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let target_hostname = hostname.trim().to_lowercase();
        let mut new_ingress = Vec::new();

        for rule in current_ingress {
            if rule
                .get("hostname")
                .and_then(|v| v.as_str())
                .map(|h| h.to_lowercase())
                == Some(target_hostname.clone())
            {
                continue;
            }
            new_ingress.push(rule);
        }

        let put_body = json!({
            "config": {
                "ingress": new_ingress
            }
        });

        self.request::<serde_json::Value>(Method::PUT, &path, token, Some(put_body))
            .await?;

        if let Some(zid) = zone_id {
            let check_path = format!(
                "/zones/{}/dns_records?name={}&type=CNAME",
                zid, target_hostname
            );
            if let Ok(existing_dns) = self
                .request::<serde_json::Value>(Method::GET, &check_path, token, None)
                .await
            {
                if let Some(records) = existing_dns.get("result").and_then(|v| v.as_array()) {
                    for r in records {
                        if let Some(rec_id) = r.get("id").and_then(|v| v.as_str()) {
                            let del_dns_path = format!("/zones/{}/dns_records/{}", zid, rec_id);
                            let _ = self
                                .request::<serde_json::Value>(
                                    Method::DELETE,
                                    &del_dns_path,
                                    token,
                                    None,
                                )
                                .await;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // KV Methods
    pub async fn ensure_kv_namespace(
        &self,
        account_id: &str,
        token: &str,
        title: &str,
    ) -> Result<String, String> {
        let list_path = format!("/accounts/{}/storage/kv/namespaces?per_page=100", account_id);
        let res: serde_json::Value = self.request(Method::GET, &list_path, token, None).await?;

        if let Some(namespaces) = res.get("result").and_then(|v| v.as_array()) {
            for ns in namespaces {
                if ns.get("title").and_then(|v| v.as_str()) == Some(title) {
                    if let Some(id) = ns.get("id").and_then(|v| v.as_str()) {
                        return Ok(id.to_string());
                    }
                }
            }
        }

        let create_path = format!("/accounts/{}/storage/kv/namespaces", account_id);
        let create_res: serde_json::Value = self
            .request(
                Method::POST,
                &create_path,
                token,
                Some(json!({ "title": title })),
            )
            .await?;

        let ns_id = create_res
            .get("result")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Gagal membuat KV namespace pada Cloudflare".to_string())?;

        Ok(ns_id.to_string())
    }

    pub async fn get_kv_metadata(
        &self,
        account_id: &str,
        namespace_id: &str,
        key: &str,
        token: &str,
    ) -> Result<Option<CFSyncMetadata>, String> {
        let path = format!(
            "/accounts/{}/storage/kv/namespaces/{}/metadata/{}",
            account_id, namespace_id, key
        );

        let url = format!("{}{}", CF_BASE, path);
        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .header(USER_AGENT, "TermDeck-Cloudflare/1.0")
            .send()
            .await
            .map_err(|e| format!("KV metadata request failed: {}", e))?;

        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(format!("KV metadata returned status {}", resp.status()));
        }

        let val = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Invalid metadata JSON: {}", e))?;

        if let Some(res) = val.get("result") {
            let meta: CFSyncMetadata = serde_json::from_value(res.clone())
                .map_err(|e| format!("Failed to parse CFSyncMetadata: {}", e))?;
            Ok(Some(meta))
        } else {
            Ok(None)
        }
    }

    pub async fn get_kv_value(
        &self,
        account_id: &str,
        namespace_id: &str,
        key: &str,
        token: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        let path = format!(
            "/accounts/{}/storage/kv/namespaces/{}/values/{}",
            account_id, namespace_id, key
        );

        let url = format!("{}{}", CF_BASE, path);
        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .header(USER_AGENT, "TermDeck-Cloudflare/1.0")
            .send()
            .await
            .map_err(|e| format!("KV value request failed: {}", e))?;

        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(format!("KV value returned status {}", resp.status()));
        }

        let val = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Invalid KV value JSON: {}", e))?;

        Ok(Some(val))
    }

    pub async fn put_kv_value(
        &self,
        account_id: &str,
        namespace_id: &str,
        key: &str,
        token: &str,
        envelope: &serde_json::Value,
        metadata: &CFSyncMetadata,
    ) -> Result<(), String> {
        let path = format!(
            "/accounts/{}/storage/kv/namespaces/{}/values/{}",
            account_id, namespace_id, key
        );
        let url = format!("{}{}", CF_BASE, path);

        let value_str = serde_json::to_string(envelope)
            .map_err(|e| format!("Failed to serialize envelope: {}", e))?;
        let meta_str = serde_json::to_string(metadata)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))?;

        let form = reqwest::multipart::Form::new()
            .text("value", value_str)
            .text("metadata", meta_str);

        let resp = self
            .client
            .put(&url)
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .header(USER_AGENT, "TermDeck-Cloudflare/1.0")
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("PUT KV request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("PUT KV failed: {}", body));
        }

        Ok(())
    }
}
