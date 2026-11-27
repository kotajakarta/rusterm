use std::io::Write;
use std::process::{Command, Output, Stdio};
use tempfile::tempdir;
use xterm_backend::services::ssh_pool::FILES_AGENT_SRC;

fn run_agent(stdin: &[u8]) -> Output {
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(FILES_AGENT_SRC)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("python3 harus tersedia");
    let mut pipe = child.stdin.take().unwrap();
    // The agent may stop reading early (abort); a broken pipe here is expected then.
    let _ = pipe.write_all(stdin);
    drop(pipe);
    child.wait_with_output().unwrap()
}

fn big_payload() -> Vec<u8> {
    (0..3 * 1024 * 1024u32).map(|i| (i % 251) as u8).collect()
}

fn make_tree() -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/big.bin"), big_payload()).unwrap();
    std::fs::write(dir.path().join("a.txt"), b"hello").unwrap();
    std::os::unix::fs::symlink("a.txt", dir.path().join("link")).unwrap();
    dir
}

fn pack_header(root: &std::path::Path) -> Vec<u8> {
    let mut h = serde_json::json!({
        "op": "pack",
        "root": root.to_str().unwrap(),
        "rels": ["."],
        "ignore": [],
        "read_gitignore": true,
        "credit": true,
    })
    .to_string()
    .into_bytes();
    h.push(b'\n');
    h
}

#[test]
fn pack_with_credits_streams_complete_tar() {
    let tree = make_tree();
    let mut stdin = pack_header(tree.path());
    stdin.extend_from_slice(&[1u8; 64]);
    let out = run_agent(&stdin);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let work = tempdir().unwrap();
    let tar_path = work.path().join("out.tar");
    std::fs::write(&tar_path, &out.stdout).unwrap();
    let extract = work.path().join("x");
    let status = Command::new("python3")
        .arg("-c")
        .arg("import sys,tarfile; tarfile.open(sys.argv[1]).extractall(sys.argv[2])")
        .arg(&tar_path)
        .arg(&extract)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(std::fs::read(extract.join("sub/big.bin")).unwrap(), big_payload());
    assert_eq!(std::fs::read(extract.join("a.txt")).unwrap(), b"hello");
    assert_eq!(
        std::fs::read_link(extract.join("link")).unwrap().to_str().unwrap(),
        "a.txt"
    );
}

#[test]
fn pack_without_credits_aborts_when_stdin_closes() {
    let tree = make_tree();
    let out = run_agent(&pack_header(tree.path()));
    assert_eq!(out.status.code(), Some(1), "pack must abort without credits");
    let err: serde_json::Value = serde_json::from_slice(&out.stderr).expect("stderr JSON");
    assert_eq!(err["ok"], false);
    assert!(err["error"].as_str().unwrap().contains("cancelled"), "{}", err);
    // At most one credit window worth of data may have been emitted.
    assert!(out.stdout.len() <= 2 * 1024 * 1024, "{} bytes", out.stdout.len());
}

#[test]
fn delete_removes_readonly_dirs_files_symlinks_and_nested_paths() {
    use std::os::unix::fs::PermissionsExt;

    let work = tempdir().unwrap();
    let ro_dir = work.path().join("ro_dir");
    let nested = ro_dir.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    let ro_file = nested.join("secret.txt");
    std::fs::write(&ro_file, b"locked").unwrap();
    std::fs::set_permissions(&ro_file, std::fs::Permissions::from_mode(0o444)).unwrap();
    std::fs::set_permissions(&nested, std::fs::Permissions::from_mode(0o555)).unwrap();
    std::fs::set_permissions(&ro_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let plain = work.path().join("plain.txt");
    std::fs::write(&plain, b"ok").unwrap();
    let broken_link = work.path().join("broken");
    std::os::unix::fs::symlink("/does/not/exist", &broken_link).unwrap();

    let req = serde_json::json!({
        "op": "delete",
        "paths": [
            ro_file.to_str().unwrap(),
            ro_dir.to_str().unwrap(),
            plain.to_str().unwrap(),
            broken_link.to_str().unwrap(),
        ]
    })
    .to_string();

    let out = run_agent(req.as_bytes());
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let res: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout JSON");
    assert_eq!(res["ok"], true, "{}", res);
    assert!(!ro_dir.exists());
    assert!(!plain.exists());
    assert!(!broken_link.exists());
}

#[test]
fn paste_stream_emits_scan_progress_and_done_for_copy_and_move() {
    let src_root = make_tree();
    let dst_copy = tempdir().unwrap();
    let copy_req = serde_json::json!({
        "op": "paste_stream",
        "paths": [
            src_root.path().join("sub").to_str().unwrap(),
            src_root.path().join("a.txt").to_str().unwrap(),
        ],
        "dest_dir": dst_copy.path().to_str().unwrap(),
        "mode": "copy",
    })
    .to_string()
        + "\n";

    let out = run_agent(copy_req.as_bytes());
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let events: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid NDJSON line"))
        .collect();
    assert_eq!(events.first().unwrap()["event"], "scan");
    assert!(events.iter().any(|e| e["event"] == "progress"));
    let done = events.last().unwrap();
    assert_eq!(done["event"], "done");
    assert_eq!(done["result"]["files"], 2);
    assert_eq!(
        std::fs::read(dst_copy.path().join("sub/big.bin")).unwrap(),
        big_payload()
    );
    assert!(src_root.path().join("sub/big.bin").exists());

    // Now test move
    let dst_move = tempdir().unwrap();
    let move_req = serde_json::json!({
        "op": "paste_stream",
        "paths": [src_root.path().join("a.txt").to_str().unwrap()],
        "dest_dir": dst_move.path().to_str().unwrap(),
        "mode": "move",
    })
    .to_string()
        + "\n";
    let move_out = run_agent(move_req.as_bytes());
    assert!(move_out.status.success(), "stderr: {}", String::from_utf8_lossy(&move_out.stderr));
    let move_events: Vec<serde_json::Value> = String::from_utf8_lossy(&move_out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid NDJSON line"))
        .collect();
    assert_eq!(move_events.last().unwrap()["event"], "done");
    assert!(!src_root.path().join("a.txt").exists());
    assert_eq!(std::fs::read(dst_move.path().join("a.txt")).unwrap(), b"hello");
}
