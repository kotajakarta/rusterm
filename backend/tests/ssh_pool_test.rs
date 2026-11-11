use xterm_backend::controllers::terminal_ws::{WsClientMsg, WsServerMsg};

#[test]
fn test_ws_client_msg_deserialization() {
    let input_json = r#"{"type": "input", "data": "ls -la\n"}"#;
    let msg: WsClientMsg = serde_json::from_str(input_json).expect("failed to deserialize input");
    match msg {
        WsClientMsg::Input { data } => assert_eq!(data, "ls -la\n"),
        _ => panic!("wrong message variant"),
    }

    let resize_json = r#"{"type": "resize", "cols": 120, "rows": 40}"#;
    let msg: WsClientMsg = serde_json::from_str(resize_json).expect("failed to deserialize resize");
    match msg {
        WsClientMsg::Resize { cols, rows } => {
            assert_eq!(cols, 120);
            assert_eq!(rows, 40);
        }
        _ => panic!("wrong message variant"),
    }
}

#[test]
fn test_ws_server_msg_serialization() {
    let connecting = WsServerMsg::Status {
        status: "connecting".to_string(),
    };
    let json = serde_json::to_string(&connecting).unwrap();
    assert!(json.contains(r#""type":"status""#));
    assert!(json.contains(r#""status":"connecting""#));

    let err = WsServerMsg::Error {
        message: "Connection refused".to_string(),
    };
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains(r#""type":"error""#));
    assert!(json.contains("Connection refused"));
}

#[test]
fn test_parse_stats() {
    use xterm_backend::services::ssh_pool::parse_stats;

    let sample_output = r#"
@@hostname
server01.local
@@os
Rocky Linux 10 (Red Quartz)
@@kernel
Linux 6.12.0
@@uptime
12345.67 89012.34
@@load
0.12 0.45 0.78
@@cpu
4
@@mem
MemTotal:       16384000 kB
MemFree:         2048000 kB
MemAvailable:    8192000 kB
@@disk
/dev/sda1 104857600 52428800 52428800 50% /
"#;

    let stats = parse_stats(sample_output);
    assert_eq!(stats.hostname, "server01.local");
    assert_eq!(stats.os, "Rocky Linux 10 (Red Quartz)");
    assert_eq!(stats.kernel, "Linux 6.12.0");
    assert_eq!(stats.uptime_seconds, Some(12345.67));
    assert_eq!(stats.load, vec![0.12, 0.45, 0.78]);
    assert_eq!(stats.cpu_count, Some(4));
    assert_eq!(stats.mem_total, Some(16384000 * 1024));
    assert_eq!(stats.mem_used, Some((16384000 - 8192000) * 1024));
    assert_eq!(stats.disk_total, Some(104857600 * 1024));
    assert_eq!(stats.disk_used, Some(52428800 * 1024));
}

#[test]
fn test_files_agent_source_and_command() {
    use xterm_backend::services::ssh_pool::{build_agent_command, FILES_AGENT_SRC};

    assert!(!FILES_AGENT_SRC.is_empty());
    assert!(FILES_AGENT_SRC.contains("File-manager agent"));
    assert!(FILES_AGENT_SRC.contains("MAX_READ_BYTES"));

    let cmd = build_agent_command();
    assert!(cmd.starts_with("python3 -c '"));
    assert!(cmd.ends_with('\''));
}

#[tokio::test]
async fn test_ssh_pool_lifecycle_and_timeout() {
    use xterm_backend::models::server::DecryptedServerCreds;
    use xterm_backend::services::ssh_pool::SshPool;

    let pool = SshPool::new();
    assert!(!pool.is_session_alive(999).await);

    pool.disconnect(999).await;
    pool.disconnect_all().await;

    // Test connection to an unreachable address fails cleanly without hanging
    let dummy_creds = DecryptedServerCreds {
        id: 999,
        name: "test-unreachable".into(),
        host: "192.0.2.1".into(), // TEST-NET-1 (non-routable, guaranteed timeout)
        port: 22,
        username: "test".into(),
        auth_type: "password".into(),
        password: Some("test".into()),
        private_key: None,
        passphrase: None,
    };

    let start = std::time::Instant::now();
    // Wrap with a test timeout slightly above 10s to ensure our internal 10s timeout triggers
    let res = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        pool.test_connection(&dummy_creds),
    )
    .await;

    assert!(res.is_ok(), "test_connection should have finished within 12s");
    let err = res.unwrap().unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("timed out") || err_str.contains("Gagal menghubungkan"),
        "Expected timeout or connection failure, got: {}",
        err_str
    );
    assert!(start.elapsed().as_secs() <= 11);
}

#[test]
fn test_check_host_key_trust_on_first_use() {
    use russh_keys::key::KeyPair;
    use xterm_backend::services::ssh_pool::{
        check_host_key, host_key_fingerprint, host_key_string, HostKeyCheck,
    };

    let key = KeyPair::generate_ed25519().unwrap().clone_public_key().unwrap();
    let other = KeyPair::generate_ed25519().unwrap().clone_public_key().unwrap();
    let stored = host_key_string(&key);
    assert!(stored.starts_with("ssh-ed25519 "), "{}", stored);
    assert!(host_key_fingerprint(&key).starts_with("SHA256:"));

    assert_eq!(check_host_key(None, &key), HostKeyCheck::FirstUse);
    assert_eq!(check_host_key(Some(&stored), &key), HostKeyCheck::Trusted);
    match check_host_key(Some(&stored), &other) {
        HostKeyCheck::Mismatch { expected_fp, presented_fp } => {
            assert_eq!(expected_fp, host_key_fingerprint(&key));
            assert_eq!(presented_fp, host_key_fingerprint(&other));
        }
        res => panic!("expected mismatch, got {:?}", res),
    }
}
