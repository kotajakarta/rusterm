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
