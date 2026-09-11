// tests/history_tests.rs

use std::path::PathBuf;
use video_downloader::config::{
    DEFAULT_HISTORY_LIMIT, format_config_full, load_history_limit_from_path,
    parse_config_history_limit,
};
use video_downloader::history::{
    HistoryEntry, HistoryStatus, load_history_from_path, open_in_file_manager, prune_history,
    save_history_to_path,
};

#[test]
fn test_history_entry_serialization_and_deserialization() {
    let entry = HistoryEntry::new_completed(
        1,
        "https://example.com/video1.mp4".to_string(),
        Some("video1.mp4".to_string()),
        Some("/downloads/video1.mp4".to_string()),
    );

    let json = serde_json::to_string(&entry).expect("Failed to serialize HistoryEntry");
    let deserialized: HistoryEntry =
        serde_json::from_str(&json).expect("Failed to deserialize HistoryEntry");

    assert_eq!(entry.id, deserialized.id);
    assert_eq!(entry.url, deserialized.url);
    assert_eq!(entry.title, deserialized.title);
    assert_eq!(entry.file_path, deserialized.file_path);
    assert_eq!(deserialized.status, HistoryStatus::Completed);
    assert_eq!(entry.status.label(), "Completed");
    assert!(deserialized.error_message.is_none());
}

#[test]
fn test_history_entry_failed_stores_error_message() {
    let entry = HistoryEntry::new_failed(
        2,
        "https://example.com/bad_video.mp4".to_string(),
        None,
        Some("./downloads".to_string()),
        "HTTP 404: Not Found".to_string(),
    );

    let json = serde_json::to_string_pretty(&entry).expect("Failed to serialize");
    let deserialized: HistoryEntry = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(deserialized.status, HistoryStatus::Failed);
    assert_eq!(deserialized.status.label(), "Failed");
    assert_eq!(
        deserialized.error_message.as_deref(),
        Some("HTTP 404: Not Found")
    );
    assert_eq!(deserialized.file_path.as_deref(), Some("./downloads"));
}

