// tests/app_tests.rs

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use video_downloader::app::{App, InputMode, InputScheme, ItemStatus};
use video_downloader::events::handle_key_event;

fn press_key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn press_char(c: char) -> KeyEvent {
    press_key(KeyCode::Char(c))
}

fn press_ctrl(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

#[test]
fn test_app_initial_state() {
    let app = App::new();
    assert_eq!(app.input_mode, InputMode::Normal);
    assert_eq!(app.input_scheme, InputScheme::StandardModal);
    assert!(app.input_buffer.is_empty());
    assert_eq!(app.cursor_position, 0);
    assert!(app.downloads.is_empty());
    assert!(!app.should_quit);
}

#[test]
fn test_input_mode_and_typing() {
    let mut app = App::new();

    // In Normal mode, pressing 'i' switches to Editing
    handle_key_event(&mut app, press_char('i'));
    assert_eq!(app.input_mode, InputMode::Editing);

    // Type "http://test.com"
    for c in "http://test.com".chars() {
        handle_key_event(&mut app, press_char(c));
    }
    assert_eq!(app.input_buffer, "http://test.com");
    assert_eq!(app.cursor_position, 15);

    // Test backspace
    handle_key_event(&mut app, press_key(KeyCode::Backspace));
    assert_eq!(app.input_buffer, "http://test.co");
    assert_eq!(app.cursor_position, 14);

    // Test Esc to return to Normal mode without clearing buffer
    handle_key_event(&mut app, press_key(KeyCode::Esc));
    assert_eq!(app.input_mode, InputMode::Normal);
    assert_eq!(app.input_buffer, "http://test.co");
}

#[test]
fn test_submit_download_and_concurrent_management() {
    let mut app = App::new();

    // Enter editing and type URL
    handle_key_event(&mut app, press_char('i'));
    for c in "https://example.com/video1".chars() {
        handle_key_event(&mut app, press_char(c));
    }

    // Press Enter to submit
    let result = handle_key_event(&mut app, press_key(KeyCode::Enter));
    assert!(result.is_some());
    let (id1, url1) = result.unwrap();
    assert_eq!(id1, 1);
    assert_eq!(url1, "https://example.com/video1");
    assert_eq!(app.downloads.len(), 1);
    assert_eq!(app.downloads[0].status, ItemStatus::Queued);
    assert!(app.input_buffer.is_empty());
    assert_eq!(app.input_mode, InputMode::Normal);

    // Enqueue a second download concurrently
    let id2 = app.enqueue_download("https://example.com/video2".to_string());
    assert_eq!(id2, 2);
    assert_eq!(app.downloads.len(), 2);

    // Update progress independently
    app.update_progress(id1, 1, 35.5);
    app.update_progress(id2, 1, 72.0);

    assert_eq!(app.downloads[0].progress, 35.5);
    assert_eq!(app.downloads[0].status, ItemStatus::Downloading);
    assert_eq!(app.downloads[1].progress, 72.0);
    assert_eq!(app.downloads[1].status, ItemStatus::Downloading);

    // Transition track and merging
    app.update_progress(id1, 2, 80.0);
    assert_eq!(app.downloads[0].track, 2);

    app.update_merging(id1);
    assert_eq!(app.downloads[0].status, ItemStatus::Merging);
    assert_eq!(app.downloads[0].progress, 100.0);

    app.update_success(id1);
    assert_eq!(app.downloads[0].status, ItemStatus::Completed);

    app.update_error(id2, "Network error 404".to_string());
    assert_eq!(
        app.downloads[1].status,
        ItemStatus::Failed("Network error 404".to_string())
    );
}

#[test]
fn test_vim_keybinding_scheme() {
    let mut app = App::new();

    // Toggle to Vim mode using F2
    handle_key_event(&mut app, press_key(KeyCode::F(2)));
    assert_eq!(app.input_scheme, InputScheme::Vim);

    // Press 'i' to insert text
    handle_key_event(&mut app, press_char('i'));
    assert_eq!(app.input_mode, InputMode::Editing);
    for c in "video.mp4".chars() {
        handle_key_event(&mut app, press_char(c));
    }
    assert_eq!(app.input_buffer, "video.mp4");

    // Press Esc to return to Normal mode
    handle_key_event(&mut app, press_key(KeyCode::Esc));
    assert_eq!(app.input_mode, InputMode::Normal);

    // In Vim normal mode, '0' moves cursor to beginning
    handle_key_event(&mut app, press_char('0'));
    assert_eq!(app.cursor_position, 0);

    // In Vim normal mode, 'x' deletes char under cursor ('v')
    handle_key_event(&mut app, press_char('x'));
    assert_eq!(app.input_buffer, "ideo.mp4");

    // In Vim normal mode, '$' moves cursor to end
    handle_key_event(&mut app, press_char('$'));
    assert_eq!(app.cursor_position, 8);

    // Toggle back with F2
    handle_key_event(&mut app, press_key(KeyCode::F(2)));
    assert_eq!(app.input_scheme, InputScheme::StandardModal);
}

#[test]
fn test_detail_modal_open_scroll_and_dismiss() {
    let mut app = App::new();
    let id = app.enqueue_download("https://test.com/fail".to_string());
    for i in 1..=10 {
        app.append_log(id, format!("Log line {}", i));
    }
    app.update_error(id, "Crash".to_string());

    // Press 'e' in normal mode to open modal
    handle_key_event(&mut app, press_char('e'));
    assert!(app.detail_modal.is_some());
    let modal = app.detail_modal.as_ref().unwrap();
    assert_eq!(modal.scroll_offset, 0);
    assert_eq!(modal.logs.len(), 11); // 10 logs + 1 error line

    // Press 'j' or Down to scroll modal
    handle_key_event(&mut app, press_char('j'));
    assert_eq!(app.detail_modal.as_ref().unwrap().scroll_offset, 1);

    handle_key_event(&mut app, press_char('k'));
    assert_eq!(app.detail_modal.as_ref().unwrap().scroll_offset, 0);

    // Press Esc to dismiss modal
    handle_key_event(&mut app, press_key(KeyCode::Esc));
    assert!(app.detail_modal.is_none());
}

#[test]
fn test_path_modal_open_via_p_in_standard_modal() {
    let mut app = App::new();
    assert!(app.path_modal.is_none());
    handle_key_event(&mut app, press_char('p'));
    assert!(app.path_modal.is_some());
    assert_eq!(app.path_modal.as_ref().unwrap().input, app.output_dir);
}

#[test]
fn test_path_modal_open_via_p_in_vim_mode() {
    let mut app = App::new();
    handle_key_event(&mut app, press_key(KeyCode::F(2)));
    assert_eq!(app.input_scheme, InputScheme::Vim);
    handle_key_event(&mut app, press_char('p'));
    assert!(app.path_modal.is_some());
}

#[test]
fn test_path_modal_f3_global_hotkey_in_normal_and_editing() {
    let mut app = App::new();
    // Normal mode: F3 opens
    handle_key_event(&mut app, press_key(KeyCode::F(3)));
    assert!(app.path_modal.is_some());
    // F3 while open: toggles (closes)
    handle_key_event(&mut app, press_key(KeyCode::F(3)));
    assert!(app.path_modal.is_none());

    // Enter editing mode
    handle_key_event(&mut app, press_char('i'));
    assert_eq!(app.input_mode, InputMode::Editing);
    // In editing mode, F3 opens path modal
    handle_key_event(&mut app, press_key(KeyCode::F(3)));
    assert!(app.path_modal.is_some());
    // F3 toggles (closes)
    handle_key_event(&mut app, press_key(KeyCode::F(3)));
    assert!(app.path_modal.is_none());
}

#[test]
fn test_path_modal_cancel_with_esc() {
    let mut app = App::new();
    let original_dir = app.output_dir.clone();
    handle_key_event(&mut app, press_char('p'));
    assert!(app.path_modal.is_some());

    // Type some characters into modal
    handle_key_event(&mut app, press_char('X'));
    assert_ne!(app.path_modal.as_ref().unwrap().input, original_dir);

    // Esc cancels without saving
    handle_key_event(&mut app, press_key(KeyCode::Esc));
    assert!(app.path_modal.is_none());
    assert_eq!(app.output_dir, original_dir);
}

#[test]
fn test_path_modal_ctrl_d_and_ctrl_u() {
    let mut app = App::new();
    handle_key_event(&mut app, press_char('p'));
    assert!(app.path_modal.is_some());

    // Ctrl+U clears
    handle_key_event(&mut app, press_ctrl('u'));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "");
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 0);

    // Ctrl+D resets to ./downloads
    handle_key_event(&mut app, press_ctrl('d'));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "./downloads");
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 11);
}

