use xterm_backend::services::updater::{
    cleanup_stale_old_files, validate_executable_bytes, UpdateProgress,
};

#[test]
fn test_validate_executable_bytes() {
    // 1. Empty or too small
    assert!(validate_executable_bytes(&[]).is_err());
    assert!(validate_executable_bytes(&[0u8; 100]).is_err());

    // 2. Windows PE executable magic header (MZ)
    let mut pe_bytes = vec![0u8; 2048];
    pe_bytes[0] = b'M';
    pe_bytes[1] = b'Z';
    assert!(validate_executable_bytes(&pe_bytes).is_ok());

    // 3. Linux ELF executable magic header (\x7fELF)
    let mut elf_bytes = vec![0u8; 2048];
    elf_bytes[0] = 0x7f;
    elf_bytes[1] = b'E';
    elf_bytes[2] = b'L';
    elf_bytes[3] = b'F';
    assert!(validate_executable_bytes(&elf_bytes).is_ok());

    // 4. Random non-executable bytes
    let random_bytes = vec![0x12, 0x34, 0x56, 0x78];
    assert!(validate_executable_bytes(&random_bytes).is_err());
}

#[tokio::test]
async fn test_update_progress_tracker() {
    let tracker = UpdateProgress::new();
    let initial = tracker.get_progress().await;
    assert_eq!(initial.state, "idle");
    assert_eq!(initial.progress_percent, 0);

    tracker
        .set_downloading(500, 1000, "Mengunduh... 50%")
        .await;
    let down = tracker.get_progress().await;
    assert_eq!(down.state, "downloading");
    assert_eq!(down.progress_percent, 50);
    assert_eq!(down.downloaded_bytes, 500);
    assert_eq!(down.total_bytes, 1000);

    tracker.set_applying("Memasang pembaruan...").await;
    let applying = tracker.get_progress().await;
    assert_eq!(applying.state, "applying");
    assert_eq!(applying.progress_percent, 100);

    tracker.set_completed("Pembaruan selesai!").await;
    let done = tracker.get_progress().await;
    assert_eq!(done.state, "completed");

    tracker.set_error("Gagal mengunduh").await;
    let err = tracker.get_progress().await;
    assert_eq!(err.state, "error");
    assert_eq!(err.message, "Gagal mengunduh");
}

#[test]
fn test_cleanup_stale_old_files() {
    let temp_dir = std::env::temp_dir().join(format!("rusterm_test_cleanup_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let old_file = temp_dir.join("rusterm.exe.old");
    std::fs::write(&old_file, b"old binary").unwrap();
    assert!(old_file.exists());

    let normal_file = temp_dir.join("rusterm.exe");
    std::fs::write(&normal_file, b"normal binary").unwrap();
    assert!(normal_file.exists());

    cleanup_stale_old_files(&temp_dir);

    assert!(!old_file.exists());
    assert!(normal_file.exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
