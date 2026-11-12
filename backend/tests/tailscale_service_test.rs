use axum::{
    extract::Path,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tokio::net::TcpListener;
use xterm_backend::services::tailscale::TailscaleClient;

#[tokio::test]
async fn test_tailscale_service_mock_api() {
    // 1. Build a mock Tailscale API server
    let mock_app = Router::new()
        .route(
            "/api/v2/tailnet/:tailnet/devices",
            get(|Path(_tailnet): Path<String>| async {
                Json(json!({
                    "devices": [
                        {
                            "id": "dev_123",
                            "name": "node1.example.ts.net",
                            "user": "alice@example.com",
                            "os": "linux",
                            "hostname": "node1",
                            "clientVersion": "1.76.0",
                            "updateAvailable": true,
                            "addresses": ["100.64.0.1", "fd7a:115c:a1e0::1"],
                            "created": "2024-01-01T00:00:00Z",
                            "lastSeen": chrono::Utc::now().to_rfc3339(),
                            "keyExpiryDisabled": false,
                            "expires": "2025-01-01T00:00:00Z",
                            "authorized": true,
                            "isExternal": false,
                            "tags": ["tag:server"],
                            "blocksIncomingConnections": false,
                            "advertisedRoutes": ["192.168.1.0/24"],
                            "enabledRoutes": ["192.168.1.0/24"]
                        }
                    ]
                }))
            }),
        )
        .route(
            "/api/v2/device/:device_id",
            get(|Path(id): Path<String>| async move {
                Json(json!({
                    "id": id,
                    "name": "node1.example.ts.net",
                    "hostname": "node1",
                    "os": "linux",
                    "addresses": ["100.64.0.1"]
                }))
            })
            .delete(|Path(_id): Path<String>| async {
                axum::http::StatusCode::OK
            }),
        )
        .route(
            "/api/v2/device/:device_id/routes",
            get(|Path(_id): Path<String>| async {
                Json(json!({
                    "advertisedRoutes": ["192.168.1.0/24", "0.0.0.0/0"],
                    "enabledRoutes": ["192.168.1.0/24"]
                }))
            })
            .post(|Path(_id): Path<String>, Json(body): Json<serde_json::Value>| async move {
                let routes = body["routes"].as_array().cloned().unwrap_or_default();
                Json(json!({
                    "advertisedRoutes": ["192.168.1.0/24", "0.0.0.0/0"],
                    "enabledRoutes": routes
                }))
            }),
        )
        .route(
            "/api/v2/device/:device_id/authorized",
            post(|Path(_id): Path<String>| async {
                axum::http::StatusCode::OK
            }),
        )
        .route(
            "/api/v2/device/:device_id/key",
            post(|Path(_id): Path<String>| async {
                axum::http::StatusCode::OK
            }),
        )
        .route(
            "/api/v2/device/:device_id/tags",
            post(|Path(_id): Path<String>| async {
                axum::http::StatusCode::OK
            }),
        );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let mock_base = format!("http://127.0.0.1:{}", port);
    let client = TailscaleClient::with_base_url(&mock_base);

    // Test verify
    let verify_res = client.verify_token("test_token", "default").await.unwrap();
    assert!(verify_res.valid);
    assert_eq!(verify_res.devices_count, 1);

    // Test URL construction when base_url already contains /api/v2
    let client_with_prefix = TailscaleClient::with_base_url(&format!("{}/api/v2", mock_base));
    let verify_prefix = client_with_prefix.verify_token("test_token", "default").await.unwrap();
    assert!(verify_prefix.valid);

    // Test production default client URL resolution
    let default_client = TailscaleClient::new();
    assert_eq!(
        default_client.build_url("/api/v2/tailnet/-/devices"),
        "https://api.tailscale.com/api/v2/tailnet/-/devices"
    );
    assert_eq!(
        default_client.build_url("/api/v2/device/12345/routes"),
        "https://api.tailscale.com/api/v2/device/12345/routes"
    );

    // Test list_devices
    let devices = client.list_devices("test_token", "default").await.unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].id, "dev_123");
    assert_eq!(devices[0].hostname, "node1");
    assert!(devices[0].online);

    // Test get_device
    let dev = client.get_device("test_token", "dev_123").await.unwrap();
    assert_eq!(dev.id, "dev_123");
    assert_eq!(dev.hostname, "node1");

    // Test routes
    let routes = client.get_device_routes("test_token", "dev_123").await.unwrap();
    assert_eq!(routes.advertised_routes.len(), 2);
    assert_eq!(routes.enabled_routes.len(), 1);

    let updated_routes = client
        .set_device_routes("test_token", "dev_123", &["192.168.1.0/24".to_string(), "0.0.0.0/0".to_string()])
        .await
        .unwrap();
    assert_eq!(updated_routes.enabled_routes.len(), 2);

    // Test setting actions
    client.set_device_authorized("test_token", "dev_123", true).await.unwrap();
    client.set_device_key_expiry("test_token", "dev_123", true).await.unwrap();
    client.set_device_tags("test_token", "dev_123", &["tag:prod".to_string()]).await.unwrap();
    client.delete_device("test_token", "dev_123").await.unwrap();
}
