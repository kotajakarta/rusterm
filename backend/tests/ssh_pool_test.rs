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
