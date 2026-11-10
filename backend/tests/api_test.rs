use xterm_backend::models::db::DbStore;
use xterm_backend::models::server::ServerInput;
use xterm_backend::services::vault::Vault;
use tempfile::tempdir;

#[tokio::test]
async fn test_db_server_crud_and_encryption() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.sqlite");
    let vault_file = dir.path().join("vault.json");

    let vault = Vault::new(&vault_file).await.unwrap();
    let _session = vault.setup_pin("123456").await.unwrap();
    let kek = vault.get_kek().await.unwrap();

    let db = DbStore::new(&db_path).unwrap();

    // 1. Create server
    let input = ServerInput {
        name: "Production Server".to_string(),
        host: "192.168.1.100".to_string(),
        port: 22,
        username: "root".to_string(),
        auth_type: "password".to_string(),
        password: Some("my-ssh-password".to_string()),
        private_key: None,
        passphrase: None,
        group_name: "Web".to_string(),
        color: "#ff0000".to_string(),
    };

    let server_id = db.create_server(&input, &kek).unwrap();
    assert!(server_id > 0);

    // 2. List servers (secrets masked/bools)
    let list = db.list_servers().unwrap();
    assert_eq!(list.len(), 1);
    let s = &list[0];
    assert_eq!(s.id, server_id);
    assert_eq!(s.name, "Production Server");
    assert!(s.has_password);
    assert!(!s.has_private_key);

    // 3. Get server credentials (unmasked with KEK)
    let creds = db.get_server_credentials(server_id, &kek).unwrap();
    assert_eq!(creds.password.as_deref(), Some("my-ssh-password"));

    // 4. Update server without changing password
    let update_input = ServerInput {
        name: "Production Server Renamed".to_string(),
        host: "192.168.1.100".to_string(),
        port: 2222,
        username: "admin".to_string(),
        auth_type: "password".to_string(),
        password: None, // keep existing password
        private_key: None,
        passphrase: None,
        group_name: "Web".to_string(),
        color: "#ff0000".to_string(),
    };
    db.update_server(server_id, &update_input, &kek).unwrap();

    let creds = db.get_server_credentials(server_id, &kek).unwrap();
    assert_eq!(creds.password.as_deref(), Some("my-ssh-password"));
    assert_eq!(creds.port, 2222);
    assert_eq!(creds.username, "admin");

    // 5. Delete server
    db.delete_server(server_id).unwrap();
    assert!(db.list_servers().unwrap().is_empty());
}

#[tokio::test]
async fn test_reorder_servers() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_reorder.sqlite");
    let vault_file = dir.path().join("vault.json");

    let vault = Vault::new(&vault_file).await.unwrap();
    let _session = vault.setup_pin("123456").await.unwrap();
    let kek = vault.get_kek().await.unwrap();
    let db = DbStore::new(&db_path).unwrap();

    let s1 = db
        .create_server(
            &ServerInput {
                name: "Server 1".into(),
                host: "10.0.0.1".into(),
                port: 22,
                username: "u1".into(),
                auth_type: "password".into(),
                password: Some("p1".into()),
                private_key: None,
                passphrase: None,
                group_name: "G".into(),
                color: "".into(),
            },
            &kek,
        )
        .unwrap();

    let s2 = db
        .create_server(
            &ServerInput {
                name: "Server 2".into(),
                host: "10.0.0.2".into(),
                port: 22,
                username: "u2".into(),
                auth_type: "password".into(),
                password: Some("p2".into()),
                private_key: None,
                passphrase: None,
                group_name: "G".into(),
                color: "".into(),
            },
            &kek,
        )
        .unwrap();

    let list = db.list_servers().unwrap();
    assert_eq!(list[0].id, s1);
    assert_eq!(list[1].id, s2);

    // Reorder: s2 before s1
    db.reorder_servers(&[s2, s1]).unwrap();

    let list_after = db.list_servers().unwrap();
    assert_eq!(list_after[0].id, s2);
    assert_eq!(list_after[1].id, s1);
}

#[tokio::test]
async fn test_db_backup_export_and_import() {
    use xterm_backend::models::bookmark::BookmarkInput;
    use xterm_backend::models::snippet::SnippetInput;

    let dir = tempdir().unwrap();
    let db1_path = dir.path().join("db1.sqlite");
    let db2_path = dir.path().join("db2.sqlite");
    let vault_file = dir.path().join("vault.json");

    let vault = Vault::new(&vault_file).await.unwrap();
    let _session = vault.setup_pin("123456").await.unwrap();
    let kek = vault.get_kek().await.unwrap();

    let db1 = DbStore::new(&db1_path).unwrap();

    let srv_id = db1
        .create_server(
            &ServerInput {
                name: "Production".into(),
                host: "10.0.0.50".into(),
                port: 22,
                username: "ubuntu".into(),
                auth_type: "password".into(),
                password: Some("securepass".into()),
                private_key: None,
                passphrase: None,
                group_name: "Cloud".into(),
                color: "#10b981".into(),
            },
            &kek,
        )
        .unwrap();

    db1.create_bookmark(&BookmarkInput {
        server_id: srv_id,
        name: "Logs".into(),
        path: "/var/log".into(),
        startup_cmd: "tail -f syslog".into(),
    })
    .unwrap();

    db1.create_snippet(&SnippetInput {
        name: "Uptime".into(),
        command: "uptime".into(),
    })
    .unwrap();

    // Export from db1
    let backup_payload = db1.export_backup_data(&kek).unwrap();
    assert_eq!(backup_payload.servers.len(), 1);
    assert_eq!(backup_payload.servers[0].bookmarks.len(), 1);
    assert_eq!(backup_payload.snippets.len(), 1);

    // Import into db2
    let db2 = DbStore::new(&db2_path).unwrap();
    let stats = db2.import_backup_data(&backup_payload, &kek).unwrap();
    assert_eq!(stats.servers_added, 1);
    assert_eq!(stats.bookmarks_added, 1);
    assert_eq!(stats.snippets_added, 1);

    // Verify imported data in db2
    let db2_servers = db2.list_servers().unwrap();
    assert_eq!(db2_servers.len(), 1);
    assert_eq!(db2_servers[0].name, "Production");

    let db2_creds = db2.get_server_credentials(db2_servers[0].id, &kek).unwrap();
    assert_eq!(db2_creds.password.as_deref(), Some("securepass"));

    let db2_bms = db2.list_bookmarks().unwrap();
    assert_eq!(db2_bms.len(), 1);
    assert_eq!(db2_bms[0].name, "Logs");

    let db2_snippets = db2.list_snippets().unwrap();
    assert_eq!(db2_snippets.len(), 1);
    assert_eq!(db2_snippets[0].name, "Uptime");
}