#[test]
fn test_path_modal_commit_and_sanitization() {
    let mut app = App::new();
    let temp_test_dir = std::env::temp_dir().join("vidown_test_save_dir");
    let test_dir_str = temp_test_dir.to_str().unwrap().to_string();

    handle_key_event(&mut app, press_char('p'));
    handle_key_event(&mut app, press_ctrl('u'));

    // Type quoted path: "..."
    let quoted_input = format!("\"{}\"", test_dir_str);
    for c in quoted_input.chars() {
        handle_key_event(&mut app, press_char(c));
    }

    // Enter commits
    handle_key_event(&mut app, press_key(KeyCode::Enter));
    assert!(app.path_modal.is_none());
    assert_eq!(app.output_dir, test_dir_str);
    assert!(temp_test_dir.exists(), "Directory should have been created automatically");

    // Clean up temp test directory
    let _ = std::fs::remove_dir_all(&temp_test_dir);
}

#[test]
fn test_path_modal_commit_empty_resets_to_default() {
    let mut app = App::new();
    app.output_dir = "/some/custom/path".to_string();
    handle_key_event(&mut app, press_char('p'));
    handle_key_event(&mut app, press_ctrl('u')); // clear to empty
    handle_key_event(&mut app, press_key(KeyCode::Enter));
    assert_eq!(app.output_dir, "./downloads");
}

