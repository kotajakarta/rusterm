use xterm_backend::services::transfer::{
    relative_paths, TransferJob, TransferManager, TransferResult, TransferSpec,
};

fn spec() -> TransferSpec {
    TransferSpec {
        remote_id: Some(3),
        remote_name: "Sync HTML".into(),
        direction: "upload".into(),
        source_server_id: 1,
        source_root: "/var/www".into(),
        dest_server_id: 2,
        dest_root: "/backup/www".into(),
        rels: vec!["html".into()],
        ignore: vec![".git".into()],
        read_gitignore: true,
        move_sources: false,
    }
}

#[test]
fn relative_paths_maps_inside_root_and_rejects_outside() {
    let ok = relative_paths(
        "/var/www/",
        &["/var/www".into(), "/var/www/html".into(), "/var/www/a/b/../c/".into()],
    )
    .unwrap();
    assert_eq!(ok, vec![".", "html", "a/c"]);
    assert_eq!(relative_paths("/", &["/etc/nginx".into()]).unwrap(), vec!["etc/nginx"]);

    assert!(relative_paths("/var/www", &["/var/www2/x".into()]).is_err());
    assert!(relative_paths("/var/www", &["/var/www/../etc".into()]).is_err());
    assert!(relative_paths("/var/www", &["relative".into()]).is_err());
}

#[test]
fn job_json_matches_frontend_contract() {
    let manager = TransferManager::default();
    let (job, _handle) = manager.create(&spec());
    let value = serde_json::to_value(&job).unwrap();
    let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(|k| k.as_str()).collect();
    keys.sort();
    let mut expected = vec![
        "id", "remote_id", "remote_name", "direction", "state", "source_server_id",
        "source_root", "dest_server_id", "dest_root", "rels", "read_gitignore", "sent_bytes",
        "total_bytes", "total_files", "result", "error",
    ];
    expected.sort();
    assert_eq!(keys, expected);
    assert_eq!(value["state"], "running");
    assert_eq!(value["result"], serde_json::Value::Null);
    assert_eq!(job.id.len(), 32);
}

#[tokio::test]
async fn manager_tracks_progress_finish_and_cancel() {
    let manager = TransferManager::default();
    let (job, handle) = manager.create(&spec());
    handle.update(|j| j.sent_bytes = 42);
    assert_eq!(manager.get(&job.id).unwrap().sent_bytes, 42);

    handle.finish_ok(TransferResult { files: 2, dirs: 1, links: 0, bytes: 42 });
    let done = manager.get(&job.id).unwrap();
    assert_eq!(done.state, "done");
    assert_eq!(done.result.unwrap().files, 2);
    // Cancelling a finished job changes nothing.
    assert_eq!(manager.cancel(&job.id).unwrap().state, "done");

    let (job2, handle2) = manager.create(&spec());
    assert!(!handle2.is_cancelled());
    let cancelled = manager.cancel(&job2.id).unwrap();
    assert_eq!(cancelled.state, "cancelled");
    assert!(handle2.is_cancelled());
    tokio::time::timeout(std::time::Duration::from_secs(1), handle2.cancelled())
        .await
        .expect("cancelled() must resolve after cancel");
    // A late failure report must not overwrite the cancelled state.
    handle2.finish_err("koneksi putus".into());
    assert_eq!(manager.get(&job2.id).unwrap().state, "cancelled");

    assert!(manager.get("tidak-ada").is_none());
    assert!(manager.cancel("tidak-ada").is_none());
}
