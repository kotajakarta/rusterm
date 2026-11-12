use tempfile::tempdir;
use xterm_backend::models::db::DbStore;

#[test]
fn test_tailscale_accounts_db_crud() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("ts_test.sqlite");
    let db = DbStore::new(&db_path).unwrap();

    // 1. Initial list should be empty
    let initial_list = db.list_tailscale_accounts().unwrap();
    assert_eq!(initial_list.len(), 0);

    // 2. Create account
    let created = db
        .create_tailscale_account("My Tailnet", "tskey-api-k1234567890abcdef", "-")
        .unwrap();
    assert_eq!(created.name, "My Tailnet");
    assert_eq!(created.tailnet, "-");
    assert!(created.token_masked.contains("••••"));

    // 3. Get account with raw token
    let (acc, raw_token) = db.get_tailscale_account(created.id).unwrap().unwrap();
    assert_eq!(acc.id, created.id);
    assert_eq!(raw_token, "tskey-api-k1234567890abcdef");

    // 4. Update account
    let updated = db
        .update_tailscale_account(
            created.id,
            "Updated Tailnet",
            Some("tskey-api-new9876543210zyxw"),
            Some("mycompany.ts.net"),
        )
        .unwrap();
    assert_eq!(updated.name, "Updated Tailnet");
    assert_eq!(updated.tailnet, "mycompany.ts.net");
    assert!(updated.token_masked.contains("••••"));

    let (_, updated_raw) = db.get_tailscale_account(created.id).unwrap().unwrap();
    assert_eq!(updated_raw, "tskey-api-new9876543210zyxw");

    // 5. Delete account
    db.delete_tailscale_account(created.id).unwrap();
    let after_delete = db.get_tailscale_account(created.id).unwrap();
    assert!(after_delete.is_none());
}
