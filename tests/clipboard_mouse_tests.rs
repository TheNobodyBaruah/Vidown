// tests/clipboard_mouse_tests.rs

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use video_downloader::app::{App, CurrentScreen, InputMode, PathModal, SetupPhase, SetupState};
use video_downloader::clipboard::sanitize_clipboard_text;
use video_downloader::config::{get_default_download_dir, get_safe_fallback_dir};
use video_downloader::events::{handle_key_event, handle_mouse_event, handle_paste_event};

fn press_ctrl(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn press_shift_insert() -> KeyEvent {
    KeyEvent {
        code: KeyCode::Insert,
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn make_mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn test_sanitize_clipboard_text_quotes_and_whitespace() {
    assert_eq!(
        sanitize_clipboard_text("  https://www.youtube.com/watch?v=12345  "),
        "https://www.youtube.com/watch?v=12345"
    );
    assert_eq!(
        sanitize_clipboard_text("\"https://www.youtube.com/watch?v=12345\""),
        "https://www.youtube.com/watch?v=12345"
    );
    assert_eq!(
        sanitize_clipboard_text("'https://www.youtube.com/watch?v=12345'"),
        "https://www.youtube.com/watch?v=12345"
    );
    assert_eq!(
        sanitize_clipboard_text("  \"  'https://site.com/video.mp4'  \"  "),
        "https://site.com/video.mp4"
    );
}

#[test]
fn test_sanitize_clipboard_text_multiline() {
    let multi = "   \n\r  https://site.com/first_video \n https://site.com/second_video \r\n";
    assert_eq!(
        sanitize_clipboard_text(multi),
        "https://site.com/first_video"
    );
}

#[test]
fn test_sanitize_clipboard_text_empty() {
    assert_eq!(sanitize_clipboard_text(""), "");
    assert_eq!(sanitize_clipboard_text("   \n\r\t  "), "");
    assert_eq!(sanitize_clipboard_text("\"\""), "");
    assert_eq!(sanitize_clipboard_text("''"), "");
}

#[test]
fn test_app_paste_text_to_input_from_normal_mode() {
    let mut app = App::new();
    assert_eq!(app.input_mode, InputMode::Normal);
    assert_eq!(app.input_buffer, "");
    assert_eq!(app.cursor_position, 0);

    app.paste_text_to_input("https://youtu.be/dQw4w9WgXcQ");
    assert_eq!(app.input_mode, InputMode::Editing);
    assert_eq!(app.input_buffer, "https://youtu.be/dQw4w9WgXcQ");
    assert_eq!(app.cursor_position, 28);
    assert!(app.status_message.as_ref().unwrap().contains("Pasted"));
}

#[test]
fn test_app_paste_text_to_input_middle_insertion_and_multibyte() {
    let mut app = App::new();
    app.input_mode = InputMode::Editing;
    app.input_buffer = "hello world".to_string();
    app.cursor_position = 5; // right after "hello"

    app.paste_text_to_input("🦀 beautiful");
    assert_eq!(app.input_buffer, "hello🦀 beautiful world");
    // "hello" (5) + "🦀 beautiful" (11 chars) = 16
    assert_eq!(app.cursor_position, 16);
}

#[test]
fn test_handle_paste_event_in_main_screen() {
    let mut app = App::new();
    assert_eq!(app.current_screen, CurrentScreen::Main);

    handle_paste_event(&mut app, "https://vimeo.com/987654");
    assert_eq!(app.input_mode, InputMode::Editing);
    assert_eq!(app.input_buffer, "https://vimeo.com/987654");
}

#[test]
fn test_handle_paste_event_in_path_modal() {
    let mut app = App::new();
    app.path_modal = Some(PathModal::new("/current/path"));
    assert_eq!(app.path_modal.as_ref().unwrap().input, "/current/path");

    handle_paste_event(&mut app, "/extra/subfolder");
    assert_eq!(
        app.path_modal.as_ref().unwrap().input,
        "/current/path/extra/subfolder"
    );
    // URL input buffer should not have changed
    assert_eq!(app.input_buffer, "");
}

#[test]
fn test_handle_paste_event_modal_exclusion() {
    let mut app = App::new();
    app.open_help_modal();
    assert!(app.help_modal.is_some());

    handle_paste_event(&mut app, "https://ignored.com");
    assert_eq!(app.input_buffer, "");
}

#[test]
fn test_ctrl_v_and_shift_insert_key_events() {
    let mut app = App::new();

    // Verify Ctrl+V does not insert literal 'v' into buffer
    app.input_mode = InputMode::Editing;
    app.input_buffer = "http://".to_string();
    app.cursor_position = 7;

    // Press Ctrl+V (clipboard might be empty in headless CI test, but it shouldn't type 'v')
    handle_key_event(&mut app, press_ctrl('v'));
    assert_ne!(app.input_buffer, "http://v");

    // Press Shift+Insert
    handle_key_event(&mut app, press_shift_insert());
    assert!(!app.input_buffer.ends_with('v'));
}

#[test]
fn test_mouse_left_click_focus_url_input() {
    let mut app = App::new();
    app.input_buffer = "https://example.com/video".to_string();
    app.cursor_position = 0;
    app.input_mode = InputMode::Normal;

    // Click on row 4, column 10 (inside URL input area rows 3..=5)
    let mouse = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 10, 4);
    handle_mouse_event(&mut app, mouse);

    assert_eq!(app.input_mode, InputMode::Editing);
    // Cursor position should be set to col - 1 = 9
    assert_eq!(app.cursor_position, 9);
}

#[test]
fn test_mouse_left_click_select_downloads_item() {
    let mut app = App::new();
    app.enqueue_download("https://site.com/1".to_string());
    app.enqueue_download("https://site.com/2".to_string());
    app.enqueue_download("https://site.com/3".to_string());
    assert_eq!(app.selected_download, 2);

    // Row 7 is item 0 (header=3, input=3, border=1 -> row 7 is list_row 0 -> item 0)
    let mouse = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, 7);
    handle_mouse_event(&mut app, mouse);
    assert_eq!(app.selected_download, 0);

    // Row 10 is item 1 (list_row 3 -> 3/3 = 1)
    let mouse = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, 10);
    handle_mouse_event(&mut app, mouse);
    assert_eq!(app.selected_download, 1);
}

