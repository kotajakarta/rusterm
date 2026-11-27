use tempfile::tempdir;
use xterm_backend::models::db::DbStore;
use xterm_backend::models::server::BackupPayload;
use xterm_backend::models::storage::{StorageAccountInput, StorageAccountUpdateInput};

#[test]
fn test_storage_accounts_crud_and_secret_encryption() {
    let dir = tempdir().unwrap();
    let db = DbStore::new(&dir.path().join("storage.sqlite")).unwrap();
    let kek = [7u8; 32];

    assert!(db.list_storage_accounts().unwrap().is_empty());

    let id = db
        .create_storage_account(
            &StorageAccountInput {
                name: "Cloudflare R2 Prod".into(),
                provider: "r2".into(),
                endpoint: "https://acct123.r2.cloudflarestorage.com".into(),
                region: "auto".into(),
                access_key_id: "AKIA1234567890".into(),
                secret_access_key: "super-secret-r2-key".into(),
                default_bucket: "backups".into(),
                path_style: true,
            },
            &kek,
        )
        .unwrap();

    let list = db.list_storage_accounts().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].name, "Cloudflare R2 Prod");
    assert_eq!(list[0].provider, "r2");
    assert_eq!(list[0].region, "auto");
    assert_eq!(list[0].default_bucket, "backups");
    assert!(list[0].has_secret);
    assert!(list[0].path_style);

    // Ensure secret is NOT stored plaintext in SQLite
    let raw_db = rusqlite::Connection::open(dir.path().join("storage.sqlite")).unwrap();
    let stored_secret: String = raw_db
        .query_row(
            "SELECT secret_access_key FROM storage_accounts WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(stored_secret, "super-secret-r2-key");
    assert!(!stored_secret.contains("super-secret"));

    // Decrypt round-trip
    let dec = db.get_storage_account_decrypted(id, &kek).unwrap();
    assert_eq!(dec.access_key_id, "AKIA1234567890");
    assert_eq!(dec.secret_access_key, "super-secret-r2-key");

    // Update without secret preserves existing secret
    db.update_storage_account(
        id,
        &StorageAccountUpdateInput {
            name: "R2 Updated".into(),
            provider: "r2".into(),
            endpoint: "https://acct123.r2.cloudflarestorage.com".into(),
            region: "auto".into(),
            access_key_id: "AKIA1234567890".into(),
            secret_access_key: None,
            default_bucket: "media".into(),
            path_style: true,
        },
        &kek,
    )
    .unwrap();
    let dec2 = db.get_storage_account_decrypted(id, &kek).unwrap();
    assert_eq!(dec2.name, "R2 Updated");
    assert_eq!(dec2.default_bucket, "media");
    assert_eq!(dec2.secret_access_key, "super-secret-r2-key");

    // Update with empty secret also preserves existing secret
    db.update_storage_account(
        id,
        &StorageAccountUpdateInput {
            name: "R2 Updated 2".into(),
            provider: "r2".into(),
            endpoint: "https://acct123.r2.cloudflarestorage.com".into(),
            region: "auto".into(),
            access_key_id: "AKIA1234567890".into(),
            secret_access_key: Some("".into()),
            default_bucket: "media".into(),
            path_style: true,
        },
        &kek,
    )
    .unwrap();
    let dec3 = db.get_storage_account_decrypted(id, &kek).unwrap();
    assert_eq!(dec3.secret_access_key, "super-secret-r2-key");

    // Update with new secret replaces secret
    db.update_storage_account(
        id,
        &StorageAccountUpdateInput {
            name: "R2 Rotated".into(),
            provider: "r2".into(),
            endpoint: "https://acct123.r2.cloudflarestorage.com".into(),
            region: "auto".into(),
            access_key_id: "AKIA9999".into(),
            secret_access_key: Some("rotated-secret".into()),
            default_bucket: "media".into(),
            path_style: false,
        },
        &kek,
    )
    .unwrap();
    let dec4 = db.get_storage_account_decrypted(id, &kek).unwrap();
    assert_eq!(dec4.secret_access_key, "rotated-secret");
    assert!(!dec4.path_style);

    // Re-encrypt with new PIN KEK works
    let new_kek = [9u8; 32];
    db.rotate_kek(&kek, &new_kek).unwrap();
    let dec5 = db.get_storage_account_decrypted(id, &new_kek).unwrap();
    assert_eq!(dec5.secret_access_key, "rotated-secret");

    // Delete
    db.delete_storage_account(id).unwrap();
    assert!(db.list_storage_accounts().unwrap().is_empty());
}

#[test]
fn test_storage_accounts_backup_export_import_and_compat() {
    let dir = tempdir().unwrap();
    let db1 = DbStore::new(&dir.path().join("s1.sqlite")).unwrap();
    let db2 = DbStore::new(&dir.path().join("s2.sqlite")).unwrap();
    let kek1 = [1u8; 32];
    let kek2 = [2u8; 32];

    db1.create_storage_account(
        &StorageAccountInput {
            name: "Local MinIO".into(),
            provider: "minio".into(),
            endpoint: "http://127.0.0.1:9000".into(),
            region: "us-east-1".into(),
            access_key_id: "minioadmin".into(),
            secret_access_key: "miniosecret".into(),
            default_bucket: "dev".into(),
            path_style: true,
        },
        &kek1,
    )
    .unwrap();

    let payload = db1.export_backup_data(&kek1).unwrap();
    assert_eq!(payload.storage_accounts.len(), 1);
    assert_eq!(payload.storage_accounts[0].secret_access_key, "miniosecret");

    let stats = db2.import_backup_data(&payload, &kek2).unwrap();
    assert_eq!(stats.storage_accounts_added, 1);
    assert_eq!(stats.storage_accounts_updated, 0);

    let list2 = db2.list_storage_accounts().unwrap();
    assert_eq!(list2.len(), 1);
    let dec2 = db2.get_storage_account_decrypted(list2[0].id, &kek2).unwrap();
    assert_eq!(dec2.secret_access_key, "miniosecret");

    // Re-importing updates existing instead of duplicating
    let stats2 = db2.import_backup_data(&payload, &kek2).unwrap();
    assert_eq!(stats2.storage_accounts_added, 0);
    assert_eq!(stats2.storage_accounts_updated, 1);
    assert_eq!(db2.list_storage_accounts().unwrap().len(), 1);

    // Backward-compatibility: legacy BackupPayload JSON without storage_accounts field
    let legacy_json = r#"{"version":1,"servers":[],"snippets":[]}"#;
    let legacy_payload: BackupPayload = serde_json::from_str(legacy_json).unwrap();
    assert!(legacy_payload.storage_accounts.is_empty());
}