#[test]
fn test_path_modal_navigation_and_editing() {
    let mut app = App::new();
    handle_key_event(&mut app, press_char('p'));
    handle_key_event(&mut app, press_ctrl('u'));

    for c in "abc".chars() {
        handle_key_event(&mut app, press_char(c));
    }
    assert_eq!(app.path_modal.as_ref().unwrap().input, "abc");
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 3);

    // Left arrow
    handle_key_event(&mut app, press_key(KeyCode::Left));
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 2);

    // Backspace at pos 2 deletes 'b'
    handle_key_event(&mut app, press_key(KeyCode::Backspace));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "ac");
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 1);

    // Home jumps to 0
    handle_key_event(&mut app, press_key(KeyCode::Home));
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 0);

    // Delete at 0 deletes 'a'
    handle_key_event(&mut app, press_key(KeyCode::Delete));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "c");
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 0);

    // End jumps to end
    handle_key_event(&mut app, press_key(KeyCode::End));
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 1);
}

#[test]
fn test_path_modal_safe_utf8_multibyte_editing() {
    let mut app = App::new();
    handle_key_event(&mut app, press_char('p'));
    handle_key_event(&mut app, press_ctrl('u'));

    // Insert 3-byte Korean chars "다운" and 2-byte Cyrillic "фа" and emoji "🦀" (4 bytes)
    for c in "다운фа🦀".chars() {
        handle_key_event(&mut app, press_char(c));
    }
    let modal = app.path_modal.as_ref().unwrap();
    assert_eq!(modal.input, "다운фа🦀");
    assert_eq!(modal.cursor_position, 5); // 5 unicode chars

    // Move left twice (past emoji and 'а')
    handle_key_event(&mut app, press_key(KeyCode::Left));
    handle_key_event(&mut app, press_key(KeyCode::Left));
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 3);

    // Backspace deletes 'ф'
    handle_key_event(&mut app, press_key(KeyCode::Backspace));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "다운а🦀");
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 2);

    // Delete deletes 'а'
    handle_key_event(&mut app, press_key(KeyCode::Delete));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "다운🦀");
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 2);

    // Home and insert '★'
    handle_key_event(&mut app, press_key(KeyCode::Home));
    handle_key_event(&mut app, press_char('★'));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "★다운🦀");
    assert_eq!(app.path_modal.as_ref().unwrap().cursor_position, 1);
}