#[test]
fn test_mouse_left_click_double_click_opens_details() {
    let mut app = App::new();
    let id = app.enqueue_download("https://site.com/1".to_string());
    app.append_log(id, "Error log line".to_string());
    app.selected_download = 0;

    // Re-clicking on the already selected item opens details modal
    let mouse = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 15, 7);
    handle_mouse_event(&mut app, mouse);
    assert!(app.detail_modal.is_some());
    assert_eq!(
        app.detail_modal.as_ref().unwrap().title,
        "Download #1: Details & Logs"
    );
}

#[test]
fn test_mouse_scroll_wheel_navigation() {
    let mut app = App::new();
    app.enqueue_download("https://site.com/1".to_string());
    app.enqueue_download("https://site.com/2".to_string());
    app.selected_download = 0;

    // Scroll down moves to next download
    let scroll_down = make_mouse_event(MouseEventKind::ScrollDown, 10, 10);
    handle_mouse_event(&mut app, scroll_down);
    assert_eq!(app.selected_download, 1);

    // Scroll up moves to previous download
    let scroll_up = make_mouse_event(MouseEventKind::ScrollUp, 10, 10);
    handle_mouse_event(&mut app, scroll_up);
    assert_eq!(app.selected_download, 0);
}

#[test]
fn test_mouse_scroll_wheel_in_help_modal() {
    let mut app = App::new();
    app.open_help_modal();
    assert_eq!(app.help_modal.as_ref().unwrap().scroll_offset, 0);

    let scroll_down = make_mouse_event(MouseEventKind::ScrollDown, 10, 10);
    handle_mouse_event(&mut app, scroll_down);
    assert_eq!(app.help_modal.as_ref().unwrap().scroll_offset, 1);

    let scroll_up = make_mouse_event(MouseEventKind::ScrollUp, 10, 10);
    handle_mouse_event(&mut app, scroll_up);
    assert_eq!(app.help_modal.as_ref().unwrap().scroll_offset, 0);
}

#[test]
fn test_get_default_download_dir_in_tests() {
    let default_dir = get_default_download_dir();
    assert_eq!(default_dir, "./downloads");
}

#[test]
fn test_get_safe_fallback_dir_is_valid() {
    let fallback = get_safe_fallback_dir();
    assert!(!fallback.as_os_str().is_empty());
}

#[test]
fn test_setup_screen_mouse_click_starts_app_when_complete() {
    let state = SetupState {
        phase: SetupPhase::Complete,
        tools: Vec::new(),
        status_message: "Ready".to_string(),
        help_scroll: 0,
    };
    let mut app = App::new_with_setup(state);
    assert_eq!(app.current_screen, CurrentScreen::Setup);

    // Clicking in the bottom footer row triggers launch
    let mouse = make_mouse_event(MouseEventKind::Down(MouseButton::Left), 20, 22);
    handle_mouse_event(&mut app, mouse);
    assert_eq!(app.current_screen, CurrentScreen::Main);
}
