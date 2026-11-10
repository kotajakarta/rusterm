use xterm_backend::services::crypto::CryptoService;
use xterm_backend::services::vault::Vault;

#[tokio::test]
async fn test_crypto_encryption_roundtrip() {
    let key = [42u8; 32];
    let secret = b"super-secret-ssh-password";

    let encrypted = CryptoService::encrypt(secret, &key).expect("encryption failed");
    assert_ne!(encrypted, "super-secret-ssh-password");

    let decrypted = CryptoService::decrypt(&encrypted, &key).expect("decryption failed");
    assert_eq!(decrypted, secret);
}

#[tokio::test]
async fn test_crypto_wrong_key_fails() {
    let key1 = [42u8; 32];
    let key2 = [99u8; 32];
    let secret = b"my-private-key";

    let encrypted = CryptoService::encrypt(secret, &key1).unwrap();
    let result = CryptoService::decrypt(&encrypted, &key2);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_vault_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let vault_file = dir.path().join("vault.json");

    let vault = Vault::new(&vault_file).await.unwrap();

    // 1. Initially uninitialized
    let status = vault.status("invalid-session").await;
    assert!(!status.initialized);
    assert!(!status.unlocked);

    // 2. Setup PIN
    let session = vault.setup_pin("123456").await.expect("setup pin failed");
    assert!(!session.is_empty());

    let status = vault.status(&session).await;
    assert!(status.initialized);
    assert!(status.unlocked);
    assert!(status.session_valid);

    // 3. Lock
    vault.lock().await;
    let status = vault.status(&session).await;
    assert!(!status.unlocked);
    assert!(!status.session_valid);

    // 4. Login with wrong PIN fails
    assert!(vault.login("000000").await.is_err());

    // 5. Login with right PIN succeeds
    let new_session = vault.login("123456").await.expect("login failed");
    let status = vault.status(&new_session).await;
    assert!(status.unlocked);
    assert!(status.session_valid);
}

#[tokio::test]
async fn test_vault_change_pin_and_key_rotation() {
    use xterm_backend::models::db::DbStore;
    use xterm_backend::models::server::ServerInput;

    let dir = tempfile::tempdir().unwrap();
    let vault_file = dir.path().join("vault.json");
    let db_file = dir.path().join("xterm.sqlite");

    let vault = Vault::new(&vault_file).await.unwrap();
    let db = DbStore::new(&db_file).unwrap();

    let _session = vault.setup_pin("123456").await.unwrap();
    let old_kek = vault.get_kek().await.unwrap();

    let server_id = db
        .create_server(
            &ServerInput {
                name: "Test Srv".to_string(),
                host: "localhost".to_string(),
                port: 22,
                username: "user".to_string(),
                auth_type: "password".to_string(),
                password: Some("my-password-123".to_string()),
                private_key: None,
                passphrase: None,
                group_name: "Dev".to_string(),
                color: "".to_string(),
            },
            &old_kek,
        )
        .unwrap();

    // Change PIN from 123456 to 654321
    vault
        .change_pin("123456", "654321", &db)
        .await
        .expect("change pin failed");

    // Login with old PIN must fail
    assert!(vault.login("123456").await.is_err());

    // Login with new PIN must succeed
    let _session2 = vault.login("654321").await.unwrap();
    let new_kek = vault.get_kek().await.unwrap();

    // Verify credentials decrypt cleanly with new KEK
    let creds = db.get_server_credentials(server_id, &new_kek).unwrap();
    assert_eq!(creds.password.as_deref(), Some("my-password-123"));
}

#[tokio::test]
async fn test_backup_crypto_roundtrip() {
    use xterm_backend::models::server::{
        BackupBookmark, BackupPayload, BackupServer, BackupSnippet,
    };
    use xterm_backend::services::crypto::{BACKUP_MAGIC, BACKUP_VERSION};

    let payload = BackupPayload {
        version: 1,
        servers: vec![BackupServer {
            name: "Server 1".to_string(),
            host: "10.0.0.1".to_string(),
            port: 22,
            username: "admin".to_string(),
            auth_type: "password".to_string(),
            password: Some("pass".to_string()),
            private_key: None,
            passphrase: None,
            group_name: "Prod".to_string(),
            color: "#00ff00".to_string(),
            bookmarks: vec![BackupBookmark {
                name: "Root".to_string(),
                path: "/root".to_string(),
                startup_cmd: "ls".to_string(),
            }],
        }],
        snippets: vec![BackupSnippet {
            name: "Check disk".to_string(),
            command: "df -h".to_string(),
        }],
    };

    let envelope = CryptoService::encrypt_backup(&payload, "my-backup-pin").unwrap();
    assert_eq!(envelope.magic, BACKUP_MAGIC);
    assert_eq!(envelope.version, BACKUP_VERSION);
    assert_eq!(envelope.cipher, "AES-256-GCM");

    // Decrypt with wrong pin fails
    assert!(CryptoService::decrypt_backup(&envelope, "wrong-pin").is_err());

    // Decrypt with correct pin succeeds
    let decrypted = CryptoService::decrypt_backup(&envelope, "my-backup-pin").unwrap();
    assert_eq!(decrypted.servers.len(), 1);
    assert_eq!(decrypted.servers[0].name, "Server 1");
    assert_eq!(decrypted.snippets.len(), 1);
    assert_eq!(decrypted.snippets[0].name, "Check disk");
}
