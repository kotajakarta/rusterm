use xterm_backend::models::containers::{normalize_container_item, parse_port_entry};

#[test]
fn test_parse_port_entry_string() {
    let raw = serde_json::json!("0.0.0.0:8080->80/tcp, :::8080->80/tcp, 443/tcp");
    let ports = parse_port_entry(&raw);
    assert_eq!(ports.len(), 3);

    assert_eq!(ports[0].host_ip.as_deref(), Some("0.0.0.0"));
    assert_eq!(ports[0].host_port, Some(8080));
    assert_eq!(ports[0].container_port, 80);
    assert_eq!(ports[0].protocol.as_deref(), Some("tcp"));

    assert_eq!(ports[2].host_port, None);
    assert_eq!(ports[2].container_port, 443);
    assert_eq!(ports[2].protocol.as_deref(), Some("tcp"));
}

#[test]
fn test_normalize_container_item_docker() {
    let raw = serde_json::json!({
        "ID": "d4e5f6a1b2c3d4e5",
        "Names": "/nginx-prod",
        "Image": "nginx:alpine",
        "State": "running",
        "Status": "Up 3 days",
        "Ports": "0.0.0.0:80->80/tcp",
        "Command": "nginx -g 'daemon off;'",
        "CreatedAt": "2026-11-07 10:00:00"
    });

    let item = normalize_container_item(&raw);
    assert_eq!(item.id, "d4e5f6a1b2c3");
    assert_eq!(item.names, vec!["nginx-prod"]);
    assert_eq!(item.image, "nginx:alpine");
    assert_eq!(item.state, "running");
    assert_eq!(item.status, "Up 3 days");
    assert_eq!(item.ports.len(), 1);
    assert_eq!(item.ports[0].container_port, 80);
}

#[test]
fn test_normalize_container_item_podman() {
    let raw = serde_json::json!({
        "Id": "9a8b7c6d5e4f3a2b1c",
        "Names": ["redis-cache"],
        "Image": "docker.io/library/redis:7",
        "State": "exited",
        "Status": "Exited (0) 5 minutes ago",
        "Ports": [{
            "hostIP": "127.0.0.1",
            "hostPort": 6379,
            "containerPort": 6379,
            "protocol": "tcp"
        }]
    });

    let item = normalize_container_item(&raw);
    assert_eq!(item.id, "9a8b7c6d5e4f");
    assert_eq!(item.names, vec!["redis-cache"]);
    assert_eq!(item.state, "exited");
    assert_eq!(item.ports.len(), 1);
    assert_eq!(item.ports[0].host_ip.as_deref(), Some("127.0.0.1"));
    assert_eq!(item.ports[0].host_port, Some(6379));
}