#[test]
fn test_save_and_load_history_from_path_roundtrip() {
    let temp_dir = std::env::temp_dir().join("vidown_test_history_roundtrip");
    let history_file = temp_dir.join("history.json");

    let entries = vec![
        HistoryEntry::new_completed(
            1,
            "https://site.com/vid1".to_string(),
            Some("vid1.mp4".to_string()),
            Some("/tmp/vid1.mp4".to_string()),
        ),
        HistoryEntry::new_failed(
            2,
            "https://site.com/vid2".to_string(),
            Some("vid2.mp4".to_string()),
            Some("/tmp/vid2.mp4".to_string()),
            "Process terminated".to_string(),
        ),
    ];

    save_history_to_path(&history_file, &entries).expect("Failed to save history");

    let loaded = load_history_from_path(&history_file);
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].url, "https://site.com/vid1");
    assert_eq!(loaded[0].status, HistoryStatus::Completed);
    assert_eq!(loaded[1].url, "https://site.com/vid2");
    assert_eq!(loaded[1].status, HistoryStatus::Failed);
    assert_eq!(
        loaded[1].error_message.as_deref(),
        Some("Process terminated")
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_load_history_from_non_existent_file() {
    let non_existent = PathBuf::from("non_existent_dir_vidown_12345/history.json");
    let loaded = load_history_from_path(&non_existent);
    assert!(loaded.is_empty());
}

#[test]
fn test_load_history_from_corrupted_json_file() {
    let temp_dir = std::env::temp_dir().join("vidown_test_history_corrupt");
    let history_file = temp_dir.join("history.json");

    let _ = std::fs::create_dir_all(&temp_dir);
    std::fs::write(&history_file, "INVALID JSON {{{ [[[").expect("Failed to write corrupted file");

    let loaded = load_history_from_path(&history_file);
    assert!(
        loaded.is_empty(),
        "Corrupted JSON should gracefully load as empty vec"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_history_fifo_pruning_order() {
    let mut entries = Vec::new();
    for i in 1..=10 {
        entries.push(HistoryEntry::new_completed(
            i,
            format!("https://site.com/{}", i),
            Some(format!("file_{}.mp4", i)),
            Some(format!("/downloads/file_{}.mp4", i)),
        ));
    }

    assert_eq!(entries.len(), 10);
    // Prune down to 5
    prune_history(&mut entries, 5);

    assert_eq!(entries.len(), 5);
    // Index 0 is the newest (entry #1), tail was pruned
    assert_eq!(entries[0].id, 1);
    assert_eq!(entries[4].id, 5);
}

#[test]
fn test_default_history_limit_at_least_50() {
    const {
        assert!(
            DEFAULT_HISTORY_LIMIT >= 50,
            "Requirement specifies default history limit must be at least 50"
        );
    }
}

#[test]
fn test_config_history_limit_parsing_and_formatting() {
    let toml = r#"
# Vidown Config
output_dir = "/custom/dir"
history_limit = 75
"#;
    assert_eq!(parse_config_history_limit(toml), Some(75));

    let toml_inline_comment = r#"
history_limit = 100 # inline comment
"#;
    assert_eq!(parse_config_history_limit(toml_inline_comment), Some(100));

    let toml_quoted = r#"
history_limit = "60"
"#;
    assert_eq!(parse_config_history_limit(toml_quoted), Some(60));

    let formatted = format_config_full("/my/output", 120);
    assert_eq!(parse_config_history_limit(&formatted), Some(120));
}

#[test]
fn test_open_in_file_manager_does_not_panic() {
    // Calling open_in_file_manager with any valid or invalid path must not panic
    let res = open_in_file_manager(".");
    let _ = res; // May succeed or fail depending on desktop environment in headless CI

    let res_nonexistent = open_in_file_manager("nonexistent/test/path/video.mp4");
    let _ = res_nonexistent;
}

#[test]
fn test_load_history_limit_from_path() {
    let temp_dir = std::env::temp_dir().join("vidown_test_limit_parsing");
    let config_file = temp_dir.join("config.toml");
    let _ = std::fs::create_dir_all(&temp_dir);
    std::fs::write(&config_file, "history_limit = 88\n").unwrap();

    let limit = load_history_limit_from_path(&config_file);
    assert_eq!(limit, Some(88));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_app_persistence_with_custom_history_path() {
    let temp_dir = std::env::temp_dir().join("vidown_test_app_persistence");
    let history_file = temp_dir.join("history.json");
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut app = video_downloader::app::App::new_empty();
    app.history_path = Some(history_file.clone());
    app.persist_history = true;

    let entry = HistoryEntry::new_completed(
        1,
        "https://persisted.com/vid.mp4".to_string(),
        Some("vid.mp4".to_string()),
        Some("/dest/vid.mp4".to_string()),
    );
    app.add_history_entry(entry);

    // Verify file written to disk
    let loaded = load_history_from_path(&history_file);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].url, "https://persisted.com/vid.mp4");

    // Delete entry
    app.open_history_modal();
    app.delete_selected_history_entry();

    let loaded2 = load_history_from_path(&history_file);
    assert!(loaded2.is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_resolve_exact_path() {
    use video_downloader::app::resolve_exact_path;

    // Relative directory with filename resolves to an absolute path
    let resolved = resolve_exact_path("./downloads", Some("cool_video.mp4"));
    assert!(resolved.is_some());
    let path_str = resolved.unwrap();
    let path = std::path::Path::new(&path_str);
    assert!(path.is_absolute(), "Resolved path must be absolute");
    assert!(path_str.contains("cool_video.mp4"));

    // Absolute directory preserves absolute path
    #[cfg(target_os = "windows")]
    let abs_dir = r"C:\MyVideos";
    #[cfg(not(target_os = "windows"))]
    let abs_dir = "/var/videos";

    let abs_resolved = resolve_exact_path(abs_dir, Some("video.mp4")).unwrap();
    assert!(
        std::path::Path::new(&abs_resolved).is_absolute(),
        "Must remain absolute"
    );
    assert!(abs_resolved.contains("video.mp4"));
}

#[test]
fn test_atomic_save_overwrites_existing_file_cleanly() {
    let temp_dir = std::env::temp_dir().join("vidown_test_atomic_save");
    let history_file = temp_dir.join("history.json");
    let _ = std::fs::create_dir_all(&temp_dir);

    // Initial save of 2 entries
    let initial_entries = vec![
        HistoryEntry::new_completed(1, "https://a.com".to_string(), None, None),
        HistoryEntry::new_completed(2, "https://b.com".to_string(), None, None),
    ];
    save_history_to_path(&history_file, &initial_entries).expect("Save 1 failed");
    assert_eq!(load_history_from_path(&history_file).len(), 2);

    // Overwrite save of 4 entries
    let updated_entries = vec![
        HistoryEntry::new_completed(3, "https://c.com".to_string(), None, None),
        HistoryEntry::new_completed(4, "https://d.com".to_string(), None, None),
        HistoryEntry::new_completed(5, "https://e.com".to_string(), None, None),
        HistoryEntry::new_completed(6, "https://f.com".to_string(), None, None),
    ];
    save_history_to_path(&history_file, &updated_entries).expect("Save 2 failed");
    let reloaded = load_history_from_path(&history_file);
    assert_eq!(reloaded.len(), 4);
    assert_eq!(reloaded[0].url, "https://c.com");
    assert_eq!(reloaded[3].url, "https://f.com");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_open_selected_history_in_file_manager_states() {
    let mut app = video_downloader::app::App::new_empty();

    // 1. On empty history, status indicates no entry selected
    app.open_history_modal();
    app.open_selected_history_in_file_manager();
    assert_eq!(
        app.status_message.as_deref(),
        Some("No download history entry selected.")
    );

    // 2. On entry with None file_path, status indicates no file destination recorded
    let failed_entry = HistoryEntry::new_failed(
        1,
        "https://fail.com".to_string(),
        None,
        None,
        "Network error".to_string(),
    );
    app.add_history_entry(failed_entry);
    app.open_selected_history_in_file_manager();
    assert_eq!(
        app.status_message.as_deref(),
        Some("No file destination recorded for this history entry.")
    );

    // 3. On entry with valid file_path, attempts to open and records in status_message
    let completed_entry = HistoryEntry::new_completed(
        2,
        "https://ok.com".to_string(),
        Some("vid.mp4".to_string()),
        Some("./downloads/vid.mp4".to_string()),
    );
    app.add_history_entry(completed_entry);
    app.history_modal.as_mut().unwrap().selected = 0; // Newest entry
    app.open_selected_history_in_file_manager();
    assert!(
        app.status_message
            .as_ref()
            .map(|s| s.contains("file explorer"))
            .unwrap_or(false),
        "Status message must record file explorer attempt: {:?}",
        app.status_message
    );
}
