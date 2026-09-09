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
