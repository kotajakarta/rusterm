use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use tempfile::tempdir;
use xterm_backend::services::ssh_pool::FILES_AGENT_SRC;

fn call_agent(payload: &serde_json::Value) -> serde_json::Value {
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(FILES_AGENT_SRC)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("python3 must be available");
    let mut input = payload.to_string().into_bytes();
    input.push(b'\n');
    child.stdin.take().unwrap().write_all(&input).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "agent exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("agent stdout must be valid JSON")
}

#[test]
fn search_finds_matches_and_excludes_default_dirs_gitignore_and_binary() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();

    std::fs::write(dir.path().join(".gitignore"), "ignored.log\n").unwrap();
    std::fs::write(
        dir.path().join("src/app.ts"),
        "const foo = \"Foo\";\nexport const foobar = foo;\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("node_modules/pkg/index.js"),
        "const foo = 1;\n",
    )
    .unwrap();
    std::fs::write(dir.path().join(".git/config"), "foo = bar\n").unwrap();
    std::fs::write(dir.path().join("ignored.log"), "foo in gitignore\n").unwrap();
    std::fs::write(dir.path().join("bin.dat"), b"foo\x00bar\n").unwrap();

    let res = call_agent(&serde_json::json!({
        "op": "search",
        "path": dir.path().to_str().unwrap(),
        "query": "foo",
        "use_gitignore": true
    }));
    assert_eq!(res["ok"], true, "res={}", res);
    let r = &res["result"];
    assert_eq!(r["files_matched"], 1);
    assert_eq!(r["total_matches"], 4);
    assert_eq!(r["skipped_binary"], 1);
    assert_eq!(r["truncated"], false);
    assert_eq!(r["files"][0]["rel_path"], "src/app.ts");
    assert_eq!(r["files"][0]["matches"][0]["line"], 1);
    assert_eq!(r["files"][0]["matches"][0]["col"], 7);
}

#[test]
fn search_supports_case_sensitive_whole_word_regex_include_and_invalid_regex() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("code.ts"),
        "const foo = \"Foo\";\nconst foobar = foo;\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("notes.md"), "foo in markdown\n").unwrap();

    // Case sensitive
    let res = call_agent(&serde_json::json!({
        "op": "search",
        "path": dir.path().to_str().unwrap(),
        "query": "Foo",
        "case_sensitive": true
    }));
    assert_eq!(res["ok"], true);
    assert_eq!(res["result"]["total_matches"], 1);

    // Whole word (case-insensitive)
    let res = call_agent(&serde_json::json!({
        "op": "search",
        "path": dir.path().to_str().unwrap(),
        "query": "foo",
        "whole_word": true,
        "include": ["*.ts"]
    }));
    assert_eq!(res["ok"], true);
    assert_eq!(res["result"]["files_matched"], 1);
    assert_eq!(res["result"]["total_matches"], 3); // foo, Foo, foo (not foobar)

    // Regex
    let res = call_agent(&serde_json::json!({
        "op": "search",
        "path": dir.path().to_str().unwrap(),
        "query": r"const\s+\w+",
        "use_regex": true
    }));
    assert_eq!(res["ok"], true);
    assert_eq!(res["result"]["total_matches"], 2);

    // Invalid regex returns clean error
    let res = call_agent(&serde_json::json!({
        "op": "search",
        "path": dir.path().to_str().unwrap(),
        "query": "[unclosed",
        "use_regex": true
    }));
    assert_eq!(res["ok"], false);
    assert!(
        res["error"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .contains("regex"),
        "res={}",
        res
    );
}

#[test]
fn search_truncates_at_max_results() {
    let dir = tempdir().unwrap();
    let content = (0..20).map(|i| format!("hit line {}\n", i)).collect::<String>();
    std::fs::write(dir.path().join("many.txt"), content).unwrap();

    let res = call_agent(&serde_json::json!({
        "op": "search",
        "path": dir.path().to_str().unwrap(),
        "query": "hit",
        "max_results": 5
    }));
    assert_eq!(res["ok"], true);
    assert_eq!(res["result"]["total_matches"], 5);
    assert_eq!(res["result"]["truncated"], true);
}

#[test]
fn replace_updates_selected_targets_preserves_mode_and_checks_mtime() {
    let dir = tempdir().unwrap();
    let a_path = dir.path().join("a.txt");
    let b_path = dir.path().join("b.txt");
    std::fs::write(&a_path, "hello world\nhello\n").unwrap();
    std::fs::write(&b_path, "hello other\n").unwrap();
    std::fs::set_permissions(&a_path, std::fs::Permissions::from_mode(0o640)).unwrap();

    let search = call_agent(&serde_json::json!({
        "op": "search",
        "path": dir.path().to_str().unwrap(),
        "query": "hello"
    }));
    assert_eq!(search["ok"], true);
    let a_entry = search["result"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["rel_path"] == "a.txt")
        .unwrap();
    let a_mtime = a_entry["mtime"].as_f64().unwrap();

    // Replace only a.txt
    let rep = call_agent(&serde_json::json!({
        "op": "replace",
        "path": dir.path().to_str().unwrap(),
        "query": "hello",
        "replacement": "hi",
        "targets": [{ "path": a_path.to_str().unwrap(), "expected_mtime": a_mtime }]
    }));
    assert_eq!(rep["ok"], true, "rep={}", rep);
    assert_eq!(rep["result"]["files_modified"], 1);
    assert_eq!(rep["result"]["total_replacements"], 2);
    assert_eq!(rep["result"]["errors"].as_array().unwrap().len(), 0);
    assert_eq!(std::fs::read_to_string(&a_path).unwrap(), "hi world\nhi\n");
    assert_eq!(std::fs::read_to_string(&b_path).unwrap(), "hello other\n");
    let mode = std::fs::metadata(&a_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640);

    // Stale expected_mtime should be skipped with error
    let stale = call_agent(&serde_json::json!({
        "op": "replace",
        "path": dir.path().to_str().unwrap(),
        "query": "hi",
        "replacement": "yo",
        "targets": [{ "path": a_path.to_str().unwrap(), "expected_mtime": a_mtime - 10.0 }]
    }));
    assert_eq!(stale["ok"], true);
    assert_eq!(stale["result"]["files_modified"], 0);
    assert_eq!(stale["result"]["errors"].as_array().unwrap().len(), 1);
    assert_eq!(std::fs::read_to_string(&a_path).unwrap(), "hi world\nhi\n");
}

#[test]
fn replace_rejects_path_outside_root_and_symlink_escape() {
    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let secret = outside.path().join("secret.txt");
    std::fs::write(&secret, "secret token\n").unwrap();
    let link = root.path().join("escape.txt");
    std::os::unix::fs::symlink(&secret, &link).unwrap();

    let rep = call_agent(&serde_json::json!({
        "op": "replace",
        "path": root.path().to_str().unwrap(),
        "query": "secret",
        "replacement": "hacked",
        "targets": [
            { "path": secret.to_str().unwrap() },
            { "path": link.to_str().unwrap() }
        ]
    }));
    assert_eq!(rep["ok"], true, "rep={}", rep);
    assert_eq!(rep["result"]["files_modified"], 0);
    assert_eq!(rep["result"]["errors"].as_array().unwrap().len(), 2);
    assert_eq!(std::fs::read_to_string(&secret).unwrap(), "secret token\n");
}