#[test]
fn test_path_modal_strict_mutual_exclusion_with_detail_modal() {
    let mut app = App::new();
    let id = app.enqueue_download("https://example.com/test".to_string());
    app.update_error(id, "Error".to_string());

    // Open detail modal with 'e'
    handle_key_event(&mut app, press_char('e'));
    assert!(app.detail_modal.is_some());
    assert!(app.path_modal.is_none());

    // Opening path modal closes detail modal
    app.open_path_modal();
    assert!(app.path_modal.is_some());
    assert!(app.detail_modal.is_none());

    // While path modal is open, 'e' does NOT open detail modal (it inserts 'e')
    handle_key_event(&mut app, press_char('e'));
    assert!(app.detail_modal.is_none());
    assert!(app.path_modal.is_some());

    // Direct open_selected_details is blocked while path_modal is active
    app.open_selected_details();
    assert!(app.detail_modal.is_none());

    // F3 closes path modal
    handle_key_event(&mut app, press_key(KeyCode::F(3)));
    assert!(app.path_modal.is_none());
    assert!(app.detail_modal.is_none());
}

#[test]
fn test_sanitize_path_function() {
    use video_downloader::app::sanitize_path;

    assert_eq!(sanitize_path(""), "./downloads");
    assert_eq!(sanitize_path("   "), "./downloads");
    assert_eq!(sanitize_path("\"C:\\My Videos\""), "C:\\My Videos");
    assert_eq!(sanitize_path("'C:\\My Videos'"), "C:\\My Videos");
    assert_eq!(sanitize_path("  \"/home/user/downloads\"  "), "/home/user/downloads");
    assert_eq!(sanitize_path("~/downloads"), "~/downloads"); // Literal without ~ expansion
    assert_eq!(sanitize_path("./custom"), "./custom");
}

#[test]
fn test_download_item_tracks_output_dir() {
    let mut app = App::new();
    app.output_dir = "/first/dir".to_string();
    let _id1 = app.enqueue_download("https://example.com/1".to_string());
    assert_eq!(app.downloads[0].output_dir, "/first/dir");

    app.output_dir = "/second/dir".to_string();
    let _id2 = app.enqueue_download("https://example.com/2".to_string());
    assert_eq!(app.downloads[1].output_dir, "/second/dir");
    // Ensure item 1 still remembers /first/dir
    assert_eq!(app.downloads[0].output_dir, "/first/dir");

    // Open detail modal for item 2
    app.selected_download = 1;
    app.open_selected_details();
    assert_eq!(app.detail_modal.as_ref().unwrap().output_dir, "/second/dir");
}

#[test]
fn test_path_modal_ctrl_d_and_ctrl_u_case_insensitive() {
    let mut app = App::new();
    handle_key_event(&mut app, press_char('p'));

    // Ctrl+U (uppercase) clears
    handle_key_event(&mut app, press_ctrl('U'));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "");

    // Ctrl+D (uppercase) resets default
    handle_key_event(&mut app, press_ctrl('D'));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "./downloads");
}

#[test]
fn test_path_modal_cursor_out_of_bounds_resilience() {
    let mut app = App::new();
    handle_key_event(&mut app, press_char('p'));
    if let Some(modal) = &mut app.path_modal {
        modal.cursor_position = 999;
        modal.insert_char('Z');
        assert_eq!(modal.input, "./downloadsZ");
        assert_eq!(modal.cursor_position, 12);
    }
}

#[test]
fn test_sanitize_path_nested_and_unclosed_quotes() {
    use video_downloader::app::sanitize_path;

    assert_eq!(sanitize_path("\"\"/nested/path\"\""), "/nested/path");
    assert_eq!(sanitize_path("''/nested/path''"), "/nested/path");
    assert_eq!(sanitize_path("\"\""), "./downloads");
    assert_eq!(sanitize_path("''"), "./downloads");
    assert_eq!(sanitize_path("\"  \""), "./downloads");
}
