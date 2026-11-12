use crate::models::tailscale::{TailscaleDevice, TailscaleDeviceRoutes, TailscaleVerifyOut};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::json;

pub const TS_BASE: &str = "https://api.tailscale.com/api/v2";

#[derive(Clone)]
pub struct TailscaleClient {
    client: Client,
    base_url: String,
}

impl Default for TailscaleClient {
    fn default() -> Self {
        Self::new()
    }
}

pub fn calculate_online(last_seen_str: &str) -> bool {
    if last_seen_str.is_empty() {
        return false;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(last_seen_str) {
        let now = chrono::Utc::now();
        let diff = now.signed_duration_since(dt.with_timezone(&chrono::Utc));
        diff.num_seconds() <= 180 && diff.num_seconds() >= -60
    } else {
        false
    }
}

impl TailscaleClient {
    pub fn new() -> Self {
        Self::with_base_url(TS_BASE)
    }

    pub fn with_base_url(base_url: &str) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn build_url(&self, path: &str) -> String {
        let path_clean = path.trim_start_matches('/');
        let base = self.base_url.trim_end_matches('/');
        if base.ends_with("/api/v2") && path_clean.starts_with("api/v2/") {
            format!("{}/{}", base, &path_clean["api/v2/".len()..])
        } else if !base.ends_with("/api/v2") && !path_clean.starts_with("api/v2/") {
            format!("{}/api/v2/{}", base, path_clean)
        } else {
            format!("{}/{}", base, path_clean)
        }
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, String> {
        let url = self.build_url(path);

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token.trim()))
                .map_err(|e| format!("Invalid auth token format: {}", e))?,
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("RusTerm-Tailscale/1.0"));

        let mut req = self.client.request(method, &url).headers(headers);
        if let Some(b) = body {
            req = req.header(CONTENT_TYPE, "application/json").json(&b);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if !status.is_success() {
            if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(msg) = err_json.get("message").and_then(|m| m.as_str()) {
                    return Err(format!("Tailscale API error ({}): {}", status, msg));
                }
            }
            return Err(format!("Tailscale API error ({}): {}", status, text));
        }

        if std::any::type_name::<T>() == "()" {
            return serde_json::from_str::<T>("null").map_err(|e| e.to_string());
        }

        let trimmed = text.trim();
        if trimmed.is_empty() {
            if let Ok(empty_val) = serde_json::from_str::<T>("{}") {
                return Ok(empty_val);
            }
        }

        serde_json::from_str::<T>(&text)
            .map_err(|e| format!("Failed to parse response: {} (Body: {})", e, text))
    }

    pub async fn verify_token(&self, token: &str, tailnet: &str) -> Result<TailscaleVerifyOut, String> {
        let path = format!("/api/v2/tailnet/{}/devices", tailnet);
        let resp: serde_json::Value = self.request(Method::GET, &path, token, None).await?;
        let devices_arr = resp.get("devices").and_then(|d| d.as_array());
        let count = devices_arr.map(|a| a.len()).unwrap_or(0);

        let detected_tailnet = if tailnet == "-" || tailnet.trim().is_empty() {
            devices_arr
                .and_then(|a| a.first())
                .and_then(|dev| dev.get("name"))
                .and_then(|n| n.as_str())
                .and_then(|n| n.split_once('.'))
                .map(|(_, domain)| domain.to_string())
                .unwrap_or_else(|| "-".to_string())
        } else {
            tailnet.to_string()
        };

        Ok(TailscaleVerifyOut {
            valid: true,
            tailnet: detected_tailnet,
            devices_count: count,
        })
    }

    pub async fn list_devices(&self, token: &str, tailnet: &str) -> Result<Vec<TailscaleDevice>, String> {
        let path = format!("/api/v2/tailnet/{}/devices", tailnet);
        let resp: serde_json::Value = self.request(Method::GET, &path, token, None).await?;
        let devices_val = resp.get("devices").cloned().unwrap_or(serde_json::Value::Array(vec![]));
        let mut devices: Vec<TailscaleDevice> = serde_json::from_value(devices_val)
            .map_err(|e| format!("Failed to deserialize devices: {}", e))?;

        for d in &mut devices {
            d.online = calculate_online(&d.last_seen);
        }

        Ok(devices)
    }

    pub async fn get_device(&self, token: &str, device_id: &str) -> Result<TailscaleDevice, String> {
        let path = format!("/api/v2/device/{}", device_id);
        let mut device: TailscaleDevice = self.request(Method::GET, &path, token, None).await?;
        device.online = calculate_online(&device.last_seen);
        Ok(device)
    }

    pub async fn get_device_routes(
        &self,
        token: &str,
        device_id: &str,
    ) -> Result<TailscaleDeviceRoutes, String> {
        let path = format!("/api/v2/device/{}/routes", device_id);
        self.request(Method::GET, &path, token, None).await
    }

    pub async fn set_device_routes(
        &self,
        token: &str,
        device_id: &str,
        routes: &[String],
    ) -> Result<TailscaleDeviceRoutes, String> {
        let path = format!("/api/v2/device/{}/routes", device_id);
        self.request(
            Method::POST,
            &path,
            token,
            Some(json!({ "routes": routes })),
        )
        .await
    }

    pub async fn set_device_authorized(
        &self,
        token: &str,
        device_id: &str,
        authorized: bool,
    ) -> Result<(), String> {
        let path = format!("/api/v2/device/{}/authorized", device_id);
        self.request::<()>(
            Method::POST,
            &path,
            token,
            Some(json!({ "authorized": authorized })),
        )
        .await
    }

    pub async fn set_device_key_expiry(
        &self,
        token: &str,
        device_id: &str,
        disabled: bool,
    ) -> Result<(), String> {
        let path = format!("/api/v2/device/{}/key", device_id);
        self.request::<()>(
            Method::POST,
            &path,
            token,
            Some(json!({ "keyExpiryDisabled": disabled })),
        )
        .await
    }

    pub async fn set_device_tags(
        &self,
        token: &str,
        device_id: &str,
        tags: &[String],
    ) -> Result<(), String> {
        let path = format!("/api/v2/device/{}/tags", device_id);
        self.request::<()>(
            Method::POST,
            &path,
            token,
            Some(json!({ "tags": tags })),
        )
        .await
    }

    pub async fn delete_device(&self, token: &str, device_id: &str) -> Result<(), String> {
        let url = self.build_url(&format!("/api/v2/device/{}", device_id));

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token.trim()))
                .map_err(|e| format!("Invalid auth token format: {}", e))?,
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("RusTerm-Tailscale/1.0"));

        let resp = self
            .client
            .delete(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        if !resp.status().is_success() && resp.status() != StatusCode::NO_CONTENT {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Tailscale API error ({}): {}", status, text));
        }

        Ok(())
    }
}