#[tokio::test]
async fn test_db_file_remotes_and_bookmarks() {
    use xterm_backend::models::files::FileRemoteInput;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_files.sqlite");
    let vault_file = dir.path().join("vault.json");
    let vault = Vault::new(&vault_file).await.unwrap();
    let _ = vault.setup_pin("123456").await.unwrap();
    let kek = vault.get_kek().await.unwrap();
    let db = DbStore::new(&db_path).unwrap();

    let s1 = db.create_server(&ServerInput {
        name: "S1".into(),
        host: "1.1.1.1".into(),
        port: 22,
        username: "root".into(),
        auth_type: "password".into(),
        password: Some("p".into()),
        private_key: None,
        passphrase: None,
        group_name: "".into(),
        color: "".into(),
    }, &kek).unwrap();

    let s2 = db.create_server(&ServerInput {
        name: "S2".into(),
        host: "2.2.2.2".into(),
        port: 22,
        username: "root".into(),
        auth_type: "password".into(),
        password: Some("p".into()),
        private_key: None,
        passphrase: None,
        group_name: "".into(),
        color: "".into(),
    }, &kek).unwrap();

    // 1. Create file remote
    let remote_in = FileRemoteInput {
        name: "Sync Web".into(),
        local_server_id: s1,
        local_path: "/var/www/html".into(),
        remote_server_id: s2,
        remote_path: "/backup/html".into(),
        ignore: vec![".git".into(), "node_modules".into()],
    };
    let remote = db.create_file_remote(&remote_in).unwrap();
    assert!(remote.id > 0);
    assert_eq!(remote.name, "Sync Web");
    assert_eq!(remote.ignore.len(), 2);

    // 2. List remotes
    let list = db.list_file_remotes().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, remote.id);

    // 3. Update remote
    let updated_in = FileRemoteInput {
        name: "Sync Web Updated".into(),
        local_server_id: s1,
        local_path: "/var/www/html".into(),
        remote_server_id: s2,
        remote_path: "/backup/html2".into(),
        ignore: vec![".git".into()],
    };
    let updated = db.update_file_remote(remote.id, &updated_in).unwrap();
    assert_eq!(updated.name, "Sync Web Updated");
    assert_eq!(updated.remote_path, "/backup/html2");
    assert_eq!(updated.ignore.len(), 1);

    // 4. File bookmarks toggle and sync
    let b1 = db.toggle_file_bookmark(s1, "/var/log").unwrap();
    assert!(b1); // added
    let paths = db.list_file_bookmark_paths(s1).unwrap();
    assert_eq!(paths, vec!["/var/log"]);

    let b2 = db.toggle_file_bookmark(s1, "/var/log").unwrap();
    assert!(!b2); // toggled off
    assert!(db.list_file_bookmark_paths(s1).unwrap().is_empty());

    // Sync multiple bookmarks
    let synced = db.sync_file_bookmarks(s1, &["/etc/nginx".into(), "/var/www".into()]).unwrap();
    assert_eq!(synced.len(), 2);

    // 5. Delete remote
    db.delete_file_remote(remote.id).unwrap();
    assert!(db.list_file_remotes().unwrap().is_empty());
}

#[tokio::test]
async fn test_db_cf_accounts() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_cf.sqlite");
    let db = DbStore::new(&db_path).unwrap();

    // 1. Create account
    let acc = db
        .create_cf_account("My CF", "abcdef1234567890xyz", "acc_id_1", "Personal")
        .unwrap();
    assert_eq!(acc.name, "My CF");
    assert_eq!(acc.account_id, "acc_id_1");
    assert_eq!(acc.account_name, "Personal");
    assert!(acc.token_masked.contains("••••"));

    // 2. List accounts
    let list = db.list_cf_accounts().unwrap();
    assert_eq!(list.len(), 1);

    // 3. Get account with secret token
    let (acc_get, token) = db.get_cf_account(acc.id).unwrap().unwrap();
    assert_eq!(acc_get.id, acc.id);
    assert_eq!(token, "abcdef1234567890xyz");

    // 4. Update account
    let updated = db
        .update_cf_account(acc.id, "My CF Renamed", Some("newtoken12345678"))
        .unwrap();
    assert_eq!(updated.name, "My CF Renamed");
    let (_, new_tok) = db.get_cf_account(acc.id).unwrap().unwrap();
    assert_eq!(new_tok, "newtoken12345678");

    // 5. Delete account
    db.delete_cf_account(acc.id).unwrap();
    assert!(db.list_cf_accounts().unwrap().is_empty());
}
